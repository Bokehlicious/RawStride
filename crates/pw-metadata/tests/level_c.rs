//! Real-file tests with the level-C samples (ADR 0007 I12, T127): files that Photo Mechanic,
//! Lightroom Classic and a Xiaomi phone really wrote, read with `read_metadata` and written on
//! copies with `write_edits_safely`.
//!
//! The samples are in `PW_TESTDATA\level-c` (ADR 0007 §12, Q11). The tests are ignored by default;
//! run them with `$env:PW_TESTDATA = "<folder>"; cargo test --workspace --test level_c -- --ignored`.
//! They need ExifTool (`exiftool` on PATH, or `PW_EXIFTOOL_PL=/path/to/exiftool` run through
//! `perl`) and fail with an explanation when it is missing (docs/TESTING.md §4.5).
//!
//! The samples are never written: every write is done on a copy in a temporary folder, and each
//! test records the size and SHA-256 of the samples it uses at the start and checks them at the
//! end. Values that are the owner's (name, e-mail, phone, copyright text) are never written here:
//! they are compared with what ExifTool itself reads from the file. The safe write uses Windows
//! file operations, so the file is built on Windows only (as `tests/edits.rs`).
//!
//! The samples were also written by RawStride (ExifTool 13.59) on 2026-11-19; the tests follow
//! them as they are since then (2026-11-20, T234), and the samples are kept read-only.
#![cfg(windows)]

use std::io::Read;
use std::path::{Path, PathBuf};

use pw_metadata::ExifTool;
use pw_metadata::edit::{Edit, EditOptions};
use pw_metadata::iim::{CodedCharset, Fallback};
use pw_metadata::journal::Journal;
use pw_metadata::properties;
use pw_metadata::read::{FileNotice, MetadataRead};
use pw_metadata::reconcile::{IimDigest, Notice, same_value};
use pw_metadata::safe_write::{NoImageCheck, WriteReport};
use pw_metadata::value::{Origin, Value};
use pw_metadata::write_plan::WriteNotice;
use serde_json::{Map, Value as Json};

const PM_JPG: &str = "PM-jpg.JPG";
const XIAOMI_JPG: &str = "xiaomi_jpg.jpg";
const PM_RAW: &str = "PM-raw.ARW";
const PM_RAW_SIDECAR: &str = "PM-raw.XMP";
const XIAOMI_DNG: &str = "xiaomi_dng.dng";
const TN_DNG: &str = "TN002363.DNG";

/// The six files the folder must hold (T127 point 1).
const SAMPLES: &[&str] = &[
    PM_JPG,
    XIAOMI_JPG,
    PM_RAW,
    PM_RAW_SIDECAR,
    XIAOMI_DNG,
    TN_DNG,
];

/// The neutral Headline the write tests set.
const HEADLINE: &str = "Level-C write test";
/// The length of the long Description in `PM-jpg.JPG` and `PM-raw.XMP`.
const LONG_DESCRIPTION: usize = 2191;

/// The properties whose IIM and XMP values must agree in the two JPEGs (point 4).
const IIM_AND_XMP: &[&str] = &[
    "dc:description",
    "dc:creator",
    "dc:subject",
    "photoshop:City",
    "dc:rights",
];

/// The four PLUS properties of the Photo Mechanic samples and their codes. ExifTool removes
/// [`PLUS_PREFIX`] when it reads (also with `-n`) and adds it when it writes; the file holds the
/// full URI.
const PLUS: &[(&str, &str)] = &[
    ("plus:MinorModelAgeDisclosure", "AG-A25"),
    ("plus:ModelReleaseStatus", "MR-NAP"),
    ("plus:PropertyReleaseStatus", "PR-NON"),
    ("plus:DataMining", "DMI-UNSPECIFIED"),
];
/// The prefix of the PLUS vocabulary URIs.
const PLUS_PREFIX: &str = "http://ns.useplus.org/ldf/vocab/";

/// The XMP that a write must leave as it is (point 5), read by ExifTool before and after.
const KEPT_XMP: &[&str] = &[
    "-XMP-photomech:all",
    "-XMP-xmpMM:all",
    "-XMP-iptcExt:LocationCreated",
    "-XMP-iptcExt:ModelAge",
    "-XMP-plus:all",
];

// ---------------------------------------------------------------------------------------------
// The samples
// ---------------------------------------------------------------------------------------------

