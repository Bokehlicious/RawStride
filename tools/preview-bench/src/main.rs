//! `preview-bench` – measures how fast embedded RAW previews can be read and decoded.
//!
//! Input: the `metadata.csv` written by `tools/raw-preview-survey.ps1` (ExifTool CSV with
//! numeric `…Start#`/`…Offset#` and `…Length#` columns). For every preview found it measures:
//!
//! * `read_uncached` – open + read of the preview byte range, bypassing the Windows file cache
//!   (`FILE_FLAG_NO_BUFFERING`, sector-aligned). On other OSes this falls back to a normal read.
//! * `read_cached`   – the same range through the normal (warm) file cache.
//! * `decode_zune`   – full-size decode with zune-jpeg (single thread).
//! * `decode_jd_1/N` – decode with jpeg-decoder at scale 1/1, 1/2, 1/4, 1/8 (scaled IDCT).
//! * `decode_tj_1/N` – decode with libjpeg-turbo (TurboJPEG API) at 1/1 … 1/8, single thread.
//!   Only with `--features turbojpeg` (static link from the vcpkg manifest install).
//!
//! For decode metrics the process CPU time over all repetitions is recorded as well
//! (`cpu_ratio` = CPU time / wall time). A ratio clearly above 1.0 means the decoder used
//! more than one thread. Windows' process times have ~15.6 ms resolution, so the ratio is only
//! meaningful when the repetitions add up to well over 100 ms.
//!
//! Per file it also measures `file_uncached`: reading the whole file uncached in 8 MiB chunks.
//!
//! This is a development tool, not application code.
//!
//! Usage:
//! `cargo run -p preview-bench --profile profiling -- <metadata.csv> [--reps N] [--out results.csv]`

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const SECTOR: usize = 4096;
const CHUNK: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
struct Preview {
    file: PathBuf,
    model: String,
    kind: String,
    offset: u64,
    length: usize,
}

#[derive(Debug)]
struct Row {
    file: String,
    model: String,
    kind: String,
    metric: String,
    out_w: u32,
    out_h: u32,
    bytes: usize,
    times: Vec<Duration>,
    cpu: Option<Duration>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut csv_path: Option<PathBuf> = None;
    let mut reps: usize = 5;
    let mut out: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--reps" => {
                reps = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .filter(|&n: &usize| n > 0)
                    .ok_or("--reps needs a positive number")?;
            }
            "--out" => out = Some(args.next().ok_or("--out needs a path")?.into()),
            _ if csv_path.is_none() => csv_path = Some(a.into()),
            _ => return Err(format!("unexpected argument: {a}")),
        }
    }
    let csv_path =
        csv_path.ok_or("usage: preview-bench <metadata.csv> [--reps N] [--out results.csv]")?;
    let text =
        std::fs::read_to_string(&csv_path).map_err(|e| format!("{}: {e}", csv_path.display()))?;
    let previews = parse_previews(text.trim_start_matches('\u{feff}'))?;
    if previews.is_empty() {
        return Err("no previews with offset/length found in the CSV".into());
    }
    let out = out.unwrap_or_else(|| csv_path.with_file_name("preview-bench.csv"));
    eprintln!("{} previews, {} repetitions each", previews.len(), reps);

    let mut rows = Vec::new();
    let mut done_files: Vec<PathBuf> = Vec::new();
    for p in &previews {
        eprintln!("  {} {} ({} KB)", p.file.display(), p.kind, p.length / 1024);
        if !done_files.contains(&p.file) {
            done_files.push(p.file.clone());
            let len = std::fs::metadata(&p.file).map_err(|e| e.to_string())?.len() as usize;
            let mut times = Vec::new();
            for _ in 0..reps.min(3) {
                times.push(read_whole_uncached(&p.file).map_err(|e| e.to_string())?);
            }
            rows.push(row(p, "file", "file_uncached", 0, 0, len, times));
        }
        bench_preview(p, reps, &mut rows)?;
    }

    write_csv(&out, &rows).map_err(|e| format!("{}: {e}", out.display()))?;
    print_summary(&rows);
    eprintln!("\nresults: {}", out.display());
    Ok(())
}

fn row(
    p: &Preview,
    kind: &str,
    metric: &str,
    w: u32,
    h: u32,
    bytes: usize,
    times: Vec<Duration>,
) -> Row {
    Row {
        file: p
            .file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        model: p.model.clone(),
        kind: kind.to_string(),
        metric: metric.to_string(),
        out_w: w,
        out_h: h,
        bytes,
        times,
        cpu: None,
    }
}

