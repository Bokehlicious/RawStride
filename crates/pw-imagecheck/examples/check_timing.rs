//! Measures the DNG image check of the safe metadata write step by step (T264, ADR 0007 §9.6
//! point 3, Q19), and which check refuses a damaged copy.
//!
//! For each DNG and run, the file is copied twice into the work folder (an "original" and a
//! "copy") and these are timed on the unchanged copy:
//!
//! - `RawImageCheck::without_release().check_copy(original, copy, FileKind::Dng)`: the whole check
//!   as the app runs it;
//! - `pw_libraw::Session::open_with_diagnostics(copy)`: its LibRaw part (the shim's open and
//!   unpack), the session dropped at once;
//! - LibRaw's own C API: `libraw_open_wfile` alone, `libraw_unpack` alone and `libraw_close`, with
//!   each call's return code and the data errors LibRaw reported during the unpack (Windows only);
//! - `check_copy(original, copy, FileKind::Tiff)`: for any kind other than DNG the check runs only
//!   the preview comparison, so this is the preview comparison alone.
//!
//! Then, once per DNG, two damaged copies as `tests/real_files.rs` makes them (cut at half its
//! length; one byte changed in the middle of its largest preview) are checked against the
//! unchanged original by each of: the image data hash (`ExifTool::image_data_hash`), the preview
//! comparison alone, LibRaw's open alone, LibRaw's unpack, the shim's verdict, and the whole
//! `check_copy`. The output says which refuses each copy and how long it took.
//!
//! Run in a release build:
//!
//! ```text
//! cargo run --release -p pw-imagecheck --example check_timing -- [--runs <n>] [--work <folder>] <files>
//! ```
//!
//! `--runs` is the number of runs per file for the step timings (default 3); `--work` is the
//! folder the copies are written in, which must be new or empty (default: a new folder under the
//! system's temporary folder). A file whose extension is not `.dng` is skipped. ExifTool (used only
//! for the image data hash) is `PW_EXIFTOOL_PL` run through `perl` when that is set, otherwise
//! `exiftool` on PATH.
//!
//! The program only reads the files it is given: it copies them first and writes only copies in
//! its work folder, which it deletes afterwards.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use pw_imagecheck::RawImageCheck;
use pw_libraw::{DAMAGED, Session};
use pw_metadata::ExifTool;
use pw_metadata::reconcile::FileKind;
use pw_metadata::safe_write::ImageCheck;

/// The number of runs per file when `--runs` is not given.
const DEFAULT_RUNS: usize = 3;

/// The note printed with the LibRaw C API's times.
const C_API_NOTE: &str = "The shim (pwlr_open) opens through its own data stream and with its own \
                          settings, so LibRaw's open and unpack through the C API need not add up \
                          to Session::open_with_diagnostics.";

/// The note printed with the preview comparison's times.
const PREVIEWS_NOTE: &str = "The preview comparison alone is check_copy with FileKind::Tiff: for \
                             any kind other than DNG, check_copy runs only check_previews.";

/// The command line.
struct Options {
    files: Vec<PathBuf>,
    runs: usize,
    work: Option<PathBuf>,
}

/// What LibRaw's C API did with one file.
struct CSteps {
    open: Duration,
    open_rc: i32,
    /// The unpack's time, return code and the data errors LibRaw reported during it; `None` when
    /// the open failed.
    unpack: Option<(Duration, i32, i32)>,
    close: Duration,
}

/// The times of one run of point 2 (an unchanged copy).
struct Run {
    check_dng: Duration,
    shim: Duration,
    /// `None` when LibRaw's C API is left out or failed (the reason is printed).
    c: Option<CSteps>,
    previews: Duration,
}

/// Whether one check refused a damaged copy, how long it took, and what it said.
struct Verdict {
    refused: bool,
    time: Duration,
    note: String,
}

/// The checks of one damaged copy (point 3).
struct Damage {
    label: &'static str,
    hash: Verdict,
    previews: Verdict,
    /// `None` when LibRaw's C API is left out or failed.
    c_open: Option<Verdict>,
    /// `None` when LibRaw's open failed (or the C API is left out).
    c_unpack: Option<Verdict>,
    shim: Verdict,
    check_dng: Verdict,
}

