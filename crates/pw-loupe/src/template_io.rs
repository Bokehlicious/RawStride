//! Template files read and saved on a thread of their own (ADR 0017 U13, part U13c; T292).
//!
//! [`ExifTool::read_template_file`] and [`ExifTool::write_template_file`] run ExifTool, so they
//! cannot run on the window's thread. [`TemplateIo`] owns one thread with **its own ExifTool
//! process**, started at the first request with `-config` and the file
//! [`write_exiftool_config`] makes, so the + boxes (`rawstride:AddTo`) can be saved. The metadata
//! worker's and the import's processes are never started with the config. A config that cannot
//! be used only stops saving: ExifTool is then started without it, loading works as before, and
//! every save answers [`SaveError::NoConfig`] until ExifTool is started again.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use pw_metadata::template_file::{
    TemplateFile, TemplateFileError, TemplateRead, write_exiftool_config,
};
use pw_metadata::{ExifTool, exiftool_program};

/// Why a template could not be saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// [`ExifTool::write_template_file`] failed, or ExifTool could not be started.
    File(TemplateFileError),
    /// ExifTool cannot be given its settings file for templates; holds the reason (error detail,
    /// made by `no_config_reason`).
    NoConfig(String),
}

/// An answer of the template thread, to the request with the same `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateEvent {
    /// The file at `path` was read, or could not be.
    Loaded {
        id: u64,
        path: PathBuf,
        result: Result<TemplateRead, TemplateFileError>,
    },
    /// The template was saved at `path`, or could not be.
    Saved {
        id: u64,
        path: PathBuf,
        result: Result<(), SaveError>,
    },
}

/// The template thread has ended, so a request cannot be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateIoStopped;

enum Request {
    Load {
        id: u64,
        path: PathBuf,
    },
    Save {
        id: u64,
        path: PathBuf,
        template: TemplateFile,
    },
}

/// A thread that loads and saves template files with one ExifTool of its own. Requests are
/// answered in order. Dropping it closes the request channel without waiting; the thread then
/// ends, and its ExifTool with it.
pub struct TemplateIo {
    requests: Option<Sender<Request>>,
    events: Receiver<TemplateEvent>,
    thread: Option<JoinHandle<()>>,
    next_id: u64,
}

impl std::fmt::Debug for TemplateIo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TemplateIo")
            .field("thread_running", &self.thread.is_some())
            .field("next_id", &self.next_id)
            .finish_non_exhaustive()
    }
}

impl TemplateIo {
    /// Starts the thread and returns at once. ExifTool is `program` (or [`exiftool_program`] when
    /// `None`), started at the first request; its config is written in `config_folder`
    /// (`%LOCALAPPDATA%\RawStride`; `None` when there is none, and then no template is saved).
    pub fn start(
        program: Option<String>,
        config_folder: Option<PathBuf>,
    ) -> std::io::Result<TemplateIo> {
        let (request_sender, request_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = mpsc::channel();
        let program = program.unwrap_or_else(exiftool_program);
        let thread = std::thread::Builder::new()
            .name("pw-template-io".into())
            .spawn(move || {
                let mut session = Session {
                    program,
                    config_folder,
                    exiftool: None,
                    config: Err(NoConfig::NoFolder),
                };
                for request in request_receiver {
                    let event = session.answer(request);
                    if event_sender.send(event).is_err() {
                        // The receiver is gone: the answer is dropped.
                        break;
                    }
                }
            })?;
        Ok(TemplateIo {
            requests: Some(request_sender),
            events: event_receiver,
            thread: Some(thread),
            next_id: 0,
        })
    }

    /// Asks for the template file at `path` to be read; the answer is a
    /// [`TemplateEvent::Loaded`] with the returned id.
    pub fn load(&mut self, path: PathBuf) -> Result<u64, TemplateIoStopped> {
        let id = self.next_id.wrapping_add(1);
        self.send(Request::Load { id, path })?;
        self.next_id = id;
        Ok(id)
    }

    /// Asks for `template` to be saved at `path`; the answer is a [`TemplateEvent::Saved`] with
    /// the returned id.
    pub fn save(
        &mut self,
        path: PathBuf,
        template: TemplateFile,
    ) -> Result<u64, TemplateIoStopped> {
        let id = self.next_id.wrapping_add(1);
        self.send(Request::Save { id, path, template })?;
        self.next_id = id;
        Ok(id)
    }

    /// The next answer, without waiting.
    pub fn try_recv(&self) -> Option<TemplateEvent> {
        self.events.try_recv().ok()
    }

    fn send(&self, request: Request) -> Result<(), TemplateIoStopped> {
        let requests = self.requests.as_ref().ok_or(TemplateIoStopped)?;
        requests.send(request).map_err(|_| TemplateIoStopped)
    }

    /// The thread's handle, so a test can see it end.
    #[cfg(test)]
    fn take_thread(&mut self) -> Option<JoinHandle<()>> {
        self.thread.take()
    }
}

impl Drop for TemplateIo {
    fn drop(&mut self) {
        self.requests.take();
        // Do not wait for ExifTool to stop on the window's thread.
        self.thread.take();
    }
}

/// Why ExifTool cannot be given the config.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NoConfig {
    /// No config folder (`LOCALAPPDATA` is not set).
    NoFolder,
    /// The config could not be written in this folder.
    NotWritten { folder: PathBuf, error: String },
    /// The config's path cannot be passed on ExifTool's command line.
    NotAscii(PathBuf),
}

