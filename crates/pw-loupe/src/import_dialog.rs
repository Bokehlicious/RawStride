use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::import_text::{ImportText, import_text};
use crate::{Field, FieldError, ImportDone, ImportEvent, ImportForm, Preview, RequestId};

const PREVIEW_DELAY: Duration = Duration::from_millis(300);

/// The request the UI should submit to the import worker.
#[derive(Clone, Debug)]
pub enum DialogRequest {
    Preview { form: ImportForm, rescan: bool },
    Run { form: ImportForm },
}

/// The current import-dialog state.
#[derive(Clone, Debug)]
pub enum ImportDialogState {
    Editing {
        preview: Option<Preview>,
        error: Option<FieldError>,
    },
    Previewing,
    Running {
        progress: Option<pw_ingest::Progress>,
    },
    Ejecting {
        source: PathBuf,
    },
    Done {
        done: ImportDone,
    },
}

/// Plain-Rust import form state and transitions; file access and worker submission stay outside.
#[derive(Clone, Debug)]
pub struct ImportDialog {
    form: ImportForm,
    state: ImportDialogState,
    changed_at: Option<Instant>,
    source_changed: bool,
    request_id: Option<RequestId>,
    /// The last successful preview, kept while the form is edited so the list can still be
    /// drawn (dimmed) until the next preview comes (T276).
    shown: Option<Preview>,
}

/// True when `a` and `b` give the same preview: every field of `ImportForm` the worker's
/// `preview` reads is equal. `iptc` and `eject` are left out because the preview does not read
/// them; a field added to `ImportForm` must be placed on one side or the other here.
fn preview_fields_equal(a: &ImportForm, b: &ImportForm) -> bool {
    let ImportForm {
        source,
        dest,
        backup_on,
        backup,
        folder,
        name,
        job,
        first_seq,
        iptc: _,
        eject: _,
        incremental,
    } = a;
    *source == b.source
        && *dest == b.dest
        && *backup_on == b.backup_on
        && *backup == b.backup
        && *folder == b.folder
        && *name == b.name
        && *job == b.job
        && *first_seq == b.first_seq
        && *incremental == b.incremental
}

impl ImportDialog {
    /// Creates a dialog and schedules its initial preview.
    pub fn new(form: ImportForm) -> Self {
        Self {
            form,
            state: ImportDialogState::Editing {
                preview: None,
                error: None,
            },
            changed_at: Some(Instant::now()),
            source_changed: true,
            request_id: None,
            shown: None,
        }
    }

    pub fn form(&self) -> &ImportForm {
        &self.form
    }

    pub fn state(&self) -> &ImportDialogState {
        &self.state
    }

    /// Updates form fields and restarts the preview debounce.
    ///
    /// In `Editing` and `Previewing`, an edit that changes only the IPTC template or Eject
    /// updates the form and nothing else (T276): the worker's `preview` reads neither, so the
    /// preview shown or on its way stays current. Every other field restarts the preview, as
    /// does an edit that changes nothing. Leaving `Done` clears the kept preview, which no
    /// longer matches the card after an import.
    pub fn edit(&mut self, change: impl FnOnce(&mut ImportForm), now: Instant) {
        let previous = self.form.clone();
        change(&mut self.form);
        if matches!(
            self.state,
            ImportDialogState::Editing { .. } | ImportDialogState::Previewing
        ) && self.form != previous
            && preview_fields_equal(&self.form, &previous)
        {
            return;
        }
        self.source_changed |= self.form.source != previous.source;
        self.changed_at = Some(now);
        if matches!(
            self.state,
            ImportDialogState::Running { .. } | ImportDialogState::Ejecting { .. }
        ) {
            return;
        }
        if matches!(self.state, ImportDialogState::Done { .. }) {
            self.shown = None;
        }
        self.request_id = None;
        self.state = ImportDialogState::Editing {
            preview: None,
            error: None,
        };
    }

    /// Returns a debounced preview request with the current form.
    pub fn tick(&mut self, now: Instant) -> Option<DialogRequest> {
        let changed_at = self.changed_at?;
        if now.saturating_duration_since(changed_at) < PREVIEW_DELAY
            || matches!(
                self.state,
                ImportDialogState::Running { .. } | ImportDialogState::Ejecting { .. }
            )
            || !matches!(self.state, ImportDialogState::Editing { .. })
        {
            return None;
        }

        self.changed_at = None;
        self.state = ImportDialogState::Previewing;
        Some(DialogRequest::Preview {
            form: self.form.clone(),
            rescan: self.source_changed,
        })
    }

    /// Starts a run only when the current preview contains files to copy, or when the run only
    /// marks the card as imported (`marks_only`, T166).
    pub fn start(&mut self) -> Option<DialogRequest> {
        if !self.can_start() {
            return None;
        }
        self.changed_at = None;
        self.state = ImportDialogState::Running { progress: None };
        Some(DialogRequest::Run {
            form: self.form.clone(),
        })
    }

