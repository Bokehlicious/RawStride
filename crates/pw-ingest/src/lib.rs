//! Ingest: copy images from a memory card (or any folder) to a destination, renamed by template,
//! optionally to a second backup destination, with every copy verified.
//!
//! Three steps, so the UI can show a preview before anything is written:
//!
//! 1. [`scan`] walks the source and groups files into *shots*: files in the same folder with the
//!    same name stem (`TN0001.ARW` + `TN0001.JPG` + `TN0001.xmp`). A shot gets one capture time
//!    (EXIF `DateTimeOriginal`, else the file date) and one running number, so RAW+JPEG pairs and
//!    sidecars keep matching names.
//! 2. [`plan`] renders folder and file-name templates and resolves collisions. A destination
//!    file with the same size and modification time is treated as already ingested and skipped,
//!    so re-running an ingest after an interruption only copies what is missing. A different file
//!    with the same name gets a suffix (`_1`, `_2`, …); nothing is ever overwritten.
//! 3. [`execute`] copies. Each file is written to a temporary `*.pwpart`, flushed to disk, given
//!    the source's modification time, read back and compared (hash) with what was read from the
//!    source, and only then renamed to its final name. A failed or cancelled copy leaves no
//!    final-named file behind.
//!
//! The source is only ever opened for reading.
//!
//! Not yet: IPTC templates, catalog registration, unbuffered read-back (the read-back currently
//! may come from the OS cache).

mod eject;
mod paths;
mod request;
mod summary;
pub mod template;
mod volumes;

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::hash::{DefaultHasher, Hasher};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub use eject::{drive_of, eject};
pub use paths::{inside, resolve};
pub use request::{OptionsInput, validate_options, validate_source};
pub use summary::folder_summary;
pub use template::{CaptureTime, Template, TemplateError};
pub use volumes::card_volumes;

/// Extensions copied together with an image of the same stem (but never on their own).
pub const COMPANION_EXTENSIONS: &[&str] = &["xmp"];

/// Folders that are never descended into.
const SKIP_DIRS: &[&str] = &["system volume information", "$recycle.bin"];

const CHUNK: usize = 4 * 1024 * 1024;

/// One file on the source.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: PathBuf,
    /// Extension as found (case kept).
    pub ext: String,
    pub size: u64,
    pub modified: SystemTime,
}

/// Files that belong together (same folder, same stem).
#[derive(Debug, Clone)]
pub struct Shot {
    pub stem: String,
    pub files: Vec<SourceFile>,
    pub time: Option<CaptureTime>,
    /// True if `time` came from EXIF; false if from the file date (UTC).
    pub time_from_exif: bool,
    pub make: Option<String>,
    pub model: Option<String>,
}

fn ext_lower(p: &Path) -> Option<String> {
    p.extension()?.to_str().map(|e| e.to_ascii_lowercase())
}

fn is_image(ext: &str) -> bool {
    pw_raw::SUPPORTED_EXTENSIONS.contains(&ext)
}

fn is_jpeg_or_tiff(ext: &str) -> bool {
    matches!(ext, "jpg" | "jpeg" | "tif" | "tiff")
}

/// Walks `source` recursively and groups image files and their companions into shots.
pub fn scan(source: &Path) -> io::Result<Vec<Shot>> {
    let mut groups: HashMap<(PathBuf, String), Vec<SourceFile>> = HashMap::new();
    let mut stack = vec![source.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let ft = entry.file_type()?;
            if ft.is_dir() {
                if !name.starts_with('.') && !SKIP_DIRS.contains(&name.to_lowercase().as_str()) {
                    stack.push(path);
                }
                continue;
            }
            if !ft.is_file() || name.starts_with('.') {
                continue;
            }
            let Some(ext) = ext_lower(&path) else {
                continue;
            };
            if !is_image(&ext) && !COMPANION_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            let meta = entry.metadata()?;
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            groups
                .entry((dir.clone(), stem.to_lowercase()))
                .or_default()
                .push(SourceFile {
                    ext: path
                        .extension()
                        .map(|e| e.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    path,
                    size: meta.len(),
                    modified: meta.modified().unwrap_or(UNIX_EPOCH),
                });
        }
    }

    let mut shots = Vec::new();
    for (_, mut files) in groups {
        // Images first (RAW before JPEG/TIFF), companions last, then by name.
        files.sort_by_key(|f| {
            let e = f.ext.to_ascii_lowercase();
            let rank = if !is_image(&e) {
                2
            } else if is_jpeg_or_tiff(&e) {
                1
            } else {
                0
            };
            (rank, f.path.clone())
        });
        let Some(primary) = files.first() else {
            continue;
        };
        if !is_image(&primary.ext.to_ascii_lowercase()) {
            continue; // a sidecar without its image
        }
        let stem = primary
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let info = pw_raw::inspect(&primary.path).ok();
        let exif_time = info
            .as_ref()
            .and_then(|i| i.date_time_original.as_deref())
            .and_then(CaptureTime::parse_exif);
        let file_time = primary
            .modified
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|d| CaptureTime::from_unix(d.as_secs() as i64));
        shots.push(Shot {
            stem,
            time: exif_time.or(file_time),
            time_from_exif: exif_time.is_some(),
            make: info.as_ref().and_then(|i| i.make.clone()),
            model: info.as_ref().and_then(|i| i.model.clone()),
            files,
        });
    }
    // Capture order (then folder and name): this is the order of the running number.
    shots.sort_by(|a, b| (a.time, &a.files[0].path).cmp(&(b.time, &b.files[0].path)));
    Ok(shots)
}

