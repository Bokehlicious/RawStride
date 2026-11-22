# 2026-11-21 – pw-metadata: why writing metadata takes long

- Module: `pw-metadata` (`safe_write.rs`, `verify.rs`, `edit.rs`, `worker.rs`), `pw-imagecheck`
- Question: the owner finds that writing metadata takes a long time. Where does the time go in one
  safe write (ADR 0007 §9), and what could make it shorter?
- Build: no RawStride build was measured. The ExifTool steps of the safe write were run by a
  stand-alone script with the same arguments as the code at commit `d195170`
  (`feature/gui-workspace`). ExifTool 13.59 (`ExifTool.exe`, the Windows package), in `-stay_open`
  mode like `ExifTool::start_with`.
- Machine and conditions: thanos (i7-9700K, Arc B580). Each image was copied from
  `D:\PhotoWorkflowData\RAW` to the session's temporary folder on C: and measured there, right after
  the copy, so the file was in the Windows file cache (**warm cache**). One run per file.
- Kind: measurement and reading of the code. **No code was changed.** Status: no decision and no
  task.

## The safe write, step by step

What one write to an existing file does, from the code
([`safe_write.rs`](../../crates/pw-metadata/src/safe_write.rs),
[`edit.rs`](../../crates/pw-metadata/src/edit.rs)):

1. `read_metadata` of the image (and its sidecars), to build the write plan (`edit.rs`,
   `write_*_safely`).
2. §9.1: `ImageCheck::release`, `fileops::can_write`, then a **snapshot** of the target.
3. The journal entry.
4. §9.2: one ExifTool call `-o <name>.pwtmp-<hex>.<ext>` writes a **complete copy** of the file.
   For a TIFF or DNG whose plan writes `-IPTCDigest=new`: the **digest pass**
   (`-IPTCDigest=new -overwrite_original <copy>`), which writes the copy once more. After a TIFF
   write with `-IPTCDigest=new`: **ExifTool is restarted** (T92).
5. §9.3: a **snapshot** of the copy, `verify_copy` against the original's, then
   `ImageCheck::check_copy` (JPEG, TIFF, DNG; never a sidecar), then `FlushFileBuffers`.
6. §9.4: `ReplaceFileW`, then the final check: a **third snapshot** of the target, whose size and
   `ImageDataHash` must equal the copy's. Then the backup is deleted and the journal entry ended.

A snapshot (`SNAPSHOT_ARGS` in [`verify.rs`](../../crates/pw-metadata/src/verify.rs)) is
`-j -a -G1 -n -struct -api structformat=jsonq -api ImageHashType=SHA256 -all -ImageDataHash
-validate -warning`: every tag, plus a SHA-256 of all image data, plus ExifTool's validation.

All writes go through one ExifTool process in one worker thread
([`worker.rs`](../../crates/pw-metadata/src/worker.rs)), so the images of a batch (an edit of
several images, the template at import) are written one after another.

## Measured

Plan written: `-IPTC:City=Lund -XMP-photoshop:City=Lund` (a small plan; the size of the plan barely
matters next to the file). The step "image check" (`RawImageCheck::check_copy`) is **not
included**: it is Rust code (pw-raw, LibRaw) the script cannot run.