    /// Records the worker request ID for the request most recently submitted.
    pub fn sent(&mut self, id: RequestId) {
        self.request_id = Some(id);
    }

    /// Applies an event only when it belongs to the currently tracked request.
    pub fn apply(&mut self, event: &ImportEvent) {
        let Some(id) = self.request_id else {
            return;
        };
        match event {
            ImportEvent::Previewed {
                id: event_id,
                result,
            } if *event_id == id && matches!(self.state, ImportDialogState::Previewing) => {
                self.request_id = None;
                match result {
                    Ok(preview) => {
                        self.source_changed = false;
                        self.shown = Some(preview.clone());
                        self.state = ImportDialogState::Editing {
                            preview: Some(preview.clone()),
                            error: None,
                        };
                    }
                    Err(error) => {
                        self.shown = None;
                        self.state = ImportDialogState::Editing {
                            preview: None,
                            error: Some(error.clone()),
                        };
                    }
                }
            }
            ImportEvent::Progress {
                id: event_id,
                progress,
            } if *event_id == id => {
                if let ImportDialogState::Running {
                    progress: current_progress,
                    ..
                } = &mut self.state
                {
                    *current_progress = Some(progress.clone());
                }
            }
            ImportEvent::Ejecting { id: event_id }
                if *event_id == id && matches!(self.state, ImportDialogState::Running { .. }) =>
            {
                self.state = ImportDialogState::Ejecting {
                    source: PathBuf::from(&self.form.source),
                };
            }
            ImportEvent::Done { id: event_id, done }
                if *event_id == id
                    && matches!(
                        self.state,
                        ImportDialogState::Running { .. } | ImportDialogState::Ejecting { .. }
                    ) =>
            {
                self.request_id = None;
                self.state = ImportDialogState::Done { done: done.clone() };
            }
            _ => {}
        }
    }

    /// Replaces a pending preview or run with a visible stopped-worker error.
    pub fn disconnected(&mut self) {
        match self.state {
            ImportDialogState::Previewing => {
                self.request_id = None;
                self.shown = None;
                self.state = ImportDialogState::Editing {
                    preview: None,
                    error: Some(FieldError {
                        field: Field::Source,
                        message: import_text(ImportText::StoppedMessage).to_string(),
                    }),
                };
            }
            ImportDialogState::Running { .. } | ImportDialogState::Ejecting { .. } => {
                self.request_id = None;
                self.state = ImportDialogState::Done {
                    done: ImportDone {
                        error: Some(import_text(ImportText::StoppedMessage).to_string()),
                        ..ImportDone::default()
                    },
                };
            }
            _ => {}
        }
    }

    pub fn cancel(&self) -> bool {
        matches!(
            self.state,
            ImportDialogState::Running { .. } | ImportDialogState::Ejecting { .. }
        )
    }

    pub fn can_close(&self) -> bool {
        !self.cancel()
    }

    /// Returns from the result view and schedules a fresh preview.
    pub fn back_to_editing(&mut self) {
        if matches!(self.state, ImportDialogState::Done { .. }) {
            self.state = ImportDialogState::Editing {
                preview: None,
                error: None,
            };
            self.changed_at = Some(Instant::now());
            self.source_changed = true;
            self.request_id = None;
            self.shown = None;
        }
    }

    /// The last successful preview, for drawing the list: kept while the state is `Editing`
    /// without a field error or `Previewing`, so the list stays until the next preview comes
    /// (T276); `None` with a field error, while running or ejecting, and in `Done`.
    pub fn shown_preview(&self) -> Option<&Preview> {
        match self.state {
            ImportDialogState::Editing { error: None, .. } | ImportDialogState::Previewing => {
                self.shown.as_ref()
            }
            ImportDialogState::Editing { error: Some(_), .. }
            | ImportDialogState::Running { .. }
            | ImportDialogState::Ejecting { .. }
            | ImportDialogState::Done { .. } => None,
        }
    }

    /// True when the preview shown belongs to the form as it is now (the same condition as
    /// `can_start` and `marks_only`); the list is drawn dimmed otherwise.
    pub fn preview_is_current(&self) -> bool {
        matches!(
            self.state,
            ImportDialogState::Editing {
                preview: Some(_),
                error: None
            }
        )
    }

    pub fn can_start(&self) -> bool {
        let ImportDialogState::Editing {
            preview: Some(preview),
            error: None,
        } = &self.state
        else {
            return false;
        };
        preview.files_to_copy > 0 || self.marks_only()
    }

    /// True when a run would copy nothing but add the files at the destination already to the
    /// register (T166, ADR 0005 choice 6): "copy new images only" is on, the current preview has
    /// nothing to copy, at least one file is at the destination already, and the card's register
    /// can be used (no register note). A write-protected card is not seen here; its run ends with
    /// the usual `register_error`.
    pub fn marks_only(&self) -> bool {
        let ImportDialogState::Editing {
            preview: Some(preview),
            error: None,
        } = &self.state
        else {
            return false;
        };
        self.form.incremental
            && preview.files_to_copy == 0
            && preview.present > 0
            && preview.register_note.is_none()
    }