/// All measurements of one DNG.
struct Row {
    name: String,
    megabytes: f64,
    runs: Vec<Run>,
    failed: usize,
    damages: Vec<Damage>,
}

fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            eprintln!(
                "Usage: cargo run --release -p pw-imagecheck --example check_timing -- \
                 [--runs <n>] [--work <folder>] <files>"
            );
            return ExitCode::FAILURE;
        }
    };
    match run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        files: Vec::new(),
        runs: DEFAULT_RUNS,
        work: None,
    };
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--runs" => {
                let value = args.next().ok_or("--runs needs a number")?;
                options.runs = match value.parse::<usize>() {
                    Ok(runs) if runs > 0 => runs,
                    _ => return Err(format!("--runs needs a number above 0, not {value}")),
                };
            }
            "--work" => {
                let value = args.next().ok_or("--work needs a folder")?;
                options.work = Some(PathBuf::from(value));
            }
            _ => options.files.push(PathBuf::from(arg)),
        }
    }
    if options.files.is_empty() {
        return Err("No DNG files given.".to_string());
    }
    Ok(options)
}

fn run(options: &Options) -> Result<(), String> {
    let (work, made_work) = match &options.work {
        Some(work) => {
            refuse_used_folder(work)?;
            (work.clone(), false)
        }
        None => (
            std::env::temp_dir().join(format!("pw-check-timing-{}", std::process::id())),
            true,
        ),
    };
    fs::create_dir_all(&work)
        .map_err(|e| format!("The work folder {} cannot be made: {e}", work.display()))?;
    refuse_source_folders(&work, &options.files)?;
    println!("Work folder: {}", work.display());
    if !libraw_c::AVAILABLE {
        println!("LibRaw's C API is Windows only here: its steps are left out.");
    }

    let mut et = start_exiftool()?;

    let mut rows = Vec::new();
    for source in &options.files {
        if !is_dng(source) {
            println!();
            println!("{}: skipped, not a .dng file", source.display());
            continue;
        }
        rows.push(measure(&mut et, source, &work, options.runs));
    }
    println!();
    println!("{C_API_NOTE}");
    println!("{PREVIEWS_NOTE}");
    println!();
    print_step_table(&rows);
    println!();
    print_damage_table(&rows);

    if made_work && let Err(e) = fs::remove_dir(&work) {
        println!(
            "The work folder {} could not be deleted: {e}",
            work.display()
        );
    }
    Ok(())
}

/// A DNG is told by its extension (case-insensitive): `pw_raw::inspect` gives DNG as a TIFF.
fn is_dng(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("dng"))
}

/// Refuses a `--work` folder that exists and is not empty: the copies would overwrite its files
/// of the same name, and the cleanup deletes them.
fn refuse_used_folder(work: &Path) -> Result<(), String> {
    let mut entries = match fs::read_dir(work) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(format!(
                "The work folder {} cannot be read: {e}",
                work.display()
            ));
        }
    };
    if entries.next().is_some() {
        return Err(format!(
            "The work folder {} is not empty; choose a new or empty one.",
            work.display()
        ));
    }
    Ok(())
}

/// Refuses a work folder that is the folder of one of the files to measure: the program never
/// writes where the sources are.
fn refuse_source_folders(work: &Path, files: &[PathBuf]) -> Result<(), String> {
    let work = fs::canonicalize(work)
        .map_err(|e| format!("The work folder {} cannot be found: {e}", work.display()))?;
    for file in files {
        let folder = match file.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        if fs::canonicalize(folder).is_ok_and(|folder| folder == work) {
            return Err(format!(
                "The work folder {} holds {}; choose another one.",
                work.display(),
                file.display()
            ));
        }
    }
    Ok(())
}