| Step | JPEG 13.5 MB (`TN006331.JPG`) | DNG 11.7 MB (`IMG_20261026_062851.dng`) | TIFF 41 MB (`TN002683.tif`) | TIFF 177 MB (`TN006025.tif`) | Sidecar (`P1240231.xmp`) |
|---|---|---|---|---|---|
| `read_metadata` (edit's first read) | 94 ms | 17 ms | 52 ms | 32 ms | 6 ms |
| Snapshot of the original | 222 ms | 46 ms | 212 ms | 803 ms | 3 ms |
| Write the copy (`-o`) | 187 ms | 50 ms | 166 ms | 659 ms | 41 ms |
| Digest pass | – | 44 ms | 171 ms | 626 ms | – |
| ExifTool restart | – | – | 221 ms | 218 ms | – |
| Snapshot of the copy | 196 ms | 48 ms | 390 ms | 980 ms | 4 ms |
| Flush of the copy | 9 ms | 9 ms | 37 ms | 118 ms | 2 ms |
| Replace (two renames in the script) | 1 ms | 1 ms | 1 ms | 1 ms | 1 ms |
| Final snapshot | 193 ms | 49 ms | 217 ms | 806 ms | 3 ms |
| Delete the backup | 2 ms | 1 ms | 4 ms | 16 ms | 0 ms |
| **Total, without the image check** | **905 ms** | **266 ms** | **1,472 ms** | **4,258 ms** | **60 ms** |
| Of which the three snapshots | 611 ms (68 %) | 143 ms (54 %) | 819 ms (56 %) | 2,589 ms (61 %) | 10 ms |

What a snapshot of the original costs without parts of its arguments (the same file, right after):

| Snapshot of the original | JPEG 13.5 MB | DNG 11.7 MB | TIFF 41 MB | TIFF 177 MB |
|---|---|---|---|---|
| Full (`SNAPSHOT_ARGS`) | 222 ms | 46 ms | 212 ms | 803 ms |
| Without `-validate -warning` | 182 ms | 40 ms | 206 ms | 793 ms |
| Without `-ImageDataHash` | 111 ms | 17 ms | 36 ms | 39 ms |

ExifTool start (spawn and first answer to `-ver`): 201 ms (218–221 ms for the restarts above).

The script, run as `python time_write.py <file names in D:\PhotoWorkflowData\RAW>`, is in the
appendix.

## Interpretation

1. **Inference:** the snapshots are the largest cost, more than half of the time for every image
   file, and almost all of it is `-ImageDataHash`: a SHA-256 of all image data, read three times
   per write (original, copy, final). Without the hash a TIFF snapshot falls from 803 ms to 39 ms.
   `-validate` costs little (6–40 ms).
2. **Inference:** the time grows with the size of the file. Per write, a TIFF is read whole about
   five times (three hashes, `-o`, the digest pass) and written whole twice (`-o` and the digest
   pass). A 177 MB TIFF takes over 4 s with a warm cache.
3. **Inference:** a RAW with a sidecar is cheap (60 ms): only the small XMP file is written and
   hashed, and there is no image check.
4. **Inference:** a JPEG costs about 110 ms per snapshot even without the hash, so 3 × 110 ms
   besides the hashes. **Hypothesis:** `-all` reading the maker notes; it could be confirmed by
   timing `-all` against `-all --MakerNotes:all` on the same file.
5. **Inference:** for TIFF and DNG the digest pass and (TIFF) the restart add 15–30 % (392 ms of
   1,472 ms for the 41 MB TIFF).
6. **Inference:** a batch costs the sum of its images, because all writes go through one process
   in one thread: 50 JPEGs from the camera take about 50 × 0.9 s ≈ 45 s plus the image checks.
7. **Hypothesis:** with a cold cache, or with the images on a slower disk than C:, the reads
   dominate even more, since each file is read about five times. Could be confirmed by the same
   script on files not read since a restart, in their own folder on D:.
8. **Not measured:**
   - The image check (`RawImageCheck::check_copy`). It reads the embedded previews of both files
     and compares them byte for byte; for a DNG it also opens the copy with LibRaw. It comes on top
     of the totals above.
   - What the shell does after a write (reading the metadata again, new thumbnails).
   - The time inside the RawStride process (the worker's queue, the event to the shell).
   A timed log around `write_safely` and `check_copy` in a release build would answer them.

## Possible changes

Each one touches a safety choice of ADR 0007 §9 and so needs the owner's decision (and an ADR
revision) before a task. The estimated savings are from the tables above.

| Change | What it saves | What it costs or risks |
|---|---|---|
| A cheaper final check (§9.4): size, or a hash without `-all`, after `ReplaceFileW` | about 20 % (one of three hashes) | the final check would no longer prove the replaced file's image data |
| The hash in its own narrow call (`-ImageDataHash` without `-all -validate`), the listing without the hash | mostly JPEG (the `-all` cost of the final check) | two ExifTool calls per snapshot where one is needed |
| Several ExifTool processes in parallel for a batch | the largest gain for import and the template, limited by the disk | the journal, the release of RAW sessions and the order of the answers must allow it |
| The digest pass and the TIFF restart decided again (T92) | 15–30 % for TIFF and DNG | the measured reasons for them (§9.2, T92) must still hold |

## Follow-up

- The owner decided (2026-11-21) to measure what is missing first: T261 measured the image check
  and the write as RawStride runs it in a release build ("Measured in T261" below; done
  2026-11-22). The shell's work after a write and a cold cache are still not measured.
- The owner decides whether any of the changes above, or a change of the image check for DNG, is
  worth an ADR 0007 revision. **The owner, 2026-11-22:** make it faster without a weaker check;
  ADR 0007 revision 29 (§9.6, Q17–Q19), approved by the owner 2026-11-22. The cheaper
  final check is not taken up.

## Measured in T261

- Build: `cargo run --release -p pw-imagecheck --example write_timing` (T261,
  [`write_timing.rs`](../../crates/pw-imagecheck/examples/write_timing.rs)), at commit `eea851c`
  (`feature/gui-workspace`) with the example added, uncommitted. The example calls the app's own
  code: `ExifTool::read_metadata`, `ExifTool::snapshot` and `ExifTool::write_safely` with
  `RawImageCheck::without_release()` wrapped in a timer, and a journal in the work folder.
  ExifTool 13.59 (`exiftool` on PATH), started once; its start and first answer took 189–193 ms.
- Measured 2026-11-22 on thanos. Sources in `D:\PhotoWorkflowData\RAW`, copied for every run into
  the work folder: first a new folder under `%TEMP%` on C: (Kingston NV3, NVMe), then a new folder
  on D: (Samsung 850 PRO, SATA SSD). **Warm cache.** Three runs per file, one after another;
  times are medians. Every run (48 on each drive) wrote successfully.
- Plan: the copy's own read metadata with `photoshop:City` = "Malmö" (`Metadata::set`), through
  `write_plan::plan` with the default settings: the plan of a City edit in the editor.
- `write_safely` is the whole transaction as the app runs it: the snapshot of the original, the
  journal, the copy, the digest pass and (TIFF) the ExifTool restart, the snapshot and check of
  the copy, the flush, `ReplaceFileW`, the final snapshot. "Read + write" is what one edit costs
  in the worker (its `read_metadata`, then `write_safely`); "snapshot" is one snapshot of the
  target alone, for comparison.

Medians in ms, work folder on C::

| File | Case | MB | Kind | `read_metadata` | Snapshot | `write_safely` | `check_copy` | Write without check | Read + write |
|---|---|---:|---|---:|---:|---:|---:|---:|---:|
| `TN006331.JPG` | – | 13.5 | JPEG | 23 | 191 | 626 | 0 | 626 | 649 |
| `TN005701_DxO.jpg` | – | 16.4 | JPEG | 40 | 266 | 877 | 0 | 877 | 921 |
| `IMG_20261026_062851.dng` | – | 11.7 | DNG | 16 | 48 | 462 | 261 | 199 | 477 |
| `IMG_20261026_084628.dng` | – | 12.2 | DNG | 16 | 49 | 457 | 254 | 203 | 472 |
| `IMG_20261026_091722.dng` | – | 11.4 | DNG | 18 | 50 | 462 | 256 | 209 | 489 |
| `IMG_20261026_092349.dng` | – | 18.9 | DNG | 19 | 74 | 570 | 273 | 297 | 589 |
| `IMG_20261026_092351.dng` | – | 20.4 | DNG | 20 | 75 | 581 | 275 | 306 | 602 |
| `TN002512.DNG` | – | 36.0 | DNG | 31 | 178 | 1,203 | 540 | 662 | 1,233 |
| `TN002513.DNG` | – | 30.5 | DNG | 30 | 155 | 1,082 | 486 | 597 | 1,112 |
| `TN002683.tif` | – | 41.2 | TIFF | 33 | 212 | 1,304 | 1 | 1,303 | 1,337 |
| `TN004596.tif` | – | 97.1 | TIFF | 32 | 448 | 2,326 | 1 | 2,325 | 2,357 |
| `TN006025.tif` | – | 177.3 | TIFF | 35 | 791 | 3,772 | 0 | 3,772 | 3,809 |
| `P1240231.RW2` | sidecar exists | 19.6 | RAW | 25 | 5 | 31 | 0 | 31 | 56 |
| `P1240231.RW2` | new sidecar | 19.6 | RAW | 19 | – | 21 | 0 | 21 | 41 |
| `P1240233.RW2` | sidecar exists | 19.7 | RAW | 24 | 5 | 33 | 0 | 33 | 57 |
| `P1240233.RW2` | new sidecar | 19.7 | RAW | 19 | – | 22 | 0 | 22 | 42 |

With the work folder on D:, `read_metadata`, the snapshot and `check_copy` were the same within
2 % (at most 8 ms); `write_safely` was longer:

| Kind | `write_safely` on C: | on D: | Difference |
|---|---:|---:|---:|
| JPEG (2 files) | 626, 877 | 677, 904 | +27 to +51 ms (3–8 %) |
| DNG (7 files) | 457–1,203 | 488–1,273 | +26 to +70 ms (5–8 %) |
| TIFF 41, 97, 177 MB | 1,304, 2,326, 3,772 | 1,372, 2,441, 3,974 | +68, +115, +202 ms (5 %) |
| RAW, sidecar exists | 31, 33 | 45, 43 | +10 to +14 ms |
| RAW, new sidecar | 21, 22 | 32, 32 | +10 to +11 ms |

The share of `check_copy` in the edit's total (read + write), on C::

| Kind | `check_copy` | Share |
|---|---:|---:|
| JPEG | 0.3–0.5 ms | under 0.1 % |
| DNG, 11–20 MB (5 files) | 254–275 ms | 46–55 % |
| DNG, 30–36 MB (2 files) | 486–540 ms | 44 % |
| TIFF | 0.4–1 ms | under 0.1 % |
| RAW (both cases) | not called (sidecar) | 0 % |

1. **Inference:** the image check matters only for DNG, where it is about half of an edit: 250–275
   ms for an 11–20 MB DNG, 490–540 ms for a 30–36 MB one, very even between runs (±5 ms). For a
   JPEG or TIFF it is under 1 ms: `check_copy` then only compares embedded previews
   (`check_previews`), and these files have none or few that `pw_raw::inspect` finds. For a DNG it
   also opens the copy with LibRaw (`check_dng`); the original is opened only when the copy fails.
2. **Hypothesis:** most of a DNG's check is LibRaw opening the copy, not the preview comparison
   (the JPEG and TIFF previews cost under 1 ms, and the time grows less than the file: 22 ms/MB at
   11.7 MB, 15 ms/MB at 36 MB). Could be confirmed by timing `check_dng` and `check_previews`
   apart.
3. **Inference:** without the image check, the write costs what the script found or a little
   less, so the script's conclusions 1–6 above hold: the time grows with the file, the snapshots
   are the same as the script's (191/222, 48/46, 212/212 and 791/803 ms for the JPEG, the
   11.7 MB DNG and the two TIFFs), and a RAW with a sidecar is cheap (41–57 ms per edit). The
   script's write totals without its read (811, 249, 1,420 and 4,226 ms) are 9–30 % above the
   medians here (626, 199, 1,303, 3,772 ms).
