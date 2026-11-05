//! Integration tests against a real ExifTool. Skipped unless ExifTool is available:
//! `exiftool` on PATH, or `PW_EXIFTOOL_PL=/path/to/exiftool` (run through `perl`).

use std::path::PathBuf;

use pw_metadata::{ExifTool, Iptc, Rating, sidecar_path, supports_sidecar};

fn exiftool() -> Option<ExifTool> {
    if let Ok(pl) = std::env::var("PW_EXIFTOOL_PL") {
        return ExifTool::start_with("perl", &[&pl]).ok();
    }
    let mut et = ExifTool::start().ok()?;
    et.execute(&["-ver".to_string()]).ok().map(|_| et)
}

fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pw-metadata-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn write_read_update_clear() {
    let Some(mut et) = exiftool() else {
        eprintln!("skipping: ExifTool not available");
        return;
    };
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
    let Some(mut et) = exiftool() else { return };
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
    let Some(mut et) = exiftool() else { return };
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
    let Some(mut et) = exiftool() else { return };
    let dir = temp_dir("embedded");
    let jpg = dir.join("IMG_0001.JPG");
    std::fs::write(&jpg, tiny_jpeg()).unwrap();
    check_embedded(&mut et, &jpg);
    let tif = dir.join("Scan 01.tif");
    std::fs::write(&tif, tiny_tiff()).unwrap();
    check_embedded(&mut et, &tif);
}

/// Uses a real DNG from the test set (copied, never the original).
#[test]
fn real_dng_is_written_in_place() {
    let Some(mut et) = exiftool() else { return };
    let Some(src) = std::env::var_os("PW_TESTDATA")
        .map(|d| PathBuf::from(d).join("IMG_20261026_062851.dng"))
        .filter(|p| p.exists())
    else {
        eprintln!("skipping: PW_TESTDATA DNG not found");
        return;
    };
    let dir = temp_dir("dng");
    let dng = dir.join("copy.dng");
    std::fs::copy(&src, &dng).unwrap();
    check_embedded(&mut et, &dng);
}

#[test]
fn iptc_template_in_sidecar_and_embedded() {
    let Some(mut et) = exiftool() else {
        eprintln!("skipping: ExifTool not available");
        return;
    };
    let dir = temp_dir("iptc");
    let raw = dir.join("TN0002.ARW");
    std::fs::write(&raw, b"raw").unwrap();
    let jpg = dir.join("Bild ä.jpg");
    std::fs::write(&jpg, tiny_jpeg()).unwrap();
    let iptc = Iptc {
        creator: "Tobias Nykänen".into(),
        copyright: "© 2026 Tobias Nykänen".into(),
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