/// What to do and where.
#[derive(Debug, Clone)]
pub struct Options {
    /// Primary destination root.
    pub dest: PathBuf,
    /// Optional second destination; receives the same folder structure and names.
    pub backup: Option<PathBuf>,
    /// Folder below the root, e.g. `{year}/{date}`. Empty = directly in the root.
    pub folder: Template,
    /// File name without extension, e.g. `{name}` (keep) or `{date}_{seq:4}`.
    pub name: Template,
    pub job: String,
    /// First running number.
    pub first_seq: u32,
}

/// One file to copy.
#[derive(Debug, Clone)]
pub struct PlanItem {
    pub source: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    /// Path below the destination root(s).
    pub rel: PathBuf,
    /// Needs copying to the primary destination (false = identical file already there).
    pub to_dest: bool,
    /// Needs copying to the backup destination.
    pub to_backup: bool,
    /// The name got a suffix because another file already had it.
    pub renamed_for_conflict: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub items: Vec<PlanItem>,
}

impl Plan {
    /// Bytes that will actually be written (both destinations).
    pub fn bytes_to_copy(&self) -> u64 {
        self.items
            .iter()
            .map(|i| i.size * (u64::from(i.to_dest) + u64::from(i.to_backup)))
            .sum()
    }

    pub fn files_to_copy(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.to_dest || i.to_backup)
            .count()
    }

    pub fn already_present(&self) -> usize {
        self.items
            .iter()
            .filter(|i| !i.to_dest && !i.to_backup)
            .count()
    }
}

/// State of a destination path for a given source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Free,
    /// Already ingested (see [`slot`]).
    Same,
    /// A companion (sidecar) exists; it counts as ours only if the shot's images are `Same`.
    Present,
    Taken,
}

/// Formats whose metadata is edited in place after ingest (ratings, IPTC; ADR 0004), so their
/// size can change while the modification time is kept (`exiftool -P`).
const EMBEDDED_METADATA: &[&str] = &["dng", "jpg", "jpeg", "tif", "tiff"];

fn secs(t: SystemTime) -> Option<u64> {
    t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Whether `path` can receive `f`:
/// - images: the same modification time (to the second) and the same size mean "already
///   ingested"; for DNG/JPEG/TIFF the size may differ, because ratings and IPTC are written into
///   those files afterwards with the date kept;
/// - sidecars: any existing file is `Present` (it is edited after ingest, so neither size nor date
///   says anything); `plan` decides from the shot's images.
fn slot(path: &Path, f: &SourceFile) -> Slot {
    let Ok(m) = fs::metadata(path) else {
        return Slot::Free;
    };
    if !m.is_file() {
        return Slot::Taken;
    }
    let ext = f.ext.to_ascii_lowercase();
    if COMPANION_EXTENSIONS.contains(&ext.as_str()) {
        return Slot::Present;
    }
    let same_time = m.modified().ok().and_then(secs) == secs(f.modified);
    let same_size = m.len() == f.size || EMBEDDED_METADATA.contains(&ext.as_str());
    if same_time && same_size {
        Slot::Same
    } else {
        Slot::Taken
    }
}

/// Resolves `Present` sidecars: ours if every image of the shot is already there, else a clash.
fn resolve_companions(states: &mut [Slot]) {
    let images_same = states
        .iter()
        .filter(|s| **s != Slot::Present)
        .all(|s| *s == Slot::Same);
    for s in states.iter_mut().filter(|s| **s == Slot::Present) {
        *s = if images_same { Slot::Same } else { Slot::Taken };
    }
}

/// Renders names for every shot and decides what needs copying. Writes nothing.
pub fn plan(shots: &[Shot], opt: &Options) -> Plan {
    let mut items = Vec::new();
    // Destination paths already used by this plan (lower case: Windows is case-insensitive).
    let mut used: HashSet<String> = HashSet::new();
    for (n, shot) in shots.iter().enumerate() {
        let vars = template::Vars {
            time: shot.time,
            make: shot.make.as_deref(),
            model: shot.model.as_deref(),
            name: &shot.stem,
            seq: opt.first_seq.saturating_add(n as u32),
            job: &opt.job,
        };
        let folder: PathBuf = opt
            .folder
            .render(&vars)
            .split(['/', '\\'])
            .map(template::clean_segment)
            .filter(|s| !s.is_empty())
            .collect();
        let mut base = template::clean_segment(&opt.name.render(&vars));
        if base.is_empty() {
            base = shot.stem.clone();
        }
        // Find a stem that is free (or identical) for every file of the shot, in both roots.
        let mut k = 0u32;
        let states = loop {
            let stem = if k == 0 {
                base.clone()
            } else {
                format!("{base}_{k}")
            };
            let mut rels = Vec::new();
            let mut ds = Vec::new();
            let mut bs = Vec::new();
            for f in &shot.files {
                let rel = folder.join(format!("{stem}.{}", f.ext));
                ds.push(slot(&opt.dest.join(&rel), f));
                bs.push(
                    opt.backup
                        .as_ref()
                        .map_or(Slot::Free, |r| slot(&r.join(&rel), f)),
                );
                rels.push(rel);
            }
            resolve_companions(&mut ds);
            resolve_companions(&mut bs);
            let keys: Vec<String> = rels
                .iter()
                .map(|r| r.to_string_lossy().to_lowercase())
                .collect();
            let clash = keys.iter().any(|k| used.contains(k))
                || ds.contains(&Slot::Taken)
                || bs.contains(&Slot::Taken);
            if !clash {
                let has_backup = opt.backup.is_some();
                break rels
                    .into_iter()
                    .zip(keys)
                    .zip(ds.into_iter().zip(bs))
                    .map(|((rel, key), (d, b))| (rel, key, d, has_backup.then_some(b)))
                    .collect::<Vec<_>>();
            }
            k += 1;
        };
        for (f, (rel, key, d, b)) in shot.files.iter().zip(states) {
            used.insert(key);
            items.push(PlanItem {
                source: f.path.clone(),
                size: f.size,
                modified: f.modified,
                rel,
                to_dest: d == Slot::Free,
                to_backup: b == Some(Slot::Free),
                renamed_for_conflict: k > 0,
            });
        }
    }
    Plan { items }
}

/// Progress, reported after every file.
#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub files_done: usize,
    pub files_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub current: PathBuf,
}

