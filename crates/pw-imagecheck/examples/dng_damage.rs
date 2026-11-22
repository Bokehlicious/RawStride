//! Which check of the safe metadata write refuses which kind of DNG damage (T273, ADR 0007 §9.7
//! point 4, Q23).
//!
//! For each DNG the program finds the main RAW image's IFD with a small TIFF reader (IFD0's chain
//! and every IFD's `SubIFDs`, nested ones too, depth first in file order; the first IFD whose
//! `NewSubfileType` is 0 or missing and that has strip or tile offsets), prints it, and makes one
//! damaged copy per damage kind, each from a fresh copy of the original and with only the named
//! bytes changed in place (the file's length unchanged):
//!
//! - a. data moved: the first strip or tile offset + 4096;
//! - b. two pieces swapped: the first two strip or tile offsets exchanged;
//! - c. a short piece: the first strip or tile byte count halved;
//! - d. data zeroed: 4096 bytes (or half the piece) set to 0 in the middle of the first piece;
//! - e. a wrong width: the RAW IFD's `ImageWidth` − 16;
//! - f. a wrong compression: the RAW IFD's `Compression` changed to 1 (or to 7 when it is 1);
//! - g. a broken link: the pointer that leads to the RAW IFD set past the end of the file.
//!
//! Each damaged copy is checked against an unchanged copy of the original by: the snapshot
//! comparison of the safe write (`ExifTool::snapshot` of both, each by a new ExifTool process,
//! then `verify_copy` with a plan that writes nothing), the image data hash alone, the preview
//! comparison alone (`check_copy` with `FileKind::Tiff`), LibRaw's open and unpack through its C
//! API (Windows only), the shim's verdict (`Session::open_with_diagnostics`) and the whole check
//! (`RawImageCheck::without_release().check_copy(…, FileKind::Dng)`). Since a damaged copy may make
//! LibRaw or the shim crash or hang, each pair is checked in a child process (the program run
//! again with the hidden option `--one <original> <damaged copy>`), which is killed after 120 s.
//! At the end it prints a Markdown table and the copies only the unpack (or the shim's stream)
//! refuses.
//!
//! Run in a release build:
//!
//! ```text
//! cargo run --release -p pw-imagecheck --example dng_damage -- [--work <folder>] <files>
//! ```
//!
//! `--work` is the folder the copies are written in, which must be new or empty and not inside a
//! source file's folder (default: a new folder under the system's temporary folder). A file whose
//! extension is not `.dng` is skipped. ExifTool is the one `ExifTool::start` finds: `exiftool.exe`
//! in the program's folder, otherwise `exiftool` on PATH.
//!
//! The program only reads the files it is given: it copies them first and writes only copies in
//! its work folder, whose files it deletes afterwards.

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use pw_imagecheck::RawImageCheck;
use pw_libraw::{DAMAGED, Session};
use pw_metadata::ExifTool;
use pw_metadata::reconcile::FileKind;
use pw_metadata::safe_write::ImageCheck;
use pw_metadata::verify::verify_copy;
use pw_metadata::write_plan::WritePlan;

/// How long a child process may take for one (file, damage kind) before it is killed.
const CHILD_LIMIT: Duration = Duration::from_secs(120);

/// The hidden option that makes the program check one pair (the child's part).
const ONE: &str = "--one";

/// The start of each result line a child prints.
const RESULT_MARK: &str = "@result";

/// How far the moved offset (a) goes, and how many bytes are zeroed (d).
const SHIFT: u32 = 4096;

/// The ExifTool file name looked for in the program's folder (as `ExifTool::start` does).
const EXIFTOOL_FILE_NAME: &str = "exiftool.exe";

/// The checks, in the order a child runs them: (key, column title).
const CHECKS: [(&str, &str); 7] = [
    ("snapshot", "Snapshot comparison"),
    ("hash", "Image data hash"),
    ("previews", "Preview comparison"),
    ("open", "LibRaw open"),
    ("unpack", "LibRaw unpack"),
    ("shim", "Shim"),
    ("whole", "Whole check"),
];

/// The TIFF tags the program reads.
const NEW_SUBFILE_TYPE: u16 = 254;
const IMAGE_WIDTH: u16 = 256;
const IMAGE_LENGTH: u16 = 257;
const COMPRESSION: u16 = 259;
const STRIP_OFFSETS: u16 = 273;
const STRIP_BYTE_COUNTS: u16 = 279;
const TILE_OFFSETS: u16 = 324;
const TILE_BYTE_COUNTS: u16 = 325;
const SUB_IFDS: u16 = 330;

/// The deepest `SubIFDs` nesting followed, and the most IFDs read (a guard against loops).
const MAX_DEPTH: usize = 8;
const MAX_IFDS: usize = 256;

/// The command line of the parent.
struct Options {
    files: Vec<PathBuf>,
    work: Option<PathBuf>,
}

// ---------------------------------------------------------------------------------------------
// The TIFF reader
// ---------------------------------------------------------------------------------------------

/// One IFD entry: where it is and where its value lies.
#[derive(Clone, Copy)]
struct Entry {
    tag: u16,
    kind: u16,
    count: u32,
    /// The byte position of the value: inside the entry when it fits in four bytes, otherwise
    /// where the entry's offset points.
    value_at: usize,
    /// Whether the value lies inside the entry.
    inline: bool,
}

/// One IFD read from the file.
struct Ifd {
    /// Where it was found: `IFD0`, `IFD1`, `IFD0 SubIFDs[0]`, …
    path: String,
    entries: Vec<Entry>,
    /// The byte position of the pointer that leads to this IFD (`None` for IFD0, whose pointer is
    /// in the header), and what that pointer is.
    link: Option<(usize, String)>,
}

/// The file's byte order and its bytes.
struct Tiff<'a> {
    bytes: &'a [u8],
    little: bool,
}

