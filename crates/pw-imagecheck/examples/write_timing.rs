//! Measures the time of each step of the safe metadata write as RawStride runs it (T261): the read
//! every edit makes before its write, the snapshot of the file the write targets, the safe write
//! with the real image check, the image check (`RawImageCheck::check_copy`) alone, and the write
//! without it. The plan is one City edit (`photoshop:City` = "Malmö").
//!
//! Run in a release build:
//!
//! ```text
//! cargo run --release -p pw-imagecheck --example write_timing -- [--runs <n>] [--work <folder>] <files>
//! ```
//!
//! `--runs` is the number of runs per file (default 3); `--work` is the folder the copies are
//! written in, which must be new or empty (default: a new folder under the system's temporary
//! folder). ExifTool is
//! `PW_EXIFTOOL_PL` run through `perl` when that is set, otherwise `exiftool` on PATH.
//!
//! The program only reads the files it is given. Each run copies the image (and a manufacturer
//! RAW's `<basename>.xmp`) into the work folder, writes only the copy, and deletes the copies
//! afterwards. A manufacturer RAW is measured twice: with its sidecar copied ("RAW, sidecar
//! exists", when it has one) and without ("RAW, new sidecar").
//!
//! The safe write uses Windows file operations, so the writes succeed only on Windows; elsewhere
//! the program compiles and runs, but every write fails with the file operations' refusal.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use pw_imagecheck::RawImageCheck;
use pw_metadata::ExifTool;
use pw_metadata::iim::Fallback;
use pw_metadata::journal::Journal;
use pw_metadata::reconcile::FileKind;
use pw_metadata::safe_write::ImageCheck;
use pw_metadata::value::Value;
use pw_metadata::write_plan::{WriteSettings, plan};

/// The number of runs per file when `--runs` is not given.
const DEFAULT_RUNS: usize = 3;

/// The command line.
struct Options {
    files: Vec<PathBuf>,
    runs: usize,
    work: Option<PathBuf>,
}

/// What is measured for one file: the image, and for a manufacturer RAW the sidecar copied with
/// it (`None`: the write makes a new sidecar).
struct Case {
    source: PathBuf,
    sidecar: Option<PathBuf>,
    label: &'static str,
}

/// The times of one successful run.
struct RunTimes {
    kind: FileKind,
    read: Duration,
    /// `None` when the write takes no snapshot of an original (a new sidecar).
    snapshot: Option<Duration>,
    write: Duration,
    check: Duration,
}

/// All runs of one case.
struct Row {
    name: String,
    label: &'static str,
    megabytes: f64,
    kind: Option<FileKind>,
    runs: Vec<RunTimes>,
    failed: usize,
}

/// [`RawImageCheck`] that records how long `check_copy` took (added up, should it be called more
/// than once).
struct Timed {
    check: RawImageCheck,
    time: Cell<Option<Duration>>,
}

impl ImageCheck for Timed {
    fn release(&self, image: &Path) {
        self.check.release(image);
    }

    fn release_for_move(&self, image: &Path) {
        self.check.release_for_move(image);
    }

    fn check_copy(&self, original: &Path, copy: &Path, kind: FileKind) -> Result<(), String> {
        let start = Instant::now();
        let result = self.check.check_copy(original, copy, kind);
        let elapsed = start.elapsed();
        let total = self.time.get().map_or(elapsed, |before| before + elapsed);
        self.time.set(Some(total));
        result
    }
}

fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            eprintln!(
                "Usage: cargo run --release -p pw-imagecheck --example write_timing -- \
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
        return Err("No image files given.".to_string());
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
            std::env::temp_dir().join(format!("pw-write-timing-{}", std::process::id())),
            true,
        ),
    };
    fs::create_dir_all(&work)
        .map_err(|e| format!("The work folder {} cannot be made: {e}", work.display()))?;
    refuse_source_folders(&work, &options.files)?;
    let journal_dir = work.join("journal");
    let journal = Journal::open(&journal_dir).map_err(|e| {
        format!(
            "The journal in {} cannot be opened: {e}",
            journal_dir.display()
        )
    })?;
    println!("Work folder: {}", work.display());

    let mut et = start_exiftool()?;

    let mut rows = Vec::new();
    for source in &options.files {
        for case in cases(source)? {
            rows.push(measure(&mut et, &journal, &case, &work, options.runs));
        }
    }
    println!();
    print_table(&rows);

    if let Err(e) = fs::remove_dir_all(&journal_dir) {
        println!(
            "The journal folder {} could not be deleted: {e}",
            journal_dir.display()
        );
    }
    if made_work && let Err(e) = fs::remove_dir(&work) {
        println!(
            "The work folder {} could not be deleted: {e}",
            work.display()
        );
    }
    Ok(())
}