fn bench_preview(p: &Preview, reps: usize, rows: &mut Vec<Row>) -> Result<(), String> {
    let err = |e: io::Error| format!("{}: {e}", p.file.display());

    let mut t_unc = Vec::new();
    let mut data = Vec::new();
    for _ in 0..reps {
        let (t, d) = read_range_uncached(&p.file, p.offset, p.length).map_err(err)?;
        t_unc.push(t);
        data = d;
    }
    if data.len() < 2 || data[0] != 0xFF || data[1] != 0xD8 {
        eprintln!("    skipped: not a JPEG at offset {}", p.offset);
        return Ok(());
    }
    rows.push(row(p, &p.kind, "read_uncached", 0, 0, p.length, t_unc));

    let _ = read_range_cached(&p.file, p.offset, p.length).map_err(err)?; // warm the cache
    let mut t_c = Vec::new();
    for _ in 0..reps {
        t_c.push(
            read_range_cached(&p.file, p.offset, p.length)
                .map_err(err)?
                .0,
        );
    }
    rows.push(row(p, &p.kind, "read_cached", 0, 0, p.length, t_c));

    // zune-jpeg, full size
    let mut t_z = Vec::new();
    let (mut zw, mut zh) = (0, 0);
    let c0 = process_cpu_time();
    for _ in 0..reps {
        let t = Instant::now();
        let mut dec = zune_jpeg::JpegDecoder::new(&data);
        let pixels = dec.decode().map_err(|e| format!("zune-jpeg: {e:?}"))?;
        t_z.push(t.elapsed());
        if let Some((w, h)) = dec.dimensions() {
            (zw, zh) = (w as u32, h as u32);
        }
        std::hint::black_box(pixels);
    }
    let mut r = row(p, &p.kind, "decode_zune", zw, zh, p.length, t_z);
    r.cpu = cpu_delta(c0);
    rows.push(r);

    // jpeg-decoder at 1/1, 1/2, 1/4, 1/8 (scaled IDCT)
    for div in [1u32, 2, 4, 8] {
        let mut t_j = Vec::new();
        let (mut ow, mut oh) = (0, 0);
        let c0 = process_cpu_time();
        for _ in 0..reps {
            let t = Instant::now();
            let mut dec = jpeg_decoder::Decoder::new(io::Cursor::new(&data));
            dec.read_info().map_err(|e| format!("jpeg-decoder: {e}"))?;
            let info = dec.info().ok_or("jpeg-decoder: no info")?;
            let req_w = (u32::from(info.width).div_ceil(div)) as u16;
            let req_h = (u32::from(info.height).div_ceil(div)) as u16;
            let (w, h) = dec
                .scale(req_w, req_h)
                .map_err(|e| format!("jpeg-decoder scale: {e}"))?;
            let pixels = dec.decode().map_err(|e| format!("jpeg-decoder: {e}"))?;
            t_j.push(t.elapsed());
            (ow, oh) = (u32::from(w), u32::from(h));
            std::hint::black_box(pixels);
        }
        let mut r = row(
            p,
            &p.kind,
            &format!("decode_jd_1/{div}"),
            ow,
            oh,
            p.length,
            t_j,
        );
        r.cpu = cpu_delta(c0);
        rows.push(r);
    }

    // libjpeg-turbo at 1/1, 1/2, 1/4, 1/8
    #[cfg(feature = "turbojpeg")]
    for div in [1u32, 2, 4, 8] {
        let mut t_t = Vec::new();
        let (mut ow, mut oh) = (0, 0);
        let c0 = process_cpu_time();
        for _ in 0..reps {
            let t = Instant::now();
            let mut dec = turbo::Decompressor::new()?;
            let (w, h, pixels) = dec.decode(&data, div)?;
            t_t.push(t.elapsed());
            (ow, oh) = (w, h);
            std::hint::black_box(pixels);
        }
        let mut r = row(
            p,
            &p.kind,
            &format!("decode_tj_1/{div}"),
            ow,
            oh,
            p.length,
            t_t,
        );
        r.cpu = cpu_delta(c0);
        rows.push(r);
    }
    Ok(())
}

// ---------- CPU time ----------

/// Total CPU time (user + kernel) of this process, all threads.
#[cfg(windows)]
fn process_cpu_time() -> Option<Duration> {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessTimes(
            process: *mut c_void,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
    }
    let mut c = FileTime::default();
    let mut e = FileTime::default();
    let mut k = FileTime::default();
    let mut u = FileTime::default();
    // SAFETY: GetCurrentProcess returns a pseudo-handle; all out-pointers are valid locals.
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
    if ok == 0 {
        return None;
    }
    let t100 = |f: FileTime| (u64::from(f.high) << 32) | u64::from(f.low);
    Some(Duration::from_nanos((t100(k) + t100(u)) * 100))
}