/// The size in bytes of one value of a TIFF type; `None` for an unknown type.
fn type_size(kind: u16) -> Option<u32> {
    match kind {
        1 | 2 | 6 | 7 => Some(1),
        3 | 8 => Some(2),
        4 | 9 | 11 | 13 => Some(4),
        5 | 10 | 12 => Some(8),
        _ => None,
    }
}

fn type_name(kind: u16) -> &'static str {
    match kind {
        3 => "SHORT",
        4 => "LONG",
        13 => "IFD",
        _ => "other",
    }
}

impl<'a> Tiff<'a> {
    fn new(bytes: &'a [u8]) -> Result<Tiff<'a>, String> {
        let little = match bytes.get(0..2) {
            Some(b"II") => true,
            Some(b"MM") => false,
            _ => return Err("not a TIFF file (no II or MM byte order)".to_string()),
        };
        let tiff = Tiff { bytes, little };
        if tiff.u16_at(2)? != 42 {
            return Err("not a classic TIFF file (no 42 in the header)".to_string());
        }
        Ok(tiff)
    }

    fn u16_at(&self, at: usize) -> Result<u16, String> {
        let raw: [u8; 2] = self
            .bytes
            .get(at..at.saturating_add(2))
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| format!("a 2-byte value at byte {at} is past the end of the file"))?;
        Ok(if self.little {
            u16::from_le_bytes(raw)
        } else {
            u16::from_be_bytes(raw)
        })
    }

    fn u32_at(&self, at: usize) -> Result<u32, String> {
        let raw: [u8; 4] = self
            .bytes
            .get(at..at.saturating_add(4))
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| format!("a 4-byte value at byte {at} is past the end of the file"))?;
        Ok(if self.little {
            u32::from_le_bytes(raw)
        } else {
            u32::from_be_bytes(raw)
        })
    }

    /// Reads the IFD at `at`: its entries and the offset of the next IFD.
    fn read_ifd(&self, at: usize) -> Result<(Vec<Entry>, u32, usize), String> {
        let count = usize::from(self.u16_at(at)?);
        let mut entries = Vec::with_capacity(count);
        for index in 0..count {
            let entry_at = at
                .checked_add(2)
                .and_then(|a| a.checked_add(index.checked_mul(12)?))
                .ok_or("an IFD entry's position is out of range")?;
            let tag = self.u16_at(entry_at)?;
            let kind = self.u16_at(entry_at + 2)?;
            let value_count = self.u32_at(entry_at + 4)?;
            let Some(size) = type_size(kind).and_then(|s| s.checked_mul(value_count)) else {
                continue;
            };
            let inline = size <= 4;
            let value_at = if inline {
                entry_at + 8
            } else {
                to_usize(self.u32_at(entry_at + 8)?)?
            };
            entries.push(Entry {
                tag,
                kind,
                count: value_count,
                value_at,
                inline,
            });
        }
        let next_at = at
            .checked_add(2)
            .and_then(|a| a.checked_add(count.checked_mul(12)?))
            .ok_or("an IFD's next pointer is out of range")?;
        Ok((entries, self.u32_at(next_at)?, next_at))
    }

    /// The `index`th value of an integer entry (BYTE, SHORT, LONG or IFD).
    fn value(&self, entry: &Entry, index: u32) -> Result<u32, String> {
        if index >= entry.count {
            return Err(format!("tag {} has no value {index}", entry.tag));
        }
        let size = type_size(entry.kind).ok_or("an unknown type")?;
        let at = entry
            .value_at
            .checked_add(to_usize(index.saturating_mul(size))?)
            .ok_or("a value's position is out of range")?;
        match entry.kind {
            1 => self
                .bytes
                .get(at)
                .map(|b| u32::from(*b))
                .ok_or_else(|| format!("a byte at {at} is past the end of the file")),
            3 => self.u16_at(at).map(u32::from),
            4 | 13 => self.u32_at(at),
            other => Err(format!(
                "tag {} has type {other}, not an integer",
                entry.tag
            )),
        }
    }

    /// The byte position of the `index`th value of an entry.
    fn value_position(entry: &Entry, index: u32) -> Result<usize, String> {
        let size = type_size(entry.kind).ok_or("an unknown type")?;
        entry
            .value_at
            .checked_add(to_usize(index.saturating_mul(size))?)
            .ok_or_else(|| "a value's position is out of range".to_string())
    }

    /// Every IFD: IFD0's chain, each IFD followed at once by its `SubIFDs` (nested ones too),
    /// depth first in file order.
    fn ifds(&self) -> Result<Vec<Ifd>, String> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        let mut at = self.u32_at(4)?;
        let mut link: Option<(usize, String)> = None;
        let mut number = 0usize;
        while at != 0 && found.len() < MAX_IFDS {
            let position = to_usize(at)?;
            if !seen.insert(position) {
                break;
            }
            let path = format!("IFD{number}");
            let (entries, next, next_at) = self.read_ifd(position)?;
            found.push(Ifd {
                path: path.clone(),
                entries: entries.clone(),
                link: link.take(),
            });
            self.sub_ifds(&entries, &path, 1, &mut seen, &mut found)?;
            link = Some((next_at, format!("{path}'s next-IFD offset")));
            at = next;
            number += 1;
        }
        Ok(found)
    }

    fn sub_ifds(
        &self,
        entries: &[Entry],
        parent: &str,
        depth: usize,
        seen: &mut HashSet<usize>,
        found: &mut Vec<Ifd>,
    ) -> Result<(), String> {
        if depth > MAX_DEPTH {
            return Ok(());
        }
        let Some(sub) = entries.iter().find(|e| e.tag == SUB_IFDS) else {
            return Ok(());
        };
        for index in 0..sub.count {
            if found.len() >= MAX_IFDS {
                break;
            }
            let position = to_usize(self.value(sub, index)?)?;
            if !seen.insert(position) {
                continue;
            }
            let path = format!("{parent} SubIFDs[{index}]");
            let (child_entries, _, _) = self.read_ifd(position)?;
            let place = if sub.inline {
                "in the entry"
            } else {
                "in the value list outside the entry"
            };
            found.push(Ifd {
                path: path.clone(),
                entries: child_entries.clone(),
                link: Some((
                    Tiff::value_position(sub, index)?,
                    format!("{parent}'s SubIFDs[{index}] ({place})"),
                )),
            });
            self.sub_ifds(&child_entries, &path, depth + 1, seen, found)?;
        }
        Ok(())
    }

    /// Writes an integer value of `kind` at `at` (in the file's byte order).
    fn put(&self, bytes: &mut [u8], at: usize, kind: u16, value: u32) -> Result<(), String> {
        let raw: Vec<u8> = match kind {
            3 => {
                let short =
                    u16::try_from(value).map_err(|_| format!("{value} does not fit in a SHORT"))?;
                if self.little {
                    short.to_le_bytes().to_vec()
                } else {
                    short.to_be_bytes().to_vec()
                }
            }
            4 | 13 => {
                if self.little {
                    value.to_le_bytes().to_vec()
                } else {
                    value.to_be_bytes().to_vec()
                }
            }
            other => return Err(format!("type {other} is not written by this program")),
        };
        let target = bytes
            .get_mut(at..at.saturating_add(raw.len()))
            .ok_or_else(|| format!("byte {at} is past the end of the file"))?;
        target.copy_from_slice(&raw);
        Ok(())
    }
}