/// `PW_TESTDATA\level-c`. Fails the test when `PW_TESTDATA` is not set or the folder is missing.
fn level_c() -> PathBuf {
    let Some(dir) = std::env::var_os("PW_TESTDATA") else {
        panic!("PW_TESTDATA is not set; set it to the real camera test-data folder");
    };
    let dir = PathBuf::from(dir).join("level-c");
    assert!(
        dir.is_dir(),
        "the level-C folder does not exist: {}",
        dir.display()
    );
    dir
}

/// The sample `name`. A missing file fails the test with its name (it is not skipped).
fn sample(name: &str) -> PathBuf {
    let dir = level_c();
    let path = dir.join(name);
    assert!(
        path.is_file(),
        "the level-C sample {name} was not found in {}",
        dir.display()
    );
    path
}

/// The size and SHA-256 of samples, recorded at the start of a test and checked at its end
/// (point 2).
struct Fingerprints {
    files: Vec<(PathBuf, u64, String)>,
}

impl Fingerprints {
    fn of(names: &[&str]) -> Fingerprints {
        let files = names
            .iter()
            .map(|name| {
                let path = sample(name);
                let (size, hash) = size_and_sha256(&path);
                (path, size, hash)
            })
            .collect();
        Fingerprints { files }
    }

    fn assert_unchanged(&self) {
        for (path, size, hash) in &self.files {
            let (now_size, now_hash) = size_and_sha256(path);
            assert_eq!(
                (now_size, &now_hash),
                (*size, hash),
                "the sample {} has changed",
                path.display()
            );
        }
    }
}

fn size_and_sha256(path: &Path) -> (u64, String) {
    let mut file =
        std::fs::File::open(path).unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file
            .read(&mut buffer)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    (hasher.len, hasher.finish_hex())
}

/// SHA-256 (FIPS 180-4), here so the test needs no new dependency.
struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    filled: usize,
    len: u64,
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Sha256 {
    fn new() -> Sha256 {
        Sha256 {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            block: [0; 64],
            filled: 0,
            len: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.len += data.len() as u64;
        while !data.is_empty() {
            let take = (64 - self.filled).min(data.len());
            self.block[self.filled..self.filled + take].copy_from_slice(&data[..take]);
            self.filled += take;
            data = &data[take..];
            if self.filled == 64 {
                let block = self.block;
                self.compress(&block);
                self.filled = 0;
            }
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (state, value) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *state = state.wrapping_add(value);
        }
    }

    fn finish_hex(mut self) -> String {
        let bits = self.len.wrapping_mul(8);
        let len = self.len;
        self.update(&[0x80]);
        while self.filled != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        self.len = len;
        self.state
            .iter()
            .map(|word| format!("{word:08x}"))
            .collect()
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finish_hex()
}

/// The SHA-256 above against the test vectors of FIPS 180-4 (not ignored: it needs no samples).
#[test]
fn the_sha256_of_the_fingerprints_matches_the_standard_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    let million_a = vec![b'a'; 1_000_000];
    assert_eq!(
        sha256_hex(&million_a),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

// ---------------------------------------------------------------------------------------------
// ExifTool
// ---------------------------------------------------------------------------------------------

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

/// One tag of `file` as ExifTool prints it (`-s3 -f`): `-` when it is missing.
fn tag(et: &mut ExifTool, file: &Path, name: &str) -> String {
    et.execute(&[
        "-s3".to_string(),
        "-f".to_string(),
        format!("-{name}"),
        file.to_string_lossy().into_owned(),
    ])
    .unwrap()
    .trim_end_matches(['\r', '\n'])
    .to_string()
}

/// The tags `tags` of `file` as ExifTool's JSON object (`-j -G1 -n -struct`), without
/// `SourceFile`.
fn json_tags(et: &mut ExifTool, file: &Path, tags: &[&str]) -> Map<String, Json> {
    let mut args: Vec<String> = ["-j", "-G1", "-n", "-struct"]
        .iter()
        .chain(tags)
        .map(|arg| arg.to_string())
        .collect();
    args.push(file.to_string_lossy().into_owned());
    let answer = et.execute(&args).unwrap();
    let stdout = answer
        .split_once("--- stderr ---\n")
        .map_or(answer.as_str(), |(out, _)| out);
    let json: Json = serde_json::from_str(stdout)
        .unwrap_or_else(|e| panic!("no JSON for {}: {e}: {answer}", file.display()));
    let Some(Json::Object(mut object)) = json.as_array().and_then(|items| items.first()).cloned()
    else {
        panic!("no JSON object for {}: {answer}", file.display());
    };
    object.remove("SourceFile");
    object
}

/// The XMP packet of `file` as ExifTool gives it (`-b -XMP`). The packet does not end with a line
/// break, so `-echo3` prints a line after it; otherwise ExifTool's `{ready}` marker would follow on
/// the same line and `execute` would wait for it forever (as `tests/edits.rs`).
fn xmp_packet(et: &mut ExifTool, file: &Path) -> String {
    let packet = et
        .execute(&[
            "-b".to_string(),
            "-XMP".to_string(),
            "-echo3".to_string(),
            "--end of XMP--".to_string(),
            file.to_string_lossy().into_owned(),
        ])
        .unwrap();
    assert!(
        packet.contains("<x:xmpmeta"),
        "no XMP packet for {}: {packet}",
        file.display()
    );
    packet
}

/// Each PLUS code is in `packet` as its full URI exactly once, and the prefix is never doubled.
fn assert_plus_uris_once(name: &str, packet: &str) {
    for (property, code) in PLUS {
        let uri = format!("{PLUS_PREFIX}{code}");
        assert_eq!(
            packet.matches(uri.as_str()).count(),
            1,
            "{name}: {property}: the full URI {uri} is not in the XMP exactly once"
        );
    }
    let doubled = format!("{PLUS_PREFIX}{PLUS_PREFIX}");
    assert!(
        !packet.contains(&doubled),
        "{name}: the PLUS prefix is doubled"
    );
}

/// ExifTool's SHA-256 `ImageDataHash` of `file`.
fn image_data_hash(et: &mut ExifTool, file: &Path) -> String {
    let answer = et
        .execute(&[
            "-s3".to_string(),
            "-api".to_string(),
            "ImageHashType=SHA256".to_string(),
            "-ImageDataHash".to_string(),
            file.to_string_lossy().into_owned(),
        ])
        .unwrap();
    let hash = answer.trim().to_string();
    assert!(
        hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()),
        "no ImageDataHash for {}: {answer}",
        file.display()
    );
    hash
}

/// `read_metadata` with the default fallback; fails the test on an error.
fn read(et: &mut ExifTool, image: &Path) -> MetadataRead {
    et.read_metadata(image, Fallback::default())
        .unwrap_or_else(|e| panic!("read_metadata failed for {}: {e}", image.display()))
}

/// The value of `property` in `read`.
fn value<'a>(read: &'a MetadataRead, property: &str) -> Option<&'a Value> {
    read.reconciled
        .metadata
        .get(property)
        .map(|stored| &stored.value)
}

