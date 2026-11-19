//! Tests of which file holds an image's metadata (`supports_sidecar`, `sidecar_path`,
//! `rating_file`), and, on Windows, integration tests of the safe write keeping a sidecar's name,
//! against a real ExifTool: `exiftool` on PATH, or `PW_EXIFTOOL_PL=/path/to/exiftool` (run through
//! `perl`).
//!
//! The ExifTool tests fail with an explanation when ExifTool is missing, instead of returning early
//! and being counted as passed (docs/TESTING.md §4.5). To leave them out deliberately, run e.g.
//! `cargo test --workspace --exclude pw-metadata`, so the omission is visible.

#[cfg(windows)]
use std::path::PathBuf;

#[cfg(windows)]
use pw_metadata::ExifTool;
use pw_metadata::{sidecar_path, supports_sidecar};

/// Starts ExifTool and checks that it answers. Panics (fails the test) if it is unavailable.
#[cfg(windows)]
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

#[cfg(windows)]
fn temp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pw-metadata-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
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