    pub fn rescan(&mut self, now: Instant) {
        if matches!(
            self.state,
            ImportDialogState::Running { .. } | ImportDialogState::Ejecting { .. }
        ) {
            return;
        }
        self.changed_at = Some(now);
        self.source_changed = true;
        self.request_id = None;
        self.state = ImportDialogState::Editing {
            preview: None,
            error: None,
        };
    }
}

/// How often the open dialog asks the import worker for the cards again (T161, ADR 0010
/// "Source"): a card put into a reader that is already plugged in does not change the drive
/// letters, so the dialog asks on a timer.
pub const CARD_REFRESH_INTERVAL: Duration = Duration::from_secs(2);

/// When the open import dialog asks for the cards again (T161). Plain Rust; the shell makes the
/// ask and reports the answer.
#[derive(Debug)]
pub struct CardRefresh {
    asked_at: Option<Instant>,
    waiting: bool,
}

impl CardRefresh {
    /// Counts the ask made when the dialog opens.
    pub fn new(now: Instant) -> Self {
        Self {
            asked_at: Some(now),
            waiting: true,
        }
    }

    /// True when the dialog is idle, no ask is waiting for its answer, and at least
    /// `CARD_REFRESH_INTERVAL` has passed since the last ask.
    pub fn due(&self, now: Instant, idle: bool) -> bool {
        if !idle || self.waiting {
            return false;
        }
        self.asked_at
            .is_none_or(|asked_at| now.saturating_duration_since(asked_at) >= CARD_REFRESH_INTERVAL)
    }

    /// An ask was sent to the worker.
    pub fn asked(&mut self, now: Instant) {
        self.asked_at = Some(now);
        self.waiting = true;
    }

    /// The worker answered, or the ask could not be sent.
    pub fn answered(&mut self) {
        self.waiting = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    struct TempDirs(PathBuf);

    impl TempDirs {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pw-loupe-import-dialog-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path)
                .unwrap_or_else(|error| panic!("could not create {}: {error}", path.display()));
            Self(path)
        }

        fn form(&self) -> ImportForm {
            ImportForm {
                source: self.0.join("source").to_string_lossy().into_owned(),
                dest: self.0.join("destination").to_string_lossy().into_owned(),
                ..ImportForm::default()
            }
        }

