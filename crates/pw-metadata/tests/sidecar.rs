//! Integration tests against a real ExifTool: `exiftool` on PATH, or
//! `PW_EXIFTOOL_PL=/path/to/exiftool` (run through `perl`).
//!
//! These tests need ExifTool and fail with an explanation when it is missing, instead of returning
//! early and being counted as passed (docs/TESTING.md §4.5). To leave them out deliberately, run
//! e.g. `cargo test --workspace --exclude pw-metadata`, so the omission is visible.
//!
//! The real-DNG test is ignored by default. Run it with
//! `$env:PW_TESTDATA = "<folder>"; cargo test -p pw-metadata -- --ignored`.

use std::path::PathBuf;

use pw_metadata::{ExifTool, Iptc, LocalizedValue, Rating, sidecar_path, supports_sidecar};

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

/// Starts ExifTool and checks that it answers. Panics (fails the test) if it is unavailable.
fn exiftool() -> ExifTool {
    let (started, source) = match std::env::var("PW_EXIFTOOL_PL") {
        Ok(pl) => (
            ExifTool::start_with("perl", &[&pl]),
            format!("perl {pl} (PW_EXIFTOOL_PL)"),
        ),
        Err(_) => (ExifTool::start(), "exiftool on PATH".to_string()),
    };
    let mut et = match started {
        Ok(et) => et,
        Err(e) => panic!(
            "ExifTool is required for this test but could not be started from {source}: {e}. \
             Install ExifTool or set PW_EXIFTOOL_PL (docs/TESTING.md §4.5)."
        ),
    };
    if let Err(e) = et.execute(&["-ver".to_string()]) {
        panic!("ExifTool from {source} does not answer: {e} (docs/TESTING.md §4.5)");
    }
    et
}

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pw-metadata-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn write_read_update_clear() {
    let mut et = exiftool();
    let dir = temp_dir("rw");
    let raw = dir.join("TN0001.ARW");
    std::fs::write(&raw, b"not really a raw file").unwrap();

    // create
    let r = Rating {
        stars: Some(3),
        label: Some("Red".into()),
    };
    et.write_rating(&raw, &r).unwrap();
    assert!(sidecar_path(&raw).exists());
    assert_eq!(
        et.read_ratings(std::slice::from_ref(&raw))
            .unwrap()
            .get(&raw),
        Some(&r)
    );
    assert_eq!(
        std::fs::read(&raw).unwrap(),
        b"not really a raw file",
        "RAW untouched"
    );

    // update only the rating, keep the label
    let r2 = Rating {
        stars: Some(5),
        label: Some("Red".into()),
    };
    et.write_rating(&raw, &r2).unwrap();
    assert_eq!(
        et.read_ratings(std::slice::from_ref(&raw))
            .unwrap()
            .get(&raw),
        Some(&r2)
    );

    // reject
    let rej = Rating {
        stars: Some(-1),
        label: None,
    };
    et.write_rating(&raw, &rej).unwrap();
    assert_eq!(
        et.read_ratings(std::slice::from_ref(&raw))
            .unwrap()
            .get(&raw),
        Some(&rej)
    );

    // clear
    et.write_rating(&raw, &Rating::default()).unwrap();
    assert!(
        !et.read_ratings(std::slice::from_ref(&raw))
            .unwrap()
            .contains_key(&raw)
    );
    assert!(
        !std::fs::read_dir(&dir).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with("_original")
        }),
        "no ExifTool backup files left behind"
    );
}

