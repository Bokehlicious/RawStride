//! End-to-end ingest tests on temporary folders. The real-ARW test is ignored by default; run it
//! with `$env:PW_TESTDATA = "<folder>"; cargo test -p pw-ingest -- --ignored`.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pw_ingest::{
    Options, REGISTER_FILE, RegisterWriter, Template, execute, execute_recording, new_shots, plan,
    read_register, scan,
};

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
fn raf_files_are_scanned_and_planned() {
    let src = Tmp::new("raf-src");
    let dst = Tmp::new("raf-dst");
    // Fake bytes: inspecting fails, so the time falls back to the file date.
    write(
        &src.0.join("DCIM").join("100_FUJI").join("_DSF0001.RAF"),
        &[7u8; 4000],
        T0,
    );
    let shots = scan(&src.0).unwrap();
    let names: Vec<_> = shots
        .iter()
        .map(|s| (s.stem.as_str(), s.files.len()))
        .collect();
    assert_eq!(names, vec![("_DSF0001", 1)]);
    assert!(!shots[0].time_from_exif, "fake file has no EXIF");
    let p = plan(&shots, &opts(&dst.0, "{date}", "{name}"));
    assert_eq!(rels(&p), vec!["2026-10-26/_DSF0001.RAF"]);
    assert_eq!((p.files_to_copy(), p.bytes_to_copy()), (1, 4000));
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
    #[cfg(windows)]
    {
        use std::fs::FileTimes;
        use std::os::windows::fs::FileTimesExt;
        File::options()
            .write(true)
            .open(dst.0.join("A001.ARW"))
            .unwrap()
            .set_times(FileTimes::new().set_created(UNIX_EPOCH + Duration::from_secs(T0 + 5)))
            .unwrap();
    }
    let clash = plan(&scan(&src.0).unwrap(), &o);
    assert_eq!(
        rels(&clash)[..2],
        ["A001_1.ARW".to_string(), "A001_1.xmp".to_string()]
    );
}

#[cfg(windows)]
#[test]
fn a_dng_written_after_ingest_with_a_new_time_still_counts_as_ingested() {
    use std::fs::FileTimes;
    use std::os::windows::fs::FileTimesExt;
    let src = Tmp::new("dng-ctime-src");
    let dst = Tmp::new("dng-ctime-dst");
    write(&src.0.join("B001.DNG"), &[2u8; 500], T0);
    let o = opts(&dst.0, "", "{name}");
    let r = execute(
        &plan(&scan(&src.0).unwrap(), &o),
        &o,
        &AtomicBool::new(false),
        |_| {},
    );
    assert_eq!(r.copied, 1);
    // A later write in place: new size and a new modification time; the creation time stays.
    write(&dst.0.join("B001.DNG"), &[2u8; 900], T0 + 999);
    let again = plan(&scan(&src.0).unwrap(), &o);
    assert_eq!(
        (again.files_to_copy(), again.already_present()),
        (0, 1),
        "{:?}",
        rels(&again)
    );
    // With the creation time changed too, nothing says it is the card's file: a clash.
    File::options()
        .write(true)
        .open(dst.0.join("B001.DNG"))
        .unwrap()
        .set_times(FileTimes::new().set_created(UNIX_EPOCH + Duration::from_secs(T0 + 5)))
        .unwrap();
    let clash = plan(&scan(&src.0).unwrap(), &o);
    assert_eq!(rels(&clash), vec!["B001_1.DNG"]);
}