fn to_usize(value: u32) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("{value} does not fit in usize"))
}

impl Ifd {
    fn entry(&self, tag: u16) -> Option<&Entry> {
        self.entries.iter().find(|e| e.tag == tag)
    }
}

/// The main RAW image's IFD and its piece tags.
struct RawIfd {
    ifd: Ifd,
    offsets: Entry,
    counts: Entry,
    tiles: bool,
}

/// The first IFD whose `NewSubfileType` is 0 (or missing) and that has strip or tile offsets.
fn find_raw(tiff: &Tiff) -> Result<RawIfd, String> {
    for ifd in tiff.ifds()? {
        let subfile = match ifd.entry(NEW_SUBFILE_TYPE) {
            Some(entry) => tiff.value(entry, 0)?,
            None => 0,
        };
        if subfile != 0 {
            continue;
        }
        let pieces = match (ifd.entry(TILE_OFFSETS), ifd.entry(TILE_BYTE_COUNTS)) {
            (Some(o), Some(c)) => Some((*o, *c, true)),
            _ => match (ifd.entry(STRIP_OFFSETS), ifd.entry(STRIP_BYTE_COUNTS)) {
                (Some(o), Some(c)) => Some((*o, *c, false)),
                _ => None,
            },
        };
        if let Some((offsets, counts, tiles)) = pieces {
            return Ok(RawIfd {
                ifd,
                offsets,
                counts,
                tiles,
            });
        }
    }
    Err("no IFD with NewSubfileType 0 (or missing) and strip or tile offsets".to_string())
}

// ---------------------------------------------------------------------------------------------
// The damaged copies
// ---------------------------------------------------------------------------------------------

/// One damage kind.
#[derive(Clone, Copy)]
enum Kind {
    Moved,
    Swapped,
    Short,
    Zeroed,
    Width,
    Compression,
    Link,
}

const KINDS: [Kind; 7] = [
    Kind::Moved,
    Kind::Swapped,
    Kind::Short,
    Kind::Zeroed,
    Kind::Width,
    Kind::Compression,
    Kind::Link,
];

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Moved => "a. data moved",
            Kind::Swapped => "b. two pieces swapped",
            Kind::Short => "c. a short piece",
            Kind::Zeroed => "d. data zeroed",
            Kind::Width => "e. a wrong width",
            Kind::Compression => "f. a wrong compression",
            Kind::Link => "g. a broken link",
        }
    }

    fn file_tag(self) -> char {
        match self {
            Kind::Moved => 'a',
            Kind::Swapped => 'b',
            Kind::Short => 'c',
            Kind::Zeroed => 'd',
            Kind::Width => 'e',
            Kind::Compression => 'f',
            Kind::Link => 'g',
        }
    }
}

/// What a damage kind did to the bytes: the changes made, or why it was skipped.
enum Made {
    Changed(Vec<String>),
    Skipped(String),
}

/// The text naming where a value lies.
fn place(entry: &Entry) -> &'static str {
    if entry.inline {
        "in the entry"
    } else {
        "in the value list outside the entry"
    }
}

fn piece_names(raw: &RawIfd) -> (&'static str, &'static str) {
    if raw.tiles {
        ("TileOffsets", "TileByteCounts")
    } else {
        ("StripOffsets", "StripByteCounts")
    }
}

/// Changes one value of an entry and says what was changed.
fn change_value(
    tiff: &Tiff,
    bytes: &mut [u8],
    entry: &Entry,
    index: u32,
    name: &str,
    new: u32,
) -> Result<String, String> {
    let old = tiff.value(entry, index)?;
    let at = Tiff::value_position(entry, index)?;
    tiff.put(bytes, at, entry.kind, new)?;
    Ok(format!(
        "{name}[{index}] ({}) at byte {at} ({}): {old} -> {new}",
        type_name(entry.kind),
        place(entry)
    ))
}

