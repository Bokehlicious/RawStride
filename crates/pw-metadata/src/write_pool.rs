//! A pool of ExifTool write processes (ADR 0007 §9.6 point 2, Q18; T266).
//!
//! Writes are shared among up to four ExifTool processes, each owned by its own thread, and every
//! file is still its own safe-write transaction (§9.1–§9.5): a job is one closure that gets the
//! thread's [`ExifTool`] and runs one file's whole safe write with it. Nothing of a transaction
//! is shared between the threads except what the closure itself brings (the journal folder and
//! the image check).
//!
//! - **Processes:** [`WritePool::start`] (the ExifTool of [`exiftool_program`]) or
//!   [`WritePool::start_with`] starts `n` threads, `n` clamped to 2..=4 (a caller with one
//!   process uses no pool). A thread starts its ExifTool process when it first needs it, so its
//!   first job pays for the start; it asks the version, and a process older than
//!   [`MIN_EXIFTOOL_VERSION`](crate::MIN_EXIFTOOL_VERSION), or whose version cannot be read,
//!   answers [`JobAnswer::TooOld`] without running the job (D9). A process that breaks
//!   ([`ExifTool::is_broken`]) is dropped and the thread's next job starts a new one.
//! - **Order:** every job has a [`WriteKey`] ([`write_key`]: the image's folder and its file stem
//!   in lower case, so the files that may share one sidecar have one key). Two jobs with one key
//!   never run at the same time and run in the order given: a job is held back while an earlier
//!   one with its key has not answered. Jobs with different keys may answer in any order.
//! - **Answers** come back on an unbounded channel, so a pool thread never waits to give one:
//!   [`WritePool::try_answer`] and [`WritePool::answer_timeout`]. Every job given is answered
//!   exactly once. [`WritePool::outstanding`] counts the jobs whose answer has not been taken.
//! - **A job that panics** is answered [`JobAnswer::Lost`]; its process is dropped as broken and
//!   the thread takes the next job. Such a job may leave its journal entry and its temporary
//!   files; [`Journal::recover`](crate::journal::Journal::recover) resolves them at the next
//!   start, as after a crash.
//! - **Discard and the end:** [`WritePool::discard`] answers the jobs not started
//!   [`JobAnswer::Discarded`]; jobs that started finish. Dropping the pool does the same and then
//!   joins every thread, so a job is never cut off in the middle of its transaction.

use std::any::Any;
use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::exiftool::{MIN_EXIFTOOL_VERSION, exiftool_program};
use crate::{ExifTool, Result, ToolVersion};

/// The fewest and the most processes of a pool (§9.6 point 2).
const MIN_PROCESSES: usize = 2;
const MAX_PROCESSES: usize = 4;

/// The result of a start's version question: the version, or the start or version error as text.
pub(crate) type VersionResult = std::result::Result<ToolVersion, String>;

/// A started ExifTool process, its version and whether that version allows writes (D9).
pub(crate) struct Running {
    pub(crate) tool: ExifTool,
    pub(crate) may_write: bool,
    pub(crate) version: VersionResult,
}

/// Starts ExifTool and asks its version. With `first`, the first result (the version, the version
/// error or the start error) is kept there. A process that broke while answering is dropped and
/// its error returned, so the next request starts a new one.
pub(crate) fn start_tool(
    program: &str,
    pre_args: &[&str],
    first: Option<&OnceLock<VersionResult>>,
) -> std::result::Result<Running, String> {
    let mut tool = match ExifTool::start_with(program, pre_args) {
        Ok(tool) => tool,
        Err(error) => {
            let text = error.to_string();
            if let Some(first) = first {
                let _ = first.set(Err(text.clone()));
            }
            return Err(text);
        }
    };
    let version = tool.version().map_err(|error| error.to_string());
    let may_write = version
        .as_ref()
        .is_ok_and(|version| *version >= MIN_EXIFTOOL_VERSION);
    let broken = tool.is_broken();
    if let Some(first) = first {
        let _ = first.set(version.clone());
    }
    if broken {
        let error = version.err();
        return Err(error.unwrap_or_else(|| "ExifTool process is unusable".into()));
    }
    Ok(Running {
        tool,
        may_write,
        version,
    })
}

/// What decides which jobs may run at the same time: the image's folder as given and its file
/// stem in lower case ([`write_key`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WriteKey {
    folder: PathBuf,
    stem: String,
}

