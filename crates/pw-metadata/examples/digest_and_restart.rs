//! Checks whether the two reasons for extra steps in the safe write still hold with the ExifTool in
//! use, and times both steps (T265, ADR 0007 §9.6 point 3, Q19):
//!
//! - **The digest pass** (§5.2, §9.2): measured with ExifTool 13.59 in T92, `-IPTCDigest=new` in
//!   the same call as an IIM write stores an out-of-date checksum in a TIFF or DNG, so the safe
//!   write runs a second call `-IPTCDigest=new -overwrite_original <copy>`. For each file (a JPEG
//!   is the control) one copy is written by the one call alone and one the way the safe write
//!   writes it (the same call, then the digest pass); each copy's checksum is read by a new
//!   ExifTool process that has written nothing, and the digest pass is timed.
//! - **The TIFF restart** (§9.3): measured in T92, after a process has written the checksum to a
//!   TIFF whose Photoshop block holds a checksum followed by an `AgHg` resource, that process reads
//!   the block of every such TIFF wrongly until it ends. For each TIFF a process P writes one copy
//!   the way the safe write does, then P and a new process Q each take the snapshot of a second,
//!   untouched copy; the two snapshots are compared without the `System` group. Then P's
//!   [`ExifTool::restart`] is timed; it includes ending the old process (up to 2 s, `Drop`). It
//!   returns once the new process is started, so the restart is also timed up to the new
//!   process's first answer (`-ver`).
//!   Only TIFFs are rechecked here, not the DNG half of the reason.
//!
//! Run in a release build:
//!
//! ```text
//! cargo run --release -p pw-metadata --example digest_and_restart -- [--runs <n>] [--work <folder>] <files>
//! ```
//!
//! `--runs` is the number of runs per file, for the times (default 3); `--work` is the folder the
//! copies are written in, which must be new or empty and not inside the folder of a file given
//! (default: a new folder under the system's temporary folder). ExifTool is `PW_EXIFTOOL_PL` run
//! through `perl` when that is set, otherwise the ExifTool `ExifTool::start` finds.
//!
//! The program only reads the files it is given. It copies each into the work folder, writes only
//! those copies, and deletes them afterwards; a file that cannot be deleted is named.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use pw_metadata::ExifTool;
use pw_metadata::verify::FileSnapshot;

/// The number of runs per file when `--runs` is not given.
const DEFAULT_RUNS: usize = 3;

/// The write of the T92 tests: an IIM and XMP change and the checksum in one call.
const WRITE_ARGS: &[&str] = &[
    "-IPTC:CodedCharacterSet=UTF8",
    "-IPTC:City=Lund",
    "-XMP-photoshop:City=Lund",
    "-IPTCDigest=new",
];

/// The argument that writes a new checksum; with `-overwrite_original` the safe write's digest
/// pass.
const NEW_DIGEST: &str = "-IPTCDigest=new";

/// The listing keys of the stored and the current checksum (as the T92 tests' `digest_state`).
const STORED_DIGEST: &str = "Photoshop:IPTCDigest";
const CURRENT_DIGEST: &str = "File:CurrentIPTCDigest";

/// The group left out when two snapshots of one file are compared (`System:FileAccessDate` may
/// change with a read).
const IGNORED_GROUP: &str = "System:";

/// The `AgHg` resource as `ag_hg_block` in the T92 tests lays it out: the signature, then the
/// big-endian id 0x1f40.
const AG_HG: &[u8] = b"AgHg\x1f\x40";

/// The command line.
struct Options {
    files: Vec<PathBuf>,
    runs: usize,
    work: Option<PathBuf>,
}

/// The kinds of file the program measures, by extension.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Jpeg,
    Tiff,
    Dng,
}