#[cfg(windows)]
#[test]
fn the_copy_gets_the_card_time_as_creation_time() {
    let src = Tmp::new("ctime-src");
    let dst = Tmp::new("ctime-dst");
    let bak = Tmp::new("ctime-bak");
    card(&src.0);
    let mut o = opts(&dst.0, "", "{name}");
    o.backup = Some(bak.0.clone());
    let p = plan(&scan(&src.0).unwrap(), &o);
    let r = execute(&p, &o, &AtomicBool::new(false), |_| {});
    assert_eq!((r.copied, r.failed.len()), (4, 0));
    let to_secs = |t: SystemTime| t.duration_since(UNIX_EPOCH).unwrap().as_secs();
    for item in &p.items {
        let card = to_secs(item.modified);
        for root in [&dst.0, &bak.0] {
            let m = fs::metadata(root.join(&item.rel)).unwrap();
            let out = root.join(&item.rel);
            assert_eq!(
                to_secs(m.created().unwrap()),
                card,
                "created: {}",
                out.display()
            );
            assert_eq!(
                to_secs(m.modified().unwrap()),
                card,
                "modified: {}",
                out.display()
            );
        }
    }
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

/// The lines of the register in `root` after its version line.
fn register_lines(root: &Path) -> Vec<String> {
    let text = fs::read_to_string(root.join(REGISTER_FILE)).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("RawStride ingest register 1"));
    lines.map(str::to_string).collect()
}

/// Imports everything new on the card in `src` into `dst`, with the register in `src`.
fn import_with_register(src: &Path, dst: &Path) -> pw_ingest::Report {
    let o = opts(dst, "", "{name}");
    let register = read_register(src);
    let (shots, _) = new_shots(&scan(src).unwrap(), src, &register);
    let mut writer = RegisterWriter::open(src, &register).unwrap();
    execute_recording(
        &plan(&shots, &o),
        &o,
        &AtomicBool::new(false),
        Some(&mut writer),
        |_| {},
    )
}

#[test]
fn register_gets_a_line_per_copied_file_and_per_file_already_present() {
    let src = Tmp::new("reg-lines-src");
    let dst = Tmp::new("reg-lines-dst");
    card(&src.0);
    // Two files are at the destination already, from an import without a register.
    let o = opts(&dst.0, "", "{name}");
    let first: Vec<_> = scan(&src.0).unwrap().into_iter().take(1).collect();
    let r = execute(&plan(&first, &o), &o, &AtomicBool::new(false), |_| {});
    assert_eq!((r.copied, r.register_error), (2, None));
    assert!(!src.0.join(REGISTER_FILE).exists(), "no register asked for");

    let register = read_register(&src.0);
    let shots = scan(&src.0).unwrap();
    let p = plan(&shots, &o);
    assert_eq!((p.already_present(), p.files_to_copy()), (2, 2));
    let mut writer = RegisterWriter::open(&src.0, &register).unwrap();
    let r = execute_recording(&p, &o, &AtomicBool::new(false), Some(&mut writer), |_| {});
    assert_eq!((r.copied, r.skipped_present, r.failed.len()), (2, 2, 0));
    assert_eq!(r.register_error, None);
    // The files already present come first, then the copied ones in the plan's order.
    assert_eq!(
        register_lines(&src.0),
        vec![
            format!("DCIM/100MSDCF/A001.ARW\t5000\t{T0}"),
            format!("DCIM/100MSDCF/A001.xmp\t12\t{}", T0 + 5),
            format!("DCIM/100MSDCF/A002.JPG\t3000\t{}", T0 + 60),
            format!("DCIM/100MSDCF/A003.ARW\t7000\t{}", T0 + 120),
        ]
    );
    let register = read_register(&src.0);
    for shot in &shots {
        for f in &shot.files {
            assert!(register.contains(&src.0, f), "{}", f.path.display());
        }
    }
}

#[test]
fn second_import_with_a_register_copies_nothing_even_elsewhere() {
    let src = Tmp::new("reg-again-src");
    let dst = Tmp::new("reg-again-dst");
    let other = Tmp::new("reg-again-other");
    card(&src.0);
    assert_eq!(import_with_register(&src.0, &dst.0).copied, 4);
    let lines = register_lines(&src.0);

    // Another destination and another name template: without the register all four are new.
    let o = opts(&other.0, "{date}", "{job}_{seq:4}");
    let shots = scan(&src.0).unwrap();
    assert_eq!(plan(&shots, &o).files_to_copy(), 4);
    let register = read_register(&src.0);
    let (new, left_out) = new_shots(&shots, &src.0, &register);
    assert!(new.is_empty());
    assert_eq!(left_out, 4);
    let p = plan(&new, &o);
    assert!(p.items.is_empty());
    let mut writer = RegisterWriter::open(&src.0, &register).unwrap();
    let r = execute_recording(&p, &o, &AtomicBool::new(false), Some(&mut writer), |_| {});
    assert_eq!((r.copied, r.skipped_present, r.cancelled), (0, 0, false));
    assert_eq!(r.register_error, None);
    assert!(snapshot(&other.0).is_empty(), "nothing copied");
    assert_eq!(register_lines(&src.0), lines, "nothing added");
}