/// Outcome of [`execute`].
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Files written (a file written to both destinations counts once).
    pub copied: usize,
    pub skipped_present: usize,
    pub bytes: u64,
    /// Source file and error, for files that could not be copied or verified.
    pub failed: Vec<(PathBuf, String)>,
    pub cancelled: bool,
}

/// Copies according to `plan`. Stops between chunks when `cancel` is set.
pub fn execute(
    plan: &Plan,
    opt: &Options,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&Progress),
) -> Report {
    let mut report = Report {
        skipped_present: plan.already_present(),
        ..Report::default()
    };
    let mut p = Progress {
        files_total: plan.files_to_copy(),
        bytes_total: plan.bytes_to_copy(),
        ..Progress::default()
    };
    for item in plan.items.iter().filter(|i| i.to_dest || i.to_backup) {
        if cancel.load(Ordering::Relaxed) {
            report.cancelled = true;
            break;
        }
        p.current = item.source.clone();
        let mut targets = Vec::new();
        if item.to_dest {
            targets.push(opt.dest.join(&item.rel));
        }
        if item.to_backup
            && let Some(b) = &opt.backup
        {
            targets.push(b.join(&item.rel));
        }
        let mut ok = true;
        for target in targets {
            match copy_verified(&item.source, &target, item.modified, cancel) {
                Ok(n) => {
                    report.bytes += n;
                    p.bytes_done += n;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                    report.cancelled = true;
                    ok = false;
                    break;
                }
                Err(e) => {
                    report
                        .failed
                        .push((item.source.clone(), format!("{}: {e}", target.display())));
                    ok = false;
                }
            }
        }
        if report.cancelled {
            break;
        }
        if ok {
            report.copied += 1;
        }
        p.files_done += 1;
        progress(&p);
    }
    report
}

fn part_path(target: &Path) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".pwpart");
    target.with_file_name(name)
}

/// Copies `src` to `target` through a temporary file, verifies it and renames it into place.
/// Returns the number of bytes copied. `Interrupted` means cancelled.
pub fn copy_verified(
    src: &Path,
    target: &Path,
    modified: SystemTime,
    cancel: &AtomicBool,
) -> io::Result<u64> {
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir)?;
    }
    if target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination exists (will not overwrite)",
        ));
    }
    let part = part_path(target);
    let result = (|| {
        let mut input = File::open(src)?;
        let mut out = File::create(&part)?;
        let mut buf = vec![0u8; CHUNK];
        let mut hasher = DefaultHasher::new();
        let mut total = 0u64;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
            }
            let n = input.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.write(&buf[..n]);
            out.write_all(&buf[..n])?;
            total += n as u64;
        }
        out.sync_all()?;
        out.set_modified(modified)?;
        drop(out);
        // Read back and compare.
        let mut check = File::open(&part)?;
        let mut verify = DefaultHasher::new();
        let mut back = 0u64;
        loop {
            let n = check.read(&mut buf)?;
            if n == 0 {
                break;
            }
            verify.write(&buf[..n]);
            back += n as u64;
        }
        if back != total || verify.finish() != hasher.finish() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "verification failed: the copy differs from the source",
            ));
        }
        drop(check);
        if target.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "destination appeared during copy (will not overwrite)",
            ));
        }
        fs::rename(&part, target)?;
        Ok(total)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}