#[test]
fn existing_sidecar_content_is_preserved() {
    let mut et = exiftool();
    let dir = temp_dir("keep");
    let raw = dir.join("P1000.RW2");
    std::fs::write(&raw, b"x").unwrap();
    // A sidecar written by "another program" with a title and keywords.
    et.execute(&[
        "-overwrite_original".into(),
        "-XMP-dc:Title=Sunset".into(),
        "-XMP-dc:Subject=beach".into(),
        "-XMP-dc:Subject=sea".into(),
        sidecar_path(&raw).to_string_lossy().into_owned(),
    ])
    .unwrap();
    et.write_rating(
        &raw,
        &Rating {
            stars: Some(4),
            label: Some("Green".into()),
        },
    )
    .unwrap();
    let out = et
        .execute(&[
            "-T".into(),
            "-XMP-dc:Title".into(),
            "-XMP-dc:Subject".into(),
            "-XMP-xmp:Rating".into(),
            "-XMP-xmp:Label".into(),
            sidecar_path(&raw).to_string_lossy().into_owned(),
        ])
        .unwrap();
    assert_eq!(out.trim(), "Sunset\tbeach, sea\t4\tGreen");
}

#[test]
fn unicode_and_spaces_in_paths() {
    let mut et = exiftool();
    let dir = temp_dir("åäö mapp");
    let raw = dir.join("Bild ÅÄÖ 01.ARW");
    std::fs::write(&raw, b"x").unwrap();
    let r = Rating {
        stars: Some(2),
        label: Some("Blue".into()),
    };
    et.write_rating(&raw, &r).unwrap();
    assert_eq!(
        et.read_ratings(std::slice::from_ref(&raw))
            .unwrap()
            .get(&raw),
        Some(&r)
    );
}

#[test]
fn which_file_holds_the_rating() {
    use std::path::Path;
    assert!(supports_sidecar(Path::new("a.ARW")) && supports_sidecar(Path::new("a.rw2")));
    assert!(supports_sidecar(Path::new("a.RAF")));
    assert_eq!(
        sidecar_path(Path::new("x/_DSF4243.RAF")),
        Path::new("x/_DSF4243.xmp")
    );
    for f in ["a.dng", "a.JPG", "a.jpeg", "a.tif", "a.TIFF"] {
        assert!(pw_metadata::supports_embedded(Path::new(f)), "{f}");
        assert_eq!(pw_metadata::rating_file(Path::new(f)), Path::new(f));
    }
    assert_eq!(
        pw_metadata::rating_file(Path::new("d/a.ARW")),
        Path::new("d/a.xmp")
    );
    assert!(!pw_metadata::supports_rating(Path::new("a.png")));
}

/// Minimal baseline JPEG (header + a few bytes of "scan data").
fn tiny_jpeg() -> Vec<u8> {
    let mut j = vec![0xFF, 0xD8];
    j.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x02, 0x00, 0x02, 0x01, 0x01, 0x11, 0x00,
    ]);
    j.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
    j.extend_from_slice(&[0x12, 0x34, 0x56, 0x78]);
    j.extend_from_slice(&[0xFF, 0xD9]);
    j
}

/// Minimal uncompressed 2x2 RGB little-endian TIFF.
fn tiny_tiff() -> Vec<u8> {
    let entries: [(u16, u16, u32, u32); 8] = [
        (256, 3, 1, 2),   // ImageWidth
        (257, 3, 1, 2),   // ImageLength
        (258, 3, 1, 8),   // BitsPerSample (one value is accepted for RGB by readers)
        (259, 3, 1, 1),   // Compression: none
        (262, 3, 1, 2),   // Photometric: RGB
        (273, 4, 1, 200), // StripOffsets
        (277, 3, 1, 3),   // SamplesPerPixel
        (279, 4, 1, 12),  // StripByteCounts
    ];
    let mut f = vec![0u8; 212];
    f[0..4].copy_from_slice(b"II*\0");
    f[4..8].copy_from_slice(&8u32.to_le_bytes());
    f[8..10].copy_from_slice(&(entries.len() as u16).to_le_bytes());
    for (k, (tag, typ, count, val)) in entries.iter().enumerate() {
        let e = 10 + 12 * k;
        f[e..e + 2].copy_from_slice(&tag.to_le_bytes());
        f[e + 2..e + 4].copy_from_slice(&typ.to_le_bytes());
        f[e + 4..e + 8].copy_from_slice(&count.to_le_bytes());
        f[e + 8..e + 12].copy_from_slice(&val.to_le_bytes());
    }
    f[200..212].copy_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 9, 9, 9]);
    f
}