4. **Inference:** the script's JPEG figures were raised by being the first file in a new ExifTool
   process. Here, too, the first run of `TN006331.JPG` (the first file of each run of the
   program) took 94 ms to read and 760–797 ms to write, against 22–23 ms and 626–677 ms in the next
   two runs, on both drives. **Hypothesis:** ExifTool loads code on its first JPEG; in the app
   that is the first write of a session. Could be confirmed by measuring another file first.
5. **Inference:** the destination drive matters little with a warm cache: D: (SATA) adds 3–8 %
   to an image file's write and about 10 ms to a sidecar's, all of it in writing, flushing and
   replacing (the reads, snapshots and checks are the same). Interpretation 7's hypothesis about
   a slower disk is not borne out for D: with a warm cache; a cold cache was not measured.
6. **Inference:** per edit, with the warm cache on C:, a 13.5 MB camera JPEG costs about 0.65 s,
   a 12 MB DNG about 0.48 s (of which 0.26 s the image check), a 36 MB DNG about 1.2 s, a 177 MB
   TIFF about 3.8 s and a RAW with a sidecar about 0.06 s. A batch is their sum (interpretation 6):
   50 such JPEGs about 33 s, not the 45 s estimated from the script's first-file figure.
7. **Not measured:** the shell's work after a write (the new reads, the view and the thumbnails),
   the worker's queue and events, and a cold cache (T261, "Not in this task").