/// Starts ExifTool and prints how long it took to start and answer.
fn start_exiftool() -> Result<ExifTool, String> {
    let start = Instant::now();
    let (started, source) = match std::env::var("PW_EXIFTOOL_PL") {
        Ok(pl) => (
            ExifTool::start_with("perl", &[&pl]),
            format!("perl {pl} (PW_EXIFTOOL_PL)"),
        ),
        Err(_) => (ExifTool::start(), "exiftool on PATH".to_string()),
    };
    let mut et =
        started.map_err(|e| format!("ExifTool could not be started from {source}: {e}"))?;
    let version = et
        .execute(&["-ver".to_string()])
        .map_err(|e| format!("ExifTool from {source} does not answer: {e}"))?;
    println!(
        "ExifTool {} from {source} started and answered in {:.0} ms",
        version.trim(),
        ms(start.elapsed())
    );
    Ok(et)
}

/// Points 2 and 3 for one DNG, printed as they are made.
fn measure(et: &mut ExifTool, source: &Path, work: &Path, runs: usize) -> Row {
    let name = file_name(source);
    let megabytes = fs::metadata(source)
        .map(|m| m.len() as f64 / 1_000_000.0)
        .unwrap_or(0.0);
    let mut row = Row {
        name,
        megabytes,
        runs: Vec::new(),
        failed: 0,
        damages: Vec::new(),
    };
    println!();
    println!("{} ({:.1} MB)", row.name, row.megabytes);

    let original = work.join(format!("original-{}", row.name));
    let copy = work.join(format!("copy-{}", row.name));
    for number in 1..=runs {
        let result = run_once(source, &original, &copy, number);
        remove(&[&original, &copy]);
        match result {
            Ok(run) => row.runs.push(run),
            Err(message) => {
                println!("  run {number} failed: {message}");
                row.failed += 1;
            }
        }
    }
    print_steps(&row);

    match damages(et, source, work, &row.name) {
        Ok(damages) => row.damages = damages,
        Err(message) => println!("  damaged copies: {message}"),
    }
    print_damages(&row);
    row
}

