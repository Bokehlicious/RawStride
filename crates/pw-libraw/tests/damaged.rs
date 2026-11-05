//! Damaged and incomplete files (M2 error handling) on the real test RAWs. Set PW_TESTDATA to
//! the folder with the test RAWs (D:\PhotoWorkflowData\RAW); skipped when it is not set.
//!
//! - Every complete test file opens, whatever LibRaw's processing warnings say.
//! - Files cut short inside their RAW data are refused as damaged (error contains `damaged:`),
//!   even though LibRaw itself returns success for them.
//! - Files LibRaw cannot read at all fail as before, but are not called damaged.
//! The diagnostics of every case are printed (`-- --nocapture`) for the analysis.

use std::path::{Path, PathBuf};

use pw_libraw::{DAMAGED, Session};

fn testdata() -> Option<PathBuf> {
    std::env::var_os("PW_TESTDATA").map(PathBuf::from)
}

fn raws(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                        matches!(e.to_ascii_lowercase().as_str(), "arw" | "rw2" | "dng")
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn first(dir: &Path, prefix: &str) -> Option<PathBuf> {
    raws(dir).into_iter().find(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(prefix))
    })
}

fn scratch() -> PathBuf {
    let d = std::env::temp_dir().join(format!("pw-libraw-damaged-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A copy of `src` with only the first `keep` bytes.
fn cut(src: &Path, keep: u64, dst: &Path) {
    let data = std::fs::read(src).unwrap();
    std::fs::write(dst, &data[..(keep as usize).min(data.len())]).unwrap();
}

#[test]
fn complete_files_are_never_called_damaged() {
    let Some(dir) = testdata() else {
        eprintln!("PW_TESTDATA not set; skipped");
        return;
    };
    let files = raws(&dir);
    assert!(!files.is_empty());
    let mut warned = 0;
    for f in &files {
        let (r, d) = Session::open_with_diagnostics(f);
        eprintln!(
            "complete {:<28} warnings {:#010x} data_errors {} short_reads {} missing {} -> {}",
            f.file_name().unwrap().to_string_lossy(),
            d.warnings,
            d.data_errors,
            d.short_reads,
            d.missing,
            if r.is_ok() {
                "ok".to_string()
            } else {
                format!("{}", r.as_ref().unwrap_err())
            }
        );
        if d.warnings != 0 {
            warned += 1;
        }
        assert!(r.is_ok(), "{}: {:?}", f.display(), r.err());
        assert!(!d.damaged);
    }
    eprintln!(
        "{} complete files, {warned} with LibRaw processing warnings",
        files.len()
    );
}

#[test]
fn truncated_files_are_damaged() {
    let Some(dir) = testdata() else {
        eprintln!("PW_TESTDATA not set; skipped");
        return;
    };
    let tmp = scratch();
    // (prefix, fractions that must be refused)
    let cases: [(&str, &[f64]); 4] = [
        ("TN1004", &[0.02, 0.05, 0.10, 0.40, 0.90, 0.99]), // A7R IVA
        ("P124", &[0.10, 0.40, 0.90, 0.99]),               // GM1 RW2
        ("TN0059", &[0.10, 0.50, 0.90]),                   // A1
        ("IMG_", &[0.50]),                                 // Xiaomi DNG
    ];
    for (prefix, parts) in cases {
        let Some(src) = first(&dir, prefix) else {
            eprintln!("no {prefix}* file; skipped");
            continue;
        };
        let len = std::fs::metadata(&src).unwrap().len();
        for &part in parts {
            let dst = tmp.join(format!(
                "{prefix}-{:03}.raw.{}",
                (part * 1000.0) as u32,
                src.extension().unwrap().to_string_lossy()
            ));
            cut(&src, (len as f64 * part) as u64, &dst);
            let (r, d) = Session::open_with_diagnostics(&dst);
            eprintln!(
                "cut {:<10} {:>5.1} %  warnings {:#010x} data_errors {} short_reads {} missing {} -> {}",
                prefix,
                part * 100.0,
                d.warnings,
                d.data_errors,
                d.short_reads,
                d.missing,
                match &r {
                    Ok(_) => "OPENED".to_string(),
                    Err(e) => e.to_string(),
                }
            );
            let msg = r.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
            assert!(msg.contains(DAMAGED), "{}: {msg}", dst.display());
            assert!(d.damaged);
        }
        // Small cuts at the very end. A cut that removes RAW data must be refused; one that only
        // removes data after the RAW data (DNG keeps other things at the end) may open.
        for less in [1024u64, 16 * 1024, 100 * 1024, 1024 * 1024] {
            let dst = tmp.join(format!(
                "{prefix}-minus{less}.{}",
                src.extension().unwrap().to_string_lossy()
            ));
            cut(&src, len.saturating_sub(less), &dst);
            let (r, d) = Session::open_with_diagnostics(&dst);
            eprintln!(
                "end {:<10} -{:>8} B  data_errors {} short_reads {} missing {} -> {}",
                prefix,
                less,
                d.data_errors,
                d.short_reads,
                d.missing,
                match &r {
                    Ok(_) => "OPENED".to_string(),
                    Err(e) => e.to_string(),
                }
            );
            assert_eq!(
                d.damaged,
                d.missing > 0 || d.data_errors > 0,
                "{}",
                dst.display()
            );
            assert_eq!(r.is_err(), d.damaged, "{}", dst.display());
            if prefix != "IMG_" {
                assert!(d.damaged, "{}: RAW data cut but not refused", dst.display());
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn unreadable_files_fail_but_are_not_damaged() {
    let tmp = scratch();
    // Random bytes with a RAW extension.
    let mut x: u64 = 42;
    let junk: Vec<u8> = (0..3 * 1024 * 1024 + 17)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    let f = tmp.join("garbage.ARW");
    std::fs::write(&f, junk).unwrap();
    let (r, d) = Session::open_with_diagnostics(&f);
    eprintln!("garbage.ARW -> {:?} {d:?}", r.as_ref().err());
    assert!(r.is_err());
    // A JPEG with a RAW extension (a JPEG is never a RAW for LibRaw).
    let jpeg: &[u8] = &[
        0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 0xFF,
        0xD9,
    ];
    let f = tmp.join("jpeg.ARW");
    std::fs::write(&f, jpeg).unwrap();
    let (r, d) = Session::open_with_diagnostics(&f);
    eprintln!("jpeg.ARW -> {:?} {d:?}", r.as_ref().err());
    assert!(r.is_err());
    assert!(!r.unwrap_err().to_string().contains(DAMAGED));
    assert!(!d.damaged);
    let _ = std::fs::remove_dir_all(&tmp);
}
