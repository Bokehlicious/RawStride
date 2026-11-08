//! End-to-end ingest tests on temporary folders. The real-ARW test is ignored by default; run it
//! with `$env:PW_TESTDATA = "<folder>"; cargo test -p pw-ingest -- --ignored`.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pw_ingest::{Options, Template, execute, plan, scan};

fn testdata() -> PathBuf {
    let Some(dir) = std::env::var_os("PW_TESTDATA") else {
        panic!("PW_TESTDATA is not set; set it to the real camera test-data folder");
    };
    let dir = PathBuf::from(dir);
    assert!(
        dir.is_dir(),
        "PW_TESTDATA folder does not exist: {}",
        dir.display()
    );
    dir
}

fn test_file(name: &str) -> PathBuf {
    let dir = testdata();
    let file = dir.join(name);
    assert!(
        file.is_file(),
        "required test file {name} was not found in PW_TESTDATA folder {}",
        dir.display()
    );
    file
}

struct Tmp(PathBuf);
impl Tmp {
    fn new(tag: &str) -> Tmp {
        let p = std::env::temp_dir().join(format!("pw-ingest-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
}
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 2026-10-26 06:28:51 UTC
const T0: u64 = 1_792_996_131;

fn write(path: &Path, bytes: &[u8], unix: u64) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(unix))
        .unwrap();
}

/// A fake card: DCIM/100MSDCF with a RAW+XMP pair, a JPEG, a RAW, an orphan sidecar and
/// things that must be ignored.
fn card(root: &Path) {
    let d = root.join("DCIM").join("100MSDCF");
    write(&d.join("A001.ARW"), &[1u8; 5000], T0);
    write(&d.join("A001.xmp"), b"<x:xmpmeta/>", T0 + 5);
    write(&d.join("A002.JPG"), &[2u8; 3000], T0 + 60);
    write(&d.join("A003.ARW"), &[3u8; 7000], T0 + 120);
    write(&d.join("ORPHAN.xmp"), b"<x:xmpmeta/>", T0);
    write(&d.join("notes.txt"), b"x", T0);
    write(&d.join(".hidden.ARW"), b"x", T0);
    write(
        &root.join("System Volume Information").join("Z.ARW"),
        b"x",
        T0,
    );
}

fn opts(dest: &Path, folder: &str, name: &str) -> Options {
    Options {
        dest: dest.to_path_buf(),
        backup: None,
        folder: Template::parse(folder).unwrap(),
        name: Template::parse(name).unwrap(),
        job: "Test".into(),
        first_seq: 1,
    }
}

fn rels(p: &pw_ingest::Plan) -> Vec<String> {
    p.items
        .iter()
        .map(|i| i.rel.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>, SystemTime)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let m = fs::metadata(&p).unwrap().modified().unwrap();
                out.push((p.clone(), fs::read(&p).unwrap(), m));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn scan_groups_and_ignores() {
    let src = Tmp::new("scan");
    card(&src.0);
    let shots = scan(&src.0).unwrap();
    let names: Vec<_> = shots
        .iter()
        .map(|s| (s.stem.as_str(), s.files.len()))
        .collect();
    assert_eq!(names, vec![("A001", 2), ("A002", 1), ("A003", 1)]);
    assert_eq!(shots[0].files[0].ext, "ARW", "image before its sidecar");
    assert!(!shots[0].time_from_exif, "fake files have no EXIF");
}

#[test]
fn copies_verifies_and_is_incremental() {
    let src = Tmp::new("copy-src");
    let dst = Tmp::new("copy-dst");
    card(&src.0);
    let before = snapshot(&src.0);
    let o = opts(&dst.0, "{year}/{date} {job}", "{name}");
    let shots = scan(&src.0).unwrap();
    let p = plan(&shots, &o);
    assert_eq!(
        rels(&p),
        vec![
            "2026/2026-10-26 Test/A001.ARW",
            "2026/2026-10-26 Test/A001.xmp",
            "2026/2026-10-26 Test/A002.JPG",
            "2026/2026-10-26 Test/A003.ARW",
        ]
    );
    assert_eq!(p.bytes_to_copy(), 5000 + 12 + 3000 + 7000);
    let mut calls = 0;
    let r = execute(&p, &o, &AtomicBool::new(false), |pr| {
        calls += 1;
        assert!(pr.files_done <= pr.files_total);
    });
    assert_eq!((r.copied, r.failed.len(), r.cancelled), (4, 0, false));
    assert_eq!(calls, 4);
    for item in &p.items {
        let out = dst.0.join(&item.rel);
        assert_eq!(fs::read(&out).unwrap(), fs::read(&item.source).unwrap());
        assert_eq!(
            fs::metadata(&out).unwrap().modified().unwrap(),
            item.modified,
            "modification time kept"
        );
    }
    assert!(
        snapshot(&dst.0)
            .iter()
            .all(|(p, _, _)| !p.to_string_lossy().ends_with(".pwpart")),
        "no temporary files left"
    );
    assert_eq!(snapshot(&src.0), before, "source untouched");

    // Running again copies nothing.
    let again = plan(&scan(&src.0).unwrap(), &o);
    assert_eq!((again.files_to_copy(), again.already_present()), (0, 4));
}

#[test]
fn conflicts_get_a_suffix_for_the_whole_shot() {
    let src = Tmp::new("conf-src");
    let dst = Tmp::new("conf-dst");
    card(&src.0);
    // A different file already has A001.xmp's name: the pair must move together.
    write(&dst.0.join("A001.xmp"), b"other", T0);
    let o = opts(&dst.0, "", "{name}");
    let p = plan(&scan(&src.0).unwrap(), &o);
    assert_eq!(
        rels(&p),
        vec!["A001_1.ARW", "A001_1.xmp", "A002.JPG", "A003.ARW"]
    );
    assert!(p.items[0].renamed_for_conflict && !p.items[2].renamed_for_conflict);
    execute(&p, &o, &AtomicBool::new(false), |_| {});
    assert_eq!(
        fs::read(dst.0.join("A001.xmp")).unwrap(),
        b"other",
        "never overwritten"
    );
}

#[test]
fn sequence_names_and_duplicate_names_from_two_folders() {
    let src = Tmp::new("seq-src");
    let dst = Tmp::new("seq-dst");
    // Two cameras, same file names, interleaved capture times.
    write(&src.0.join("cam1").join("IMG_0001.ARW"), &[1; 10], T0);
    write(&src.0.join("cam2").join("IMG_0001.ARW"), &[2; 10], T0 + 1);
    write(&src.0.join("cam1").join("IMG_0002.ARW"), &[3; 10], T0 + 2);
    let o = opts(&dst.0, "", "{name}");
    assert_eq!(
        rels(&plan(&scan(&src.0).unwrap(), &o)),
        vec!["IMG_0001.ARW", "IMG_0001_1.ARW", "IMG_0002.ARW"]
    );
    let mut o = opts(&dst.0, "", "{year2}{month}{day}_{seq:4}");
    o.first_seq = 9;
    assert_eq!(
        rels(&plan(&scan(&src.0).unwrap(), &o)),
        vec!["261026_0009.ARW", "261026_0010.ARW", "261026_0011.ARW"]
    );
}

#[test]
fn backup_gets_what_is_missing_there() {
    let src = Tmp::new("bk-src");
    let dst = Tmp::new("bk-dst");
    let bak = Tmp::new("bk-bak");
    card(&src.0);
    let o = opts(&dst.0, "{date}", "{name}");
    let p = plan(&scan(&src.0).unwrap(), &o);
    execute(&p, &o, &AtomicBool::new(false), |_| {});
    let mut o2 = o.clone();
    o2.backup = Some(bak.0.clone());
    let p2 = plan(&scan(&src.0).unwrap(), &o2);
    assert!(p2.items.iter().all(|i| !i.to_dest && i.to_backup));
    let r = execute(&p2, &o2, &AtomicBool::new(false), |_| {});
    assert_eq!(r.copied, 4);
    let a: Vec<_> = snapshot(&dst.0)
        .into_iter()
        .map(|(_, b, m)| (b, m))
        .collect();
    let b: Vec<_> = snapshot(&bak.0)
        .into_iter()
        .map(|(_, b, m)| (b, m))
        .collect();
    assert_eq!(a, b);
}

#[test]
fn files_edited_after_ingest_still_count_as_ingested() {
    let src = Tmp::new("edit-src");
    let dst = Tmp::new("edit-dst");
    write(&src.0.join("A001.ARW"), &[1u8; 500], T0);
    write(&src.0.join("A001.xmp"), b"<x:xmpmeta/>", T0);
    write(&src.0.join("B001.DNG"), &[2u8; 500], T0);
    write(&src.0.join("C001.ARW"), &[3u8; 500], T0);
    let o = opts(&dst.0, "", "{name}");
    execute(
        &plan(&scan(&src.0).unwrap(), &o),
        &o,
        &AtomicBool::new(false),
        |_| {},
    );
    // After ingest: the sidecar gets IPTC (new size and date), the DNG gets an embedded rating
    // (new size, date kept), and C001 gets a sidecar that did not exist on the card.
    write(
        &dst.0.join("A001.xmp"),
        b"<x:xmpmeta>longer</x:xmpmeta>",
        T0 + 999,
    );
    write(&dst.0.join("B001.DNG"), &[2u8; 900], T0);
    write(&dst.0.join("C001.xmp"), b"<x:xmpmeta/>", T0 + 999);
    let again = plan(&scan(&src.0).unwrap(), &o);
    assert_eq!(again.files_to_copy(), 0, "{:?}", rels(&again));
    // But a different RAW under the same name is still a clash, and so is its sidecar.
    write(&dst.0.join("A001.ARW"), &[9u8; 500], T0 + 5);
    let clash = plan(&scan(&src.0).unwrap(), &o);
    assert_eq!(
        rels(&clash)[..2],
        ["A001_1.ARW".to_string(), "A001_1.xmp".to_string()]
    );
}

#[test]
fn cancel_leaves_no_files() {
    let src = Tmp::new("cancel-src");
    let dst = Tmp::new("cancel-dst");
    card(&src.0);
    let o = opts(&dst.0, "x", "{name}");
    let p = plan(&scan(&src.0).unwrap(), &o);
    let r = execute(&p, &o, &AtomicBool::new(true), |_| {});
    assert!(r.cancelled);
    assert_eq!(r.copied, 0);
    assert!(snapshot(&dst.0).is_empty());
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn real_raw_uses_exif_time() {
    let src = Tmp::new("real-src");
    let dst = Tmp::new("real-dst");
    let raw = test_file("TN005956.ARW");
    let copy = src.0.join("TN005956.ARW");
    fs::copy(&raw, &copy).unwrap();
    let shots = scan(&src.0).unwrap();
    assert!(shots[0].time_from_exif);
    assert_eq!(shots[0].model.as_deref(), Some("ILCE-1"));
    let o = opts(&dst.0, "{date}", "{model}_{time}");
    let p = plan(&shots, &o);
    let t = shots[0].time.unwrap();
    let expected = format!(
        "{:04}-{:02}-{:02}/ILCE-1_{:02}{:02}{:02}.ARW",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    );
    assert_eq!(rels(&p), vec![expected]);
    let started = std::time::Instant::now();
    let r = execute(&p, &o, &AtomicBool::new(false), |_| {});
    eprintln!(
        "copied+verified {} bytes in {:?}",
        r.bytes,
        started.elapsed()
    );
    assert_eq!(r.copied, 1);
    assert_eq!(
        fs::read(dst.0.join(&p.items[0].rel)).unwrap(),
        fs::read(&raw).unwrap()
    );
}