/// The `x-default` (or shown) text of a Lang Alt value.
fn lang_alt_text(value: Option<&Value>) -> Option<&str> {
    match value {
        Some(Value::LangAlt(lang_alt)) => lang_alt.display().map(|(text, _)| text),
        _ => None,
    }
}

/// Every property of the table with its value in `read`, `skip` left out.
fn all_values(read: &MetadataRead, skip: &str) -> Vec<(&'static str, Option<Value>)> {
    properties::properties()
        .filter(|def| def.xmp_id != skip)
        .map(|def| (def.xmp_id, value(read, def.xmp_id).cloned()))
        .collect()
}

/// Prints the conflicts and notices of `read` for the review.
fn print_read(name: &str, read: &MetadataRead) {
    println!("--- {name}");
    for conflict in read.reconciled.metadata.conflicts() {
        println!(
            "conflict: {} rule {:?}: {:?}",
            conflict.property, conflict.rule, conflict.values
        );
    }
    for notice in &read.reconciled.notices {
        println!("notice: {notice:?}");
    }
    for notice in &read.file_notices {
        println!("file notice: {notice:?}");
    }
    println!(
        "IIM present {}, charset {:?}, digest {:?}",
        read.state.iim_present, read.state.iim_charset, read.state.digest
    );
}

/// The file in `dir` whose name is `name` without regard to case, with its name as stored.
fn stored_name(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .find(|stored| stored.eq_ignore_ascii_case(name))
}

// ---------------------------------------------------------------------------------------------
// Reading (point 4) and what the samples show (point 6)
// ---------------------------------------------------------------------------------------------