/// Applies one damage kind to `bytes` (a fresh copy of the original).
fn damage(tiff: &Tiff, raw: &RawIfd, kind: Kind, bytes: &mut [u8]) -> Result<Made, String> {
    let (offsets_name, counts_name) = piece_names(raw);
    let length = bytes.len();
    let made = match kind {
        Kind::Moved => {
            let old = tiff.value(&raw.offsets, 0)?;
            let new = old.checked_add(SHIFT).ok_or("the moved offset overflows")?;
            if to_usize(new)? >= length {
                return Ok(Made::Skipped(format!(
                    "{offsets_name}[0] + {SHIFT} = {new} is not within the file ({length} bytes)"
                )));
            }
            if raw.offsets.kind == 3 && new > u32::from(u16::MAX) {
                return Ok(Made::Skipped(format!(
                    "{offsets_name}[0] + {SHIFT} = {new} does not fit in its SHORT"
                )));
            }
            Made::Changed(vec![change_value(
                tiff,
                bytes,
                &raw.offsets,
                0,
                offsets_name,
                new,
            )?])
        }
        Kind::Swapped => {
            if raw.offsets.count < 2 {
                return Ok(Made::Skipped(format!(
                    "only one piece ({offsets_name} has {} value)",
                    raw.offsets.count
                )));
            }
            let first = tiff.value(&raw.offsets, 0)?;
            let second = tiff.value(&raw.offsets, 1)?;
            Made::Changed(vec![
                change_value(tiff, bytes, &raw.offsets, 0, offsets_name, second)?,
                change_value(tiff, bytes, &raw.offsets, 1, offsets_name, first)?,
            ])
        }
        Kind::Short => {
            let old = tiff.value(&raw.counts, 0)?;
            Made::Changed(vec![change_value(
                tiff,
                bytes,
                &raw.counts,
                0,
                counts_name,
                old / 2,
            )?])
        }
        Kind::Zeroed => {
            let offset = to_usize(tiff.value(&raw.offsets, 0)?)?;
            let count = to_usize(tiff.value(&raw.counts, 0)?)?;
            let shift = to_usize(SHIFT)?;
            let zeroed = if count >= shift { shift } else { count / 2 };
            if zeroed == 0 {
                return Ok(Made::Skipped(format!(
                    "the first piece is {count} bytes, too short to zero"
                )));
            }
            let start = offset
                .checked_add(count / 2)
                .and_then(|m| m.checked_sub(zeroed / 2))
                .ok_or("the zeroed range is out of range")?;
            let end = start
                .checked_add(zeroed)
                .ok_or("the zeroed range is out of range")?;
            let target = bytes.get_mut(start..end).ok_or_else(|| {
                format!("bytes {start}..{end} are past the end of the file ({length} bytes)")
            })?;
            let nonzero = target.iter().filter(|b| **b != 0).count();
            target.fill(0);
            Made::Changed(vec![format!(
                "{zeroed} bytes at {start}..{end}, the middle of the first piece ({offsets_name}[0] \
                 {offset}, {counts_name}[0] {count}), set to 0 ({nonzero} of them were not 0)"
            )])
        }
        Kind::Width => {
            let entry = raw
                .ifd
                .entry(IMAGE_WIDTH)
                .ok_or("the RAW IFD has no ImageWidth")?;
            let old = tiff.value(entry, 0)?;
            let new = old.checked_sub(16).ok_or("ImageWidth is below 16")?;
            Made::Changed(vec![change_value(
                tiff,
                bytes,
                entry,
                0,
                "ImageWidth",
                new,
            )?])
        }
        Kind::Compression => {
            let entry = raw
                .ifd
                .entry(COMPRESSION)
                .ok_or("the RAW IFD has no Compression")?;
            let old = tiff.value(entry, 0)?;
            let new = if old == 1 { 7 } else { 1 };
            Made::Changed(vec![change_value(
                tiff,
                bytes,
                entry,
                0,
                "Compression",
                new,
            )?])
        }
        Kind::Link => {
            let Some((at, what)) = &raw.ifd.link else {
                return Ok(Made::Skipped(
                    "the RAW IFD is IFD0: breaking the header's pointer makes the whole file \
                     unreadable, which any check finds"
                        .to_string(),
                ));
            };
            let old = tiff.u32_at(*at)?;
            let past = u32::try_from(length)
                .ok()
                .and_then(|l| l.checked_add(SHIFT))
                .ok_or("the file is too large for a pointer past its end")?;
            tiff.put(bytes, *at, 4, past)?;
            Made::Changed(vec![format!(
                "{what} at byte {at}: {old} -> {past} (the file is {length} bytes)"
            )])
        }
    };
    Ok(made)
}

// ---------------------------------------------------------------------------------------------
// The parent
// ---------------------------------------------------------------------------------------------

/// The result of one check of one pair.
#[derive(Clone)]
enum Outcome {
    Refused(String),
    Passed(String),
    /// ExifTool could not make a snapshot; counts as refused (the safe write would fail).
    SnapshotFailed(String),
    NotRun(String),
    Crashed(String),
    TimedOut,
}

impl Outcome {
    fn refused(&self) -> bool {
        matches!(self, Outcome::Refused(_) | Outcome::SnapshotFailed(_))
    }

    fn passed(&self) -> bool {
        matches!(self, Outcome::Passed(_))
    }

    fn word(&self) -> String {
        match self {
            Outcome::Refused(_) => "refused".to_string(),
            Outcome::Passed(_) => "passed".to_string(),
            Outcome::SnapshotFailed(_) => "snapshot failed".to_string(),
            Outcome::NotRun(_) => "not run".to_string(),
            Outcome::Crashed(code) => format!("crashed ({code})"),
            Outcome::TimedOut => "timed out".to_string(),
        }
    }

    fn note(&self) -> &str {
        match self {
            Outcome::Refused(n)
            | Outcome::Passed(n)
            | Outcome::SnapshotFailed(n)
            | Outcome::NotRun(n) => n,
            Outcome::Crashed(_) | Outcome::TimedOut => "",
        }
    }
}

/// One check's result with its time (none when the child did not report it).
#[derive(Clone)]
struct CheckResult {
    outcome: Outcome,
    time: Option<Duration>,
}

/// One row of the final table.
struct Row {
    file: String,
    kind: Kind,
    /// `None` when the damage kind was skipped (the reason is in `skipped`).
    checks: Option<Vec<CheckResult>>,
    skipped: Option<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some(ONE) {
        return match (args.get(1), args.get(2), args.len()) {
            (Some(original), Some(damaged), 3) => {
                child(Path::new(original), Path::new(damaged));
                ExitCode::SUCCESS
            }
            _ => {
                eprintln!("{ONE} needs <original> <damaged copy>");
                ExitCode::FAILURE
            }
        };
    }
    let options = match parse_args(args.into_iter()) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            eprintln!(
                "Usage: cargo run --release -p pw-imagecheck --example dng_damage -- \
                 [--work <folder>] <files>"
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
        work: None,
    };
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--work" => {
                let value = args.next().ok_or("--work needs a folder")?;
                options.work = Some(PathBuf::from(value));
            }
            _ => options.files.push(PathBuf::from(arg)),
        }
    }
    if options.files.is_empty() {
        return Err("No DNG files given.".to_string());
    }
    Ok(options)
}

