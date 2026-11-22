//! Times a batch write with 1, 2 and 4 ExifTool write processes (ADR 0007 §9.6 point 2, Q18;
//! T267): the metadata worker ([`MetadataWorker`]) is given one `write_edits` per file of the
//! batch, each with one City edit (`photoshop:City` = "Malmö"), and the wall time is taken from
//! the first request until every file has answered. The number of processes is the worker's
//! `WriteSetup::write_processes`.
//!
//! Run in a release build:
//!
//! ```text
//! cargo run --release -p pw-imagecheck --example batch_timing -- [--copies <k>] [--processes <list>] [--runs <n>] [--work <folder>] <files>
//! ```
//!
//! - `--copies` is how many times each source file is copied into the batch (default 3);
//! - `--processes` is the list of process counts to time, each 1–4 (default `1,2,4`);
//! - `--runs` is the number of runs per process count (default 3);
//! - `--work` is the folder the copies are written in, which must be new or empty and not the
//!   folder of a source file or inside it (default: a new folder under the system's temporary
//!   folder, deleted at the end).
//!
//! ExifTool is `PW_EXIFTOOL_PL` run through `perl` when that is set, otherwise the worker's own
//! choice: `exiftool.exe` in the program folder, else `exiftool` on PATH.
//!
//! Each run starts a new worker (its start is not timed) and writes the batch twice, fresh
//! copies each time: with more than one process the pool's processes start when they are first
//! needed, so the first batch ("first") includes up to that many ExifTool starts, the second
//! ("warm") none. While it sends, the program takes the worker's events (the worker blocks when
//! its event channel is full); with nothing to send and no event it sleeps [`IDLE_SLEEP`] (1 ms),
//! which adds at most about 1 ms to a run's time. After a run the worker is dropped and the
//! program waits [`SETTLE`] (ExifTool's drop waits at most 2 s for a process to end) before the
//! copies and the journal are deleted.
//!
//! The program only reads the files it is given: it copies them into the work folder as
//! `<stem>-<s>-<i>.<ext>` (`s` the source's index, `i` the copy's) and writes only the copies.
//!
//! The safe write uses Windows file operations, so the writes succeed only on Windows; elsewhere
//! the program compiles and runs, but every write fails with the file operations' refusal.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

use pw_imagecheck::RawImageCheck;
use pw_metadata::edit::{Edit, EditOptions};
use pw_metadata::journal::Journal;
use pw_metadata::safe_write::WriteReport;
use pw_metadata::value::Value;
use pw_metadata::{MIN_EXIFTOOL_VERSION, MetadataWorker, SubmitError, WorkerEvent, WriteSetup};

/// How many times each source is copied into the batch when `--copies` is not given.
const DEFAULT_COPIES: usize = 3;
/// The process counts timed when `--processes` is not given.
const DEFAULT_PROCESSES: [usize; 3] = [1, 2, 4];
/// The number of runs per process count when `--runs` is not given.
const DEFAULT_RUNS: usize = 3;
/// The most write processes a worker has (ADR 0007 §9.6 point 2).
const MAX_PROCESSES: usize = 4;
/// How long the program sleeps when it has nothing to send and no event is waiting.
const IDLE_SLEEP: Duration = Duration::from_millis(1);
/// How long the program waits after dropping a worker before it deletes the copies: at least
/// ExifTool's own shutdown wait (2 s), so no process of the worker still holds a file.
const SETTLE: Duration = Duration::from_millis(3000);
/// How long the program waits for a new worker's ExifTool version before it gives up.
const VERSION_WAIT: Duration = Duration::from_secs(60);

/// The command line.
struct Options {
    files: Vec<PathBuf>,
    copies: usize,
    processes: Vec<usize>,
    runs: usize,
    work: Option<PathBuf>,
}

/// Which of a run's two batches.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Batch {
    /// The worker's first batch: includes the pool's process starts.
    First,
    /// The second batch on the same worker.
    Warm,
}

impl Batch {
    fn name(self) -> &'static str {
        match self {
            Batch::First => "first",
            Batch::Warm => "warm",
        }
    }
}

/// One timed batch.
struct Timing {
    processes: usize,
    run: usize,
    batch: Batch,
    files: usize,
    failed: usize,
    unchanged: usize,
    wall: Duration,
}

fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            eprintln!(
                "Usage: cargo run --release -p pw-imagecheck --example batch_timing -- \
                 [--copies <k>] [--processes <list>] [--runs <n>] [--work <folder>] <files>"
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
        copies: DEFAULT_COPIES,
        processes: DEFAULT_PROCESSES.to_vec(),
        runs: DEFAULT_RUNS,
        work: None,
    };
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--copies" => {
                let value = args.next().ok_or("--copies needs a number")?;
                options.copies = positive(&value, "--copies")?;
            }
            "--runs" => {
                let value = args.next().ok_or("--runs needs a number")?;
                options.runs = positive(&value, "--runs")?;
            }
            "--processes" => {
                let value = args
                    .next()
                    .ok_or("--processes needs a list such as 1,2,4")?;
                options.processes = parse_processes(&value)?;
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
    for file in &options.files {
        if !file.is_file() {
            return Err(format!("{} is not a file.", file.display()));
        }
    }
    Ok(options)
}

fn positive(value: &str, flag: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(number) if number > 0 => Ok(number),
        _ => Err(format!("{flag} needs a number above 0, not {value}")),
    }
}

/// A comma-separated list of process counts, each 1 to [`MAX_PROCESSES`].
fn parse_processes(value: &str) -> Result<Vec<usize>, String> {
    let mut list = Vec::new();
    for part in value.split(',') {
        match part.trim().parse::<usize>() {
            Ok(number) if (1..=MAX_PROCESSES).contains(&number) => list.push(number),
            _ => {
                return Err(format!(
                    "--processes needs numbers from 1 to {MAX_PROCESSES}, not {part}"
                ));
            }
        }
    }
    if list.is_empty() {
        return Err("--processes needs at least one number".to_string());
    }
    Ok(list)
}

fn run(options: &Options) -> Result<(), String> {
    let (work, made_work) = match &options.work {
        Some(work) => {
            refuse_used_folder(work)?;
            (work.clone(), false)
        }
        None => (
            std::env::temp_dir().join(format!("pw-batch-timing-{}", std::process::id())),
            true,
        ),
    };
    fs::create_dir_all(&work)
        .map_err(|e| format!("The work folder {} cannot be made: {e}", work.display()))?;
    refuse_source_folders(&work, &options.files)?;
    println!("Work folder: {}", work.display());

    let batch = batch_names(&options.files, options.copies);
    println!(
        "Batch: {} files ({} sources, {} copies each)",
        batch.len(),
        options.files.len(),
        options.copies
    );

    let result = measure_all(options, &work, &batch);

    if made_work && let Err(e) = fs::remove_dir(&work) {
        println!(
            "The work folder {} could not be deleted: {e}",
            work.display()
        );
    }
    let timings = result?;
    println!();
    print_table(&timings, &options.processes);
    Ok(())
}

/// Every run of every process count, printed as each batch ends.
fn measure_all(
    options: &Options,
    work: &Path,
    batch: &[(PathBuf, String)],
) -> Result<Vec<Timing>, String> {
    let mut timings = Vec::new();
    let mut version_printed = false;
    for &processes in &options.processes {
        for run in 1..=options.runs {
            let result = measure_run(work, batch, processes, run, &mut version_printed);
            cleanup_run(work, batch);
            timings.extend(result?);
        }
    }
    Ok(timings)
}

/// One run: a new worker with `processes` write processes, the batch written twice ("first",
/// "warm"), the worker dropped and waited for. The copies left and the journal are deleted by the
/// caller.
fn measure_run(
    work: &Path,
    batch: &[(PathBuf, String)],
    processes: usize,
    run: usize,
    version_printed: &mut bool,
) -> Result<Vec<Timing>, String> {
    let journal_dir = work.join("journal");
    let journal = Journal::open(&journal_dir).map_err(|e| {
        format!(
            "The journal in {} cannot be opened: {e}",
            journal_dir.display()
        )
    })?;
    let setup = WriteSetup {
        journal,
        checks: Arc::new(RawImageCheck::without_release()),
        options: EditOptions::default(),
        write_processes: processes,
    };
    let mut worker = start_worker(setup, version_printed)?;

    let mut timings = Vec::new();
    let mut outcome = Ok(());
    for kind in [Batch::First, Batch::Warm] {
        let files = match copy_batch(work, batch) {
            Ok(files) => files,
            Err(message) => {
                outcome = Err(message);
                break;
            }
        };
        match write_batch(&mut worker, &files) {
            Ok((failed, unchanged, wall)) => {
                let timing = Timing {
                    processes,
                    run,
                    batch: kind,
                    files: files.len(),
                    failed,
                    unchanged,
                    wall,
                };
                print_timing(&timing);
                timings.push(timing);
            }
            Err(message) => {
                outcome = Err(message);
                break;
            }
        }
        // Every write has answered; the next batch is a fresh copy under the same names.
        delete_copies(work, batch);
    }
    drop(worker);
    std::thread::sleep(SETTLE);
    outcome.map(|()| timings)
}

