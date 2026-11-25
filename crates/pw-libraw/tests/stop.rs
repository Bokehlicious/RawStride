//! A LibRaw render that can be stopped, and how quickly it stops (ADR 0027 W5-Q1; T327) on the real
//! test RAWs. Run with `$env:PW_TESTDATA = "<folder>"; cargo test --workspace -- --ignored`; the
//! timing wants `--release` and `--nocapture` (`timing_of_a_stop`).
//!
//! - A stop that answers "stop" at once ends both stoppable renders as cancelled, leaves the
//!   session usable, and a following render gives the bytes of a fresh session.
//! - A stop that never answers "stop" gives the bytes of the plain renders.
//! - A stop set from another thread during a render: the render ends cancelled, or (LibRaw asks only
//!   between its stages, and an X-Trans PPG demosaic has no ask) finishes anyway with the plain
//!   bytes. Found in LibRaw 0.22: a stop while LibRaw works always spends the session (every later
//!   render, the statistics and the clipping map fail with the `spent:` text). The not-spent branch
//!   of `check_after_cancel` (a later render gives the bytes of a fresh session) is a defence and
//!   is not expected; the tests assert whichever LibRaw does and print which.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use pw_libraw::{Demosaic, Rect, Rgb8, Session};

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

/// The files of `PW_TESTDATA` with one of the extensions (lower case), sorted; at least one.
fn files_with(extensions: &[&str]) -> Vec<PathBuf> {
    let dir = testdata();
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| {
        panic!(
            "could not read PW_TESTDATA folder {}: {error}",
            dir.display()
        )
    });
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| extensions.contains(&e.to_ascii_lowercase().as_str()))
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no {extensions:?} test files were found in PW_TESTDATA folder {}",
        dir.display()
    );
    files
}

fn arw_files() -> Vec<PathBuf> {
    files_with(&["arw"])
}

fn raf_files() -> Vec<PathBuf> {
    files_with(&["raf"])
}

fn open(path: &Path) -> Session {
    Session::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    /// `render_half` / `render_half_stoppable`
    Half,
    /// The whole image as one PPG region (margin 0)
    WholePpg,
}

fn plain(session: &mut Session, kind: Kind) -> pw_libraw::Result<Rgb8> {
    match kind {
        Kind::Half => session.render_half(),
        Kind::WholePpg => {
            let r = Rect::new(0, 0, session.width(), session.height());
            session.render_region_with(r, 0, Demosaic::Ppg)
        }
    }
}

fn stoppable(
    session: &mut Session,
    kind: Kind,
    stop: &(dyn Fn() -> bool + Sync),
) -> pw_libraw::Result<Rgb8> {
    match kind {
        Kind::Half => session.render_half_stoppable(stop),
        Kind::WholePpg => {
            let r = Rect::new(0, 0, session.width(), session.height());
            session.render_region_stoppable(r, 0, Demosaic::Ppg, stop)
        }
    }
}