impl Kind {
    fn of(path: &Path) -> Option<Kind> {
        let extension = path.extension()?.to_string_lossy().to_lowercase();
        match extension.as_str() {
            "jpg" | "jpeg" => Some(Kind::Jpeg),
            "tif" | "tiff" => Some(Kind::Tiff),
            "dng" => Some(Kind::Dng),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Jpeg => "JPEG",
            Kind::Tiff => "TIFF",
            Kind::Dng => "DNG",
        }
    }
}

/// The state of a copy's stored checksum, read by a process that has written nothing.
#[derive(Clone, PartialEq, Eq)]
enum Digest {
    Current,
    OutOfDate,
    Missing,
    /// A stored checksum but no `File:CurrentIPTCDigest` to compare it with.
    NoCurrent,
}

impl Digest {
    fn text(&self) -> &'static str {
        match self {
            Digest::Current => "current",
            Digest::OutOfDate => "out of date",
            Digest::Missing => "missing",
            Digest::NoCurrent => "no current checksum to compare",
        }
    }
}

/// All runs of one file.
struct Row {
    name: String,
    kind: Kind,
    megabytes: f64,
    /// The checksum of the copy written by the one call, per successful run.
    same_call: Vec<Digest>,
    /// The checksum of the copy written the safe write's way, per successful run.
    safe_write: Vec<Digest>,
    digest_pass: Vec<Duration>,
    /// For a TIFF: whether it has an `AgHg` resource (`None` when it could not be read).
    ag_hg: Option<bool>,
    /// For a TIFF: point 3's result per successful run ("same" or the first difference).
    misread: Vec<String>,
    restart: Vec<Duration>,
    /// For a TIFF: the restart and the new process's first answer (`-ver`), per successful run.
    restart_answer: Vec<Duration>,
    failed: usize,
}

fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            eprintln!(
                "Usage: cargo run --release -p pw-metadata --example digest_and_restart -- \
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
            std::env::temp_dir().join(format!("pw-digest-restart-{}", std::process::id())),
            true,
        ),
    };
    fs::create_dir_all(&work)
        .map_err(|e| format!("The work folder {} cannot be made: {e}", work.display()))?;
    refuse_source_folders(&work, &options.files)?;
    println!("Work folder: {}", work.display());

    let source = exiftool_source();
    let mut first = start_exiftool(&source)?;
    let version = first
        .version()
        .map_err(|e| format!("ExifTool from {} does not answer: {e}", source.text))?;
    println!("ExifTool {version} from {}", source.text);
    drop(first);

    let mut rows = Vec::new();
    for file in &options.files {
        match Kind::of(file) {
            Some(kind) => rows.push(measure(&source, file, kind, &work, options.runs)),
            None => println!(
                "\n{}: not a JPEG, TIFF or DNG by its extension; skipped",
                file.display()
            ),
        }
    }
    println!();
    print_table(&rows);

    if made_work && let Err(e) = fs::remove_dir(&work) {
        println!(
            "The work folder {} could not be deleted: {e}",
            work.display()
        );
    }
    Ok(())
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

/// Refuses a work folder that is the folder of one of the files given, or inside it: the program
/// never writes where the sources are.
fn refuse_source_folders(work: &Path, files: &[PathBuf]) -> Result<(), String> {
    let work = fs::canonicalize(work)
        .map_err(|e| format!("The work folder {} cannot be found: {e}", work.display()))?;
    for file in files {
        let folder = match file.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        if fs::canonicalize(folder).is_ok_and(|folder| work.starts_with(folder)) {
            return Err(format!(
                "The work folder {} is in the folder of {}; choose another one.",
                work.display(),
                file.display()
            ));
        }
    }
    Ok(())
}

/// Where ExifTool comes from: `PW_EXIFTOOL_PL` through `perl`, or what `ExifTool::start` finds.
struct Source {
    perl_script: Option<String>,
    text: String,
}