/// Every sample is there and is read without error; prints the conflicts and notices, the empty
/// top-level XMP properties of `PM-jpg.JPG` and the sidecar's file name (point 6).
#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn every_sample_is_there_and_is_read_without_error() {
    let fingerprints = Fingerprints::of(SAMPLES);
    let mut et = exiftool();

    for name in [PM_JPG, XIAOMI_JPG, PM_RAW, XIAOMI_DNG, TN_DNG] {
        let read = read(&mut et, &sample(name));
        print_read(name, &read);
    }

    // Point 6: Photo Mechanic removes an emptied field (expected: none listed).
    let xmp = json_tags(&mut et, &sample(PM_JPG), &["-XMP:all"]);
    let empty: Vec<&String> = xmp
        .iter()
        .filter(|(_, value)| match value {
            Json::String(text) => text.is_empty(),
            Json::Array(items) => items.is_empty(),
            Json::Object(fields) => fields.is_empty(),
            _ => false,
        })
        .map(|(key, _)| key)
        .collect();
    println!("{PM_JPG}: top-level XMP properties with an empty value: {empty:?}");
    println!(
        "the sidecar of {PM_RAW} is named {:?}",
        stored_name(&level_c(), PM_RAW_SIDECAR)
    );

    fingerprints.assert_unchanged();
}

/// Every sample is read-only, so a hand check cannot change it (T234).
#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn every_sample_is_read_only() {
    let writable: Vec<&str> = SAMPLES
        .iter()
        .copied()
        .filter(|name| {
            !std::fs::metadata(sample(name))
                .unwrap()
                .permissions()
                .readonly()
        })
        .collect();
    assert!(
        writable.is_empty(),
        "these level-C samples are writable: {writable:?}; make the level-C samples read-only \
         (attrib +R) so a hand check cannot change them"
    );
}

/// The two JPEGs: IIM read as UTF-8; Description, Creator, Keywords, City and Copyright Notice
/// are all in IIM and agree with XMP with no conflict; the stored digest matches (§5.1).
/// Since 2026-11-20 (T234): the samples were written outside the tests on 2026-11-19 (stored
/// digest and City in IIM added) and there is no backup, so the test follows them as they are.
#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn the_two_jpegs_read_iim_as_utf8_in_agreement_with_xmp() {
    let fingerprints = Fingerprints::of(&[PM_JPG, XIAOMI_JPG]);
    let mut et = exiftool();

    for name in [PM_JPG, XIAOMI_JPG] {
        let file = sample(name);
        let read = read(&mut et, &file);
        print_read(name, &read);

        assert!(read.state.iim_present, "{name}: no IIM");
        assert_eq!(read.state.iim_charset, CodedCharset::Utf8, "{name}");
        assert!(
            !read.reconciled.notices.iter().any(|notice| matches!(
                notice,
                Notice::Iim(pw_metadata::iim_read::IimNotice::Converted { .. })
            )),
            "{name}: IIM text converted: {:?}",
            read.reconciled.notices
        );
        assert_eq!(read.state.digest, IimDigest::Matches, "{name}");
        let stored = tag(&mut et, &file, "Photoshop:IPTCDigest");
        assert_ne!(stored, "-", "{name}: no stored digest");
        assert_eq!(
            stored,
            tag(&mut et, &file, "File:CurrentIPTCDigest"),
            "{name}: the stored digest is not the current one"
        );

        for property in IIM_AND_XMP {
            assert!(
                !read
                    .reconciled
                    .metadata
                    .conflicts()
                    .iter()
                    .any(|conflict| conflict.property == *property),
                "{name}: a conflict for {property}: {:?}",
                read.reconciled.metadata.conflicts()
            );
            let Some(shown) = value(&read, property) else {
                panic!("{name}: no value for {property}");
            };
            let iim = read
                .state
                .iim_values
                .iter()
                .find(|(id, _)| id == property)
                .map(|(_, value)| value);
            match iim {
                Some(iim) => assert!(
                    same_value(property, iim, shown),
                    "{name}: {property} differs: IIM {iim:?}, shown {shown:?}"
                ),
                None => panic!("{name}: {property} is not in IIM"),
            }
        }
        // The owner's values are compared with what ExifTool reads, never written here.
        assert_eq!(
            lang_alt_text(value(&read, "dc:rights")),
            Some(tag(&mut et, &file, "XMP-dc:Rights").as_str()),
            "{name}"
        );
        assert_eq!(
            value(&read, "dc:creator"),
            Some(&Value::List(vec![tag(&mut et, &file, "XMP-dc:Creator")])),
            "{name}"
        );
        let city = tag(&mut et, &file, "XMP-photoshop:City");
        assert!(
            !city.is_empty() && city != "-",
            "{name}: ExifTool read no XMP-photoshop:City: {city:?}"
        );
        assert_eq!(
            value(&read, "photoshop:City"),
            Some(&Value::Text(city)),
            "{name}"
        );
        assert_eq!(
            value(&read, "dc:subject"),
            Some(&Value::List(
                ["en", "testcarta", "colorchecker", "colorchecker passport"]
                    .iter()
                    .map(|keyword| keyword.to_string())
                    .collect()
            )),
            "{name}"
        );
    }

    fingerprints.assert_unchanged();
}