fn image_hash(et: &mut ExifTool, p: &std::path::Path) -> String {
    let out = et
        .execute(&[
            "-s3".into(),
            "-ImageDataHash".into(),
            p.to_string_lossy().into_owned(),
        ])
        .unwrap();
    let h = out.lines().next().unwrap_or("").trim().to_string();
    assert!(h.len() == 32, "no ImageDataHash for {}: {out}", p.display());
    h
}

fn check_embedded(et: &mut ExifTool, file: &std::path::Path) {
    let mtime = std::fs::metadata(file).unwrap().modified().unwrap();
    let hash = image_hash(et, file);
    let r = Rating {
        stars: Some(4),
        label: Some("Purple".into()),
    };
    et.write_rating(file, &r).unwrap();
    assert_eq!(
        et.read_ratings(&[file.to_path_buf()]).unwrap().get(file),
        Some(&r),
        "{}",
        file.display()
    );
    assert!(
        !sidecar_path(file).exists(),
        "no sidecar for embedded formats"
    );
    assert_eq!(
        image_hash(et, file),
        hash,
        "image data unchanged: {}",
        file.display()
    );
    // ExifTool's -P keeps the modification time to the second (sub-second part may be dropped).
    let secs =
        |t: std::time::SystemTime| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let after = std::fs::metadata(file).unwrap().modified().unwrap();
    assert_eq!(secs(after), secs(mtime), "mtime preserved");
    et.write_rating(file, &Rating::default()).unwrap();
    assert!(
        !et.read_ratings(&[file.to_path_buf()])
            .unwrap()
            .contains_key(file)
    );
    assert_eq!(image_hash(et, file), hash);
}

#[test]
fn jpeg_and_tiff_are_written_in_place() {
    let mut et = exiftool();
    let dir = temp_dir("embedded");
    let jpg = dir.join("IMG_0001.JPG");
    std::fs::write(&jpg, tiny_jpeg()).unwrap();
    check_embedded(&mut et, &jpg);
    let tif = dir.join("Scan 01.tif");
    std::fs::write(&tif, tiny_tiff()).unwrap();
    check_embedded(&mut et, &tif);
}

/// Writes `new` to `image` while `file` (the file that holds its rating) is read-only, restores the
/// permissions, and returns the result of the write. The restore comes before any assertion, so a
/// failing test still leaves a folder that can be removed.
fn write_while_read_only(
    et: &mut ExifTool,
    image: &std::path::Path,
    file: &std::path::Path,
    new: &Rating,
) -> pw_metadata::Result<()> {
    let original = std::fs::metadata(file).unwrap().permissions();
    let mut read_only = original.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(file, read_only).unwrap();
    let result = et.write_rating(image, new);
    std::fs::set_permissions(file, original).unwrap();
    result
}