fn exiftool_source() -> Source {
    match std::env::var("PW_EXIFTOOL_PL") {
        Ok(pl) => Source {
            text: format!("perl {pl} (PW_EXIFTOOL_PL)"),
            perl_script: Some(pl),
        },
        Err(_) => Source {
            perl_script: None,
            text: "the program folder or PATH (ExifTool::start)".to_string(),
        },
    }
}

/// Starts a new ExifTool process from `source`.
fn start_exiftool(source: &Source) -> Result<ExifTool, String> {
    let started = match &source.perl_script {
        Some(pl) => ExifTool::start_with("perl", &[pl]),
        None => ExifTool::start(),
    };
    started.map_err(|e| format!("ExifTool could not be started from {}: {e}", source.text))
}

/// All runs of one file, printed as they are made.
fn measure(source: &Source, file: &Path, kind: Kind, work: &Path, runs: usize) -> Row {
    let name = file_name(file);
    let megabytes = fs::metadata(file)
        .map(|m| m.len() as f64 / 1_000_000.0)
        .unwrap_or(0.0);
    println!();
    println!("{name}: {}, {megabytes:.1} MB", kind.name());
    let mut row = Row {
        name,
        kind,
        megabytes,
        same_call: Vec::new(),
        safe_write: Vec::new(),
        digest_pass: Vec::new(),
        ag_hg: None,
        misread: Vec::new(),
        restart: Vec::new(),
        restart_answer: Vec::new(),
        failed: 0,
    };
    if kind == Kind::Tiff {
        row.ag_hg = match fs::read(file) {
            Ok(bytes) => Some(bytes.windows(AG_HG.len()).any(|w| w == AG_HG)),
            Err(e) => {
                println!("  {} cannot be read: {e}", file.display());
                None
            }
        };
        println!(
            "  AgHg resource (id 0x1f40): {}",
            match row.ag_hg {
                Some(true) => "yes",
                Some(false) => "no",
                None => "unknown",
            }
        );
    }
    for number in 1..=runs {
        let mut made = Vec::new();
        let result = run_once(source, file, kind, work, &mut made, &mut row);
        cleanup(&made);
        if let Err(message) = result {
            println!("  run {number} failed: {message}");
            row.failed += 1;
        }
    }
    print_row(&row);
    row
}

/// One run: point 2 for every file, point 3 for a TIFF. Every file it makes is put in `made`, so
/// the caller can delete it.
fn run_once(
    source: &Source,
    file: &Path,
    kind: Kind,
    work: &Path,
    made: &mut Vec<PathBuf>,
    row: &mut Row,
) -> Result<(), String> {
    let (same_call, safe_write, digest_pass) = digest_in_same_call(source, file, work, made)?;
    println!(
        "  checksum: one call {}; the safe write's sequence {}; digest pass {:.1} ms",
        same_call.text(),
        safe_write.text(),
        ms(digest_pass)
    );
    row.same_call.push(same_call);
    row.safe_write.push(safe_write);
    row.digest_pass.push(digest_pass);
    if kind == Kind::Tiff {
        let (misread, restart, restart_answer) = misread_after_write(source, file, work, made)?;
        println!(
            "  untouched copy read by the writing process and a new one: {misread}; \
             restart {:.1} ms (includes ending the old process, up to 2 s), with the new \
             process's first answer {:.1} ms",
            ms(restart),
            ms(restart_answer)
        );
        row.misread.push(misread);
        row.restart.push(restart);
        row.restart_answer.push(restart_answer);
    }
    Ok(())
}