/// One run of point 2: copies the source twice and times each step on the unchanged copy. The
/// copies are deleted by the caller.
fn run_once(source: &Path, original: &Path, copy: &Path, number: usize) -> Result<Run, String> {
    copy_file(source, original)?;
    copy_file(source, copy)?;
    let check = RawImageCheck::without_release();

    let start = Instant::now();
    let checked = check.check_copy(original, copy, FileKind::Dng);
    let check_dng = start.elapsed();

    let start = Instant::now();
    let (opened, _) = Session::open_with_diagnostics(copy);
    let shim_ok = match opened {
        Ok(session) => {
            drop(session);
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    };
    let shim = start.elapsed();

    let c = match libraw_c::steps(copy) {
        Ok(steps) => Some(steps),
        Err(message) => {
            if libraw_c::AVAILABLE {
                println!("  run {number}: LibRaw's C API: {message}");
            }
            None
        }
    };

    let start = Instant::now();
    let previews_ok = check.check_copy(original, copy, FileKind::Tiff);
    let previews = start.elapsed();

    let c_text = match &c {
        Some(steps) => format!("; {}", c_steps_text(steps)),
        None => String::new(),
    };
    println!(
        "  run {number}: check_copy (DNG) {}; shim {}; previews {}{c_text}",
        result_text(&checked),
        result_text(&shim_ok),
        result_text(&previews_ok)
    );
    Ok(Run {
        check_dng,
        shim,
        c,
        previews,
    })
}

/// The return codes and data errors of LibRaw's C API, for one line of output.
fn c_steps_text(steps: &CSteps) -> String {
    match steps.unpack {
        Some((_, rc, errors)) => format!(
            "libraw_open_wfile rc {}, libraw_unpack rc {rc}, {errors} data error(s) in the unpack",
            steps.open_rc
        ),
        None => format!("libraw_open_wfile rc {} (failed, no unpack)", steps.open_rc),
    }
}

fn result_text(result: &Result<(), String>) -> String {
    match result {
        Ok(()) => "passed".to_string(),
        Err(error) => format!("refused ({error})"),
    }
}

/// Point 3 for one DNG: the two damaged copies, each checked against the unchanged original.
fn damages(
    et: &mut ExifTool,
    source: &Path,
    work: &Path,
    name: &str,
) -> Result<Vec<Damage>, String> {
    let original = work.join(format!("original-{name}"));
    copy_file(source, &original)?;
    let bytes = match fs::read(source) {
        Ok(bytes) => bytes,
        Err(e) => {
            remove(&[&original]);
            return Err(format!("{} cannot be read: {e}", source.display()));
        }
    };
    let original_hash = et.image_data_hash(&original);
    if let Err(e) = &original_hash {
        println!("  the original's image data hash failed: {e}");
    }

    let mut damages = Vec::new();

    // (a) Cut at half its length, inside its RAW data (as tests/real_files.rs cuts it).
    let cut = work.join(format!("cut-{name}"));
    let half = bytes.len() / 2;
    match bytes.get(..half).map(|kept| fs::write(&cut, kept)) {
        Some(Ok(())) => {
            damages.push(check_damage(
                et,
                &original,
                &original_hash,
                &cut,
                "cut at half",
            ));
        }
        Some(Err(e)) => println!("  the cut copy cannot be written: {e}"),
        None => println!("  the cut copy cannot be made"),
    }
    remove(&[&cut]);

    // (b) One byte changed in the middle of its largest preview.
    let changed = work.join(format!("changed-{name}"));
    match changed_preview(&original, &bytes) {
        Ok(changed_bytes) => match fs::write(&changed, changed_bytes) {
            Ok(()) => damages.push(check_damage(
                et,
                &original,
                &original_hash,
                &changed,
                "one preview byte",
            )),
            Err(e) => println!("  the copy with a changed preview byte cannot be written: {e}"),
        },
        Err(message) => println!("  the copy with a changed preview byte: {message}"),
    }
    remove(&[&changed, &original]);
    Ok(damages)
}

/// The file's bytes with one byte changed (`^= 0x01`) in the middle of its largest preview
/// (`pw_raw::inspect(...).previews.last()`), as tests/real_files.rs changes it.
fn changed_preview(original: &Path, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let info = pw_raw::inspect(original).map_err(|e| format!("pw-raw cannot inspect it: {e}"))?;
    let largest = info.previews.last().ok_or("it has no preview")?;
    let at = largest
        .offset
        .checked_add(largest.length / 2)
        .and_then(|at| usize::try_from(at).ok())
        .ok_or("the preview's middle is out of range")?;
    let mut changed = bytes.to_vec();
    let byte = changed
        .get_mut(at)
        .ok_or("the preview's middle is past the end of the file")?;
    *byte ^= 0x01;
    Ok(changed)
}

/// Every check of point 3 on one damaged copy.
fn check_damage(
    et: &mut ExifTool,
    original: &Path,
    original_hash: &pw_metadata::Result<Option<String>>,
    damaged: &Path,
    label: &'static str,
) -> Damage {
    let check = RawImageCheck::without_release();

    // The image data hash: only the damaged copy's call is timed (in the app the original's hash
    // comes from its snapshot).
    let start = Instant::now();
    let damaged_hash = et.image_data_hash(damaged);
    let time = start.elapsed();
    let hash = match (original_hash, &damaged_hash) {
        (Ok(before), Ok(after)) => {
            let refused = match (before, after) {
                (Some(before), Some(after)) => before != after,
                (None, None) => false,
                _ => true,
            };
            Verdict {
                refused,
                time,
                note: format!(
                    "original {}, damaged {}",
                    hash_text(before.as_deref()),
                    hash_text(after.as_deref())
                ),
            }
        }
        (Err(e), _) => Verdict {
            refused: true,
            time,
            note: format!("the original's call failed: {e}"),
        },
        (_, Err(e)) => Verdict {
            refused: true,
            time,
            note: format!("the call failed: {e}"),
        },
    };

    let start = Instant::now();
    let result = check.check_copy(original, damaged, FileKind::Tiff);
    let previews = verdict(start.elapsed(), &result);

    let (c_open, c_unpack) = match libraw_c::steps(damaged) {
        Ok(steps) => {
            let open = Verdict {
                refused: steps.open_rc != 0,
                time: steps.open,
                note: format!("rc {}", steps.open_rc),
            };
            let unpack = steps.unpack.map(|(time, rc, errors)| Verdict {
                refused: rc != 0 || errors > 0,
                time,
                note: format!("rc {rc}, {errors} data error(s)"),
            });
            (Some(open), unpack)
        }
        Err(message) => {
            if libraw_c::AVAILABLE {
                println!("  {label}: LibRaw's C API: {message}");
            }
            (None, None)
        }
    };

    let start = Instant::now();
    let (opened, diagnostics) = Session::open_with_diagnostics(damaged);
    let shim = match opened {
        Ok(session) => {
            drop(session);
            Verdict {
                refused: false,
                time: start.elapsed(),
                note: "opened".to_string(),
            }
        }
        Err(error) => {
            let time = start.elapsed();
            let text = error.to_string();
            let damaged_text = if text.contains(DAMAGED) {
                "damaged"
            } else {
                "not damaged"
            };
            Verdict {
                refused: true,
                time,
                note: format!(
                    "{damaged_text}: {text} (data errors {}, missing {} bytes)",
                    diagnostics.data_errors, diagnostics.missing
                ),
            }
        }
    };

    let start = Instant::now();
    let result = check.check_copy(original, damaged, FileKind::Dng);
    let check_dng = verdict(start.elapsed(), &result);

    Damage {
        label,
        hash,
        previews,
        c_open,
        c_unpack,
        shim,
        check_dng,
    }
}

fn verdict(time: Duration, result: &Result<(), String>) -> Verdict {
    Verdict {
        refused: result.is_err(),
        time,
        note: match result {
            Ok(()) => "passed".to_string(),
            Err(error) => error.clone(),
        },
    }
}

fn hash_text(hash: Option<&str>) -> String {
    match hash {
        Some(hash) => hash.chars().take(12).collect::<String>() + "…",
        None => "Ok(None)".to_string(),
    }
}

/// The steps of point 2, each with the times of the successful runs.
fn steps(row: &Row) -> Vec<(&'static str, Vec<Duration>)> {
    vec![
        (
            "check_copy (DNG)",
            row.runs.iter().map(|r| r.check_dng).collect(),
        ),
        (
            "Session::open_with_diagnostics",
            row.runs.iter().map(|r| r.shim).collect(),
        ),
        (
            "libraw_open_wfile",
            row.runs
                .iter()
                .filter_map(|r| r.c.as_ref().map(|c| c.open))
                .collect(),
        ),
        (
            "libraw_unpack",
            row.runs
                .iter()
                .filter_map(|r| r.c.as_ref().and_then(|c| c.unpack.map(|u| u.0)))
                .collect(),
        ),
        (
            "libraw_close",
            row.runs
                .iter()
                .filter_map(|r| r.c.as_ref().map(|c| c.close))
                .collect(),
        ),
        (
            "check_previews (check_copy, TIFF)",
            row.runs.iter().map(|r| r.previews).collect(),
        ),
    ]
}

fn print_steps(row: &Row) {
    println!("  {} runs, {} failed", row.runs.len(), row.failed);
    for (step, times) in steps(row) {
        let list = times
            .iter()
            .map(|t| format!("{:.1}", ms(*t)))
            .collect::<Vec<_>>()
            .join(", ");
        match median(&times) {
            Some(m) => println!("  {step}: {list} ms; median {:.1} ms", ms(m)),
            None => println!("  {step}: none"),
        }
    }
}

fn print_damages(row: &Row) {
    for damage in &row.damages {
        println!("  {}:", damage.label);
        let lines = [
            ("image data hash", Some(&damage.hash)),
            ("preview comparison", Some(&damage.previews)),
            ("LibRaw open (C API)", damage.c_open.as_ref()),
            ("LibRaw unpack (C API)", damage.c_unpack.as_ref()),
            ("shim (Session::open_with_diagnostics)", Some(&damage.shim)),
            ("check_copy (DNG)", Some(&damage.check_dng)),
        ];
        for (check, verdict) in lines {
            match verdict {
                Some(v) => println!(
                    "    {check}: {}, {:.1} ms; {}",
                    yes_no(v.refused),
                    ms(v.time),
                    v.note
                ),
                None => println!("    {check}: not run"),
            }
        }
    }
}

fn yes_no(refused: bool) -> &'static str {
    if refused { "refused" } else { "passed" }
}