## Appendix: the timing script

```python
"""Times the ExifTool steps of RawStride's safe write on copies of test images.
Read-only toward the originals: each is copied to a work folder first."""
import os, shutil, subprocess, sys, time

ET = os.path.expandvars(r"%LOCALAPPDATA%\Programs\ExifTool\ExifTool.exe")
SRC = r"D:\PhotoWorkflowData\RAW"
WORK = os.path.join(os.path.dirname(os.path.abspath(__file__)), "work")
FILES = sys.argv[1:] or ["TN006331.JPG", "IMG_20261026_062851.dng", "TN002683.tif", "P1240231.xmp"]

SNAP = ["-j", "-a", "-G1", "-n", "-struct", "-api", "structformat=jsonq", "-api",
        "ImageHashType=SHA256", "-all", "-ImageDataHash", "-validate", "-warning"]
SNAP_NOVAL = [a for a in SNAP if a not in ("-validate", "-warning")]
SNAP_NOHASH = [a for a in SNAP if a != "-ImageDataHash"]
READ = ["-j", "-n", "-b", "-struct", "-api", "structformat=jsonq", "-G1", "-XMP:all", "-IPTC",
        "-Photoshop:IPTCDigest", "-CurrentIPTCDigest", "-IFD0:Rating"]


class Et:
    def __init__(self):
        t = time.perf_counter()
        self.p = subprocess.Popen([ET, "-stay_open", "True", "-@", "-"], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        self.n = 0
        self.run(["-ver"])
        self.start_s = time.perf_counter() - t

    def run(self, args):
        self.n += 1
        cmd = "-charset\nfilename=utf8\n" + "\n".join(args) + f"\n-execute{self.n}\n"
        self.p.stdin.write(cmd.encode("utf-8"))
        self.p.stdin.flush()
        out = b""
        marker = f"{{ready{self.n}}}".encode()
        while marker not in out:
            line = self.p.stdout.readline()
            if not line:
                raise RuntimeError("exiftool died")
            out += line
        return out

    def close(self):
        try:
            self.p.stdin.write(b"-stay_open\nFalse\n"); self.p.stdin.flush(); self.p.wait(10)
        except Exception:
            self.p.kill()


def timed(label, fn, rows):
    t = time.perf_counter(); fn(); dt = time.perf_counter() - t
    rows.append((label, dt)); return dt


def main():
    os.makedirs(WORK, exist_ok=True)
    et = Et()
    print(f"ExifTool start (spawn + first answer): {et.start_s*1000:.0f} ms\n")
    for name in FILES:
        src = os.path.join(SRC, name)
        tgt = os.path.join(WORK, name)
        shutil.copy2(src, tgt)
        base, ext = os.path.splitext(tgt)
        copy = base + ".pwtmp-0" + ext
        bak = base + ".pwbak-0" + ext
        for f in (copy, bak):
            if os.path.exists(f): os.remove(f)
        is_tiff = ext.lower() in (".tif", ".dng")
        rows = []
        size = os.path.getsize(tgt) / 1e6
        timed("read_metadata (edit's first read)", lambda: et.run(READ + [tgt]), rows)
        timed("snapshot original (full)", lambda: et.run(SNAP + [tgt]), rows)
        timed("  - same without -validate", lambda: et.run(SNAP_NOVAL + [tgt]), rows)
        timed("  - same without ImageDataHash", lambda: et.run(SNAP_NOHASH + [tgt]), rows)
        timed("write copy (-o)", lambda: et.run(["-IPTC:City=Lund", "-XMP-photoshop:City=Lund",
                                                 "-o", copy, tgt]), rows)
        if is_tiff:
            timed("digest pass", lambda: et.run(["-IPTCDigest=new", "-overwrite_original", copy]), rows)
        if ext.lower() == ".tif":
            def restart():
                nonlocal et
                et.close(); et = Et()
            timed("restart ExifTool", restart, rows)
        timed("snapshot copy (full)", lambda: et.run(SNAP + [copy]), rows)
        timed("flush copy (FlushFileBuffers)", lambda: _flush(copy), rows)
        timed("replace (rename x2)", lambda: (os.replace(tgt, bak), os.replace(copy, tgt)), rows)
        timed("snapshot final (full)", lambda: et.run(SNAP + [tgt]), rows)
        timed("delete backup", lambda: os.remove(bak), rows)
        real = [r for r in rows if not r[0].startswith("  -")]
        total = sum(d for _, d in real)
        print(f"== {name} ({size:.1f} MB) total {total*1000:.0f} ms (excl. image check)")
        for label, d in rows:
            share = "" if label.startswith("  -") else f"{d/total*100:4.0f}%"
            print(f"   {label:40s} {d*1000:7.0f} ms {share}")
        print()
        os.remove(tgt)
    et.close()


def _flush(path):
    with open(path, "r+b") as f:
        os.fsync(f.fileno())


if __name__ == "__main__":
    main()
```

Notes on the script: the digest pass is run for every TIFF and DNG, and the restart for every
TIFF, as in the code when the plan writes `-IPTCDigest=new`. The replace is two renames, not
`ReplaceFileW`; both take about a millisecond.