/// Point 2: a copy written by the one call, and a copy written the safe write's way (the same
/// call, then the digest pass), each checksum read by a new process. The writes are made by a new
/// process too, so a misread left by an earlier TIFF write cannot affect them. Returns both states
/// and the time of the digest pass.
fn digest_in_same_call(
    source: &Source,
    file: &Path,
    work: &Path,
    made: &mut Vec<PathBuf>,
) -> Result<(Digest, Digest, Duration), String> {
    let source_copy = copy_into(file, work, "src", made)?;
    let one_call = work_path(file, work, "one-call");
    let safe = work_path(file, work, "safe");
    let mut writer = start_exiftool(source)?;

    made.push(one_call.clone());
    write_copy(&mut writer, &source_copy, &one_call, false)?;
    made.push(safe.clone());
    write_copy(&mut writer, &source_copy, &safe, false)?;
    let start = Instant::now();
    digest_pass(&mut writer, &safe, false)?;
    let digest_time = start.elapsed();
    drop(writer);

    let same_call = digest_state(source, &one_call)?;
    let safe_write = digest_state(source, &safe)?;
    Ok((same_call, safe_write, digest_time))
}

/// Point 3: a new process P writes copy A the safe write's way, then P and a new process Q take
/// the snapshot of the untouched copy B. Returns "same" or the first difference, the time of P's
/// restart, and the time of the restart and the new process's first answer (`-ver`):
/// [`ExifTool::restart`] returns once the new process is started, so its start-up is paid by the
/// next call.
fn misread_after_write(
    source: &Source,
    file: &Path,
    work: &Path,
    made: &mut Vec<PathBuf>,
) -> Result<(String, Duration, Duration), String> {
    let a = copy_into(file, work, "a", made)?;
    let b = copy_into(file, work, "b", made)?;
    let a2 = work_path(file, work, "a2");

    let mut p = start_exiftool(source)?;
    made.push(a2.clone());
    write_copy(&mut p, &a, &a2, true)?;
    digest_pass(&mut p, &a2, true)?;
    let by_p = p
        .snapshot(&b)
        .map_err(|e| format!("snapshot of {} by P: {e}", b.display()))?;
    let mut q = start_exiftool(source)?;
    let by_q = q
        .snapshot(&b)
        .map_err(|e| format!("snapshot of {} by Q: {e}", b.display()))?;

    let difference = first_difference(&by_p, &by_q);
    if by_p.validate != by_q.validate {
        print_warnings("P (wrote A)", &mut p, &b);
        print_warnings("Q (new)", &mut q, &b);
    }
    drop(q);

    let start = Instant::now();
    p.restart().map_err(|e| format!("restart of P: {e}"))?;
    let restart = start.elapsed();
    p.version()
        .map_err(|e| format!("first answer of P after the restart: {e}"))?;
    let restart_answer = start.elapsed();
    Ok((
        difference.unwrap_or_else(|| "same".to_string()),
        restart,
        restart_answer,
    ))
}

/// Writes `copy` from `source` with [`WRITE_ARGS`] and `-o`, as the safe write's first call does.
fn write_copy(et: &mut ExifTool, source: &Path, copy: &Path, always: bool) -> Result<(), String> {
    let mut args = vec!["-o".to_string(), path_text(copy)];
    args.extend(WRITE_ARGS.iter().map(|arg| arg.to_string()));
    args.push(path_text(source));
    let answer = et
        .execute(&args)
        .map_err(|e| format!("write of {}: {e}", copy.display()))?;
    print_unusual_answer(&format!("write of {}", file_name(copy)), &answer, always);
    if !copy.is_file() {
        return Err(format!(
            "ExifTool did not write {}: {}",
            copy.display(),
            answer.trim()
        ));
    }
    Ok(())
}

/// The safe write's digest pass: `-IPTCDigest=new -overwrite_original <copy>`.
fn digest_pass(et: &mut ExifTool, copy: &Path, always: bool) -> Result<(), String> {
    let args = [
        NEW_DIGEST.to_string(),
        "-overwrite_original".to_string(),
        path_text(copy),
    ];
    let answer = et
        .execute(&args)
        .map_err(|e| format!("digest pass on {}: {e}", copy.display()))?;
    print_unusual_answer(
        &format!("digest pass on {}", file_name(copy)),
        &answer,
        always,
    );
    Ok(())
}