/// The medians of point 2 for all files, with the shares of LibRaw's open, its unpack and the
/// preview comparison in `check_copy`.
fn print_step_table(rows: &[Row]) {
    println!(
        "| File | MB | check_copy (DNG) | Session::open_with_diagnostics | libraw_open_wfile | \
         libraw_unpack | libraw_close | Preview comparison | Open share | Unpack share | \
         Preview share | Runs (failed) |"
    );
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    for row in rows {
        let medians: Vec<Option<Duration>> = steps(row).iter().map(|(_, t)| median(t)).collect();
        let cells = medians
            .iter()
            .map(|m| m.map_or("–".to_string(), |m| format!("{:.0}", ms(m))))
            .collect::<Vec<_>>()
            .join(" | ");
        let whole = medians.first().copied().flatten();
        let share = |index: usize| match (medians.get(index).copied().flatten(), whole) {
            (Some(part), Some(whole)) if !whole.is_zero() => {
                format!("{:.0} %", 100.0 * part.as_secs_f64() / whole.as_secs_f64())
            }
            _ => "–".to_string(),
        };
        println!(
            "| {} | {:.1} | {cells} | {} | {} | {} | {} ({}) |",
            row.name,
            row.megabytes,
            share(2),
            share(3),
            share(5),
            row.runs.len(),
            row.failed
        );
    }
    println!();
    println!(
        "Times are medians in ms over the successful runs; a share is the step's median over \
         check_copy's (DNG) median."
    );
}