#[test]
fn a_new_file_on_the_card_is_the_only_one_copied_and_gets_the_first_number() {
    let src = Tmp::new("reg-new-src");
    let dst = Tmp::new("reg-new-dst");
    let fresh = Tmp::new("reg-new-fresh");
    card(&src.0);
    assert_eq!(import_with_register(&src.0, &dst.0).copied, 4);
    // Taken between the photos imported earlier.
    let added = src.0.join("DCIM").join("100MSDCF").join("A004.ARW");
    write(&added, &[4u8; 900], T0 + 30);

    let mut o = opts(&fresh.0, "", "{seq:4}");
    o.first_seq = 7;
    let register = read_register(&src.0);
    let (new, left_out) = new_shots(&scan(&src.0).unwrap(), &src.0, &register);
    assert_eq!(left_out, 4);
    let p = plan(&new, &o);
    assert_eq!(rels(&p), vec!["0007.ARW"]);
    let mut writer = RegisterWriter::open(&src.0, &register).unwrap();
    let r = execute_recording(&p, &o, &AtomicBool::new(false), Some(&mut writer), |_| {});
    assert_eq!((r.copied, r.failed.len(), r.register_error), (1, 0, None));
    let copied = snapshot(&fresh.0);
    assert_eq!(copied.len(), 1);
    assert_eq!(copied[0].0, fresh.0.join("0007.ARW"));
    assert_eq!(copied[0].1, fs::read(&added).unwrap());
    assert_eq!(
        register_lines(&src.0).last().cloned(),
        Some(format!("DCIM/100MSDCF/A004.ARW\t900\t{}", T0 + 30))
    );
    let (new, left_out) = new_shots(&scan(&src.0).unwrap(), &src.0, &read_register(&src.0));
    assert!(new.is_empty());
    assert_eq!(left_out, 5);
}

#[cfg(windows)]
#[test]
fn a_read_only_register_cannot_be_opened_and_the_import_runs_without_it() {
    /// Makes the file writable again when dropped, so the temporary folder can be removed.
    struct Writable(PathBuf, fs::Permissions);
    impl Drop for Writable {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, self.1.clone());
        }
    }

    let src = Tmp::new("reg-ro-src");
    let dst = Tmp::new("reg-ro-dst");
    card(&src.0);
    let file = src.0.join(REGISTER_FILE);
    drop(RegisterWriter::open(&src.0, &read_register(&src.0)).unwrap());
    let before = fs::read(&file).unwrap();
    let writable = fs::metadata(&file).unwrap().permissions();
    let mut read_only = writable.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&file, read_only).unwrap();
    // Declared after `src`, so it is dropped (and the flag cleared) before the folder is removed.
    let _restore = Writable(file.clone(), writable);

    let register = read_register(&src.0);
    assert_eq!(register.problem(), None, "it can be read");
    let error = RegisterWriter::open(&src.0, &register).unwrap_err();
    assert!(
        error.starts_with("The card could not be marked as imported: ") && error.ends_with('.'),
        "{error}"
    );
    assert!(!error.contains("was not made by"), "{error}");

    let o = opts(&dst.0, "", "{name}");
    let (shots, left_out) = new_shots(&scan(&src.0).unwrap(), &src.0, &register);
    assert_eq!(left_out, 0);
    let r = execute_recording(&plan(&shots, &o), &o, &AtomicBool::new(false), None, |_| {});
    assert_eq!((r.copied, r.failed.len(), r.cancelled), (4, 0, false));
    assert_eq!(r.register_error, None);
    assert_eq!(snapshot(&dst.0).len(), 4);
    assert_eq!(
        fs::read(&file).unwrap(),
        before,
        "the register is unchanged"
    );
}