/// Starts a worker as point 1 of T267 says and waits until its ExifTool version is known. The
/// first worker's version and source are printed; an unknown or too old version stops the program.
fn start_worker(setup: WriteSetup, version_printed: &mut bool) -> Result<MetadataWorker, String> {
    let (started, source) = match std::env::var("PW_EXIFTOOL_PL") {
        Ok(pl) => (
            MetadataWorker::start_with("perl", &[&pl], Some(setup)),
            format!("perl {pl} (PW_EXIFTOOL_PL)"),
        ),
        Err(_) => (
            MetadataWorker::start(Some(setup)),
            "exiftool.exe in the program folder, else exiftool on PATH".to_string(),
        ),
    };
    let worker =
        started.map_err(|e| format!("The worker could not be started with {source}: {e}"))?;
    let deadline = Instant::now() + VERSION_WAIT;
    let version = loop {
        if let Some(version) = worker.exiftool_version() {
            break version;
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "ExifTool from {source} gave no version within {} s.",
                VERSION_WAIT.as_secs()
            ));
        }
        std::thread::sleep(IDLE_SLEEP);
    };
    let version =
        version.map_err(|e| format!("ExifTool from {source} could not be started: {e}"))?;
    if version < MIN_EXIFTOOL_VERSION {
        return Err(format!(
            "ExifTool {version} from {source} is older than {MIN_EXIFTOOL_VERSION}."
        ));
    }
    if !*version_printed {
        println!("ExifTool {version} from {source}");
        *version_printed = true;
    }
    Ok(worker)
}

/// Sends one `write_edits` per file and takes the events until every file has answered. Returns
/// the failures, the unchanged writes and the wall time.
fn write_batch(
    worker: &mut MetadataWorker,
    files: &[PathBuf],
) -> Result<(usize, usize, Duration), String> {
    let mut sent = 0;
    let mut answered = 0;
    let mut failed = 0;
    let mut unchanged = 0;
    let start = Instant::now();
    while answered < files.len() {
        let mut progressed = false;
        loop {
            match worker.try_recv() {
                Ok(WorkerEvent::EditsWritten { path, result, .. }) => {
                    answered += 1;
                    progressed = true;
                    match result {
                        Ok(report) => {
                            if matches!(report.report, WriteReport::Unchanged) {
                                unchanged += 1;
                                println!("  {}: unchanged", path.display());
                            }
                        }
                        Err(e) => {
                            failed += 1;
                            println!("  {}: write failed: {e}", path.display());
                        }
                    }
                }
                Ok(other) => {
                    progressed = true;
                    println!("  unexpected event: {other:?}");
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err("The worker stopped before every file had answered.".to_string());
                }
            }
        }
        if let Some(file) = files.get(sent) {
            match worker.write_edits(file.clone(), vec![city_edit()]) {
                Ok(_) => {
                    sent += 1;
                    progressed = true;
                }
                Err(SubmitError::QueueFull) => {}
                Err(SubmitError::Stopped) => {
                    return Err("The worker stopped while the batch was sent.".to_string());
                }
            }
        }
        if !progressed {
            std::thread::sleep(IDLE_SLEEP);
        }
    }
    Ok((failed, unchanged, start.elapsed()))
}

/// The edit every file gets.
fn city_edit() -> Edit {
    Edit::Set {
        property: "photoshop:City".into(),
        field: None,
        value: Value::Text("Malmö".into()),
    }
}

/// The batch: each source with the name of each of its copies, `<stem>-<s>-<i>.<ext>`.
fn batch_names(files: &[PathBuf], copies: usize) -> Vec<(PathBuf, String)> {
    let mut batch = Vec::new();
    for (s, source) in files.iter().enumerate() {
        let stem = source
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = source
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        for i in 1..=copies {
            batch.push((source.clone(), format!("{stem}-{s}-{i}{ext}")));
        }
    }
    batch
}

/// Copies every source into the work folder under its batch names.
fn copy_batch(work: &Path, batch: &[(PathBuf, String)]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::with_capacity(batch.len());
    for (source, name) in batch {
        let copy = work.join(name);
        fs::copy(source, &copy).map_err(|e| {
            format!(
                "{} could not be copied to {}: {e}",
                source.display(),
                copy.display()
            )
        })?;
        files.push(copy);
    }
    Ok(files)
}