/// Which check refused each damaged copy, for all files.
fn print_damage_table(rows: &[Row]) {
    println!(
        "| File | Damage | Image data hash | Preview comparison | LibRaw open | LibRaw unpack | \
         Shim | check_copy (DNG) |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    for row in rows {
        for damage in &row.damages {
            let unpack = match &damage.c_unpack {
                Some(v) => format!("{} ({}), {:.0} ms", yes_no(v.refused), v.note, ms(v.time)),
                None => "–".to_string(),
            };
            let shim = if damage.shim.refused {
                let kind = if damage.shim.note.starts_with("damaged") {
                    "damaged"
                } else {
                    "not damaged"
                };
                format!("refused ({kind}), {:.0} ms", ms(damage.shim.time))
            } else {
                format!("passed, {:.0} ms", ms(damage.shim.time))
            };
            println!(
                "| {} | {} | {} | {} | {} | {unpack} | {shim} | {} |",
                row.name,
                damage.label,
                cell(Some(&damage.hash)),
                cell(Some(&damage.previews)),
                cell(damage.c_open.as_ref()),
                cell(Some(&damage.check_dng)),
            );
        }
    }
    println!();
    println!(
        "\"refused\" or \"passed\" and the time in ms. The image data hash's time is the damaged \
         copy's call alone. LibRaw unpack: its return code and the data errors LibRaw reported \
         during it, apart; \"–\" when the open failed."
    );
}

fn cell(verdict: Option<&Verdict>) -> String {
    match verdict {
        Some(v) => format!("{}, {:.0} ms", yes_no(v.refused), ms(v.time)),
        None => "–".to_string(),
    }
}

fn copy_file(source: &Path, target: &Path) -> Result<(), String> {
    fs::copy(source, target).map(|_| ()).map_err(|e| {
        format!(
            "{} could not be copied to {}: {e}",
            source.display(),
            target.display()
        )
    })
}

/// Deletes the given copies; names any that cannot be deleted.
fn remove(files: &[&Path]) {
    for file in files {
        if file.exists()
            && let Err(e) = fs::remove_file(file)
        {
            println!("  {} could not be deleted: {e}", file.display());
        }
    }
}