/// Prints an ExifTool answer to a write: always when `always` is set (point 3's process P),
/// otherwise unless it is exactly one file updated or created.
fn print_unusual_answer(what: &str, answer: &str, always: bool) {
    let answer = answer.trim();
    if always || (answer != "1 image files updated" && answer != "1 image files created") {
        println!("  ExifTool's answer to the {what}:");
        for line in answer.lines() {
            println!("    {line}");
        }
    }
}

/// The state of `file`'s stored checksum, read by a new process that has written nothing.
fn digest_state(source: &Source, file: &Path) -> Result<Digest, String> {
    let mut reader = start_exiftool(source)?;
    let snapshot = reader
        .snapshot(file)
        .map_err(|e| format!("snapshot of {}: {e}", file.display()))?;
    let stored = snapshot.listing.get(STORED_DIGEST);
    let current = snapshot.listing.get(CURRENT_DIGEST);
    Ok(match (stored, current) {
        (None, _) => Digest::Missing,
        (Some(_), None) => Digest::NoCurrent,
        (Some(stored), Some(current)) if stored == current => Digest::Current,
        (Some(_), Some(_)) => Digest::OutOfDate,
    })
}

/// The first difference between two snapshots of one file, without the `System` group: the file
/// type, size, image data hash, validation counts, then the first listing key that differs.
fn first_difference(p: &FileSnapshot, q: &FileSnapshot) -> Option<String> {
    if p.file_type != q.file_type {
        return Some(format!("File type: P {}, Q {}", p.file_type, q.file_type));
    }
    if p.size != q.size {
        return Some(format!("size: P {}, Q {}", p.size, q.size));
    }
    if p.image_data_hash != q.image_data_hash {
        return Some(format!(
            "image data hash: P {}, Q {}",
            or_none(p.image_data_hash.as_deref()),
            or_none(q.image_data_hash.as_deref())
        ));
    }
    if p.validate != q.validate {
        let text = |v: Option<pw_metadata::verify::Validate>| {
            v.map_or("(none)".to_string(), |v| v.to_string())
        };
        return Some(format!(
            "validate: P {}, Q {}",
            text(p.validate),
            text(q.validate)
        ));
    }
    let differing: BTreeSet<&String> = p
        .listing
        .keys()
        .chain(q.listing.keys())
        .filter(|key| !key.starts_with(IGNORED_GROUP))
        .filter(|key| p.listing.get(*key) != q.listing.get(*key))
        .collect();
    differing.first().map(|key| {
        format!(
            "{key}: P {}, Q {}",
            or_none(p.listing.get(*key).map(String::as_str)),
            or_none(q.listing.get(*key).map(String::as_str))
        )
    })
}

/// Prints the validation warnings `et` gives for `file` (`-validate -warning -a -G1 -j`).
/// JSON keeps only one of several `ExifTool:Warning`s, so the same call without `-j` follows; it
/// lists every warning on its own line.
fn print_warnings(who: &str, et: &mut ExifTool, file: &Path) {
    for (label, options) in [
        ("JSON", &["-validate", "-warning", "-a", "-G1", "-j"][..]),
        ("every warning", &["-validate", "-warning", "-a", "-G1"][..]),
    ] {
        let args: Vec<String> = options
            .iter()
            .map(|arg| arg.to_string())
            .chain([path_text(file)])
            .collect();
        println!("  validation of {} by {who} ({label}):", file_name(file));
        match et.execute(&args) {
            Ok(answer) => {
                for line in answer.trim().lines() {
                    println!("    {line}");
                }
            }
            Err(e) => println!("    could not be read: {e}"),
        }
    }
}

/// Copies `file` into `work` as `<stem>.<tag>.<ext>` and puts the copy in `made`.
fn copy_into(
    file: &Path,
    work: &Path,
    tag: &str,
    made: &mut Vec<PathBuf>,
) -> Result<PathBuf, String> {
    let copy = work_path(file, work, tag);
    made.push(copy.clone());
    fs::copy(file, &copy).map_err(|e| {
        format!(
            "{} could not be copied to {}: {e}",
            file.display(),
            copy.display()
        )
    })?;
    Ok(copy)
}