fn run(options: &Options) -> Result<(), String> {
    let (work, made_work) = match &options.work {
        Some(work) => (work.clone(), false),
        None => (
            std::env::temp_dir().join(format!("pw-dng-damage-{}", std::process::id())),
            true,
        ),
    };
    refuse_used_folder(&work)?;
    fs::create_dir_all(&work)
        .map_err(|e| format!("The work folder {} cannot be made: {e}", work.display()))?;
    refuse_source_folders(&work, &options.files)?;
    println!("Work folder: {}", work.display());
    if !libraw_c::AVAILABLE {
        println!("LibRaw's C API is Windows only here: its open and unpack are not run.");
    }
    let exe = std::env::current_exe()
        .map_err(|e| format!("The program's own path cannot be found: {e}"))?;
    print_exiftool(&exe)?;

    let mut rows = Vec::new();
    for source in &options.files {
        if !is_dng(source) {
            println!();
            println!("{}: skipped, not a .dng file", source.display());
            continue;
        }
        if let Err(message) = measure(&exe, source, &work, &mut rows) {
            println!("  {message}");
        }
    }

    println!();
    print_table(&rows);
    println!();
    print_only_unpack(&rows);

    clean_up(&work, made_work);
    Ok(())
}

/// Prints the ExifTool `ExifTool::start` finds: its version and where it came from.
fn print_exiftool(exe: &Path) -> Result<(), String> {
    let bundled = exe
        .parent()
        .map(|folder| folder.join(EXIFTOOL_FILE_NAME))
        .filter(|path| path.is_file());
    let source = match bundled {
        Some(path) => format!("{} (the program's folder)", path.display()),
        None => match on_path() {
            Some(path) => format!("exiftool on PATH ({})", path.display()),
            None => "exiftool on PATH (not found in any PATH folder)".to_string(),
        },
    };
    let start = Instant::now();
    let mut et = ExifTool::start()
        .map_err(|e| format!("ExifTool could not be started from {source}: {e}"))?;
    let version = et
        .execute(&["-ver".to_string()])
        .map_err(|e| format!("ExifTool from {source} does not answer: {e}"))?;
    println!(
        "ExifTool {} from {source} started and answered in {:.0} ms",
        version.trim(),
        ms(start.elapsed())
    );
    Ok(())
}

/// The first `exiftool` (with `.exe` on Windows) in a PATH folder.
fn on_path() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "exiftool.exe"
    } else {
        "exiftool"
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|folder| folder.join(name))
        .find(|candidate| candidate.is_file())
}

/// A DNG is told by its extension (case-insensitive).
fn is_dng(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("dng"))
}

/// Refuses a work folder that exists and is not empty: the copies would overwrite its files of
/// the same name, and the cleanup deletes its files.
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

/// Refuses a work folder that is a source file's folder or inside one: the program never writes
/// where the sources are.
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

/// One DNG: finds its RAW IFD, makes each damaged copy and checks it in a child process.
fn measure(exe: &Path, source: &Path, work: &Path, rows: &mut Vec<Row>) -> Result<(), String> {
    let name = file_name(source);
    println!();
    let bytes =
        fs::read(source).map_err(|e| format!("{} cannot be read: {e}", source.display()))?;
    println!("{name} ({:.1} MB)", bytes.len() as f64 / 1_000_000.0);
    let tiff = Tiff::new(&bytes)?;
    let raw = find_raw(&tiff)?;
    print_raw(&tiff, &raw);

    let original = work.join(format!("original-{name}"));
    copy_file(source, &original)?;
    for kind in KINDS {
        let mut damaged_bytes = bytes.clone();
        println!("  {}:", kind.label());
        let made = match damage(&tiff, &raw, kind, &mut damaged_bytes) {
            Ok(made) => made,
            Err(message) => Made::Skipped(format!("cannot be made: {message}")),
        };
        let changes = match made {
            Made::Skipped(reason) => {
                println!("    skipped: {reason}");
                rows.push(Row {
                    file: name.clone(),
                    kind,
                    checks: None,
                    skipped: Some(reason),
                });
                continue;
            }
            Made::Changed(changes) => changes,
        };
        for change in &changes {
            println!("    changed: {change}");
        }
        let damaged = work.join(format!("{}-{name}", kind.file_tag()));
        if let Err(e) = fs::write(&damaged, &damaged_bytes) {
            let reason = format!("the copy cannot be written: {e}");
            println!("    skipped: {reason}");
            remove(&[&damaged]);
            rows.push(Row {
                file: name.clone(),
                kind,
                checks: None,
                skipped: Some(reason),
            });
            continue;
        }
        let checks = check_in_child(exe, &original, &damaged);
        for ((_, title), result) in CHECKS.iter().zip(&checks) {
            let time = result
                .time
                .map_or(String::new(), |t| format!(", {:.1} ms", ms(t)));
            let note = result.outcome.note();
            let note = if note.is_empty() {
                String::new()
            } else {
                format!("; {note}")
            };
            println!("    {title}: {}{time}{note}", result.outcome.word());
        }
        remove(&[&damaged]);
        rows.push(Row {
            file: name.clone(),
            kind,
            checks: Some(checks),
            skipped: None,
        });
    }
    remove(&[&original]);
    Ok(())
}