/// The median of `times` (the mean of the middle two for an even count); `None` when empty.
fn median(times: &[Duration]) -> Option<Duration> {
    let mut sorted = times.to_vec();
    sorted.sort();
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted.get(middle).copied()
    } else {
        let low = sorted.get(middle.checked_sub(1)?)?;
        let high = sorted.get(middle)?;
        Some((*low + *high) / 2)
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// LibRaw's own C API (`libraw.h`), already linked through `pw-libraw` (static `raw_r`).
/// `libraw_open_wfile` takes a wide path and exists only on Windows.
#[cfg(windows)]
mod libraw_c {
    use std::ffi::{c_char, c_int, c_uint, c_void};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::time::Instant;

    use super::CSteps;

    /// LibRaw's C API is in this build.
    pub const AVAILABLE: bool = true;

    /// `data_callback` of `libraw_types.h`.
    type DataCallback = unsafe extern "C" fn(data: *mut c_void, file: *const c_char, offset: i64);

    unsafe extern "C" {
        fn libraw_init(flags: c_uint) -> *mut c_void;
        fn libraw_open_wfile(data: *mut c_void, path: *const u16) -> c_int;
        fn libraw_unpack(data: *mut c_void) -> c_int;
        fn libraw_close(data: *mut c_void);
        fn libraw_set_dataerror_handler(
            data: *mut c_void,
            func: Option<DataCallback>,
            datap: *mut c_void,
        );
    }

    /// Counts LibRaw's data errors (corrupt data, or an unexpected end). Setting it also keeps
    /// LibRaw from printing them to stderr.
    unsafe extern "C" fn count_data_error(data: *mut c_void, _file: *const c_char, _offset: i64) {
        // SAFETY: `data` is the pointer to the `AtomicI32` given to
        // `libraw_set_dataerror_handler` in `steps`, which outlives the handle (the handle is
        // closed before the counter is dropped). An atomic, since LibRaw's OpenMP decoders may
        // report from several threads.
        if let Some(counter) = unsafe { data.cast::<AtomicI32>().cast_const().as_ref() } {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// A LibRaw handle from `libraw_init`, closed with `libraw_close` on every path.
    struct Handle(*mut c_void);

    impl Handle {
        /// Closes the handle and gives the time `libraw_close` took.
        fn close(mut self) -> std::time::Duration {
            let start = Instant::now();
            // SAFETY: the pointer came from `libraw_init`, is not null and is closed only here
            // (it is set to null so `drop` does not close it again).
            unsafe { libraw_close(self.0) };
            self.0 = ptr::null_mut();
            start.elapsed()
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: the pointer came from `libraw_init` and has not been closed.
                unsafe { libraw_close(self.0) };
            }
        }
    }

    /// Opens and unpacks `path` with LibRaw's C API, timing each call.
    pub fn steps(path: &Path) -> Result<CSteps, String> {
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // Declared before the handle, so the handle (and LibRaw's pointer to it) goes first.
        let errors = AtomicI32::new(0);
        // SAFETY: a plain constructor; null on failure, checked below.
        let handle = Handle(unsafe { libraw_init(0) });
        if handle.0.is_null() {
            return Err("libraw_init returned null".to_string());
        }
        // SAFETY: a valid handle; the callback matches `data_callback`, and the counter outlives
        // the handle (see `count_data_error`).
        unsafe {
            libraw_set_dataerror_handler(
                handle.0,
                Some(count_data_error),
                ptr::from_ref(&errors).cast_mut().cast::<c_void>(),
            );
        }

        let start = Instant::now();
        // SAFETY: a valid handle and a NUL-terminated UTF-16 path that lives through the call.
        let open_rc = unsafe { libraw_open_wfile(handle.0, wide.as_ptr()) };
        let open = start.elapsed();
        if open_rc != 0 {
            let close = handle.close();
            return Ok(CSteps {
                open,
                open_rc,
                unpack: None,
                close,
            });
        }

        // Only the unpack's data errors are counted.
        errors.store(0, Ordering::Relaxed);
        let start = Instant::now();
        // SAFETY: a valid handle whose file was opened.
        let unpack_rc = unsafe { libraw_unpack(handle.0) };
        let unpack = start.elapsed();
        let unpack_errors = errors.load(Ordering::Relaxed);
        let close = handle.close();
        Ok(CSteps {
            open,
            open_rc,
            unpack: Some((unpack, unpack_rc, unpack_errors)),
            close,
        })
    }
}

/// LibRaw's C API is left out on other systems (`libraw_open_wfile` is Windows only).
#[cfg(not(windows))]
mod libraw_c {
    use std::path::Path;

    use super::CSteps;

    /// LibRaw's C API is not in this build.
    pub const AVAILABLE: bool = false;

    pub fn steps(_path: &Path) -> Result<CSteps, String> {
        Err("LibRaw's C API is Windows only here".to_string())
    }
}
