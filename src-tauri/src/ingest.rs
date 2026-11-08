//! Ingest dialog backend (ADR 0005): detect cards, preview the plan, copy in the background with
//! progress events, apply the IPTC template, optionally eject the card.
//!
//! Events: `ingest-progress` ([`ProgressDto`]) during the run, `ingest-done` ([`DoneDto`]) once.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use pw_ingest::{Options, Plan, Shot, Template};
use pw_metadata::{Iptc, LocalizedValue};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::ratings::Metadata;

/// How many plan lines the preview sends to the dialog.
const PREVIEW_ITEMS: usize = 300;

#[derive(Debug, Default)]
pub struct IngestState {
    /// Last scan, by source folder (scanning a card reads every file's EXIF, so it is reused
    /// while the user edits templates).
    scan: Mutex<Option<(PathBuf, Arc<Vec<Shot>>)>>,
    cancel: Arc<AtomicBool>,
    running: AtomicBool,
}

impl IngestState {
    fn scan_cache(&self) -> MutexGuard<'_, Option<(PathBuf, Arc<Vec<Shot>>)>> {
        self.scan.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn shots(&self, source: &Path, rescan: bool) -> Result<Arc<Vec<Shot>>, String> {
        if !rescan
            && let Some((p, s)) = self.scan_cache().as_ref()
            && p == source
        {
            return Ok(s.clone());
        }
        let shots = Arc::new(
            pw_ingest::scan(source)
                .map_err(|e| format!("Kunde inte läsa {}: {e}", source.display()))?,
        );
        *self.scan_cache() = Some((source.to_path_buf(), shots.clone()));
        Ok(shots)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IptcDto {
    #[serde(default)]
    creator: String,
    #[serde(default)]
    copyright: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    credit: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    url: String,
}

impl From<IptcDto> for Iptc {
    fn from(d: IptcDto) -> Iptc {
        // The dialog edits one text per field; it becomes the Lang Alt `x-default` entry, the only
        // one PhotoWorkflow writes (ADR 0007 Q7).
        Iptc {
            creator: d.creator,
            copyright: LocalizedValue::x_default(d.copyright),
            title: LocalizedValue::x_default(d.title),
            description: LocalizedValue::x_default(d.description),
            credit: d.credit,
            email: d.email,
            phone: d.phone,
            url: d.url,
        }
    }
}

/// Everything the dialog sends (field names as in JavaScript).
#[derive(Debug, Clone, Deserialize)]
pub struct IngestRequest {
    source: String,
    dest: String,
    #[serde(default)]
    backup: Option<String>,
    folder: String,
    name: String,
    #[serde(default)]
    job: String,
    #[serde(default = "one")]
    first_seq: u32,
    #[serde(default)]
    iptc: IptcDto,
    #[serde(default)]
    eject: bool,
}

fn one() -> u32 {
    1
}

impl IngestRequest {
    fn source(&self) -> Result<PathBuf, String> {
        let s = self.source.trim();
        if s.is_empty() {
            return Err("Välj en källa.".into());
        }
        let p = PathBuf::from(s);
        if !p.is_absolute() {
            return Err(format!("Källan måste vara en fullständig sökväg: {s}"));
        }
        if !p.is_dir() {
            return Err(format!("Källan {s} finns inte."));
        }
        Ok(p)
    }

    fn options(&self) -> Result<Options, String> {
        let source = self.source()?;
        let dest = self.dest.trim();
        if dest.is_empty() {
            return Err("Välj ett mål.".into());
        }
        let dest = PathBuf::from(dest);
        let backup = self
            .backup
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .map(PathBuf::from);
        // The overlap checks use resolved paths; the copy itself uses the paths as entered.
        let source_r = pw_ingest::resolve(&source, "Källan")?;
        let dest_r = pw_ingest::resolve(&dest, "Målet")?;
        let backup_r = backup
            .as_deref()
            .map(|b| pw_ingest::resolve(b, "Backupmålet"))
            .transpose()?;
        for (what, p) in [("Målet", Some(&dest_r)), ("Backupmålet", backup_r.as_ref())] {
            if let Some(p) = p
                && (pw_ingest::inside(p, &source_r) || pw_ingest::inside(&source_r, p))
            {
                return Err(format!("{what} får inte ligga på eller över källan."));
            }
        }
        if let Some(b) = &backup_r
            && (pw_ingest::inside(b, &dest_r) || pw_ingest::inside(&dest_r, b))
        {
            return Err("Backupmålet måste vara en annan mapp än målet.".into());
        }
        Ok(Options {
            dest,
            backup,
            folder: Template::parse(&self.folder).map_err(|e| format!("Mappmall: {e}"))?,
            name: Template::parse(&self.name).map_err(|e| format!("Filnamnsmall: {e}"))?,
            job: self.job.trim().to_string(),
            first_seq: self.first_seq,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ItemDto {
    source: String,
    rel: String,
    /// "copy" | "backup" | "present" | "renamed"
    action: &'static str,
}

#[derive(Debug, Serialize)]
pub struct PreviewDto {
    shots: usize,
    files_to_copy: usize,
    bytes_to_copy: u64,
    present: usize,
    renamed: usize,
    no_exif: usize,
    items_total: usize,
    items: Vec<ItemDto>,
}

fn action(i: &pw_ingest::PlanItem) -> &'static str {
    match (i.to_dest, i.to_backup) {
        (false, false) => "present",
        (true, _) if i.renamed_for_conflict => "renamed",
        (true, _) => "copy",
        (false, true) => "backup",
    }
}

fn preview_of(shots: &[Shot], plan: &Plan) -> PreviewDto {
    PreviewDto {
        shots: shots.len(),
        files_to_copy: plan.files_to_copy(),
        bytes_to_copy: plan.bytes_to_copy(),
        present: plan.already_present(),
        renamed: plan
            .items
            .iter()
            .filter(|i| i.renamed_for_conflict && (i.to_dest || i.to_backup))
            .count(),
        no_exif: shots.iter().filter(|s| !s.time_from_exif).count(),
        items_total: plan.items.len(),
        items: plan
            .items
            .iter()
            .take(PREVIEW_ITEMS)
            .map(|i| ItemDto {
                source: i
                    .source
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                rel: i.rel.to_string_lossy().into_owned(),
                action: action(i),
            })
            .collect(),
    }
}

// ---------- commands ----------

/// Drives (or mounted volumes) with a `DCIM` folder, i.e. camera cards.
#[tauri::command]
pub fn ingest_volumes() -> Vec<String> {
    let mut out = Vec::new();
    #[cfg(windows)]
    for c in b'A'..=b'Z' {
        let root = format!("{}:\\", c as char);
        if Path::new(&root).join("DCIM").is_dir() {
            out.push(root);
        }
    }
    #[cfg(not(windows))]
    for base in ["/Volumes", "/media", "/run/media"] {
        let mut stack = vec![PathBuf::from(base)];
        for _ in 0..2 {
            let mut next = Vec::new();
            for d in stack {
                for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                    let p = e.path();
                    if p.join("DCIM").is_dir() {
                        out.push(p.to_string_lossy().into_owned());
                    } else if p.is_dir() {
                        next.push(p);
                    }
                }
            }
            stack = next;
        }
    }
    out
}

/// What an ingest with these settings would do. Writes nothing.
#[tauri::command]
pub async fn ingest_preview(
    request: IngestRequest,
    rescan: bool,
    app: AppHandle,
) -> Result<PreviewDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<IngestState>();
        let opt = request.options()?;
        let shots = state.shots(&request.source()?, rescan)?;
        Ok(preview_of(&shots, &pw_ingest::plan(&shots, &opt)))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Starts the ingest in the background. Progress and result arrive as events.
#[tauri::command]
pub fn ingest_start(
    request: IngestRequest,
    app: AppHandle,
    state: State<'_, IngestState>,
) -> Result<(), String> {
    let opt = request.options()?;
    if state.running.swap(true, Ordering::SeqCst) {
        return Err("En import pågår redan.".into());
    }
    state.cancel.store(false, Ordering::SeqCst);
    let spawned = std::thread::Builder::new()
        .name("pw-ingest".into())
        .spawn(move || {
            // Clears `running` and sends `ingest-done` even if the run panics.
            let mut finish = RunFinish {
                app: &app,
                done: None,
            };
            finish.done = Some(run(&app, &request, &opt));
        });
    if let Err(e) = spawned {
        state.running.store(false, Ordering::SeqCst);
        return Err(format!("Kunde inte starta importen: {e}"));
    }
    Ok(())
}

/// Ends a run: clears `running` and emits `ingest-done` when dropped, so neither is skipped if the
/// run panics (the dialog would otherwise wait forever and every new import would be refused).
struct RunFinish<'a> {
    app: &'a AppHandle,
    /// `None` until the run returns; still `None` here means it panicked.
    done: Option<DoneDto>,
}

impl Drop for RunFinish<'_> {
    fn drop(&mut self) {
        let done = self.done.take().unwrap_or_else(|| DoneDto {
            error: Some("Importen avbröts av ett internt fel. Kontrollera målmappen.".into()),
            ..DoneDto::default()
        });
        self.app
            .state::<IngestState>()
            .running
            .store(false, Ordering::SeqCst);
        let _ = self.app.emit("ingest-done", done);
    }
}

#[tauri::command]
pub fn ingest_cancel(state: State<'_, IngestState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

// ---------- the run ----------

#[derive(Debug, Clone, Serialize)]
pub struct ProgressDto {
    /// "copy" | "iptc" | "eject"
    phase: &'static str,
    files_done: usize,
    files_total: usize,
    bytes_done: u64,
    bytes_total: u64,
    current: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DoneDto {
    copied: usize,
    present: usize,
    bytes: u64,
    seconds: f64,
    cancelled: bool,
    failed: Vec<String>,
    iptc_errors: Vec<String>,
    /// Message about ejecting, if it was requested.
    eject: Option<String>,
    /// Destination folders that received files, with file counts (largest first).
    folders: Vec<(String, usize)>,
    /// Error that stopped the run before copying.
    error: Option<String>,
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn run(app: &AppHandle, req: &IngestRequest, opt: &Options) -> DoneDto {
    let state = app.state::<IngestState>();
    let started = Instant::now();
    let source = match req.source() {
        Ok(s) => s,
        Err(e) => {
            return DoneDto {
                error: Some(e),
                ..DoneDto::default()
            };
        }
    };
    // Always scan again: the card may have been swapped since the preview (same drive letter,
    // same file names such as DSC00001.ARW), and the cached scan would then name, date and copy
    // the new card's files according to the old card's EXIF.
    let shots = match state.shots(&source, true) {
        Ok(s) => s,
        Err(e) => {
            return DoneDto {
                error: Some(e),
                ..DoneDto::default()
            };
        }
    };
    // Plan again right before copying: the destination may have changed since the preview.
    let plan = pw_ingest::plan(&shots, opt);
    let report = pw_ingest::execute(&plan, opt, &state.cancel, |p| {
        let _ = app.emit(
            "ingest-progress",
            ProgressDto {
                phase: "copy",
                files_done: p.files_done,
                files_total: p.files_total,
                bytes_done: p.bytes_done,
                bytes_total: p.bytes_total,
                current: file_name(&p.current),
            },
        );
    });
    let failed_sources: std::collections::HashSet<&PathBuf> =
        report.failed.iter().map(|(p, _)| p).collect();
    let copied: Vec<&pw_ingest::PlanItem> = plan
        .items
        .iter()
        .filter(|i| (i.to_dest || i.to_backup) && !failed_sources.contains(&i.source))
        .collect();

    let mut done = DoneDto {
        copied: report.copied,
        present: report.skipped_present,
        bytes: report.bytes,
        cancelled: report.cancelled,
        failed: report
            .failed
            .iter()
            .map(|(p, e)| format!("{}: {e}", file_name(p)))
            .collect(),
        ..DoneDto::default()
    };

    // IPTC template on the new copies (images only; their sidecars are updated through them).
    let iptc: Iptc = req.iptc.clone().into();
    if !iptc.is_empty() && !report.cancelled {
        let meta = app.state::<Metadata>();
        let targets: Vec<PathBuf> = copied
            .iter()
            .filter(|i| {
                let e = i
                    .rel
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase());
                !e.is_some_and(|e| pw_ingest::COMPANION_EXTENSIONS.contains(&e.as_str()))
            })
            .flat_map(|i| {
                let mut v = Vec::new();
                if i.to_dest {
                    v.push(opt.dest.join(&i.rel));
                }
                if let (true, Some(b)) = (i.to_backup, &opt.backup) {
                    v.push(b.join(&i.rel));
                }
                v
            })
            .collect();
        for (n, path) in targets.iter().enumerate() {
            if state.cancel.load(Ordering::Relaxed) {
                done.cancelled = true;
                break;
            }
            let _ = app.emit(
                "ingest-progress",
                ProgressDto {
                    phase: "iptc",
                    files_done: n,
                    files_total: targets.len(),
                    bytes_done: 0,
                    bytes_total: 0,
                    current: file_name(path),
                },
            );
            if let Err(e) = meta.with(|et| et.write_iptc(path, &iptc)) {
                done.iptc_errors.push(format!("{}: {e}", file_name(path)));
            }
        }
    }

    let destination_items: Vec<&pw_ingest::PlanItem> = plan
        .items
        .iter()
        .filter(|item| opt.dest.join(&item.rel).exists())
        .collect();
    done.folders = pw_ingest::folder_summary(&destination_items, &opt.dest);

    if req.eject {
        done.eject = Some(if done.cancelled || !done.failed.is_empty() {
            "Kortet matades inte ut eftersom importen inte blev komplett.".into()
        } else {
            let _ = app.emit(
                "ingest-progress",
                ProgressDto {
                    phase: "eject",
                    files_done: 0,
                    files_total: 0,
                    bytes_done: 0,
                    bytes_total: 0,
                    current: req.source.clone(),
                },
            );
            *state.scan_cache() = None;
            pw_ingest::eject(&source)
        });
    }
    done.seconds = started.elapsed().as_secs_f64();
    done
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn item(to_dest: bool, to_backup: bool, renamed: bool) -> pw_ingest::PlanItem {
        pw_ingest::PlanItem {
            source: PathBuf::from("DSC00001.ARW"),
            size: 1,
            modified: SystemTime::UNIX_EPOCH,
            rel: PathBuf::from("2026/DSC00001.ARW"),
            to_dest,
            to_backup,
            renamed_for_conflict: renamed,
        }
    }

    #[test]
    fn plan_item_actions() {
        assert_eq!(action(&item(false, false, false)), "present");
        assert_eq!(action(&item(true, false, false)), "copy");
        assert_eq!(action(&item(true, true, true)), "renamed");
        assert_eq!(action(&item(false, true, false)), "backup");
    }

    #[test]
    fn dialog_text_becomes_x_default() {
        let iptc: Iptc = IptcDto {
            title: "Titel".into(),
            ..IptcDto::default()
        }
        .into();
        assert_eq!(iptc.title.default_text(), "Titel");
        assert!(iptc.description.default_text().is_empty());
        assert!(!iptc.is_empty());
    }
}