        fn create_source(&self) {
            std::fs::create_dir_all(self.0.join("source"))
                .unwrap_or_else(|error| panic!("could not create source directory: {error}"));
        }
    }

    impl Drop for TempDirs {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn preview(files_to_copy: usize) -> Preview {
        Preview {
            shots: 1,
            files_to_copy,
            bytes_to_copy: 1,
            present: 0,
            renamed: 0,
            no_exif: 0,
            items_total: files_to_copy,
            lines: Vec::new(),
            earlier: 0,
            register_note: None,
        }
    }

    fn schedule_preview(dialog: &mut ImportDialog, now: Instant) -> DialogRequest {
        dialog.edit(|_| {}, now);
        dialog
            .tick(now + PREVIEW_DELAY)
            .expect("valid preview request")
    }

    #[test]
    fn preview_waits_for_debounce_then_returns_once() {
        let dirs = TempDirs::new();
        dirs.create_source();
        let mut dialog = ImportDialog::new(dirs.form());
        let now = Instant::now();

        dialog.edit(|_| {}, now);
        assert!(dialog.tick(now + Duration::from_millis(299)).is_none());
        assert!(matches!(
            dialog.tick(now + PREVIEW_DELAY),
            Some(DialogRequest::Preview { .. })
        ));
        assert!(
            dialog
                .tick(now + PREVIEW_DELAY + Duration::from_secs(1))
                .is_none()
        );
    }

    #[test]
    fn invalid_form_sends_preview_request_and_shows_worker_error() {
        let dirs = TempDirs::new();
        let mut form = dirs.form();
        form.source.clear();
        let mut dialog = ImportDialog::new(form);
        let now = Instant::now();

        dialog.edit(|_| {}, now);
        assert!(matches!(
            dialog.tick(now + PREVIEW_DELAY),
            Some(DialogRequest::Preview { .. })
        ));
        dialog.sent(13);
        let error = FieldError {
            field: Field::Source,
            message: "Choose a source.".to_string(),
        };
        dialog.apply(&ImportEvent::Previewed {
            id: 13,
            result: Err(error.clone()),
        });
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                error: Some(current),
                ..
            } if current == &error
        ));
    }

    #[test]
    fn preview_and_run_requests_do_not_check_source_on_the_dialog_thread() {
        let dirs = TempDirs::new();
        let form = dirs.form();
        let mut dialog = ImportDialog::new(form.clone());
        let now = Instant::now();

        assert!(matches!(
            dialog.tick(now + PREVIEW_DELAY),
            Some(DialogRequest::Preview { form: request_form, .. }) if request_form == form
        ));
        dialog.sent(14);
        dialog.apply(&ImportEvent::Previewed {
            id: 14,
            result: Ok(preview(1)),
        });

        assert!(matches!(
            dialog.start(),
            Some(DialogRequest::Run { form: request_form }) if request_form == form
        ));
    }

    #[test]
    fn source_changes_rescan_but_template_changes_reuse_the_scan() {
        let dirs = TempDirs::new();
        dirs.create_source();
        let mut dialog = ImportDialog::new(dirs.form());
        let now = Instant::now();

        assert!(matches!(
            schedule_preview(&mut dialog, now),
            DialogRequest::Preview { rescan: true, .. }
        ));
        dialog.sent(1);
        dialog.apply(&ImportEvent::Previewed {
            id: 1,
            result: Ok(preview(1)),
        });

        dialog.edit(
            |form| form.folder = "{year}".to_string(),
            now + Duration::from_secs(1),
        );
        assert!(matches!(
            dialog.tick(now + Duration::from_secs(1) + PREVIEW_DELAY),
            Some(DialogRequest::Preview { rescan: false, .. })
        ));

        dialog.edit(
            |form| form.source = dirs.0.join("other-source").to_string_lossy().into_owned(),
            now + Duration::from_secs(2),
        );
        std::fs::create_dir_all(dirs.0.join("other-source")).unwrap();
        assert!(matches!(
            dialog.tick(now + Duration::from_secs(2) + PREVIEW_DELAY),
            Some(DialogRequest::Preview { rescan: true, .. })
        ));
    }

    #[test]
    fn preview_event_from_an_invalidated_request_is_ignored() {
        let dirs = TempDirs::new();
        dirs.create_source();
        let mut dialog = ImportDialog::new(dirs.form());
        let now = Instant::now();
        let _ = schedule_preview(&mut dialog, now);
        dialog.sent(3);

        dialog.edit(
            |form| form.folder = "{year}".to_string(),
            now + PREVIEW_DELAY,
        );
        dialog.apply(&ImportEvent::Previewed {
            id: 3,
            result: Ok(preview(1)),
        });

        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                preview: None,
                error: None
            }
        ));
    }

    #[test]
    fn start_requires_a_nonempty_current_preview() {
        let dirs = TempDirs::new();
        dirs.create_source();
        let mut dialog = ImportDialog::new(dirs.form());
        assert!(dialog.start().is_none());

        let now = Instant::now();
        let _ = schedule_preview(&mut dialog, now);
        assert!(dialog.start().is_none());
        dialog.sent(4);
        dialog.apply(&ImportEvent::Previewed {
            id: 4,
            result: Ok(preview(0)),
        });
        assert!(dialog.start().is_none());

        let _ = schedule_preview(&mut dialog, now + Duration::from_secs(1));
        assert!(dialog.start().is_none());
    }

    #[test]
    fn progress_and_done_update_the_state() {
        let dirs = TempDirs::new();
        dirs.create_source();
        let mut dialog = ImportDialog::new(dirs.form());
        let now = Instant::now();
        let _ = schedule_preview(&mut dialog, now);
        dialog.sent(5);
        dialog.apply(&ImportEvent::Previewed {
            id: 5,
            result: Ok(preview(2)),
        });

        let request = dialog.start().expect("nonempty preview starts");
        assert!(matches!(request, DialogRequest::Run { .. }));
        dialog.sent(6);
        dialog.edit(|form| form.folder = "{year}".to_string(), Instant::now());
        let progress = pw_ingest::Progress {
            files_done: 1,
            files_total: 2,
            ..pw_ingest::Progress::default()
        };
        dialog.apply(&ImportEvent::Progress {
            id: 6,
            progress: progress.clone(),
        });
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Running {
                progress: Some(current),
                ..
            } if current.files_done == 1
        ));

        let done = ImportDone {
            copied: 2,
            folders: vec![
                ("destination/largest".to_string(), 2),
                ("destination/small".to_string(), 1),
            ],
            new_folders: vec![
                PathBuf::from("destination/small"),
                PathBuf::from("destination/largest"),
            ],
            ..ImportDone::default()
        };
        dialog.apply(&ImportEvent::Done {
            id: 6,
            done: done.clone(),
        });
        assert!(
            matches!(dialog.state(), ImportDialogState::Done { done: value } if value == &done)
        );
        assert!(!dialog.cancel());
        assert!(dialog.can_close());
    }

    #[test]
    fn ejecting_event_updates_state_until_done() {
        let dirs = TempDirs::new();
        dirs.create_source();
        let mut form = dirs.form();
        form.eject = true;
        let mut dialog = ImportDialog::new(form.clone());
        let now = Instant::now();
        let _ = schedule_preview(&mut dialog, now);
        dialog.sent(21);
        dialog.apply(&ImportEvent::Previewed {
            id: 21,
            result: Ok(preview(1)),
        });
        assert!(dialog.start().is_some());
        dialog.sent(22);

        dialog.apply(&ImportEvent::Ejecting { id: 22 });

        assert!(matches!(
            dialog.state(),
            ImportDialogState::Ejecting { source } if source == Path::new(&form.source)
        ));
        assert!(dialog.cancel());
        assert!(!dialog.can_close());
        dialog.apply(&ImportEvent::Ejecting { id: 21 });
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Ejecting { source } if source == Path::new(&form.source)
        ));

        let done = ImportDone {
            eject: Some("E: has been ejected and can be removed.".to_string()),
            ..ImportDone::default()
        };
        dialog.apply(&ImportEvent::Done {
            id: 22,
            done: done.clone(),
        });

        assert!(matches!(
            dialog.state(),
            ImportDialogState::Done { done: current } if current == &done
        ));
        assert!(dialog.can_close());
    }

    #[test]
    fn disconnect_while_previewing_shows_source_error() {
        let dirs = TempDirs::new();
        dirs.create_source();
        let mut dialog = ImportDialog::new(dirs.form());
        let _ = schedule_preview(&mut dialog, Instant::now());
        dialog.sent(7);
        dialog.disconnected();

        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                error: Some(FieldError {
                    field: Field::Source,
                    message
                }),
                ..
            } if message == import_text(ImportText::StoppedMessage)
        ));
    }

    #[test]
    fn preview_failure_is_attached_to_the_source_field() {
        let dirs = TempDirs::new();
        dirs.create_source();
        let mut dialog = ImportDialog::new(dirs.form());
        let _ = schedule_preview(&mut dialog, Instant::now());
        dialog.sent(12);
        dialog.apply(&ImportEvent::Previewed {
            id: 12,
            result: Err(FieldError {
                field: Field::Source,
                message: "Kunde inte läsa källa.".to_string(),
            }),
        });

        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                error: Some(FieldError {
                    field: Field::Source,
                    message
                }),
                ..
            } if message == "Kunde inte läsa källa."
        ));
    }

    #[test]
    fn preview_validation_error_is_attached_to_the_reported_field() {
        let mut dialog = ImportDialog::new(ImportForm::default());
        let now = Instant::now();
        let _ = schedule_preview(&mut dialog, now);
        dialog.sent(15);
        let error = FieldError {
            field: Field::Folder,
            message: "Folder Template: invalid".to_string(),
        };
        dialog.apply(&ImportEvent::Previewed {
            id: 15,
            result: Err(error.clone()),
        });

        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                error: Some(current),
                ..
            } if current == &error
        ));
    }

    #[test]
    fn disconnect_while_running_finishes_with_an_error() {
        let dirs = TempDirs::new();
        dirs.create_source();
        let mut dialog = ImportDialog::new(dirs.form());
        let now = Instant::now();
        let _ = schedule_preview(&mut dialog, now);
        dialog.sent(8);
        dialog.apply(&ImportEvent::Previewed {
            id: 8,
            result: Ok(preview(1)),
        });
        assert!(dialog.start().is_some());
        dialog.sent(9);
        assert!(!dialog.can_close());
        assert!(dialog.cancel());
        dialog.disconnected();

        assert!(matches!(
            dialog.state(),
            ImportDialogState::Done {
                done: ImportDone {
                    error: Some(message),
                    ..
                }
            } if message == import_text(ImportText::StoppedMessage)
        ));
        assert!(dialog.can_close());
    }

    #[test]
    fn back_to_editing_clears_preview_and_schedules_fresh_scan() {
        let dirs = TempDirs::new();
        dirs.create_source();
        let mut dialog = ImportDialog::new(dirs.form());
        let now = Instant::now();
        let _ = schedule_preview(&mut dialog, now);
        dialog.sent(10);
        dialog.apply(&ImportEvent::Previewed {
            id: 10,
            result: Ok(preview(1)),
        });
        assert!(dialog.start().is_some());
        dialog.sent(11);
        dialog.apply(&ImportEvent::Done {
            id: 11,
            done: ImportDone {
                copied: 1,
                ..ImportDone::default()
            },
        });

        dialog.back_to_editing();
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                preview: None,
                error: None
            }
        ));
        assert!(matches!(
            dialog.tick(Instant::now() + PREVIEW_DELAY),
            Some(DialogRequest::Preview { rescan: true, .. })
        ));
    }

    #[test]
    fn can_start_is_true_only_with_nonempty_preview_in_editing_mode() {
        let mut dialog = ImportDialog::new(ImportForm::default());
        assert!(!dialog.can_start());
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                preview: None,
                error: None
            }
        ));
        dialog.state = ImportDialogState::Editing {
            preview: Some(preview(0)),
            error: None,
        };
        assert!(!dialog.can_start());
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                preview: Some(current),
                error: None
            } if current.files_to_copy == 0
        ));
        dialog.state = ImportDialogState::Editing {
            preview: Some(preview(1)),
            error: None,
        };
        assert!(dialog.can_start());
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                preview: Some(current),
                error: None
            } if current.files_to_copy == 1
        ));
        dialog.state = ImportDialogState::Editing {
            preview: Some(preview(1)),
            error: Some(FieldError {
                field: Field::Dest,
                message: "Choose a destination.".to_string(),
            }),
        };
        assert!(!dialog.can_start());
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                preview: Some(_),
                error: Some(_)
            }
        ));
        dialog.state = ImportDialogState::Previewing;
        assert!(!dialog.can_start());
        assert!(matches!(dialog.state(), ImportDialogState::Previewing));
        dialog.state = ImportDialogState::Running { progress: None };
        assert!(!dialog.can_start());
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Running { progress: None }
        ));
        dialog.state = ImportDialogState::Done {
            done: ImportDone::default(),
        };
        assert!(!dialog.can_start());
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Done { done } if done == &ImportDone::default()
        ));
    }

    fn marking_preview() -> Preview {
        Preview {
            present: 2,
            ..preview(0)
        }
    }

    fn incremental_dialog(incremental: bool, preview: Preview) -> ImportDialog {
        let mut dialog = ImportDialog::new(ImportForm {
            incremental,
            ..ImportForm::default()
        });
        dialog.state = ImportDialogState::Editing {
            preview: Some(preview),
            error: None,
        };
        dialog
    }

    #[test]
    fn a_card_at_the_destination_already_can_be_marked_as_imported() {
        let mut dialog = incremental_dialog(true, marking_preview());
        assert!(dialog.marks_only());
        assert!(dialog.can_start());
        assert!(matches!(
            dialog.start(),
            Some(DialogRequest::Run { form }) if form.incremental
        ));
        assert!(matches!(dialog.state(), ImportDialogState::Running { .. }));
    }

    #[test]
    fn a_card_cannot_be_marked_without_incremental_present_files_or_a_usable_register() {
        let off = incremental_dialog(false, marking_preview());
        assert!(!off.marks_only());
        assert!(!off.can_start());

        let none_present = incremental_dialog(true, preview(0));
        assert!(!none_present.marks_only());
        assert!(!none_present.can_start());

        let noted = incremental_dialog(
            true,
            Preview {
                register_note: Some("The card's register could not be read.".to_string()),
                ..marking_preview()
            },
        );
        assert!(!noted.marks_only());
        assert!(!noted.can_start());

        let mut field_error = incremental_dialog(true, marking_preview());
        field_error.state = ImportDialogState::Editing {
            preview: Some(marking_preview()),
            error: Some(FieldError {
                field: Field::Dest,
                message: "Choose a destination.".to_string(),
            }),
        };
        assert!(!field_error.marks_only());
        assert!(!field_error.can_start());
        assert!(field_error.start().is_none());
    }

    #[test]
    fn a_preview_with_files_to_copy_starts_an_import_not_a_marking() {
        let dialog = incremental_dialog(
            true,
            Preview {
                present: 2,
                ..preview(1)
            },
        );
        assert!(!dialog.marks_only());
        assert!(dialog.can_start());
    }

    #[test]
    fn rescan_marks_form_as_changed_and_asks_for_fresh_scan() {
        let mut dialog = ImportDialog::new(ImportForm::default());
        dialog.state = ImportDialogState::Editing {
            preview: Some(preview(1)),
            error: None,
        };
        let now = Instant::now();
        dialog.rescan(now);
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                preview: None,
                error: None
            }
        ));
        assert!(matches!(
            dialog.tick(now + PREVIEW_DELAY),
            Some(DialogRequest::Preview { rescan: true, .. })
        ));
    }

    #[test]
    fn rescan_is_ignored_while_running() {
        let mut dialog = ImportDialog::new(ImportForm::default());
        dialog.changed_at = None;
        dialog.source_changed = false;
        dialog.request_id = Some(7);
        dialog.state = ImportDialogState::Running {
            progress: Some(pw_ingest::Progress::default()),
        };
        let now = Instant::now();

        dialog.rescan(now);

        assert!(matches!(
            dialog.state(),
            ImportDialogState::Running {
                progress: Some(progress),
            } if progress.files_done == 0
                && progress.files_total == 0
                && progress.bytes_done == 0
                && progress.bytes_total == 0
                && progress.current.as_os_str().is_empty()
        ));
        assert_eq!(dialog.changed_at, None);
        assert!(!dialog.source_changed);
        assert_eq!(dialog.request_id, Some(7));
        assert!(dialog.tick(now + PREVIEW_DELAY).is_none());
    }

    /// Reaches a current preview as the dialog does: a scheduled request, sent, then answered.
    fn previewed(dialog: &mut ImportDialog, now: Instant, id: RequestId, result: Preview) {
        let _ = schedule_preview(dialog, now);
        dialog.sent(id);
        dialog.apply(&ImportEvent::Previewed {
            id,
            result: Ok(result),
        });
    }

    fn is_current(dialog: &ImportDialog, files_to_copy: usize) -> bool {
        matches!(
            dialog.state(),
            ImportDialogState::Editing {
                preview: Some(current),
                error: None
            } if current.files_to_copy == files_to_copy
        )
    }

    #[test]
    fn an_iptc_or_eject_only_edit_keeps_a_current_preview() {
        let t = Instant::now();
        let mut dialog = ImportDialog::new(ImportForm::default());
        previewed(&mut dialog, t, 30, preview(2));
        assert!(dialog.can_start());

        dialog.edit(
            |form| form.iptc.creator = "A. Photographer".to_string(),
            t + Duration::from_secs(1),
        );
        assert_eq!(dialog.form().iptc.creator, "A. Photographer");
        assert!(is_current(&dialog, 2));
        assert!(dialog.can_start());
        assert!(dialog.preview_is_current());
        assert_eq!(dialog.shown_preview(), Some(&preview(2)));
        assert!(dialog.tick(t + PREVIEW_DELAY * 2).is_none());
        assert!(
            dialog
                .tick(t + Duration::from_secs(1) + PREVIEW_DELAY * 2)
                .is_none()
        );

        dialog.edit(|form| form.eject = !form.eject, t + Duration::from_secs(2));
        assert!(dialog.form().eject);
        assert!(is_current(&dialog, 2));
        assert!(dialog.can_start());
        assert!(
            dialog
                .tick(t + Duration::from_secs(2) + PREVIEW_DELAY * 2)
                .is_none()
        );
    }

    #[test]
    fn an_iptc_only_edit_while_previewing_keeps_the_request_on_its_way() {
        let t = Instant::now();
        let mut dialog = ImportDialog::new(ImportForm::default());
        let _ = schedule_preview(&mut dialog, t);
        dialog.sent(31);

        dialog.edit(
            |form| form.iptc.copyright = "(c) 2026".to_string(),
            t + PREVIEW_DELAY + Duration::from_millis(50),
        );
        assert!(matches!(dialog.state(), ImportDialogState::Previewing));

        dialog.apply(&ImportEvent::Previewed {
            id: 31,
            result: Ok(preview(3)),
        });
        assert!(is_current(&dialog, 3));
        assert_eq!(dialog.form().iptc.copyright, "(c) 2026");
    }

    #[test]
    fn an_iptc_only_edit_does_not_restart_or_cancel_a_running_debounce() {
        let t = Instant::now();
        let mut dialog = ImportDialog::new(ImportForm::default());

        dialog.edit(|form| form.folder = "{year}".to_string(), t);
        dialog.edit(
            |form| form.iptc.credit = "Agency".to_string(),
            t + Duration::from_millis(100),
        );
        assert!(dialog.tick(t + Duration::from_millis(250)).is_none());
        assert!(matches!(
            dialog.tick(t + PREVIEW_DELAY),
            Some(DialogRequest::Preview { form, .. })
                if form.folder == "{year}" && form.iptc.credit == "Agency"
        ));
    }

    #[test]
    fn an_iptc_only_edit_keeps_a_field_error() {
        let t = Instant::now();
        let mut dialog = ImportDialog::new(ImportForm::default());
        let _ = schedule_preview(&mut dialog, t);
        dialog.sent(32);
        let error = FieldError {
            field: Field::Folder,
            message: "Folder Template: invalid".to_string(),
        };
        dialog.apply(&ImportEvent::Previewed {
            id: 32,
            result: Err(error.clone()),
        });

        dialog.edit(
            |form| form.iptc.url = "https://example.com".to_string(),
            t + Duration::from_secs(1),
        );
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                preview: None,
                error: Some(current),
            } if current == &error
        ));
        assert!(dialog.shown_preview().is_none());
    }

    #[test]
    fn an_iptc_only_edit_in_done_goes_back_to_editing_without_a_preview() {
        let t = Instant::now();
        let mut dialog = ImportDialog::new(ImportForm::default());
        previewed(&mut dialog, t, 33, preview(1));
        assert!(dialog.start().is_some());
        dialog.sent(34);
        dialog.apply(&ImportEvent::Done {
            id: 34,
            done: ImportDone::default(),
        });

        dialog.edit(
            |form| form.iptc.phone = "+46 0".to_string(),
            t + Duration::from_secs(1),
        );
        assert!(matches!(
            dialog.state(),
            ImportDialogState::Editing {
                preview: None,
                error: None
            }
        ));
        // The list from before the import is not kept (the owner, 2026-11-22).
        assert!(dialog.shown_preview().is_none());
        assert!(!dialog.preview_is_current());
    }

    #[test]
    fn a_changed_name_template_keeps_the_old_list_dimmed_until_the_new_preview() {
        let t = Instant::now();
        let mut dialog = ImportDialog::new(ImportForm::default());
        previewed(&mut dialog, t, 35, preview(1));

        let edited = t + Duration::from_secs(1);
        dialog.edit(|form| form.name = "{seq}".to_string(), edited);
        assert_eq!(dialog.shown_preview(), Some(&preview(1)));
        assert!(!dialog.preview_is_current());
        assert!(!dialog.can_start());

        assert!(matches!(
            dialog.tick(edited + PREVIEW_DELAY),
            Some(DialogRequest::Preview { .. })
        ));
        dialog.sent(36);
        assert!(matches!(dialog.state(), ImportDialogState::Previewing));
        assert_eq!(dialog.shown_preview(), Some(&preview(1)));
        assert!(!dialog.preview_is_current());
        assert!(!dialog.can_start());

        dialog.apply(&ImportEvent::Previewed {
            id: 36,
            result: Ok(preview(4)),
        });
        assert_eq!(dialog.shown_preview(), Some(&preview(4)));
        assert!(dialog.preview_is_current());
        assert!(dialog.can_start());
    }

    #[test]
    fn rescan_keeps_the_shown_preview_dimmed() {
        let t = Instant::now();
        let mut dialog = ImportDialog::new(ImportForm::default());
        previewed(&mut dialog, t, 37, preview(1));

        dialog.rescan(t + Duration::from_secs(1));
        assert_eq!(dialog.shown_preview(), Some(&preview(1)));
        assert!(!dialog.preview_is_current());
    }

    #[test]
    fn the_shown_preview_is_cleared_by_an_error_a_disconnect_and_back_to_editing() {
        let t = Instant::now();

        let mut errored = ImportDialog::new(ImportForm::default());
        previewed(&mut errored, t, 38, preview(1));
        let _ = schedule_preview(&mut errored, t + Duration::from_secs(1));
        errored.sent(39);
        errored.apply(&ImportEvent::Previewed {
            id: 39,
            result: Err(FieldError {
                field: Field::Name,
                message: "Name Template: invalid".to_string(),
            }),
        });
        assert!(errored.shown_preview().is_none());
        errored.edit(
            |form| form.name = "{seq}".to_string(),
            t + Duration::from_secs(2),
        );
        assert!(errored.shown_preview().is_none());

        let mut disconnected = ImportDialog::new(ImportForm::default());
        previewed(&mut disconnected, t, 40, preview(1));
        let _ = schedule_preview(&mut disconnected, t + Duration::from_secs(1));
        disconnected.sent(41);
        disconnected.disconnected();
        assert!(disconnected.shown_preview().is_none());
        disconnected.edit(
            |form| form.name = "{seq}".to_string(),
            t + Duration::from_secs(2),
        );
        assert!(disconnected.shown_preview().is_none());

        let mut finished = ImportDialog::new(ImportForm::default());
        previewed(&mut finished, t, 42, preview(1));
        assert!(finished.start().is_some());
        finished.sent(43);
        finished.apply(&ImportEvent::Done {
            id: 43,
            done: ImportDone::default(),
        });
        finished.back_to_editing();
        assert!(finished.shown_preview().is_none());
    }

    #[test]
    fn the_shown_preview_is_none_while_running_and_in_done() {
        let t = Instant::now();
        let mut dialog = ImportDialog::new(ImportForm::default());
        previewed(&mut dialog, t, 44, preview(1));
        assert!(dialog.start().is_some());
        assert!(dialog.shown_preview().is_none());
        assert!(!dialog.preview_is_current());

        dialog.sent(45);
        dialog.apply(&ImportEvent::Done {
            id: 45,
            done: ImportDone::default(),
        });
        assert!(dialog.shown_preview().is_none());
        assert!(!dialog.preview_is_current());
    }

    #[test]
    fn card_refresh_is_not_due_before_the_first_answer() {
        let now = Instant::now();
        let refresh = CardRefresh::new(now);

        assert!(!refresh.due(now, true));
        assert!(!refresh.due(now + CARD_REFRESH_INTERVAL, true));
        assert!(!refresh.due(now + CARD_REFRESH_INTERVAL * 100, true));
    }

    #[test]
    fn card_refresh_is_due_at_the_interval_after_an_answer() {
        let now = Instant::now();
        let mut refresh = CardRefresh::new(now);
        refresh.answered();

        assert!(!refresh.due(now, true));
        assert!(!refresh.due(now + CARD_REFRESH_INTERVAL - Duration::from_millis(1), true));
        assert!(refresh.due(now + CARD_REFRESH_INTERVAL, true));

        let later = now + CARD_REFRESH_INTERVAL;
        refresh.asked(later);
        assert!(!refresh.due(later + CARD_REFRESH_INTERVAL, true));
        refresh.answered();
        assert!(!refresh.due(
            later + CARD_REFRESH_INTERVAL - Duration::from_millis(1),
            true
        ));
        assert!(refresh.due(later + CARD_REFRESH_INTERVAL, true));
    }

    #[test]
    fn card_refresh_waits_while_the_dialog_is_not_idle() {
        let now = Instant::now();
        let mut refresh = CardRefresh::new(now);
        refresh.answered();
        let due_at = now + CARD_REFRESH_INTERVAL * 3;

        assert!(!refresh.due(due_at, false));
        assert!(refresh.due(due_at, true));
    }

    #[test]
    fn card_refresh_is_not_due_before_the_last_ask() {
        let now = Instant::now() + CARD_REFRESH_INTERVAL;
        let mut refresh = CardRefresh::new(now);
        refresh.answered();

        assert!(!refresh.due(now - CARD_REFRESH_INTERVAL, true));
        assert!(!refresh.due(now - Duration::from_millis(1), true));
    }
}