/// Prints the RAW IFD: where it was found, its compression, size and pieces.
fn print_raw(tiff: &Tiff, raw: &RawIfd) {
    let value = |tag: u16| {
        raw.ifd
            .entry(tag)
            .and_then(|e| tiff.value(e, 0).ok())
            .map_or("missing".to_string(), |v| v.to_string())
    };
    let pieces = if raw.tiles { "tiles" } else { "strips" };
    let (offsets_name, counts_name) = piece_names(raw);
    println!(
        "  RAW IFD: {}; Compression {}; ImageWidth {}, ImageLength {}; {} {pieces} \
         ({offsets_name} {} {}, {counts_name} {} {})",
        raw.ifd.path,
        value(COMPRESSION),
        value(IMAGE_WIDTH),
        value(IMAGE_LENGTH),
        raw.offsets.count,
        type_name(raw.offsets.kind),
        place(&raw.offsets),
        type_name(raw.counts.kind),
        place(&raw.counts),
    );
}

/// Runs the program again with `--one` on the pair, gives it `CHILD_LIMIT`, and gives one result
/// per check; a check the child did not report is "crashed" or "timed out".
fn check_in_child(exe: &Path, original: &Path, damaged: &Path) -> Vec<CheckResult> {
    let spawned = Command::new(exe)
        .arg(ONE)
        .arg(original)
        .arg(damaged)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            let note = format!("the child process could not be started: {e}");
            return CHECKS
                .iter()
                .map(|_| CheckResult {
                    outcome: Outcome::NotRun(note.clone()),
                    time: None,
                })
                .collect();
        }
    };
    let (sender, receiver) = mpsc::channel::<String>();
    let stdout = child.stdout.take();
    let reader = stdout.map(|stdout| {
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        })
    });
    let stderr_reader = child.stderr.take().map(|mut stderr| {
        thread::spawn(move || {
            let mut text = String::new();
            // Only for printing; a read error leaves what was read.
            let _ = stderr.read_to_string(&mut text);
            text
        })
    });

    let deadline = Instant::now() + CHILD_LIMIT;
    let mut reported: Vec<Option<CheckResult>> = vec![None; CHECKS.len()];
    let mut timed_out = false;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            timed_out = true;
            break;
        }
        match receiver.recv_timeout(left) {
            Ok(line) => record(&line, &mut reported),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                timed_out = true;
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let ended = if timed_out {
        kill(&mut child);
        Outcome::TimedOut
    } else {
        wait_until(&mut child, deadline)
    };
    if let Some(reader) = reader {
        let _ = reader.join();
    }
    if let Some(text) = stderr_reader.and_then(|r| r.join().ok()) {
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            println!("    (child stderr) {line}");
        }
    }
    reported
        .into_iter()
        .map(|result| {
            result.unwrap_or_else(|| CheckResult {
                outcome: ended.clone(),
                time: None,
            })
        })
        .collect()
}

/// Waits for a child whose output has ended; what a check it did not report gets.
fn wait_until(child: &mut Child, deadline: Instant) -> Outcome {
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                return Outcome::NotRun("the child ended without reporting it".to_string());
            }
            Ok(Some(status)) => {
                let code = status
                    .code()
                    .map_or("no exit code".to_string(), |c| format!("exit code {c}"));
                return Outcome::Crashed(code);
            }
            Ok(None) if Instant::now() >= deadline => {
                kill(child);
                return Outcome::TimedOut;
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                kill(child);
                return Outcome::Crashed(format!("its state cannot be read: {e}"));
            }
        }
    }
}

fn kill(child: &mut Child) {
    // A child that already ended cannot be killed; waiting reaps it either way.
    let _ = child.kill();
    let _ = child.wait();
}

/// Records one result line of a child: `@result<TAB>key<TAB>word<TAB>ms<TAB>note`.
fn record(line: &str, reported: &mut [Option<CheckResult>]) {
    let mut parts = line.splitn(5, '\t');
    if parts.next() != Some(RESULT_MARK) {
        return;
    }
    let (Some(key), Some(word), Some(time), note) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return;
    };
    let note = note.unwrap_or_default().to_string();
    let outcome = match word {
        "refused" => Outcome::Refused(note),
        "passed" => Outcome::Passed(note),
        "snapshot failed" => Outcome::SnapshotFailed(note),
        _ => Outcome::NotRun(note),
    };
    let time = time
        .parse::<f64>()
        .ok()
        .filter(|t| t.is_finite() && *t >= 0.0)
        .map(|t| Duration::from_secs_f64(t / 1000.0));
    if let Some(slot) = CHECKS
        .iter()
        .position(|(k, _)| *k == key)
        .and_then(|index| reported.get_mut(index))
    {
        *slot = Some(CheckResult { outcome, time });
    }
}

/// The Markdown table: rows file × damage kind, columns the checks.
fn print_table(rows: &[Row]) {
    let titles = CHECKS
        .iter()
        .map(|(_, title)| *title)
        .collect::<Vec<_>>()
        .join(" | ");
    println!("| File | Damage | {titles} |");
    println!("|---|---|{}", "---|".repeat(CHECKS.len()));
    for row in rows {
        let cells = match (&row.checks, &row.skipped) {
            (Some(checks), _) => checks
                .iter()
                .map(|c| match c.time {
                    Some(t) => format!("{}, {:.0} ms", c.outcome.word(), ms(t)),
                    None => c.outcome.word(),
                })
                .collect::<Vec<_>>()
                .join(" | "),
            (None, skipped) => {
                let reason = skipped.as_deref().unwrap_or("skipped");
                let mut cells = vec![format!("skipped: {reason}")];
                cells.extend(std::iter::repeat_n("–".to_string(), CHECKS.len() - 1));
                cells.join(" | ")
            }
        };
        println!("| {} | {} | {cells} |", row.file, row.kind.label());
    }
    println!();
    println!(
        "\"snapshot failed\" counts as refused (the safe write would fail). verify_copy always \
         allows keys ending in Offset, Offsets or Start, so for a and b only the image data hash \
         or the validation can make the snapshot comparison refuse. LibRaw unpack is refused by a \
         return code other than 0 or a data error; \"not run\" when the open failed."
    );
}