/// The reason a save answers [`SaveError::NoConfig`] with: one English sentence per case, with
/// the path and the error's text filled in. It is error detail, put into the template window's
/// text for a save that failed for it.
fn no_config_reason(no_config: &NoConfig) -> String {
    match no_config {
        NoConfig::NoFolder => "LOCALAPPDATA is not set".to_string(),
        NoConfig::NotWritten { folder, error } => {
            format!("{} could not be written: {error}", folder.display())
        }
        NoConfig::NotAscii(path) => {
            format!("{} has letters ExifTool cannot be given", path.display())
        }
    }
}

/// The text passed after `-config` for the config at `path`, which must exist: ExifTool's command
/// line carries only the letters of the system's code page on Windows. The path itself when it is
/// all ASCII; otherwise, on Windows, its short (8.3) form when that is all ASCII; otherwise
/// `None`.
fn config_argument(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    if text.is_ascii() {
        return Some(text.to_string());
    }
    short_path(path).filter(|short| short.is_ascii())
}

/// The short (8.3) form of `path`, asked of the file system (`GetShortPathNameW`).
#[cfg(windows)]
fn short_path(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    let long: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `long` is NUL-terminated and lives through the call; with no buffer the function
    // only returns the length needed, including the NUL.
    let needed = unsafe { GetShortPathNameW(long.as_ptr(), std::ptr::null_mut(), 0) };
    if needed == 0 {
        return None;
    }
    let mut short = vec![0u16; usize::try_from(needed).ok()?];
    // SAFETY: `long` is NUL-terminated; `short` holds `needed` characters, the size given.
    let written = unsafe { GetShortPathNameW(long.as_ptr(), short.as_mut_ptr(), needed) };
    // On success the length without the NUL, less than the buffer; otherwise 0 or the size
    // needed (the path changed between the calls).
    if written == 0 || written >= needed {
        return None;
    }
    let short = short.get(..usize::try_from(written).ok()?)?;
    String::from_utf16(short).ok()
}

#[cfg(not(windows))]
fn short_path(_path: &Path) -> Option<String> {
    None
}

/// The text of an error starting ExifTool: the inner text of `Error::ExifTool`, so the answer
/// does not say "ExifTool:" twice.
fn start_error_text(error: pw_metadata::Error) -> String {
    match error {
        pw_metadata::Error::ExifTool(text) => text,
        other => other.to_string(),
    }
}

/// The template thread's state.
struct Session {
    program: String,
    config_folder: Option<PathBuf>,
    /// `None` until a start succeeds, and after a broken process is dropped.
    exiftool: Option<ExifTool>,
    /// The `-config` argument, or why there is none; made again before each start of ExifTool.
    config: Result<String, NoConfig>,
}

impl Session {
    fn answer(&mut self, request: Request) -> TemplateEvent {
        if self.exiftool.as_ref().is_some_and(ExifTool::is_broken) {
            self.exiftool = None;
        }
        if self.exiftool.is_none() {
            self.config = self.prepare_config();
        }
        match request {
            Request::Load { id, path } => {
                let result = self
                    .exiftool()
                    .and_then(|exiftool| exiftool.read_template_file(&path));
                TemplateEvent::Loaded { id, path, result }
            }
            Request::Save { id, path, template } => {
                let result = match &self.config {
                    Err(no_config) => Err(SaveError::NoConfig(no_config_reason(no_config))),
                    Ok(_) => self
                        .exiftool()
                        .and_then(|exiftool| exiftool.write_template_file(&path, &template))
                        .map_err(SaveError::File),
                };
                TemplateEvent::Saved { id, path, result }
            }
        }
    }