/// The bytes a fresh session gives for `kind`.
fn fresh(path: &Path, kind: Kind) -> Rgb8 {
    plain(&mut open(path), kind).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// What one render with a stop set from another thread did.
struct Run {
    result: pw_libraw::Result<Rgb8>,
    /// The time from setting the stop to the return; `None` if the render returned first (a stop
    /// set after the return does not count as set).
    stop_to_return: Option<Duration>,
    spent: bool,
    session: Session,
}

/// Renders `kind` on a fresh session of `path`, and another thread sets the stop after `delay`
/// (measured from just before the render starts).
fn run_with_stop_after(path: &Path, kind: Kind, delay: Duration) -> Run {
    let mut session = open(path);
    let flag = AtomicBool::new(false);
    let done = AtomicBool::new(false);
    // nanoseconds from `base` + 1 when the stop was set, 0 before
    let set_at = AtomicU64::new(0);
    let base = Instant::now();
    let (result, returned) = std::thread::scope(|scope| {
        scope.spawn(|| {
            // Sleep until about 1 ms before the stop, then spin: a busy thread would take a core
            // from LibRaw.
            let spin = Duration::from_millis(1);
            while base.elapsed() < delay {
                if done.load(Ordering::Acquire) {
                    return;
                }
                if base.elapsed() + spin < delay {
                    std::thread::sleep(Duration::from_millis(1));
                } else {
                    std::hint::spin_loop();
                }
            }
            set_at.store(base.elapsed().as_nanos() as u64 + 1, Ordering::Release);
            flag.store(true, Ordering::Release);
        });
        let result = stoppable(&mut session, kind, &|| flag.load(Ordering::Acquire));
        let returned = base.elapsed();
        done.store(true, Ordering::Release);
        (result, returned)
    });
    let set = set_at.load(Ordering::Acquire);
    let stop_to_return = (set != 0)
        .then(|| Duration::from_nanos(set - 1))
        .filter(|set_at| *set_at <= returned)
        .map(|set_at| returned - set_at);
    let spent = session.is_spent();
    Run {
        result,
        stop_to_return,
        spent,
        session,
    }
}

/// The time of one plain render of `kind` on a fresh session (the open is not timed).
fn unstopped_time(path: &Path, kind: Kind) -> Duration {
    let mut session = open(path);
    let start = Instant::now();
    plain(&mut session, kind).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    start.elapsed()
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v.get(v.len() / 2).copied().unwrap_or_default()
}

/// Two renders of `kind` of `path` are the same image. A half-size render of an X-Trans file
/// (`.RAF`) is not deterministic in LibRaw (two fresh sessions differ in about half the bytes;
/// found in T327), so there only the size is compared.
fn assert_same(path: &Path, kind: Kind, a: &Rgb8, b: &Rgb8, what: &str) {
    let x_trans = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("raf"));
    if x_trans && matches!(kind, Kind::Half) {
        assert_eq!(
            (a.width, a.height, a.data.len()),
            (b.width, b.height, b.data.len()),
            "{}: {what}",
            path.display()
        );
    } else {
        assert!(
            a == b,
            "{}: {kind:?}: {what}: the bytes differ",
            path.display()
        );
    }
}