/// `<work>/<stem>.<tag>.<ext>` for `file`.
fn work_path(file: &Path, work: &Path, tag: &str) -> PathBuf {
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = file
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    work.join(format!("{stem}.{tag}.{extension}"))
}

/// Deletes the files a run made (and ExifTool's temporary file of `-overwrite_original`, should
/// one be left); names each that exists and cannot be deleted.
fn cleanup(made: &[PathBuf]) {
    for path in made {
        let mut temporary = path.clone().into_os_string();
        temporary.push("_exiftool_tmp");
        for path in [path.clone(), PathBuf::from(temporary)] {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => println!("  {} could not be deleted: {e}", path.display()),
            }
        }
    }
}

fn print_row(row: &Row) {
    println!(
        "  {} runs, {} failed",
        row.digest_pass.len() + row.failed,
        row.failed
    );
    println!("  checksum after the one call: {}", states(&row.same_call));
    println!(
        "  checksum after the safe write's sequence: {}",
        states(&row.safe_write)
    );
    print_times("digest pass", &row.digest_pass);
    if row.kind == Kind::Tiff {
        println!(
            "  untouched copy read by P and Q: {}",
            distinct(&row.misread)
        );
        print_times(
            "restart (includes ending the old process, up to 2 s)",
            &row.restart,
        );
        print_times(
            "restart and the new process's first answer",
            &row.restart_answer,
        );
    }
}

fn print_times(step: &str, times: &[Duration]) {
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

fn print_table(rows: &[Row]) {
    println!(
        "| File | Kind | MB | Checksum, one call | Checksum, safe write's sequence | \
         Digest pass | AgHg | Untouched copy read by P and Q | Restart | Restart and first answer | \
         Runs (failed) |"
    );
    println!("|---|---|---:|---|---|---:|---|---|---:|---:|---:|");
    for row in rows {
        let tiff = row.kind == Kind::Tiff;
        println!(
            "| {} | {} | {:.1} | {} | {} | {} | {} | {} | {} | {} | {} ({}) |",
            row.name,
            row.kind.name(),
            row.megabytes,
            states(&row.same_call),
            states(&row.safe_write),
            median_text(&row.digest_pass),
            match (tiff, row.ag_hg) {
                (true, Some(true)) => "yes",
                (true, Some(false)) => "no",
                (true, None) => "?",
                (false, _) => "–",
            },
            if tiff {
                distinct(&row.misread)
            } else {
                "–".to_string()
            },
            if tiff {
                median_text(&row.restart)
            } else {
                "–".to_string()
            },
            if tiff {
                median_text(&row.restart_answer)
            } else {
                "–".to_string()
            },
            row.digest_pass.len() + row.failed,
            row.failed
        );
    }
    println!();
    println!(
        "Times are medians in ms over the successful runs; the restart includes ending the old \
         process (up to 2 s)."
    );
}

/// The distinct checksum states of the runs, in the order first seen.
fn states(states: &[Digest]) -> String {
    let texts: Vec<String> = states.iter().map(|s| s.text().to_string()).collect();
    distinct(&texts)
}

/// The distinct texts, in the order first seen, joined by "; " ("–" for none).
fn distinct(texts: &[String]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for text in texts {
        if !seen.contains(&text.as_str()) {
            seen.push(text);
        }
    }
    if seen.is_empty() {
        "–".to_string()
    } else {
        seen.join("; ")
    }
}

fn median_text(times: &[Duration]) -> String {
    median(times).map_or("–".to_string(), |m| format!("{:.0}", ms(m)))
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

fn or_none(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("\"{value}\""),
        None => "(none)".to_string(),
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