    /// Writes the config and gives the argument for it, or why it cannot be used.
    fn prepare_config(&self) -> Result<String, NoConfig> {
        let folder = self.config_folder.as_ref().ok_or(NoConfig::NoFolder)?;
        let path = write_exiftool_config(folder).map_err(|error| NoConfig::NotWritten {
            folder: folder.clone(),
            error: error.to_string(),
        })?;
        config_argument(&path).ok_or(NoConfig::NotAscii(path))
    }

    /// The running ExifTool, started now (with `-config` when the config can be used) when there
    /// is none.
    fn exiftool(&mut self) -> Result<&mut ExifTool, TemplateFileError> {
        let exiftool = match self.exiftool.take() {
            Some(exiftool) => exiftool,
            None => {
                let pre_args: Vec<&str> = match &self.config {
                    Ok(argument) => vec!["-config", argument.as_str()],
                    Err(_) => Vec::new(),
                };
                ExifTool::start_with(&self.program, &pre_args)
                    .map_err(|error| TemplateFileError::ExifTool(start_error_text(error)))?
            }
        };
        Ok(self.exiftool.insert(exiftool))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_metadata::value::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    /// A temporary folder, removed when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> TempDir {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pw-loupe-template-io-{tag}-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path)
                .unwrap_or_else(|error| panic!("could not create {}: {error}", path.display()));
            TempDir(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// ExifTool on PATH for the tests below, or a failure with an explanation (docs/TESTING.md
    /// §4.5).
    fn required_exiftool() {
        ExifTool::start()
            .and_then(|mut tool| tool.execute(&["-ver".to_string()]))
            .unwrap_or_else(|error| {
                panic!(
                    "ExifTool is required for this test but `exiftool` on PATH does not answer: \
                     {error}. Install ExifTool (docs/TESTING.md §4.5)."
                )
            });
    }

    /// The next answer, waited for up to 30 seconds.
    fn next_event(io: &TemplateIo) -> TemplateEvent {
        let start = Instant::now();
        loop {
            if let Some(event) = io.try_recv() {
                return event;
            }
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "no answer from the template thread"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Headline `{date}` and Keywords `a, b` with + checked.
    fn headline_and_keywords() -> TemplateFile {
        TemplateFile::new(
            vec![
                (
                    "photoshop:Headline".to_string(),
                    Value::Text("{date}".to_string()),
                ),
                (
                    "dc:subject".to_string(),
                    Value::List(vec!["a".to_string(), "b".to_string()]),
                ),
            ],
            vec!["dc:subject".to_string()],
        )
    }

    /// Saves `template` at `path` with `io` and checks that the save succeeded.
    fn save_ok(io: &mut TemplateIo, path: &Path, template: &TemplateFile) {
        let id = io.save(path.to_path_buf(), template.clone()).unwrap();
        assert_eq!(
            next_event(io),
            TemplateEvent::Saved {
                id,
                path: path.to_path_buf(),
                result: Ok(())
            }
        );
    }

    /// Loads `path` with `io` and returns the template read.
    fn load_ok(io: &mut TemplateIo, path: &Path) -> TemplateFile {
        let id = io.load(path.to_path_buf()).unwrap();
        match next_event(io) {
            TemplateEvent::Loaded {
                id: answered,
                path: loaded,
                result: Ok(read),
            } => {
                assert_eq!((answered, loaded.as_path()), (id, path));
                read.template
            }
            other => panic!("unexpected answer: {other:?}"),
        }
    }

    #[test]
    fn config_argument_gives_an_ascii_path_itself() {
        let path = Path::new(r"C:\rawstride-test\rawstride-exiftool.config");
        assert_eq!(
            config_argument(path),
            Some(path.to_str().unwrap().to_string())
        );
    }

    /// A path with a letter outside ASCII gives `None` or its short form. Both are accepted: the
    /// test machine's file system may have 8.3 names turned off (and off Windows there are none).
    #[test]
    fn config_argument_gives_a_short_ascii_form_or_none() {
        let dir = TempDir::new("short");
        let folder = dir.0.join("Mall Å");
        std::fs::create_dir_all(&folder).unwrap();
        match config_argument(&folder) {
            None => {}
            Some(text) => {
                assert!(text.is_ascii(), "{text}");
                assert_eq!(
                    std::fs::canonicalize(&text).unwrap(),
                    std::fs::canonicalize(&folder).unwrap()
                );
            }
        }
    }

    #[test]
    fn without_exiftool_a_load_and_a_save_answer_its_start_error() {
        let dir = TempDir::new("missing");
        let mut io =
            TemplateIo::start(Some("missing-exiftool".to_string()), Some(dir.0.clone())).unwrap();
        let path = dir.0.join("mall.xmp");

        io.load(path.clone()).unwrap();
        match next_event(&io) {
            TemplateEvent::Loaded {
                result: Err(TemplateFileError::ExifTool(text)),
                ..
            } => {
                assert!(text.contains("missing-exiftool"), "{text}");
                assert!(!text.starts_with("ExifTool:"), "{text}");
            }
            other => panic!("unexpected answer: {other:?}"),
        }

        io.save(path.clone(), headline_and_keywords()).unwrap();
        match next_event(&io) {
            TemplateEvent::Saved {
                result: Err(SaveError::File(TemplateFileError::ExifTool(text))),
                ..
            } => assert!(text.contains("missing-exiftool"), "{text}"),
            other => panic!("unexpected answer: {other:?}"),
        }
        assert!(!path.exists());
    }

    #[test]
    fn a_template_is_saved_and_loaded_back_with_its_add_boxes() {
        required_exiftool();
        let dir = TempDir::new("save");
        let mut io = TemplateIo::start(None, Some(dir.0.clone())).unwrap();
        let path = dir.0.join("mall.xmp");
        let template = headline_and_keywords();
        assert_eq!(template.add, ["dc:subject"]);

        save_ok(&mut io, &path, &template);
        assert!(dir.0.join("rawstride-exiftool.config").is_file());
        assert_eq!(load_ok(&mut io, &path), template);
    }

    #[test]
    fn a_missing_file_is_not_found_and_two_loads_answer_in_order() {
        required_exiftool();
        let dir = TempDir::new("order");
        let mut io = TemplateIo::start(None, Some(dir.0.clone())).unwrap();
        let saved = dir.0.join("mall.xmp");
        save_ok(&mut io, &saved, &headline_and_keywords());

        let missing = dir.0.join("saknas.xmp");
        let first = io.load(missing.clone()).unwrap();
        let second = io.load(saved.clone()).unwrap();
        assert!(first < second);
        assert_eq!(
            next_event(&io),
            TemplateEvent::Loaded {
                id: first,
                path: missing.clone(),
                result: Err(TemplateFileError::NotFound(missing))
            }
        );
        match next_event(&io) {
            TemplateEvent::Loaded {
                id,
                result: Ok(read),
                ..
            } => {
                assert_eq!(id, second);
                assert_eq!(read.template, headline_and_keywords());
            }
            other => panic!("unexpected answer: {other:?}"),
        }
    }

    #[test]
    fn without_a_config_folder_saves_answer_no_config_and_loads_work() {
        required_exiftool();
        let dir = TempDir::new("noconfig");
        let saved = dir.0.join("mall.xmp");
        {
            let mut io = TemplateIo::start(None, Some(dir.0.join("config"))).unwrap();
            save_ok(&mut io, &saved, &headline_and_keywords());
        }

        let mut io = TemplateIo::start(None, None).unwrap();
        let other = dir.0.join("annan.xmp");
        let id = io.save(other.clone(), headline_and_keywords()).unwrap();
        assert_eq!(
            next_event(&io),
            TemplateEvent::Saved {
                id,
                path: other.clone(),
                result: Err(SaveError::NoConfig(no_config_reason(&NoConfig::NoFolder)))
            }
        );
        assert!(!other.exists());
        assert_eq!(load_ok(&mut io, &saved), headline_and_keywords());
        // Still no config after ExifTool was started for the load.
        let id = io.save(other.clone(), headline_and_keywords()).unwrap();
        assert!(matches!(
            next_event(&io),
            TemplateEvent::Saved { id: answered, result: Err(SaveError::NoConfig(_)), .. }
                if answered == id
        ));
        assert!(!other.exists());
    }

    #[test]
    fn the_no_config_reasons_name_the_path_and_the_error() {
        assert_eq!(
            no_config_reason(&NoConfig::NoFolder),
            "LOCALAPPDATA is not set"
        );
        let folder = PathBuf::from(r"C:\Users\…\AppData\Local\RawStride");
        assert_eq!(
            no_config_reason(&NoConfig::NotWritten {
                folder: folder.clone(),
                error: "access denied".to_string()
            }),
            format!("{} could not be written: access denied", folder.display())
        );
        assert_eq!(
            no_config_reason(&NoConfig::NotAscii(folder.clone())),
            format!("{} has letters ExifTool cannot be given", folder.display())
        );
    }

    #[test]
    fn the_thread_ends_after_the_template_io_is_dropped() {
        let mut io = TemplateIo::start(Some("missing-exiftool".to_string()), None).unwrap();
        let thread = io.take_thread().unwrap();
        drop(io);
        let start = Instant::now();
        while !thread.is_finished() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "the template thread did not end"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