#[cfg(not(windows))]
fn process_cpu_time() -> Option<Duration> {
    None
}

fn cpu_delta(start: Option<Duration>) -> Option<Duration> {
    Some(process_cpu_time()?.saturating_sub(start?))
}

// ---------- libjpeg-turbo ----------

#[cfg(feature = "turbojpeg")]
mod turbo {
    //! Minimal FFI to the TurboJPEG 2.x API (still provided by libjpeg-turbo 3.x).
    use std::ffi::{CStr, c_char, c_int, c_ulong, c_void};

    type Handle = *mut c_void;
    const TJPF_RGB: c_int = 0;
    const TJERR_WARNING: c_int = 0;

    unsafe extern "C" {
        fn tjInitDecompress() -> Handle;
        fn tjDecompressHeader3(
            handle: Handle,
            jpeg_buf: *const u8,
            jpeg_size: c_ulong,
            width: *mut c_int,
            height: *mut c_int,
            subsamp: *mut c_int,
            colorspace: *mut c_int,
        ) -> c_int;
        fn tjDecompress2(
            handle: Handle,
            jpeg_buf: *const u8,
            jpeg_size: c_ulong,
            dst_buf: *mut u8,
            width: c_int,
            pitch: c_int,
            height: c_int,
            pixel_format: c_int,
            flags: c_int,
        ) -> c_int;
        fn tjGetErrorStr2(handle: Handle) -> *mut c_char;
        fn tjGetErrorCode(handle: Handle) -> c_int;
        fn tjDestroy(handle: Handle) -> c_int;
    }

    #[derive(Debug)]
    pub struct Decompressor(Handle);

    impl Decompressor {
        pub fn new() -> Result<Self, String> {
            // SAFETY: plain constructor; null means failure.
            let h = unsafe { tjInitDecompress() };
            if h.is_null() {
                Err("tjInitDecompress failed".into())
            } else {
                Ok(Self(h))
            }
        }

        fn error(&self) -> String {
            // SAFETY: the handle is valid and the library returns a NUL-terminated string.
            unsafe { CStr::from_ptr(tjGetErrorStr2(self.0)) }
                .to_string_lossy()
                .into_owned()
        }

        /// Decodes `data` to RGB at scale 1/`div`. Returns (width, height, pixels).
        pub fn decode(&mut self, data: &[u8], div: u32) -> Result<(u32, u32, Vec<u8>), String> {
            let size = c_ulong::try_from(data.len()).map_err(|_| "JPEG too large".to_string())?;
            let (mut w, mut h, mut ss, mut cs) = (0, 0, 0, 0);
            // SAFETY: data is valid for `size` bytes; out-pointers are valid locals.
            let rc = unsafe {
                tjDecompressHeader3(
                    self.0,
                    data.as_ptr(),
                    size,
                    &mut w,
                    &mut h,
                    &mut ss,
                    &mut cs,
                )
            };
            if rc != 0 {
                return Err(format!("tjDecompressHeader3: {}", self.error()));
            }
            // TJSCALED(dim, 1/div) == ceil(dim / div); TurboJPEG then picks exactly 1/div.
            let sw = (w as u32).div_ceil(div);
            let sh = (h as u32).div_ceil(div);
            let mut out = vec![0u8; sw as usize * sh as usize * 3];
            // SAFETY: out holds sw*sh*3 bytes for an RGB image of the requested scaled size.
            let rc = unsafe {
                tjDecompress2(
                    self.0,
                    data.as_ptr(),
                    size,
                    out.as_mut_ptr(),
                    sw as c_int,
                    0,
                    sh as c_int,
                    TJPF_RGB,
                    0,
                )
            };
            // SAFETY: the handle is valid.
            if rc != 0 && unsafe { tjGetErrorCode(self.0) } != TJERR_WARNING {
                return Err(format!("tjDecompress2: {}", self.error()));
            }
            Ok((sw, sh, out))
        }
    }

    impl Drop for Decompressor {
        fn drop(&mut self) {
            // SAFETY: the handle came from tjInitDecompress and is destroyed exactly once.
            unsafe { tjDestroy(self.0) };
        }
    }
}

// ---------- CSV ----------

/// Minimal RFC-4180 style parser (quoted fields, doubled quotes). Enough for ExifTool's `-csv`.
fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, in_quotes) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', _) => in_quotes = !in_quotes,
            (',', false) => row.push(std::mem::take(&mut field)),
            ('\r', false) => {}
            ('\n', false) => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