/// The copies the whole check refuses but the snapshot comparison, the preview comparison and
/// LibRaw's open all pass.
fn print_only_unpack(rows: &[Row]) {
    println!(
        "Refused by the whole check, but passed by the snapshot comparison, the preview \
         comparison and LibRaw's open:"
    );
    let index = |key: &str| CHECKS.iter().position(|(k, _)| *k == key);
    let mut any = false;
    for row in rows {
        let Some(checks) = &row.checks else {
            continue;
        };
        let get = |key: &str| index(key).and_then(|i| checks.get(i));
        let refused = |key: &str| get(key).is_some_and(|c| c.outcome.refused());
        let passed = |key: &str| get(key).is_some_and(|c| c.outcome.passed());
        if refused("whole") && passed("snapshot") && passed("previews") && passed("open") {
            any = true;
            let unpack = get("unpack").map_or("–".to_string(), |c| c.outcome.word());
            let shim = get("shim").map_or("–".to_string(), |c| {
                format!("{} ({})", c.outcome.word(), c.outcome.note())
            });
            println!(
                "- {} {}: LibRaw unpack {unpack}; shim {shim}",
                row.file,
                row.kind.label()
            );
        }
    }
    if !any {
        println!("- none");
    }
}

/// Deletes every file left in the work folder (naming any that cannot be deleted), and the
/// folder itself when the program made it.
fn clean_up(work: &Path, made_work: bool) {
    match fs::read_dir(work) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Err(e) = fs::remove_file(&path)
                {
                    println!("{} could not be deleted: {e}", path.display());
                }
            }
        }
        Err(e) => println!("The work folder {} cannot be read: {e}", work.display()),
    }
    if made_work && let Err(e) = fs::remove_dir(work) {
        println!(
            "The work folder {} could not be deleted: {e}",
            work.display()
        );
    }
}

fn copy_file(source: &Path, target: &Path) -> Result<(), String> {
    fs::copy(source, target).map(|_| ()).map_err(|e| {
        format!(
            "{} could not be copied to {}: {e}",
            source.display(),
            target.display()
        )
    })
}

