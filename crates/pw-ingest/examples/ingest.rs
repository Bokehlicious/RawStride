//! Try the ingest engine from the command line (no UI yet).
//!
//! ```text
//! cargo run --release -p pw-ingest --example ingest -- <source> <dest> [options]
//!   --backup <dir>      second destination
//!   --folder <tmpl>     folder template      (default "{year}/{date}")
//!   --name <tmpl>       file-name template   (default "{name}" = keep)
//!   --job <text>        value for {job}
//!   --seq <n>           first running number (default 1)
//!   --go                actually copy (without it, only the plan is shown)
//! ```

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use pw_ingest::{Options, Template};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut pos = Vec::new();
    let (mut backup, mut folder, mut name, mut job, mut seq, mut go) = (
        None,
        "{year}/{date}".to_string(),
        "{name}".to_string(),
        String::new(),
        1u32,
        false,
    );
    while let Some(a) = args.next() {
        let mut val = || args.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--backup" => backup = Some(PathBuf::from(val()?)),
            "--folder" => folder = val()?,
            "--name" => name = val()?,
            "--job" => job = val()?,
            "--seq" => seq = val()?.parse()?,
            "--go" => go = true,
            _ => pos.push(PathBuf::from(a)),
        }
    }
    let [source, dest] = pos.as_slice() else {
        return Err(
            "usage: ingest <source> <dest> [--backup DIR] [--folder T] [--name T] \
                    [--job TEXT] [--seq N] [--go]"
                .into(),
        );
    };
    let opt = Options {
        dest: dest.clone(),
        backup,
        folder: Template::parse(&folder)?,
        name: Template::parse(&name)?,
        job,
        first_seq: seq,
    };

    let t = Instant::now();
    let shots = pw_ingest::scan(source)?;
    let no_exif = shots.iter().filter(|s| !s.time_from_exif).count();
    println!(
        "scanned {} shots in {:.2?} ({no_exif} without EXIF date)",
        shots.len(),
        t.elapsed()
    );
    let plan = pw_ingest::plan(&shots, &opt);
    for i in &plan.items {
        let what = match (i.to_dest, i.to_backup) {
            (false, false) => "present ",
            (true, _) if i.renamed_for_conflict => "RENAMED ",
            (true, _) => "copy    ",
            (false, true) => "backup  ",
        };
        println!("{what} {} -> {}", i.source.display(), i.rel.display());
    }
    println!(
        "{} files to copy ({:.1} MB), {} already present",
        plan.files_to_copy(),
        plan.bytes_to_copy() as f64 / 1e6,
        plan.already_present()
    );
    if !go {
        println!("dry run: add --go to copy");
        return Ok(());
    }
    let t = Instant::now();
    let cancel = AtomicBool::new(false);
    let report = pw_ingest::execute(&plan, &opt, &cancel, |p| {
        eprint!(
            "\r{}/{} files, {:.0} MB",
            p.files_done,
            p.files_total,
            p.bytes_done as f64 / 1e6
        );
    });
    let secs = t.elapsed().as_secs_f64();
    eprintln!();
    println!(
        "copied {} files, {:.1} MB in {secs:.1} s ({:.0} MB/s written incl. verification)",
        report.copied,
        report.bytes as f64 / 1e6,
        report.bytes as f64 / 1e6 / secs.max(1e-9)
    );
    for (f, e) in &report.failed {
        println!("FAILED {}: {e}", f.display());
    }
    if !report.failed.is_empty() {
        return Err(format!("{} files failed", report.failed.len()).into());
    }
    Ok(())
}