/// `PM-jpg.JPG`: the whole long Description; Location Created with City and Country and its empty
/// sub-fields read as empty texts (the inspector hides them, T130); Model Age `NA`; the four PLUS
/// properties read as their codes, with the full URIs in the file's XMP; Copyrighted True.
#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn pm_jpg_reads_the_long_description_location_model_age_and_plus() {
    let fingerprints = Fingerprints::of(&[PM_JPG]);
    let mut et = exiftool();
    let file = sample(PM_JPG);
    let read = read(&mut et, &file);
    print_read(PM_JPG, &read);

    let description = lang_alt_text(value(&read, "dc:description"));
    assert_eq!(description.map(str::len), Some(LONG_DESCRIPTION));
    assert_eq!(
        description,
        Some(tag(&mut et, &file, "XMP-dc:Description").as_str())
    );

    let Some(Value::StructList(locations)) = value(&read, "Iptc4xmpExt:LocationCreated") else {
        panic!(
            "Location Created is not a list of structures: {:?}",
            value(&read, "Iptc4xmpExt:LocationCreated")
        );
    };
    assert_eq!(locations.len(), 1, "{locations:?}");
    let fields = &locations[0].fields;
    let field = |id: &str| {
        fields
            .iter()
            .find(|(field, _)| field == id)
            .map(|(_, value)| value)
    };
    // One location (asserted above), so ExifTool's flattened tag is that location's City alone.
    let city = tag(&mut et, &file, "XMP-iptcExt:LocationCreatedCity");
    assert!(
        !city.is_empty() && city != "-",
        "ExifTool read no XMP-iptcExt:LocationCreatedCity: {city:?}"
    );
    assert_eq!(
        field("Iptc4xmpExt:City"),
        Some(&Value::Text(city)),
        "{fields:?}"
    );
    assert_eq!(
        field("Iptc4xmpExt:CountryName"),
        Some(&Value::Text("Sverige".to_string())),
        "{fields:?}"
    );
    for id in [
        "Iptc4xmpExt:Sublocation",
        "Iptc4xmpExt:ProvinceState",
        "Iptc4xmpExt:WorldRegion",
    ] {
        assert_eq!(
            field(id),
            Some(&Value::Text(String::new())),
            "{id}: {fields:?}"
        );
    }

    assert_eq!(
        value(&read, "Iptc4xmpExt:ModelAge"),
        Some(&Value::List(vec!["NA".to_string()]))
    );
    let model_age_notices: Vec<&Notice> = read
        .reconciled
        .notices
        .iter()
        .filter(|notice| format!("{notice:?}").contains("Iptc4xmpExt:ModelAge"))
        .collect();
    assert_eq!(model_age_notices, Vec::<&Notice>::new());

    for (property, code) in PLUS {
        assert_eq!(
            value(&read, property),
            Some(&Value::Text(code.to_string())),
            "{property}"
        );
    }
    assert_plus_uris_once(PM_JPG, &xmp_packet(&mut et, &file));
    assert_eq!(
        value(&read, "xmpRights:Marked"),
        Some(&Value::Text("True".to_string()))
    );

    fingerprints.assert_unchanged();
}

/// `PM-raw.ARW`: the upper-case `PM-raw.XMP` is found as `<basename>.xmp` (Q8), with no
/// foreign-sidecar notice, and gives the whole long Description.
#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn pm_raw_finds_its_upper_case_sidecar() {
    let fingerprints = Fingerprints::of(&[PM_RAW, PM_RAW_SIDECAR]);
    let mut et = exiftool();
    let read = read(&mut et, &sample(PM_RAW));
    print_read(PM_RAW, &read);

    assert!(
        !read.file_notices.iter().any(|notice| matches!(
            notice,
            FileNotice::ForeignSidecarIgnored(_) | FileNotice::ExtSidecarIgnored(_)
        )),
        "{:?}",
        read.file_notices
    );
    let description = read.reconciled.metadata.get("dc:description");
    assert_eq!(
        description.map(|stored| stored.origin),
        Some(Origin::XmpSidecar),
        "the Description does not come from the sidecar: {description:?}"
    );
    let text = lang_alt_text(description.map(|stored| &stored.value));
    assert_eq!(text.map(str::len), Some(LONG_DESCRIPTION));
    assert_eq!(
        text,
        Some(tag(&mut et, &sample(PM_RAW_SIDECAR), "XMP-dc:Description").as_str())
    );

    fingerprints.assert_unchanged();
}