/// Deletes the given copies; names any that cannot be deleted.
fn remove(files: &[&Path]) {
    for file in files {
        if file.exists()
            && let Err(e) = fs::remove_file(file)
        {
            println!("    {} could not be deleted: {e}", file.display());
        }
    }
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// The child: the checks of one pair
// ---------------------------------------------------------------------------------------------

/// Prints one result line at once (the parent reads them as they come, so a crash later keeps
/// the results before it).
fn report(key: &str, word: &str, time: Duration, note: &str) {
    let note = note.replace(['\t', '\r', '\n'], " ");
    let mut out = std::io::stdout().lock();
    // Nothing else can be done when stdout is closed; the parent then marks the check.
    let _ = writeln!(out, "{RESULT_MARK}\t{key}\t{word}\t{:.3}\t{note}", ms(time));
    let _ = out.flush();
}

fn report_result(key: &str, time: Duration, result: &Result<(), String>) {
    match result {
        Ok(()) => report(key, "passed", time, "passed"),
        Err(error) => report(key, "refused", time, error),
    }
}

/// The snapshot of `file` by a new ExifTool process; the time excludes the process's start.
fn snapshot_alone(
    file: &Path,
) -> Result<(pw_metadata::verify::FileSnapshot, Duration, ExifTool), String> {
    let mut et = ExifTool::start().map_err(|e| format!("ExifTool could not be started: {e}"))?;
    let start = Instant::now();
    let snapshot = et.snapshot(file).map_err(|e| e.to_string())?;
    Ok((snapshot, start.elapsed(), et))
}

/// Point 3's checks on one pair, each reported as soon as it is ready.
fn child(original: &Path, damaged: &Path) {
    // The snapshot comparison: each snapshot by a new ExifTool process.
    let mut hash_tool = None;
    match snapshot_alone(original) {
        Err(e) => report(
            "snapshot",
            "snapshot failed",
            Duration::ZERO,
            &format!("the original: {e}"),
        ),
        Ok((before, first, _)) => match snapshot_alone(damaged) {
            Err(e) => report(
                "snapshot",
                "snapshot failed",
                first,
                &format!("the damaged copy: {e}"),
            ),
            Ok((after, second, et)) => {
                hash_tool = Some(et);
                let start = Instant::now();
                let result =
                    verify_copy(&before, &after, &WritePlan::default()).map_err(|e| e.to_string());
                report_result("snapshot", first + second + start.elapsed(), &result);
            }
        },
    }

    // The image data hash alone (only the damaged copy's call is timed).
    let et = match hash_tool {
        Some(et) => Ok(et),
        None => ExifTool::start().map_err(|e| format!("ExifTool could not be started: {e}")),
    };
    match et {
        Err(e) => report("hash", "not run", Duration::ZERO, &e),
        Ok(mut et) => {
            let before = et.image_data_hash(original);
            let start = Instant::now();
            let after = et.image_data_hash(damaged);
            let time = start.elapsed();
            match (before, after) {
                (Ok(before), Ok(after)) => {
                    let note = format!(
                        "original {}, damaged {}",
                        hash_text(before.as_deref()),
                        hash_text(after.as_deref())
                    );
                    let word = if before == after { "passed" } else { "refused" };
                    report("hash", word, time, &note);
                }
                (Err(e), _) => report(
                    "hash",
                    "refused",
                    time,
                    &format!("the original's call failed: {e}"),
                ),
                (_, Err(e)) => report("hash", "refused", time, &format!("the call failed: {e}")),
            }
        }
    }

    let check = RawImageCheck::without_release();

    // The preview comparison alone: for any kind other than DNG, check_copy runs only it.
    let start = Instant::now();
    let result = check.check_copy(original, damaged, FileKind::Tiff);
    report_result("previews", start.elapsed(), &result);

    // LibRaw's open and unpack through its C API.
    match libraw_c::steps(damaged, |time, rc| {
        let word = if rc == 0 { "passed" } else { "refused" };
        report("open", word, time, &format!("rc {rc}"));
    }) {
        Ok(Some((time, rc, errors))) => {
            let word = if rc != 0 || errors > 0 {
                "refused"
            } else {
                "passed"
            };
            report(
                "unpack",
                word,
                time,
                &format!("rc {rc}, {errors} data error(s)"),
            );
        }
        Ok(None) => report("unpack", "not run", Duration::ZERO, "the open failed"),
        Err(message) => {
            report("open", "not run", Duration::ZERO, &message);
            report("unpack", "not run", Duration::ZERO, &message);
        }
    }

    // The shim's verdict.
    let start = Instant::now();
    let (opened, diagnostics) = Session::open_with_diagnostics(damaged);
    match opened {
        Ok(session) => {
            drop(session);
            report("shim", "passed", start.elapsed(), "opened");
        }
        Err(error) => {
            let time = start.elapsed();
            let text = error.to_string();
            let kind = if text.contains(DAMAGED) {
                "damaged"
            } else {
                "not damaged"
            };
            report(
                "shim",
                "refused",
                time,
                &format!(
                    "{kind}: {text} (data errors {}, missing {} bytes)",
                    diagnostics.data_errors, diagnostics.missing
                ),
            );
        }
    }

    // The whole check, as the app runs it.
    let start = Instant::now();
    let result = check.check_copy(original, damaged, FileKind::Dng);
    report_result("whole", start.elapsed(), &result);
}

fn hash_text(hash: Option<&str>) -> String {
    match hash {
        Some(hash) => hash.chars().take(12).collect::<String>() + "…",
        None => "none".to_string(),
    }
}

/// LibRaw's own C API (`libraw.h`), already linked through `pw-libraw` (static `raw_r`).
/// `libraw_open_wfile` takes a wide path and exists only on Windows.
#[cfg(windows)]
mod libraw_c {
    use std::ffi::{c_char, c_int, c_uint, c_void};
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::time::{Duration, Instant};

    /// LibRaw's C API is in this build.
    pub const AVAILABLE: bool = true;

    /// `data_callback` of `libraw_types.h`.
    type DataCallback = unsafe extern "C" fn(data: *mut c_void, file: *const c_char, offset: i64);

    unsafe extern "C" {
        fn libraw_init(flags: c_uint) -> *mut c_void;
        fn libraw_open_wfile(data: *mut c_void, path: *const u16) -> c_int;
        fn libraw_unpack(data: *mut c_void) -> c_int;
        fn libraw_close(data: *mut c_void);
        fn libraw_set_dataerror_handler(
            data: *mut c_void,
            func: Option<DataCallback>,
            datap: *mut c_void,
        );
    }

    /// Counts LibRaw's data errors (corrupt data, or an unexpected end). Setting it also keeps
    /// LibRaw from printing them to stderr.
    unsafe extern "C" fn count_data_error(data: *mut c_void, _file: *const c_char, _offset: i64) {
        // SAFETY: `data` is the pointer to the `AtomicI32` given to
        // `libraw_set_dataerror_handler` in `steps`, which outlives the handle (the handle is
        // closed before the counter is dropped). An atomic, since LibRaw's OpenMP decoders may
        // report from several threads.
        if let Some(counter) = unsafe { data.cast::<AtomicI32>().cast_const().as_ref() } {
            counter.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// A LibRaw handle from `libraw_init`, closed with `libraw_close` on every path.
    struct Handle(*mut c_void);

    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: the pointer came from `libraw_init` and has not been closed.
                unsafe { libraw_close(self.0) };
            }
        }
    }

    /// Opens `path` with LibRaw's C API, gives the open's time and return code to `on_open`, and
    /// unpacks it when the open succeeded: the unpack's time, return code and the data errors
    /// LibRaw reported during it, or `None` when the open failed.
    pub fn steps(
        path: &Path,
        on_open: impl FnOnce(Duration, i32),
    ) -> Result<Option<(Duration, i32, i32)>, String> {
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // Declared before the handle, so the handle (and LibRaw's pointer to it) goes first.
        let errors = AtomicI32::new(0);
        // SAFETY: a plain constructor; null on failure, checked below.
        let handle = Handle(unsafe { libraw_init(0) });
        if handle.0.is_null() {
            return Err("libraw_init returned null".to_string());
        }
        // SAFETY: a valid handle; the callback matches `data_callback`, and the counter outlives
        // the handle (see `count_data_error`).
        unsafe {
            libraw_set_dataerror_handler(
                handle.0,
                Some(count_data_error),
                ptr::from_ref(&errors).cast_mut().cast::<c_void>(),
            );
        }

        let start = Instant::now();
        // SAFETY: a valid handle and a NUL-terminated UTF-16 path that lives through the call.
        let open_rc = unsafe { libraw_open_wfile(handle.0, wide.as_ptr()) };
        on_open(start.elapsed(), open_rc);
        if open_rc != 0 {
            return Ok(None);
        }

        // Only the unpack's data errors are counted.
        errors.store(0, Ordering::Relaxed);
        let start = Instant::now();
        // SAFETY: a valid handle whose file was opened.
        let unpack_rc = unsafe { libraw_unpack(handle.0) };
        let unpack = start.elapsed();
        let unpack_errors = errors.load(Ordering::Relaxed);
        drop(handle);
        Ok(Some((unpack, unpack_rc, unpack_errors)))
    }
}

/// LibRaw's C API is left out on other systems (`libraw_open_wfile` is Windows only).
#[cfg(not(windows))]
mod libraw_c {
    use std::path::Path;
    use std::time::Duration;

    /// LibRaw's C API is not in this build.
    pub const AVAILABLE: bool = false;

    pub fn steps(
        _path: &Path,
        _on_open: impl FnOnce(Duration, i32),
    ) -> Result<Option<(Duration, i32, i32)>, String> {
        Err("LibRaw's C API is Windows only here".to_string())
    }
}