/// Deletes the copies of the batch in `work`: every file whose name starts with a copy's stem
/// and a dot (the image, an `.xmp`, and any `.pwtmp-…` or `.pwbak-…` file). Says which it cannot.
fn delete_copies(work: &Path, batch: &[(PathBuf, String)]) {
    let prefixes: Vec<String> = batch
        .iter()
        .map(|(_, name)| {
            let stem = Path::new(name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            format!("{stem}.")
        })
        .collect();
    let entries = match fs::read_dir(work) {
        Ok(entries) => entries,
        Err(e) => {
            println!("  The work folder {} cannot be read: {e}", work.display());
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ours = path.file_name().is_some_and(|n| {
            let n = n.to_string_lossy().to_lowercase();
            prefixes.iter().any(|prefix| n.starts_with(prefix))
        });
        if ours
            && path.is_file()
            && let Err(e) = fs::remove_file(&path)
        {
            println!("  {} could not be deleted: {e}", path.display());
        }
    }
}

/// After a run: the copies left and the journal folder.
fn cleanup_run(work: &Path, batch: &[(PathBuf, String)]) {
    delete_copies(work, batch);
    let journal_dir = work.join("journal");
    if journal_dir.exists()
        && let Err(e) = fs::remove_dir_all(&journal_dir)
    {
        println!(
            "  The journal folder {} could not be deleted: {e}",
            journal_dir.display()
        );
    }
}

/// Refuses a `--work` folder that exists and is not empty: the copies would overwrite its files
/// of the same name, the cleanup deletes every file named as a copy's stem and a dot, and the
/// `journal` folder is deleted after each run.
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

/// Refuses a work folder that is the folder of one of the source files or inside it: the program
/// never writes where the sources are.
fn refuse_source_folders(work: &Path, files: &[PathBuf]) -> Result<(), String> {
    let work = fs::canonicalize(work)
        .map_err(|e| format!("The work folder {} cannot be found: {e}", work.display()))?;
    for file in files {
        let folder = match file.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        if fs::canonicalize(folder).is_ok_and(|folder| work.starts_with(&folder)) {
            return Err(format!(
                "The work folder {} is in the folder of {}; choose another one.",
                work.display(),
                file.display()
            ));
        }
    }
    Ok(())
}

fn print_timing(timing: &Timing) {
    println!(
        "p = {}, run {}, {} batch: {} files, {} failed, {} unchanged, {:.0} ms, {:.2} files/s",
        timing.processes,
        timing.run,
        timing.batch.name(),
        timing.files,
        timing.failed,
        timing.unchanged,
        ms(timing.wall),
        files_per_second(timing.files, timing.wall)
    );
}

/// The Markdown table: per process count and batch the median wall time, files per second and
/// the speed-up against one process.
fn print_table(timings: &[Timing], processes: &[usize]) {
    println!(
        "| Processes | Batch | Files | Runs | Failed | Unchanged | Median wall time (ms) | \
         Files/s | Speed-up against 1 |"
    );
    println!("|---:|---|---:|---:|---:|---:|---:|---:|---:|");
    for batch in [Batch::First, Batch::Warm] {
        let baseline = median_of(timings, 1, batch);
        for &p in processes {
            let rows: Vec<&Timing> = timings
                .iter()
                .filter(|t| t.processes == p && t.batch == batch)
                .collect();
            let Some(median) = median_of(timings, p, batch) else {
                continue;
            };
            let files = rows.first().map_or(0, |t| t.files);
            let failed: usize = rows.iter().map(|t| t.failed).sum();
            let unchanged: usize = rows.iter().map(|t| t.unchanged).sum();
            let speed_up = baseline.map_or("–".to_string(), |b| {
                format!("{:.2}×", b.as_secs_f64() / median.as_secs_f64())
            });
            println!(
                "| {p} | {} | {files} | {} | {failed} | {unchanged} | {:.0} | {:.2} | {speed_up} |",
                batch.name(),
                rows.len(),
                ms(median),
                files_per_second(files, median)
            );
        }
    }
    println!();
    println!(
        "Failed and unchanged are added up over the runs; files/s is the files divided by the \
         median wall time."
    );
}

/// The median wall time of the batches with `processes` and `batch`; `None` when there is none.
fn median_of(timings: &[Timing], processes: usize, batch: Batch) -> Option<Duration> {
    let times: Vec<Duration> = timings
        .iter()
        .filter(|t| t.processes == processes && t.batch == batch)
        .map(|t| t.wall)
        .collect();
    median(&times)
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

fn files_per_second(files: usize, wall: Duration) -> f64 {
    let seconds = wall.as_secs_f64();
    if seconds > 0.0 {
        files as f64 / seconds
    } else {
        0.0
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