/// `TN002363.DNG` (Lightroom Classic and Photo Mechanic 5) is read without error; its conflicts
/// are printed with their rule, none is asserted.
#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn tn002363_is_read_and_its_conflicts_listed() {
    let fingerprints = Fingerprints::of(&[TN_DNG]);
    let mut et = exiftool();
    let read = read(&mut et, &sample(TN_DNG));
    print_read(TN_DNG, &read);
    println!(
        "{TN_DNG}: {} conflict(s)",
        read.reconciled.metadata.conflicts().len()
    );
    fingerprints.assert_unchanged();
}

// ---------------------------------------------------------------------------------------------
// Writing on copies (point 5)
// ---------------------------------------------------------------------------------------------

/// A new, empty temporary folder.
fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("pw-metadata-level-c-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The folders of one write test: the copies, and the write journal in a folder of its own.
struct Scratch {
    dir: PathBuf,
    journal_dir: PathBuf,
    journal: Journal,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = temp_dir(name);
        let journal_dir = temp_dir(&format!("{name}-journal"));
        let journal = Journal::open(&journal_dir).unwrap();
        Scratch {
            dir,
            journal_dir,
            journal,
        }
    }

    /// Copies the samples `names` into the folder under their own names; returns the first copy.
    fn copy(&self, names: &[&str]) -> PathBuf {
        let copies: Vec<PathBuf> = names
            .iter()
            .map(|name| {
                let copy = self.dir.join(name);
                std::fs::copy(sample(name), &copy).unwrap();
                // The samples are read-only and the copy keeps that attribute, but the safe write
                // refuses a read-only file: make the copy (never the sample) writable.
                let mut permissions = std::fs::metadata(&copy).unwrap().permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                std::fs::set_permissions(&copy, permissions).unwrap();
                copy
            })
            .collect();
        copies[0].clone()
    }

    /// No `pwtmp`/`pwbak` file in the folder and an empty journal.
    fn assert_clean(&self) {
        let leftovers: Vec<String> = std::fs::read_dir(&self.dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".pwtmp-") || name.contains(".pwbak-"))
            .collect();
        assert_eq!(leftovers, Vec::<String>::new(), "files left over");
        let entries: Vec<PathBuf> = std::fs::read_dir(&self.journal_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(entries, Vec::<PathBuf>::new(), "journal entries left over");
    }

    fn remove(self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_dir_all(&self.journal_dir);
    }
}

/// What a write test checks besides the common points.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A JPEG or DNG, written in place: its `ImageDataHash` must stay.
    Embedded,
    /// The ARW: only its sidecar is written; its bytes must stay.
    Raw,
}