/// What must hold after a stopped render that ended cancelled: either the session is spent and
/// everything fails with the `spent:` text, or it is intact and renders as a fresh one. Returns
/// whether it was spent.
fn check_after_cancel(path: &Path, kind: Kind, mut run: Run) -> bool {
    let name = path.display();
    let error = run.result.as_ref().err();
    assert!(
        error.is_some_and(|e| e.is_cancelled()),
        "{name}: {kind:?}: expected a cancelled render, got {:?}",
        run.result
    );
    if run.spent {
        let again = plain(&mut run.session, kind);
        let text = again.as_ref().err().map(|e| e.0.clone());
        assert!(
            text.as_deref().is_some_and(|t| t.starts_with("spent:")),
            "{name}: {kind:?}: a spent session must refuse a render with the spent text: {again:?}"
        );
        assert!(
            !again.err().is_some_and(|e| e.is_cancelled()),
            "{name}: a spent error is not a cancelled one"
        );
        let stats = run.session.statistics();
        assert!(
            stats
                .as_ref()
                .err()
                .is_some_and(|e| e.0.starts_with("spent:")),
            "{name}: statistics of a spent session must be an error: {stats:?}"
        );
        let map = run.session.clipping_map();
        assert!(
            map.as_ref()
                .err()
                .is_some_and(|e| e.0.starts_with("spent:")),
            "{name}: the clipping map of a spent session must be an error: {map:?}"
        );
    } else {
        let again = plain(&mut run.session, kind).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_same(path, kind, &again, &fresh(path, kind), "after a stop");
    }
    run.spent
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_stop_at_once_cancels_and_leaves_the_session_untouched() {
    let files = [arw_files()[0].clone(), raf_files()[0].clone()];
    for path in &files {
        let always: &(dyn Fn() -> bool + Sync) = &|| true;
        let mut session = open(path);
        for kind in [Kind::Half, Kind::WholePpg] {
            let result = stoppable(&mut session, kind, always);
            assert!(
                result.as_ref().err().is_some_and(|e| e.is_cancelled()),
                "{}: {kind:?}: {result:?}",
                path.display()
            );
            assert!(!session.is_spent(), "{}: {kind:?}", path.display());
        }
        let after = session
            .render_half()
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_same(
            path,
            Kind::Half,
            &after,
            &fresh(path, Kind::Half),
            "after a stop at once",
        );
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_stop_that_never_answers_stop_gives_the_plain_bytes() {
    let files = [arw_files()[0].clone(), raf_files()[0].clone()];
    for path in &files {
        let never: &(dyn Fn() -> bool + Sync) = &|| false;
        let mut session = open(path);
        for kind in [Kind::Half, Kind::WholePpg] {
            let stopped = stoppable(&mut session, kind, never)
                .unwrap_or_else(|e| panic!("{}: {kind:?}: {e}", path.display()));
            assert_same(
                path,
                kind,
                &stopped,
                &fresh(path, kind),
                "a stop that never stops",
            );
            assert!(!session.is_spent());
        }
    }
}

/// A stop set about half-way through an unstopped render of `kind` on `path`.
fn stop_half_way(path: &Path, kind: Kind) {
    let half = unstopped_time(path, kind) / 2;
    let run = run_with_stop_after(path, kind, half);
    if run.result.is_ok() {
        // LibRaw did not call the handler again after the stop was set (or never calls it during
        // this render): the render finishes normally, with the plain bytes.
        println!(
            "{}: {kind:?}: stop after {half:?}: the render finished anyway",
            path.display()
        );
        let image = run.result.unwrap_or_else(|e| panic!("{e}"));
        assert_same(path, kind, &image, &fresh(path, kind), "finished anyway");
        return;
    }
    let after = run.stop_to_return;
    let spent = check_after_cancel(path, kind, run);
    println!(
        "{}: {kind:?}: stop after {half:?}: cancelled, {} (stop to return {after:?})",
        path.display(),
        if spent { "spent" } else { "not spent" }
    );
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_stop_from_another_thread_ends_a_whole_image_ppg_render() {
    // An X-Trans PPG demosaic has no progress call, so a stop may come after the last one and the
    // render finishes anyway; `stop_half_way` accepts that.
    for path in [arw_files()[0].clone(), raf_files()[0].clone()] {
        stop_half_way(&path, Kind::WholePpg);
    }
}

#[test]
#[ignore = "needs real camera files: set PW_TESTDATA and run with --ignored"]
fn a_stop_from_another_thread_during_a_half_size_render() {
    for path in [arw_files()[0].clone(), raf_files()[0].clone()] {
        stop_half_way(&path, Kind::Half);
    }
}

#[test]
#[ignore = "a time measurement on real files; run with --release (ADR 0027 W5-Q1)"]
fn timing_of_a_stop() {
    // The first file of every distinct size among the ARW files, and one RAF.
    let mut files: Vec<PathBuf> = Vec::new();
    let mut sizes: Vec<(u32, u32)> = Vec::new();
    for path in arw_files() {
        let session = open(&path);
        let size = (session.width(), session.height());
        if !sizes.contains(&size) {
            sizes.push(size);
            files.push(path);
        }
    }
    files.push(raf_files()[0].clone());

    for path in &files {
        for kind in [Kind::WholePpg, Kind::Half] {
            let unstopped = median((0..3).map(|_| unstopped_time(path, kind)).collect());
            println!(
                "{} {kind:?}: unstopped median {:.1} ms",
                path.display(),
                unstopped.as_secs_f64() * 1000.0
            );
            for percent in [10u32, 50, 90] {
                let delay = unstopped * percent / 100;
                let (mut cancelled, mut finished, mut spent) = (0, 0, 0);
                // stop to return of the cancelled runs only; of the runs that finished anyway
                // (the render time left when the stop was set) apart
                let mut after: Vec<Duration> = Vec::new();
                let mut left: Vec<Duration> = Vec::new();
                for _ in 0..3 {
                    let run = run_with_stop_after(path, kind, delay);
                    match &run.result {
                        Err(e) if e.is_cancelled() => {
                            cancelled += 1;
                            after.extend(run.stop_to_return);
                        }
                        Err(e) => panic!("{}: {kind:?}: {e}", path.display()),
                        Ok(_) => {
                            finished += 1;
                            left.extend(run.stop_to_return);
                        }
                    }
                    if run.spent {
                        spent += 1;
                    }
                }
                let ms = |d: Duration| d.as_secs_f64() * 1000.0;
                let largest = after.iter().copied().max().unwrap_or_default();
                println!(
                    "{} {kind:?} stop at {percent} % ({:.1} ms): cancelled {cancelled}, finished \
                     anyway {finished}, spent {spent}; stop to return: median {:.1} ms, largest \
                     {:.1} ms (of {} cancelled runs); finished anyway: render time left at the stop, \
                     median {:.1} ms ({} runs with the stop set before the return)",
                    path.display(),
                    ms(delay),
                    ms(median(after.clone())),
                    ms(largest),
                    after.len(),
                    ms(median(left.clone())),
                    left.len()
                );
            }
        }
    }
}