/// The key of a write of `image`: its folder (compared as given) and its file stem in lower case,
/// so `a.CR2` and `a.NEF` in one folder, which may share the sidecar `a.xmp`
/// ([`sidecar_path`](crate::sidecar_path)), have one key.
pub fn write_key(image: &Path) -> WriteKey {
    WriteKey {
        folder: image.parent().map(Path::to_path_buf).unwrap_or_default(),
        stem: image
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase())
            .unwrap_or_default(),
    }
}

/// The number of a job given to a pool, unique within the pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ticket(u64);

/// A job: one file's safe write with the thread's ExifTool process.
pub type Job<T> = Box<dyn FnOnce(&mut ExifTool) -> T + Send>;

/// The answer to one job.
#[derive(Debug, PartialEq, Eq)]
pub enum JobAnswer<T> {
    /// The job ran and gave this.
    Done(T),
    /// The thread's ExifTool process could not be started; the error's text. The job did not run.
    StartFailed(String),
    /// The thread's ExifTool is older than [`MIN_EXIFTOOL_VERSION`](crate::MIN_EXIFTOOL_VERSION)
    /// (its version), or its version is unknown (the version error's text) (D9). The job did not
    /// run.
    TooOld(std::result::Result<ToolVersion, String>),
    /// The job was not started: [`WritePool::discard`], or the pool was dropped.
    Discarded,
    /// The job panicked; the panic's text. Its process was dropped as broken.
    Lost(String),
}

/// A job given and not started.
struct Pending<T> {
    ticket: Ticket,
    key: WriteKey,
    job: Job<T>,
}

/// What the pool's threads share, under one lock.
struct Queue<T> {
    pending: VecDeque<Pending<T>>,
    /// The keys of the jobs that started and have not answered.
    running: HashSet<WriteKey>,
    stop: bool,
}

struct Shared<T> {
    queue: Mutex<Queue<T>>,
    changed: Condvar,
}

impl<T> Shared<T> {
    /// The queue; a lock poisoned by a panic elsewhere is still used (the queue is only changed
    /// in whole steps).
    fn lock(&self) -> MutexGuard<'_, Queue<T>> {
        self.queue.lock().unwrap_or_else(|error| error.into_inner())
    }
}

type Answer<T> = (Ticket, JobAnswer<T>);

/// A pool of 2 to 4 threads, each with its own ExifTool process (module comment).
pub struct WritePool<T: Send + 'static> {
    shared: Arc<Shared<T>>,
    answers_tx: Sender<Answer<T>>,
    answers: Receiver<Answer<T>>,
    threads: Vec<JoinHandle<()>>,
    processes: usize,
    next_ticket: u64,
    outstanding: usize,
}

impl<T: Send + 'static> fmt::Debug for WritePool<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WritePool")
            .field("processes", &self.processes)
            .field("outstanding", &self.outstanding)
            .finish_non_exhaustive()
    }
}