/// Writes the Headline on a copy of `names[0]` (with the other names copied beside it) and checks
/// point 5; returns the edit's notices and the read-back for the file's own checks.
fn headline_on_a_copy(test: &str, names: &[&str], kind: Kind) -> (Vec<WriteNotice>, MetadataRead) {
    let fingerprints = Fingerprints::of(names);
    let mut et = exiftool();
    let s = Scratch::new(test);
    let image = s.copy(names);
    let name = names[0];
    let sidecar_name = format!(
        "{}.xmp",
        Path::new(name).file_stem().unwrap().to_string_lossy()
    );
    let xmp_file = |dir: &Path| match kind {
        Kind::Embedded => dir.join(name),
        Kind::Raw => dir.join(stored_name(dir, &sidecar_name).expect("the sidecar is missing")),
    };

    let before = read(&mut et, &image);
    print_read(&format!("{name} before"), &before);
    let kept_before = json_tags(&mut et, &xmp_file(&s.dir), KEPT_XMP);
    assert!(
        kept_before
            .keys()
            .any(|key| key.starts_with("XMP-photomech:")),
        "{name}: no photomechanic: properties before the write: {kept_before:?}"
    );
    let image_before = match kind {
        Kind::Embedded => image_data_hash(&mut et, &image),
        Kind::Raw => size_and_sha256(&image).1,
    };
    println!(
        "{name}: sidecar before: {:?}",
        stored_name(&s.dir, &sidecar_name)
    );

    let result = et.write_edits_safely(
        &image,
        &[Edit::Set {
            property: "photoshop:Headline".to_string(),
            field: None,
            value: Value::Text(HEADLINE.to_string()),
        }],
        &EditOptions::default(),
        &NoImageCheck,
        &s.journal,
    );
    let report = match result {
        Ok(report) => report,
        Err(error) => panic!("{name}: the write failed: {error}"),
    };
    println!("{name}: report {:?}", report.report);
    println!("{name}: notices {:?}", report.notices);
    println!(
        "{name}: sidecar after: {:?}",
        stored_name(&s.dir, &sidecar_name)
    );
    assert!(
        matches!(report.report, WriteReport::Written { .. }),
        "{name}: {:?}",
        report.report
    );

    let after = read(&mut et, &image);
    print_read(&format!("{name} after"), &after);
    assert_eq!(
        value(&after, "photoshop:Headline"),
        Some(&Value::Text(HEADLINE.to_string())),
        "{name}"
    );
    let changed: Vec<String> = all_values(&before, "photoshop:Headline")
        .into_iter()
        .zip(all_values(&after, "photoshop:Headline"))
        .filter(|(old, new)| old != new)
        .map(|(old, new)| format!("{}: {:?} -> {:?}", old.0, old.1, new.1))
        .collect();
    assert_eq!(
        changed,
        Vec::<String>::new(),
        "{name}: values changed by the write"
    );

    let kept_after = json_tags(&mut et, &xmp_file(&s.dir), KEPT_XMP);
    assert_eq!(kept_after, kept_before, "{name}: kept XMP changed");
    if kept_before.keys().any(|key| key.starts_with("XMP-plus:")) {
        // The PLUS values are still full URIs in the file, once each (point 5).
        assert_plus_uris_once(name, &xmp_packet(&mut et, &xmp_file(&s.dir)));
    }
    println!("{name}: kept XMP {kept_after:?}");
    if let Some(Json::Array(locations)) = kept_after.get("XMP-iptcExt:LocationCreated") {
        // The empty sub-fields Photo Mechanic wrote are still in the file.
        for location in locations {
            assert_eq!(
                location.get("Sublocation"),
                Some(&Json::String(String::new())),
                "{name}: {location:?}"
            );
        }
    }

    let image_after = match kind {
        Kind::Embedded => image_data_hash(&mut et, &image),
        Kind::Raw => size_and_sha256(&image).1,
    };
    assert_eq!(image_after, image_before, "{name}: the image data changed");

    s.assert_clean();
    s.remove();
    fingerprints.assert_unchanged();
    (report.notices, after)
}

/// The kept XMP is there where the samples have it (so "unchanged" is not "absent before and
/// after").
fn assert_pm_values(name: &str, read: &MetadataRead) {
    assert_eq!(
        value(read, "Iptc4xmpExt:ModelAge"),
        Some(&Value::List(vec!["NA".to_string()])),
        "{name}"
    );
    for (property, code) in PLUS {
        assert_eq!(
            value(read, property),
            Some(&Value::Text(code.to_string())),
            "{name}: {property}"
        );
    }
    assert!(
        value(read, "Iptc4xmpExt:LocationCreated").is_some(),
        "{name}"
    );
}

/// `PM-jpg.JPG` (ADR 0007 §5.1 as changed by T129): XMP and IIM keep the full Description, which
/// is not rewritten (no `Truncated`, no `ConflictKept`); the stored digest matches.
#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn a_headline_on_a_copy_of_pm_jpg_keeps_the_long_description() {
    let (notices, after) = headline_on_a_copy("pm-jpg", &[PM_JPG], Kind::Embedded);
    assert_pm_values(PM_JPG, &after);
    assert_eq!(
        lang_alt_text(value(&after, "dc:description")).map(str::len),
        Some(LONG_DESCRIPTION)
    );
    assert!(
        !notices.iter().any(|notice| matches!(
            notice,
            WriteNotice::Truncated { .. } | WriteNotice::ConflictKept { .. }
        )),
        "{notices:?}"
    );
    let iim = after
        .state
        .iim_values
        .iter()
        .find(|(id, _)| id == "dc:description")
        .map(|(_, value)| value);
    let iim_len = match iim {
        Some(Value::LangAlt(lang_alt)) => lang_alt.display().map(|(text, _)| text.len()),
        Some(Value::Text(text)) => Some(text.len()),
        _ => None,
    };
    assert_eq!(iim_len, Some(LONG_DESCRIPTION), "IIM Caption-Abstract");
    assert_eq!(after.state.digest, IimDigest::Matches);
}

#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn a_headline_on_a_copy_of_xiaomi_jpg_keeps_everything_else() {
    let (_, after) = headline_on_a_copy("xiaomi-jpg", &[XIAOMI_JPG], Kind::Embedded);
    assert_pm_values(XIAOMI_JPG, &after);
    assert_ne!(after.state.digest, IimDigest::Differs);
}