#[test]
fn a_write_to_a_read_only_file_is_an_error_and_keeps_the_old_rating() {
    let mut et = exiftool();
    let dir = temp_dir("read-only");
    let old = Rating {
        stars: Some(2),
        label: Some("Red".into()),
    };
    let new = Rating {
        stars: Some(5),
        label: None,
    };

    let raw = dir.join("TN0003.ARW");
    std::fs::write(&raw, b"raw").unwrap();
    et.write_rating(&raw, &old).unwrap();
    let sidecar = sidecar_path(&raw);
    let result = write_while_read_only(&mut et, &raw, &sidecar, &new);
    assert!(
        matches!(&result, Err(pw_metadata::Error::ReadOnly(target)) if *target == sidecar),
        "read-only sidecar: {result:?}"
    );
    assert_eq!(
        et.read_ratings(std::slice::from_ref(&raw))
            .unwrap()
            .get(&raw),
        Some(&old),
        "the sidecar keeps the old rating"
    );

    let jpg = dir.join("IMG_0003.JPG");
    std::fs::write(&jpg, tiny_jpeg()).unwrap();
    et.write_rating(&jpg, &old).unwrap();
    let result = write_while_read_only(&mut et, &jpg, &jpg, &new);
    assert!(
        matches!(&result, Err(pw_metadata::Error::ReadOnly(target)) if *target == jpg),
        "read-only JPEG: {result:?}"
    );
    assert_eq!(
        et.read_ratings(std::slice::from_ref(&jpg))
            .unwrap()
            .get(&jpg),
        Some(&old),
        "the JPEG keeps the old rating"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Uses a real DNG from the test set (copied, never the original).
#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn real_dng_is_written_in_place() {
    let mut et = exiftool();
    let src = test_file("IMG_20261026_062851.dng");
    let dir = temp_dir("dng");
    let dng = dir.join("copy.dng");
    std::fs::copy(&src, &dng).unwrap();
    check_embedded(&mut et, &dng);
}

#[test]
fn iptc_template_in_sidecar_and_embedded() {
    let mut et = exiftool();
    let dir = temp_dir("iptc");
    let raw = dir.join("TN0002.ARW");
    std::fs::write(&raw, b"raw").unwrap();
    let jpg = dir.join("Bild ä.jpg");
    std::fs::write(&jpg, tiny_jpeg()).unwrap();
    let iptc = Iptc {
        creator: "Tobias Nykänen".into(),
        copyright: LocalizedValue::x_default("© 2026 Tobias Nykänen"),
        email: "foto@example.com".into(),
        url: "https://example.com\nx".into(),
        ..Iptc::default()
    };
    // A rating set before must survive the template.
    et.write_rating(
        &raw,
        &Rating {
            stars: Some(4),
            label: None,
        },
    )
    .unwrap();
    let before_hash = image_hash(&mut et, &jpg);
    for f in [&raw, &jpg] {
        et.write_iptc(f, &iptc).unwrap();
    }
    assert_eq!(std::fs::read(&raw).unwrap(), b"raw", "RAW untouched");
    assert_eq!(
        image_hash(&mut et, &jpg),
        before_hash,
        "JPEG image data untouched"
    );
    for target in [sidecar_path(&raw), jpg.clone()] {
        let out = et
            .execute(&[
                "-s3".into(),
                "-f".into(),
                "-XMP-dc:Creator".into(),
                "-XMP-dc:Rights".into(),
                "-XMP-iptcCore:CreatorWorkEmail".into(),
                "-XMP-iptcCore:CreatorWorkURL".into(),
                "-XMP-photoshop:Credit".into(),
                "-XMP-xmpRights:Marked".into(),
                target.to_string_lossy().into_owned(),
            ])
            .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines,
            [
                "Tobias Nykänen",
                "© 2026 Tobias Nykänen",
                "foto@example.com",
                "https://example.com x",
                "-",
                "True"
            ],
            "{}",
            target.display()
        );
    }
    assert_eq!(
        et.read_ratings(std::slice::from_ref(&raw)).unwrap()[&raw].stars,
        Some(4)
    );
    // Empty template: nothing written, no error.
    et.write_iptc(&jpg, &Iptc::default()).unwrap();
    assert!(Iptc::default().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A safe write keeps the sidecar's name as it is on disk (T132): a `<basename>.xmp` in any case
/// (Photo Mechanic's `.XMP`) is written under that name; only a new sidecar is named `.xmp`. The
/// safe write uses Windows file operations, so these tests are built on Windows only.
#[cfg(windows)]
mod the_sidecar_name_is_kept {
    use std::path::{Path, PathBuf};

    use pw_metadata::edit::{Edit, EditOptions};
    use pw_metadata::iim::Fallback;
    use pw_metadata::journal::Journal;
    use pw_metadata::safe_write::{NoImageCheck, WriteReport};
    use pw_metadata::value::Value;

    use super::{exiftool, temp_dir};

    /// A small XMP packet with a City, so the test can see that the existing file was written.
    const PACKET: &str = "<?xpacket begin='\u{feff}' id='W5M0MpCehiHzreSzNTczkc9d'?>\n\
        <x:xmpmeta xmlns:x='adobe:ns:meta/'>\n\
        <rdf:RDF xmlns:rdf='http://www.w3.org/1999/02/22-rdf-syntax-ns#'>\n\
        <rdf:Description rdf:about='' \
        xmlns:photoshop='http://ns.adobe.com/photoshop/1.0/'>\n\
        <photoshop:City>Lund</photoshop:City>\n\
        </rdf:Description>\n\
        </rdf:RDF>\n\
        </x:xmpmeta>\n\
        <?xpacket end='w'?>";

    /// The names in `dir`, sorted, byte-exact as listed.
    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// The value `read_metadata` gives for `property` of `image`.
    fn read_text(et: &mut pw_metadata::ExifTool, image: &Path, property: &str) -> Option<Value> {
        et.read_metadata(image, Fallback::Windows1252)
            .unwrap()
            .reconciled
            .metadata
            .get(property)
            .map(|stored| stored.value.clone())
    }

    /// Writes a Headline to `TN0001.ARW` in a new folder holding a sidecar named `sidecar` (or
    /// none) and checks the folder, the values, the report, the leftovers and the journal.
    /// `expected` is the sidecar's name after the write.
    fn headline_written(name: &str, sidecar: Option<&str>, expected: &str) {
        let mut et = exiftool();
        let dir = temp_dir(&format!("keep-name-{name}"));
        let journal_dir = temp_dir(&format!("keep-name-{name}-journal"));
        let journal = Journal::open(&journal_dir).unwrap();
        let raw = dir.join("TN0001.ARW");
        std::fs::write(&raw, b"not really a raw file").unwrap();
        if let Some(sidecar) = sidecar {
            std::fs::write(dir.join(sidecar), PACKET).unwrap();
        }

        let edits = [Edit::Set {
            property: "photoshop:Headline".to_string(),
            field: None,
            value: Value::Text("Rubrik".to_string()),
        }];
        let report = et
            .write_edits_safely(
                &raw,
                &edits,
                &EditOptions::default(),
                &NoImageCheck,
                &journal,
            )
            .unwrap();

        assert_eq!(
            listing(&dir),
            ["TN0001.ARW".to_string(), expected.to_string()],
            "the folder after the write"
        );
        match &report.report {
            WriteReport::Written { target } => {
                assert_eq!(target, &dir.join(expected));
                assert!(
                    target.to_string_lossy().ends_with(expected),
                    "the target {} ends in {expected}",
                    target.display()
                );
            }
            other => panic!("expected a write of {expected}, got {other:?}"),
        }
        assert_eq!(
            read_text(&mut et, &raw, "photoshop:Headline"),
            Some(Value::Text("Rubrik".to_string()))
        );
        if sidecar.is_some() {
            assert_eq!(
                read_text(&mut et, &raw, "photoshop:City"),
                Some(Value::Text("Lund".to_string())),
                "the existing sidecar's values are kept"
            );
        }
        assert_eq!(
            std::fs::read(&raw).unwrap(),
            b"not really a raw file",
            "RAW untouched"
        );
        let leftovers: Vec<String> = listing(&dir)
            .into_iter()
            .filter(|n| n.contains(".pwtmp-") || n.contains(".pwbak-"))
            .collect();
        assert_eq!(leftovers, Vec::<String>::new(), "files left over");
        let entries: Vec<PathBuf> = std::fs::read_dir(&journal_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(entries, Vec::<PathBuf>::new(), "journal entries left over");

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&journal_dir);
    }

    #[test]
    fn an_upper_case_sidecar_keeps_its_name() {
        headline_written("upper", Some("TN0001.XMP"), "TN0001.XMP");
    }

    #[test]
    fn a_mixed_case_sidecar_keeps_its_name() {
        headline_written("mixed", Some("TN0001.Xmp"), "TN0001.Xmp");
    }

    #[test]
    fn a_raw_without_a_sidecar_gets_a_new_lower_case_one() {
        headline_written("new", None, "TN0001.xmp");
    }
}