/// Refuses a `--work` folder that exists and is not empty: the copies would overwrite its files
/// of the same name, the cleanup deletes every file named as a measured file's stem and a dot,
/// and the `journal` folder is deleted at the end.
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

/// The cases of one file: one for a JPEG, TIFF or DNG; for a manufacturer RAW one with its
/// sidecar (when it has one) and one without.
fn cases(source: &Path) -> Result<Vec<Case>, String> {
    if !pw_metadata::supports_sidecar(source) {
        return Ok(vec![Case {
            source: source.to_path_buf(),
            sidecar: None,
            label: "",
        }]);
    }
    let mut cases = Vec::new();
    if let Some(sidecar) = find_sidecar(source)? {
        cases.push(Case {
            source: source.to_path_buf(),
            sidecar: Some(sidecar),
            label: "RAW, sidecar exists",
        });
    }
    cases.push(Case {
        source: source.to_path_buf(),
        sidecar: None,
        label: "RAW, new sidecar",
    });
    Ok(cases)
}

/// The `<basename>.xmp` next to `image`, found without regard to case.
fn find_sidecar(image: &Path) -> Result<Option<PathBuf>, String> {
    let Some(stem) = image
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
    else {
        return Ok(None);
    };
    let folder = match image.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let entries = fs::read_dir(folder)
        .map_err(|e| format!("The folder {} cannot be read: {e}", folder.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let stem_matches = path
            .file_stem()
            .is_some_and(|s| s.to_string_lossy().to_lowercase() == stem);
        let is_xmp = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("xmp"));
        if stem_matches && is_xmp && path.is_file() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// All runs of one case, printed as they are made.
fn measure(et: &mut ExifTool, journal: &Journal, case: &Case, work: &Path, runs: usize) -> Row {
    let name = file_name(&case.source);
    let megabytes = fs::metadata(&case.source)
        .map(|m| m.len() as f64 / 1_000_000.0)
        .unwrap_or(0.0);
    let mut row = Row {
        name,
        label: case.label,
        megabytes,
        kind: None,
        runs: Vec::new(),
        failed: 0,
    };
    println!();
    if row.label.is_empty() {
        println!("{}", row.name);
    } else {
        println!("{} ({})", row.name, row.label);
    }
    for number in 1..=runs {
        let result = run_once(et, journal, case, work);
        cleanup(&case.source, work);
        match result {
            Ok(times) => {
                row.kind = Some(times.kind);
                row.runs.push(times);
            }
            Err(message) => {
                println!("  run {number} failed: {message}");
                row.failed += 1;
            }
        }
    }
    print_row(&row);
    row
}

/// One run: copy, read, snapshot, plan and write. The copies are deleted by the caller.
fn run_once(
    et: &mut ExifTool,
    journal: &Journal,
    case: &Case,
    work: &Path,
) -> Result<RunTimes, String> {
    let copy = work.join(file_name(&case.source));
    fs::copy(&case.source, &copy).map_err(|e| {
        format!(
            "{} could not be copied to {}: {e}",
            case.source.display(),
            copy.display()
        )
    })?;
    let sidecar_copy = match &case.sidecar {
        Some(sidecar) => {
            let target = work.join(file_name(sidecar));
            fs::copy(sidecar, &target).map_err(|e| {
                format!(
                    "{} could not be copied to {}: {e}",
                    sidecar.display(),
                    target.display()
                )
            })?;
            Some(target)
        }
        None => None,
    };

    let start = Instant::now();
    let read = et
        .read_metadata(&copy, Fallback::default())
        .map_err(|e| format!("read: {e}"))?;
    let read_time = start.elapsed();
    let kind = read.state.kind;

    let target = match (kind, &sidecar_copy) {
        (FileKind::Raw, Some(sidecar)) => Some(sidecar.clone()),
        (FileKind::Raw, None) => None,
        _ => Some(copy.clone()),
    };
    let snapshot = match target {
        Some(target) => {
            let start = Instant::now();
            et.snapshot(&target)
                .map_err(|e| format!("snapshot of {}: {e}", target.display()))?;
            Some(start.elapsed())
        }
        None => None,
    };

    let mut metadata = read.reconciled.metadata;
    metadata
        .set("photoshop:City", Value::Text("Malmö".to_string()))
        .map_err(|e| format!("City: {e}"))?;
    let plan =
        plan(&metadata, &read.state, WriteSettings::default()).map_err(|e| format!("plan: {e}"))?;

    let check = Timed {
        check: RawImageCheck::without_release(),
        time: Cell::new(None),
    };
    let start = Instant::now();
    let report = et.write_safely(&copy, &plan, &check, journal);
    let write = start.elapsed();
    report.map_err(|e| format!("write: {e}"))?;

    Ok(RunTimes {
        kind,
        read: read_time,
        snapshot,
        write,
        check: check.time.get().unwrap_or_default(),
    })
}

/// Deletes the copies of `source` in `work`: every file whose name starts with its stem and a
/// dot (the image, its `.xmp`, and any `.pwtmp-…` or `.pwbak-…` file). Says which it cannot.
fn cleanup(source: &Path, work: &Path) {
    let Some(stem) = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
    else {
        return;
    };
    let prefix = format!("{stem}.");
    let entries = match fs::read_dir(work) {
        Ok(entries) => entries,
        Err(e) => {
            println!("  The work folder {} cannot be read: {e}", work.display());
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ours = path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().to_lowercase().starts_with(&prefix));
        if ours
            && path.is_file()
            && let Err(e) = fs::remove_file(&path)
        {
            println!("  {} could not be deleted: {e}", path.display());
        }
    }
}

fn print_row(row: &Row) {
    println!(
        "  {:.1} MB, {}, {} runs, {} failed",
        row.megabytes,
        kind_name(row.kind),
        row.runs.len(),
        row.failed
    );
    let steps = steps(row);
    for (step, times) in &steps {
        let list = times
            .iter()
            .map(|t| format!("{:.1}", ms(*t)))
            .collect::<Vec<_>>()
            .join(", ");
        match median(times) {
            Some(m) => println!("  {step}: {list} ms; median {:.1} ms", ms(m)),
            None => println!("  {step}: none"),
        }
    }
}

/// The steps of a row, each with the times of its successful runs.
fn steps(row: &Row) -> Vec<(&'static str, Vec<Duration>)> {
    let mut steps = vec![
        ("read_metadata", row.runs.iter().map(|r| r.read).collect()),
        (
            "snapshot",
            row.runs.iter().filter_map(|r| r.snapshot).collect(),
        ),
        ("write_safely", row.runs.iter().map(|r| r.write).collect()),
        ("check_copy", row.runs.iter().map(|r| r.check).collect()),
    ];
    steps.push((
        "write_safely without check_copy",
        row.runs
            .iter()
            .map(|r| r.write.saturating_sub(r.check))
            .collect(),
    ));
    steps.push((
        "read_metadata + write_safely",
        row.runs.iter().map(|r| r.read + r.write).collect(),
    ));
    steps
}

fn print_table(rows: &[Row]) {
    println!(
        "| File | Case | MB | Kind | read_metadata | snapshot | write_safely | check_copy | \
         write without check | read + write | Runs (failed) |"
    );
    println!("|---|---|---:|---|---:|---:|---:|---:|---:|---:|---:|");
    for row in rows {
        let medians = steps(row)
            .iter()
            .map(|(_, times)| median(times).map_or("–".to_string(), |m| format!("{:.0}", ms(m))))
            .collect::<Vec<_>>()
            .join(" | ");
        println!(
            "| {} | {} | {:.1} | {} | {medians} | {} ({}) |",
            row.name,
            if row.label.is_empty() {
                "–"
            } else {
                row.label
            },
            row.megabytes,
            kind_name(row.kind),
            row.runs.len(),
            row.failed
        );
    }
    println!();
    println!("Times are medians in ms over the successful runs.");
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

fn kind_name(kind: Option<FileKind>) -> &'static str {
    match kind {
        Some(FileKind::Jpeg) => "JPEG",
        Some(FileKind::Tiff) => "TIFF",
        Some(FileKind::Dng) => "DNG",
        Some(FileKind::Raw) => "RAW",
        None => "?",
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