#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn a_headline_on_a_copy_of_pm_raw_writes_only_the_sidecar() {
    let (_, after) = headline_on_a_copy("pm-raw", &[PM_RAW, PM_RAW_SIDECAR], Kind::Raw);
    assert_pm_values(PM_RAW, &after);
    assert_eq!(
        lang_alt_text(value(&after, "dc:description")).map(str::len),
        Some(LONG_DESCRIPTION)
    );
}

#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn a_headline_on_a_copy_of_xiaomi_dng_keeps_everything_else() {
    let (_, after) = headline_on_a_copy("xiaomi-dng", &[XIAOMI_DNG], Kind::Embedded);
    assert_pm_values(XIAOMI_DNG, &after);
}

/// `TN002363.DNG`: the Lightroom `xmpMM` history and the Photo Mechanic 5 properties are kept.
#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn a_headline_on_a_copy_of_tn002363_keeps_its_history() {
    let mut et = exiftool();
    let history = json_tags(&mut et, &sample(TN_DNG), &["-XMP-xmpMM:History"]);
    assert!(
        history.contains_key("XMP-xmpMM:History"),
        "precondition: no xmpMM history: {history:?}"
    );
    let (_, after) = headline_on_a_copy("tn002363", &[TN_DNG], Kind::Embedded);
    print_read(TN_DNG, &after);
}

// ---------------------------------------------------------------------------------------------
// The orientation (ADR 0024 O2, T233)
// ---------------------------------------------------------------------------------------------

/// Writes `orientation` on a copy of the DNG `name` with `NoImageCheck` and checks the common
/// points: `Written`, IFD0 Orientation is `orientation`, the image data is unchanged, nothing is
/// left over, and the same value again is `Unchanged`. Returns the ExifTool and the scratch
/// folder with the copy for the test's own checks.
fn orientation_on_a_copy(test: &str, name: &str, orientation: u16) -> (ExifTool, Scratch, PathBuf) {
    let mut et = exiftool();
    let s = Scratch::new(test);
    let image = s.copy(&[name]);
    let image_before = image_data_hash(&mut et, &image);

    let report = et.write_orientation_safely(&image, orientation, &NoImageCheck, &s.journal);
    match report {
        Ok(WriteReport::Written { target }) => assert_eq!(target, image),
        other => panic!("{name}: expected Written, got {other:?}"),
    }
    assert_eq!(
        tag(&mut et, &image, "IFD0:Orientation#"),
        orientation.to_string()
    );
    assert_eq!(image_data_hash(&mut et, &image), image_before);
    s.assert_clean();
    match et.write_orientation_safely(&image, orientation, &NoImageCheck, &s.journal) {
        Ok(WriteReport::Unchanged) => {}
        other => panic!("{name}: expected Unchanged the second time, got {other:?}"),
    }
    (et, s, image)
}

/// `xiaomi_dng.dng` (IFD0 Orientation 6) turned to 3 on a copy.
#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn an_orientation_on_a_copy_of_the_xiaomi_dng() {
    let fingerprints = Fingerprints::of(&[XIAOMI_DNG]);
    let mut et = exiftool();
    assert_eq!(
        tag(&mut et, &sample(XIAOMI_DNG), "IFD0:Orientation#"),
        "6",
        "precondition: the sample's IFD0 Orientation"
    );

    let (_, s, _) = orientation_on_a_copy("xiaomi-dng-orientation", XIAOMI_DNG, 3);

    s.remove();
    fingerprints.assert_unchanged();
}

/// `TN002363.DNG` (IFD0 and `XMP-tiff:Orientation` both 1) turned to 8 on a copy: the XMP value
/// is kept in step.
#[test]
#[ignore = "needs the level-C samples: set PW_TESTDATA and run with --ignored"]
fn an_orientation_on_a_copy_of_tn002363_keeps_xmp_in_step() {
    let fingerprints = Fingerprints::of(&[TN_DNG]);
    let mut et = exiftool();
    let original = sample(TN_DNG);
    assert_eq!(
        (
            tag(&mut et, &original, "IFD0:Orientation#"),
            tag(&mut et, &original, "XMP-tiff:Orientation#")
        ),
        ("1".to_string(), "1".to_string()),
        "precondition: the sample's orientations"
    );

    let (mut et, s, image) = orientation_on_a_copy("tn002363-orientation", TN_DNG, 8);

    assert_eq!(tag(&mut et, &image, "XMP-tiff:Orientation#"), "8");
    s.remove();
    fingerprints.assert_unchanged();
}