impl<T: Send + 'static> WritePool<T> {
    /// Starts a pool of `n` threads (clamped to 2..=4) using the ExifTool given by
    /// [`exiftool_program`], as [`MetadataWorker::start`](crate::MetadataWorker::start) does.
    pub fn start(n: usize) -> Result<Self> {
        Self::start_with(&exiftool_program(), &[], n)
    }

    /// Starts a pool of `n` threads (clamped to 2..=4) using an explicit ExifTool program and
    /// leading arguments. The processes start when their threads first need them. A thread that
    /// cannot be spawned is an error; the threads already spawned are ended first.
    pub fn start_with(program: &str, pre_args: &[&str], n: usize) -> Result<Self> {
        #[cfg(test)]
        tests::fail_start_if_asked(pre_args)?;
        let processes = n.clamp(MIN_PROCESSES, MAX_PROCESSES);
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                pending: VecDeque::new(),
                running: HashSet::new(),
                stop: false,
            }),
            changed: Condvar::new(),
        });
        let (answers_tx, answers) = mpsc::channel();
        let mut pool = WritePool {
            shared,
            answers_tx,
            answers,
            threads: Vec::with_capacity(processes),
            processes,
            next_ticket: 0,
            outstanding: 0,
        };
        for index in 0..processes {
            let shared = Arc::clone(&pool.shared);
            let answers = pool.answers_tx.clone();
            let program = program.to_string();
            let pre_args: Vec<String> = pre_args.iter().map(|arg| (*arg).to_string()).collect();
            let thread = std::thread::Builder::new()
                .name(format!("pw-metadata-write-{}", index + 1))
                .spawn(move || work(&program, &pre_args, &shared, &answers))?;
            // On an error above, `pool` is dropped: the threads spawned so far are ended.
            pool.threads.push(thread);
        }
        Ok(pool)
    }

    /// The number of threads (and at most processes) of the pool, 2..=4.
    pub fn processes(&self) -> usize {
        self.processes
    }

    /// Gives a job for the image whose key is `key` ([`write_key`]). It runs when a thread is free
    /// and no earlier job with its key is unanswered; its answer carries the returned ticket.
    pub fn submit(&mut self, key: WriteKey, job: Job<T>) -> Ticket {
        self.next_ticket = self.next_ticket.wrapping_add(1);
        let ticket = Ticket(self.next_ticket);
        self.outstanding += 1;
        let mut queue = self.shared.lock();
        queue.pending.push_back(Pending { ticket, key, job });
        drop(queue);
        self.shared.changed.notify_all();
        ticket
    }

    /// An answer, if one is waiting; never blocks.
    pub fn try_answer(&mut self) -> Option<(Ticket, JobAnswer<T>)> {
        match self.answers.try_recv() {
            Ok(answer) => Some(self.taken(answer)),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    /// An answer, waiting at most `timeout` for one.
    pub fn answer_timeout(&mut self, timeout: Duration) -> Option<(Ticket, JobAnswer<T>)> {
        match self.answers.recv_timeout(timeout) {
            Ok(answer) => Some(self.taken(answer)),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => None,
        }
    }

    /// The jobs given whose answer has not yet been taken by [`WritePool::try_answer`] or
    /// [`WritePool::answer_timeout`]; an answer waiting in the channel still counts.
    pub fn outstanding(&self) -> usize {
        self.outstanding
    }

    /// Drops the jobs given and not started, each answered [`JobAnswer::Discarded`]; jobs that
    /// started finish.
    pub fn discard(&self) {
        let mut queue = self.shared.lock();
        let dropped: Vec<Pending<T>> = queue.pending.drain(..).collect();
        drop(queue);
        for pending in dropped {
            let _ = self.answers_tx.send((pending.ticket, JobAnswer::Discarded));
        }
    }

    fn taken(&mut self, answer: Answer<T>) -> Answer<T> {
        self.outstanding = self.outstanding.saturating_sub(1);
        answer
    }

    /// Discards the jobs not started, lets the started ones finish and joins every thread.
    fn shutdown(&mut self) {
        let mut queue = self.shared.lock();
        queue.stop = true;
        let dropped: Vec<Pending<T>> = queue.pending.drain(..).collect();
        drop(queue);
        for pending in dropped {
            let _ = self.answers_tx.send((pending.ticket, JobAnswer::Discarded));
        }
        self.shared.changed.notify_all();
        for thread in self.threads.drain(..) {
            // A job's panic is caught in the thread; if it ended anyway, nothing is left to
            // clean up.
            let _ = thread.join();
        }
    }
}

impl<T: Send + 'static> Drop for WritePool<T> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Takes the first job whose key is free: no started job with it is unanswered and no earlier job
/// with it is still waiting.
fn next_job<T>(queue: &mut Queue<T>) -> Option<Pending<T>> {
    let mut passed: HashSet<&WriteKey> = HashSet::new();
    let mut found = None;
    for (index, pending) in queue.pending.iter().enumerate() {
        if !queue.running.contains(&pending.key) && !passed.contains(&pending.key) {
            found = Some(index);
            break;
        }
        passed.insert(&pending.key);
    }
    let pending = queue.pending.remove(found?)?;
    queue.running.insert(pending.key.clone());
    Some(pending)
}

/// One pool thread: takes jobs until the pool stops, with its own ExifTool process.
fn work<T>(program: &str, pre_args: &[String], shared: &Shared<T>, answers: &Sender<Answer<T>>) {
    let pre_args: Vec<&str> = pre_args.iter().map(String::as_str).collect();
    let mut running: Option<Running> = None;
    loop {
        let pending = {
            let mut queue = shared.lock();
            loop {
                if queue.stop {
                    return;
                }
                if let Some(pending) = next_job(&mut queue) {
                    break pending;
                }
                queue = match shared.changed.wait(queue) {
                    Ok(queue) => queue,
                    Err(error) => error.into_inner(),
                };
            }
        };
        let Pending { ticket, key, job } = pending;
        let answer = run_job(program, &pre_args, &mut running, job);
        // The answer first, then the key: a later job with the key starts after this answer.
        let _ = answers.send((ticket, answer));
        shared.lock().running.remove(&key);
        shared.changed.notify_all();
    }
}

/// Runs `job` with the thread's process, starting one if there is none.
fn run_job<T>(
    program: &str,
    pre_args: &[&str],
    running: &mut Option<Running>,
    job: Job<T>,
) -> JobAnswer<T> {
    if running.is_none() {
        match start_tool(program, pre_args, None) {
            Ok(process) => *running = Some(process),
            Err(error) => return JobAnswer::StartFailed(error),
        }
    }
    let Some(process) = running.as_mut() else {
        return JobAnswer::StartFailed("ExifTool process is unusable".into());
    };
    if !process.may_write {
        return JobAnswer::TooOld(process.version.clone());
    }
    let tool = &mut process.tool;
    match catch_unwind(AssertUnwindSafe(move || job(tool))) {
        Ok(value) => {
            if running
                .as_ref()
                .is_some_and(|process| process.tool.is_broken())
            {
                // Dropping it shuts it down; the next job starts a new one.
                *running = None;
            }
            JobAnswer::Done(value)
        }
        Err(payload) => {
            // The stream may be anywhere in a command: never use this process again.
            *running = None;
            JobAnswer::Lost(format!(
                "the write stopped unexpectedly: {}",
                panic_text(payload.as_ref())
            ))
        }
    }
}

/// The text of a panic's payload.
fn panic_text(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "no reason given".to_string()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::test_support::{EXIT_MARKER, SLOW_MARKER, fake_exiftool};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::thread::ThreadId;
    use std::time::Instant;

    /// A leading argument that makes [`WritePool::start_with`] fail while the file named after it
    /// exists (T266: a failed pool start, which only a failed thread spawn gives otherwise).
    pub(crate) const FAIL_START_WHILE: &str = "pw-test-pool-start-fails-while=";

    pub(super) fn fail_start_if_asked(pre_args: &[&str]) -> Result<()> {
        let asked = pre_args
            .iter()
            .find_map(|arg| arg.strip_prefix(FAIL_START_WHILE));
        match asked {
            Some(file) if Path::new(file).exists() => Err(crate::Error::Io(std::io::Error::other(
                "the test asks the pool start to fail",
            ))),
            _ => Ok(()),
        }
    }

    const LIMIT: Duration = Duration::from_secs(20);

    /// A pool of `n` over the fake ExifTool in `mode`.
    fn fake_pool<T: Send + 'static>(mode: &str, n: usize) -> WritePool<T> {
        let (program, args) = fake_exiftool(mode);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        WritePool::start_with(&program, &args, n).expect("the pool should start")
    }

    /// The next answer within [`LIMIT`].
    fn answer<T: Send + 'static>(pool: &mut WritePool<T>) -> (Ticket, JobAnswer<T>) {
        pool.answer_timeout(LIMIT)
            .expect("the pool should answer in time")
    }

    /// `count` answers within [`LIMIT`], in the order they came.
    fn answers<T: Send + 'static>(
        pool: &mut WritePool<T>,
        count: usize,
    ) -> Vec<(Ticket, JobAnswer<T>)> {
        (0..count).map(|_| answer(pool)).collect()
    }

    /// A job that runs one fake command naming `file` and gives back `name`.
    fn command(file: &str, name: &'static str) -> Job<std::result::Result<&'static str, String>> {
        let file = file.to_string();
        Box::new(move |tool: &mut ExifTool| {
            tool.execute(&[file])
                .map(|_| name)
                .map_err(|error| error.to_string())
        })
    }

    fn key(path: &str) -> WriteKey {
        write_key(Path::new(path))
    }

    fn slow(name: &str) -> String {
        format!("C:/photos/{name}-{SLOW_MARKER}.jpg")
    }

    #[test]
    fn jobs_with_different_keys_overlap() {
        let mut pool = fake_pool("slow", 2);
        let slow_ticket = pool.submit(key("C:/photos/a.jpg"), command(&slow("a"), "slow"));
        let quick_ticket = pool.submit(key("C:/photos/b.jpg"), command("C:/photos/b.jpg", "quick"));

        let got = answers(&mut pool, 2);

        assert_eq!(
            got,
            vec![
                (quick_ticket, JobAnswer::Done(Ok("quick"))),
                (slow_ticket, JobAnswer::Done(Ok("slow"))),
            ]
        );
        assert_eq!(pool.outstanding(), 0);
    }

    /// A job's name, start and end.
    type Span = (&'static str, Instant, Instant);

    /// Two jobs with the key of `first` and `second`: the first slow; they must not overlap and
    /// must answer in the order given.
    fn same_key_runs_in_order(first: &str, second: &str) {
        let mut pool = fake_pool("slow", 4);
        let spans: Arc<Mutex<Vec<Span>>> = Arc::default();
        let job = |file: String, name: &'static str| -> Job<&'static str> {
            let spans = Arc::clone(&spans);
            Box::new(move |tool: &mut ExifTool| {
                let start = Instant::now();
                let _ = tool.execute(&[file]);
                spans
                    .lock()
                    .expect("the spans")
                    .push((name, start, Instant::now()));
                name
            })
        };
        let one = pool.submit(key(first), job(slow("first"), "first"));
        let two = pool.submit(key(second), job("C:/photos/x.jpg".into(), "second"));

        assert_eq!(
            answers(&mut pool, 2),
            vec![
                (one, JobAnswer::Done("first")),
                (two, JobAnswer::Done("second"))
            ]
        );
        let spans = spans.lock().expect("the spans").clone();
        assert_eq!(spans.len(), 2);
        assert_eq!((spans[0].0, spans[1].0), ("first", "second"));
        assert!(spans[1].1 >= spans[0].2, "the jobs overlapped: {spans:?}");
    }

    #[test]
    fn two_jobs_with_one_key_never_overlap_and_answer_in_order() {
        same_key_runs_in_order("C:/photos/a.jpg", "C:/photos/a.jpg");
    }

    #[test]
    fn a_cr2_and_a_nef_with_one_stem_share_a_key() {
        same_key_runs_in_order("C:/photos/a.CR2", "C:/photos/A.nef");
    }

    #[test]
    fn a_broken_process_is_replaced_and_the_other_threads_go_on() {
        let mut pool = fake_pool("slow", 2);
        // Keeps one thread busy while the other's process breaks.
        let busy = pool.submit(key("C:/photos/busy.jpg"), command(&slow("busy"), "busy"));
        let breaking: Job<std::result::Result<&'static str, String>> =
            Box::new(|tool: &mut ExifTool| {
                let result = tool.execute(&[format!("C:/photos/{EXIT_MARKER}.jpg")]);
                assert!(tool.is_broken(), "the fake should have exited: {result:?}");
                Err("broken".to_string())
            });
        let broke = pool.submit(key("C:/photos/exit.jpg"), breaking);
        let mut after = Vec::new();
        for name in ["c", "d", "e", "f"] {
            let file = format!("C:/photos/{name}.jpg");
            after.push(pool.submit(key(&file), command(&file, "after")));
        }

        let got = answers(&mut pool, 6);

        for (ticket, answer) in &got {
            if *ticket == broke {
                assert_eq!(*answer, JobAnswer::Done(Err("broken".to_string())));
            } else if *ticket == busy {
                assert_eq!(*answer, JobAnswer::Done(Ok("busy")));
            } else {
                assert!(after.contains(ticket));
                assert_eq!(*answer, JobAnswer::Done(Ok("after")));
            }
        }
    }

    #[test]
    fn a_process_too_old_answers_too_old_without_running_the_job() {
        let ran = Arc::new(AtomicBool::new(false));
        for (mode, expected) in [
            (
                "version=12.40",
                Ok(ToolVersion {
                    major: 12,
                    minor: 40,
                }),
            ),
            (
                "version=abc",
                Err("ExifTool: unreadable ExifTool version: abc".to_string()),
            ),
        ] {
            let mut pool = fake_pool::<()>(mode, 2);
            let flag = Arc::clone(&ran);
            let ticket = pool.submit(
                key("C:/photos/a.jpg"),
                Box::new(move |_tool: &mut ExifTool| flag.store(true, Ordering::SeqCst)),
            );
            assert_eq!(answer(&mut pool), (ticket, JobAnswer::TooOld(expected)));
        }
        assert!(
            !ran.load(Ordering::SeqCst),
            "a job ran on a too old ExifTool"
        );
    }

    #[test]
    fn a_missing_program_answers_start_failed() {
        let program = std::env::temp_dir().join(format!(
            "pw-metadata-pool-no-exiftool-{}",
            std::process::id()
        ));
        let mut pool: WritePool<()> =
            WritePool::start_with(&program.to_string_lossy(), &[], 2).expect("the pool");
        let ticket = pool.submit(key("C:/photos/a.jpg"), Box::new(|_tool: &mut ExifTool| ()));
        match answer(&mut pool) {
            (got, JobAnswer::StartFailed(error)) => {
                assert_eq!(got, ticket);
                assert!(error.contains("could not start"), "{error}");
            }
            other => panic!("expected StartFailed, got {other:?}"),
        }
    }

    #[test]
    fn a_job_that_panics_is_lost_and_its_thread_takes_the_next_job() {
        let mut pool = fake_pool("slow", 2);
        let panicked: Arc<Mutex<Option<ThreadId>>> = Arc::default();
        let noted = Arc::clone(&panicked);
        let ticket = pool.submit(
            key("C:/photos/a.jpg"),
            Box::new(move |_tool: &mut ExifTool| -> ThreadId {
                *noted.lock().expect("the thread") = Some(std::thread::current().id());
                panic!("the test job panics");
            }),
        );
        match answer(&mut pool) {
            (got, JobAnswer::Lost(text)) => {
                assert_eq!(got, ticket);
                assert!(text.contains("the test job panics"), "{text}");
            }
            other => panic!("expected Lost, got {other:?}"),
        }
        let panicked = panicked
            .lock()
            .expect("the thread")
            .expect("the job noted its thread");
        // Two jobs at once, so each of the two threads takes one.
        let sleeper = || -> Job<ThreadId> {
            Box::new(|_tool: &mut ExifTool| {
                std::thread::sleep(Duration::from_millis(300));
                std::thread::current().id()
            })
        };
        pool.submit(key("C:/photos/b.jpg"), sleeper());
        pool.submit(key("C:/photos/c.jpg"), sleeper());
        let threads: Vec<ThreadId> = answers(&mut pool, 2)
            .into_iter()
            .map(|(_, answer)| match answer {
                JobAnswer::Done(thread) => thread,
                other => panic!("expected Done, got {other:?}"),
            })
            .collect();
        assert!(
            threads.contains(&panicked),
            "{threads:?} without {panicked:?}"
        );
    }

    /// A pool of 2 with two jobs started (each sleeping `ms` and counting itself in `finished`)
    /// and three more given; returns the started tickets and the waiting ones.
    fn two_started_three_waiting(
        pool: &mut WritePool<usize>,
        started: &Arc<AtomicUsize>,
        finished: &Arc<AtomicUsize>,
        ms: u64,
    ) -> (Vec<Ticket>, Vec<Ticket>) {
        let job = |number: usize| -> Job<usize> {
            let started = Arc::clone(started);
            let finished = Arc::clone(finished);
            Box::new(move |_tool: &mut ExifTool| {
                started.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(ms));
                finished.fetch_add(1, Ordering::SeqCst);
                number
            })
        };
        let running: Vec<Ticket> = (0..2)
            .map(|n| pool.submit(key(&format!("C:/photos/run{n}.jpg")), job(n)))
            .collect();
        let deadline = Instant::now() + LIMIT;
        while started.load(Ordering::SeqCst) < 2 {
            assert!(Instant::now() < deadline, "the two jobs did not start");
            std::thread::sleep(Duration::from_millis(5));
        }
        let waiting: Vec<Ticket> = (2..5)
            .map(|n| pool.submit(key(&format!("C:/photos/wait{n}.jpg")), job(n)))
            .collect();
        (running, waiting)
    }

    #[test]
    fn discard_drops_only_the_jobs_not_started() {
        let mut pool = fake_pool("slow", 2);
        let started = Arc::new(AtomicUsize::new(0));
        let finished = Arc::new(AtomicUsize::new(0));
        let (running, waiting) = two_started_three_waiting(&mut pool, &started, &finished, 400);

        pool.discard();

        let got = answers(&mut pool, 5);
        let discarded: Vec<Ticket> = got
            .iter()
            .filter(|(_, answer)| *answer == JobAnswer::Discarded)
            .map(|(ticket, _)| *ticket)
            .collect();
        let done: Vec<Ticket> = got
            .iter()
            .filter(|(_, answer)| matches!(answer, JobAnswer::Done(_)))
            .map(|(ticket, _)| *ticket)
            .collect();
        assert_eq!(discarded, waiting);
        let mut done_sorted = done.clone();
        done_sorted.sort();
        assert_eq!(done_sorted, running);
        assert_eq!(finished.load(Ordering::SeqCst), 2);
        assert_eq!(pool.outstanding(), 0);
    }

    #[test]
    fn the_end_lets_started_jobs_finish_discards_the_rest_and_joins_every_thread() {
        let mut pool = fake_pool("slow", 2);
        let started = Arc::new(AtomicUsize::new(0));
        let finished = Arc::new(AtomicUsize::new(0));
        let (running, waiting) = two_started_three_waiting(&mut pool, &started, &finished, 400);

        // What `Drop` does, so the answers can still be read afterwards.
        pool.shutdown();

        assert!(pool.threads.is_empty());
        assert_eq!(
            finished.load(Ordering::SeqCst),
            2,
            "a started job was cut off"
        );
        assert_eq!(started.load(Ordering::SeqCst), 2, "a waiting job ran");
        // Every thread has ended: only this test and the pool's channel hold the counters.
        assert_eq!(Arc::strong_count(&finished), 1);
        let mut got = Vec::new();
        while let Some(answer) = pool.try_answer() {
            got.push(answer);
        }
        assert_eq!(got.len(), 5);
        for (ticket, answer) in got {
            if waiting.contains(&ticket) {
                assert_eq!(answer, JobAnswer::Discarded);
            } else {
                assert!(running.contains(&ticket));
                assert!(matches!(answer, JobAnswer::Done(_)), "{answer:?}");
            }
        }
        drop(pool);
    }

    #[test]
    fn dropping_the_pool_waits_for_started_jobs() {
        let mut pool = fake_pool("slow", 2);
        let started = Arc::new(AtomicUsize::new(0));
        let finished = Arc::new(AtomicUsize::new(0));
        two_started_three_waiting(&mut pool, &started, &finished, 400);

        drop(pool);

        assert_eq!(finished.load(Ordering::SeqCst), 2);
        assert_eq!(started.load(Ordering::SeqCst), 2);
        assert_eq!(Arc::strong_count(&finished), 1);
    }

    #[test]
    fn the_number_of_processes_is_clamped_to_two_to_four() {
        for (asked, given) in [(0, 2), (1, 2), (2, 2), (3, 3), (4, 4), (9, 4)] {
            let pool: WritePool<()> = fake_pool("slow", asked);
            assert_eq!(pool.processes(), given, "asked for {asked}");
            assert_eq!(pool.threads.len(), given, "asked for {asked}");
        }
    }

    #[test]
    fn a_pool_start_asked_to_fail_is_an_error() {
        let marker = std::env::temp_dir().join(format!(
            "pw-metadata-pool-start-fails-{}",
            std::process::id()
        ));
        std::fs::write(&marker, b"").expect("write the marker");
        let arg = format!("{FAIL_START_WHILE}{}", marker.display());
        let failed = WritePool::<()>::start_with("exiftool", &[arg.as_str()], 2);
        std::fs::remove_file(&marker).expect("remove the marker");
        assert!(failed.is_err());
        assert!(WritePool::<()>::start_with("exiftool", &[arg.as_str()], 2).is_ok());
    }

    #[test]
    fn write_key_is_the_folder_and_the_stem_in_lower_case() {
        assert_eq!(key("C:/photos/a.CR2"), key("C:/photos/A.nef"));
        assert_eq!(key("C:/photos/IMG_1.jpg"), key("C:/photos/img_1.JPG"));
        assert_ne!(key("C:/photos/a.jpg"), key("C:/photos/b.jpg"));
        assert_ne!(key("C:/photos/a.jpg"), key("C:/other/a.jpg"));
        assert_eq!(
            key("C:/photos/a.jpg"),
            WriteKey {
                folder: PathBuf::from("C:/photos"),
                stem: "a".to_string(),
            }
        );
    }
}