fn parse_previews(text: &str) -> Result<Vec<Preview>, String> {
    let rows = parse_csv(text);
    let (header, body) = rows.split_first().ok_or("empty CSV")?;
    let col = |name: &str| header.iter().position(|h| h == name);
    let src = col("SourceFile").ok_or("CSV has no SourceFile column")?;
    let model_col = header.iter().position(|h| h.ends_with(":Model"));

    // Pair "<group>:<Tag>Start#" / "<group>:<Tag>Offset#" with "<group>:<Tag>Length#".
    let mut pairs = Vec::new();
    for (i, h) in header.iter().enumerate() {
        let base = h
            .strip_suffix("Start#")
            .or_else(|| h.strip_suffix("Offset#"));
        if let Some(base) = base
            && let Some(j) = col(&format!("{base}Length#"))
        {
            pairs.push((base.to_string(), i, j));
        }
    }
    let mut out = Vec::new();
    for r in body {
        let get = |i: usize| r.get(i).map(String::as_str).unwrap_or("");
        for (base, i, j) in &pairs {
            if let (Ok(offset), Ok(length)) = (get(*i).parse::<u64>(), get(*j).parse::<usize>())
                && length > 0
            {
                out.push(Preview {
                    file: PathBuf::from(get(src)),
                    model: model_col.map(|m| get(m).to_string()).unwrap_or_default(),
                    kind: base.clone(),
                    offset,
                    length,
                });
            }
        }
    }
    Ok(out)
}

// ---------- I/O ----------

fn read_range_cached(path: &Path, offset: u64, len: usize) -> io::Result<(Duration, Vec<u8>)> {
    let t = Instant::now();
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf)?;
    Ok((t.elapsed(), buf))
}

/// A zeroed heap buffer aligned to `SECTOR`, as `FILE_FLAG_NO_BUFFERING` requires.
struct AlignedBuf {
    ptr: *mut u8,
    layout: std::alloc::Layout,
}

impl AlignedBuf {
    fn new(len: usize) -> Self {
        let len = len.max(SECTOR).div_ceil(SECTOR) * SECTOR;
        let layout = std::alloc::Layout::from_size_align(len, SECTOR).expect("valid layout");
        // SAFETY: layout has a non-zero size.
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Self { ptr, layout }
    }
    fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: ptr is valid for layout.size() initialised (zeroed) bytes and uniquely borrowed.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.layout.size()) }
    }
    fn as_slice(&self) -> &[u8] {
        // SAFETY: ptr is valid for layout.size() initialised bytes.
        unsafe { std::slice::from_raw_parts(self.ptr, self.layout.size()) }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        // SAFETY: ptr was allocated with exactly this layout.
        unsafe { std::alloc::dealloc(self.ptr, self.layout) }
    }
}

impl std::fmt::Debug for AlignedBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AlignedBuf({} bytes)", self.layout.size())
    }
}

#[cfg(windows)]
fn open_uncached(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_NO_BUFFERING)
        .open(path)
}

#[cfg(not(windows))]
fn open_uncached(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).open(path) // no portable cache bypass; measures cached reads
}

/// Reads up to `buf.len()` bytes at the sector-aligned `offset`; returns the bytes read.
///
/// With `FILE_FLAG_NO_BUFFERING` every request must start at a sector-aligned file offset and
/// buffer address. A short read therefore ends the loop: it means end of file, and continuing
/// from the unaligned position would fail with ERROR_INVALID_PARAMETER (os error 87).
/// Only whole sectors are retried after a short, non-EOF read.
fn read_at(f: &mut File, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
    f.seek(SeekFrom::Start(offset))?;
    let mut done = 0;
    while done < buf.len() {
        let n = f.read(&mut buf[done..])?;
        done += n;
        if n == 0 || done % SECTOR != 0 {
            break; // EOF (possibly mid-sector)
        }
    }
    Ok(done)
}

fn read_range_uncached(path: &Path, offset: u64, len: usize) -> io::Result<(Duration, Vec<u8>)> {
    let start = offset / SECTOR as u64 * SECTOR as u64;
    let end = (offset + len as u64).div_ceil(SECTOR as u64) * SECTOR as u64;
    let mut buf = AlignedBuf::new((end - start) as usize);
    let t = Instant::now();
    let mut f = open_uncached(path)?;
    let got = read_at(&mut f, start, buf.as_mut_slice())?;
    let elapsed = t.elapsed();
    let s = (offset - start) as usize;
    if got < s + len {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "preview range beyond end of file",
        ));
    }
    Ok((elapsed, buf.as_slice()[s..s + len].to_vec()))
}