#[test]
fn a_cancelled_run_has_a_line_only_for_the_file_that_was_copied() {
    let src = Tmp::new("reg-cancel-src");
    let dst = Tmp::new("reg-cancel-dst");
    card(&src.0);
    let o = opts(&dst.0, "", "{name}");
    let register = read_register(&src.0);
    let p = plan(&scan(&src.0).unwrap(), &o);
    let mut writer = RegisterWriter::open(&src.0, &register).unwrap();
    let cancel = AtomicBool::new(false);
    let r = execute_recording(&p, &o, &cancel, Some(&mut writer), |_| {
        cancel.store(true, Ordering::Relaxed);
    });
    assert!(r.cancelled);
    assert_eq!((r.copied, r.register_error), (1, None));
    assert_eq!(
        register_lines(&src.0),
        vec![format!("DCIM/100MSDCF/A001.ARW\t5000\t{T0}")]
    );
    assert_eq!(snapshot(&dst.0).len(), 1);
}

/// Moves the modification time of every scanned file on the card in `src` by `secs` seconds
/// (later when positive), as Windows reads a FAT32 card after the clocks change. Only the time
/// changes, not the content.
fn shift_times(src: &Path, secs: i64) {
    for shot in scan(src).unwrap() {
        for f in &shot.files {
            let by = Duration::from_secs(secs.unsigned_abs());
            let moved = if secs >= 0 {
                f.modified + by
            } else {
                f.modified - by
            };
            File::options()
                .write(true)
                .open(&f.path)
                .unwrap()
                .set_modified(moved)
                .unwrap();
        }
    }
}

#[test]
fn a_card_read_one_hour_later_is_already_present() {
    let src = Tmp::new("dst-hour-src");
    let dst = Tmp::new("dst-hour-dst");
    card(&src.0);
    let o = opts(&dst.0, "", "{name}");
    let r = execute(
        &plan(&scan(&src.0).unwrap(), &o),
        &o,
        &AtomicBool::new(false),
        |_| {},
    );
    assert_eq!(r.copied, 4);
    shift_times(&src.0, 3600);
    let p = plan(&scan(&src.0).unwrap(), &o);
    assert_eq!((p.already_present(), p.files_to_copy()), (4, 0));
    assert!(rels(&p).iter().all(|r| !r.contains("_1")), "{:?}", rels(&p));
}

#[test]
fn a_card_read_two_hours_later_is_a_clash() {
    let src = Tmp::new("dst-2h-src");
    let dst = Tmp::new("dst-2h-dst");
    card(&src.0);
    let o = opts(&dst.0, "", "{name}");
    let r = execute(
        &plan(&scan(&src.0).unwrap(), &o),
        &o,
        &AtomicBool::new(false),
        |_| {},
    );
    assert_eq!(r.copied, 4);
    shift_times(&src.0, 7200);
    let p = plan(&scan(&src.0).unwrap(), &o);
    assert_eq!(
        rels(&p),
        vec!["A001_1.ARW", "A001_1.xmp", "A002_1.JPG", "A003_1.ARW"]
    );
}

#[test]
fn a_card_read_one_hour_earlier_is_in_the_register() {
    let src = Tmp::new("reg-hour-src");
    let dst = Tmp::new("reg-hour-dst");
    card(&src.0);
    assert_eq!(import_with_register(&src.0, &dst.0).copied, 4);
    shift_times(&src.0, -3600);
    let (new, left_out) = new_shots(&scan(&src.0).unwrap(), &src.0, &read_register(&src.0));
    assert!(new.is_empty());
    assert_eq!(left_out, 4);
}

#[test]
fn a_card_read_two_hours_earlier_is_not_in_the_register() {
    let src = Tmp::new("reg-2h-src");
    let dst = Tmp::new("reg-2h-dst");
    card(&src.0);
    assert_eq!(import_with_register(&src.0, &dst.0).copied, 4);
    shift_times(&src.0, -7200);
    let (new, left_out) = new_shots(&scan(&src.0).unwrap(), &src.0, &read_register(&src.0));
    assert_eq!(new.len(), 3);
    assert_eq!(left_out, 0);
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