fn read_whole_uncached(path: &Path) -> io::Result<Duration> {
    let mut buf = AlignedBuf::new(CHUNK);
    let t = Instant::now();
    let mut f = open_uncached(path)?;
    let mut pos = 0u64;
    loop {
        let n = read_at(&mut f, pos, buf.as_mut_slice())?;
        std::hint::black_box(&buf);
        if n < CHUNK {
            break;
        }
        pos += CHUNK as u64;
    }
    Ok(t.elapsed())
}

// ---------- Output ----------

fn stats(times: &[Duration]) -> (f64, f64, f64) {
    let mut ms: Vec<f64> = times.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    ms.sort_by(f64::total_cmp);
    let median = if ms.len() % 2 == 1 {
        ms[ms.len() / 2]
    } else {
        (ms[ms.len() / 2 - 1] + ms[ms.len() / 2]) / 2.0
    };
    (median, ms[0], ms[ms.len() - 1])
}

fn write_csv(path: &Path, rows: &[Row]) -> io::Result<()> {
    let mut f = io::BufWriter::new(File::create(path)?);
    writeln!(
        f,
        "file,model,kind,metric,out_w,out_h,bytes,reps,median_ms,min_ms,max_ms,cpu_ratio"
    )?;
    for r in rows {
        let (med, min, max) = stats(&r.times);
        writeln!(
            f,
            "\"{}\",\"{}\",{},{},{},{},{},{},{:.3},{:.3},{:.3},{}",
            r.file,
            r.model,
            r.kind,
            r.metric,
            r.out_w,
            r.out_h,
            r.bytes,
            r.times.len(),
            med,
            min,
            max,
            cpu_ratio(r).map(|c| format!("{c:.2}")).unwrap_or_default()
        )?;
    }
    f.flush()
}

/// CPU time / wall time over all repetitions of a metric (None when not measured).
fn cpu_ratio(r: &Row) -> Option<f64> {
    let wall: Duration = r.times.iter().sum();
    let cpu = r.cpu?;
    (wall > Duration::ZERO).then(|| cpu.as_secs_f64() / wall.as_secs_f64())
}

fn print_summary(rows: &[Row]) {
    println!(
        "\n{:<24} {:<20} {:<16} {:>11} {:>9} {:>10} {:>5}",
        "model", "kind", "metric", "output", "KB", "median ms", "cpu/w"
    );
    for r in rows {
        let (med, _, _) = stats(&r.times);
        let dims = if r.out_w > 0 {
            format!("{}x{}", r.out_w, r.out_h)
        } else {
            String::new()
        };
        println!(
            "{:<24} {:<20} {:<16} {:>11} {:>9} {:>10.2} {:>5}",
            r.model.chars().take(24).collect::<String>(),
            r.kind,
            r.metric,
            dims,
            r.bytes / 1024,
            med,
            cpu_ratio(r).map(|c| format!("{c:.2}")).unwrap_or_default()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quotes_and_pairs() {
        let csv = "SourceFile,IFD0:Model,IFD0:PreviewImageStart#,IFD0:PreviewImageLength#,IFD0:Software\r\n\
                   D:/a.ARW,ILCE-1,100,200,\"v1, \"\"x\"\"\"\r\n";
        let p = parse_previews(csv).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].kind, "IFD0:PreviewImage");
        assert_eq!((p[0].offset, p[0].length), (100, 200));
        assert_eq!(p[0].model, "ILCE-1");
        assert_eq!(parse_csv(csv)[1][4], "v1, \"x\"");
    }

    #[test]
    fn stats_median() {
        let t = [3, 1, 2].map(Duration::from_millis);
        assert_eq!(stats(&t), (2.0, 1.0, 3.0));
    }

    #[test]
    fn uncached_range_roundtrip() {
        let dir = std::env::temp_dir().join("preview-bench-test.bin");
        let data: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&dir, &data).unwrap();
        let (_, got) = read_range_uncached(&dir, 5_000, 7_000).unwrap();
        assert_eq!(got, &data[5_000..12_000]);
        std::fs::remove_file(&dir).ok();
    }

    #[test]
    fn range_ending_at_unaligned_eof() {
        // Regression: os error 87 when a preview ends at a non-sector-aligned end of file (DNG).
        let p = std::env::temp_dir().join("preview-bench-eof.bin");
        let data: Vec<u8> = (0..10_001u32).map(|i| (i % 253) as u8).collect();
        std::fs::write(&p, &data).unwrap();
        let (_, got) = read_range_uncached(&p, 9_000, 1_001).unwrap();
        assert_eq!(got, &data[9_000..]);
        read_whole_uncached(&p).unwrap();
        std::fs::remove_file(&p).ok();
    }
}
