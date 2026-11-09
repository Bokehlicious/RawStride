use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::{
    Field, FieldError, ImportDone, ImportEvent, ImportForm, Preview, RequestId, result_folder,
};

const PREVIEW_DELAY: Duration = Duration::from_millis(300);
const STOPPED_MESSAGE: &str = "Importen avbröts: arbetaren har stoppats.";

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
        }
    }

    pub fn form(&self) -> &ImportForm {
        &self.form
    }

    pub fn state(&self) -> &ImportDialogState {
        &self.state
    }

    /// Updates form fields and restarts the preview debounce.
    pub fn edit(&mut self, change: impl FnOnce(&mut ImportForm), now: Instant) {
        let previous_source = self.form.source.clone();
        change(&mut self.form);
        self.source_changed |= self.form.source != previous_source;
        self.changed_at = Some(now);
        if matches!(self.state, ImportDialogState::Running { .. }) {
            return;
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
            || matches!(self.state, ImportDialogState::Running { .. })
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

    /// Starts a copy only when the current preview contains files to copy.
    pub fn start(&mut self) -> Option<DialogRequest> {
        let ImportDialogState::Editing {
            preview: Some(preview),
            error: None,
        } = &self.state
        else {
            return None;
        };
        if preview.files_to_copy == 0 {
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
                        self.state = ImportDialogState::Editing {
                            preview: Some(preview.clone()),
                            error: None,
                        };
                    }
                    Err(error) => {
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
                } = &mut self.state
                {
                    *current_progress = Some(progress.clone());
                }
            }
            ImportEvent::Done { id: event_id, done }
                if *event_id == id && matches!(self.state, ImportDialogState::Running { .. }) =>
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
                self.state = ImportDialogState::Editing {
                    preview: None,
                    error: Some(FieldError {
                        field: Field::Source,
                        message: STOPPED_MESSAGE.to_string(),
                    }),
                };
            }
            ImportDialogState::Running { .. } => {
                self.request_id = None;
                self.state = ImportDialogState::Done {
                    done: ImportDone {
                        error: Some(STOPPED_MESSAGE.to_string()),
                        ..ImportDone::default()
                    },
                };
            }
            _ => {}
        }
    }

    pub fn cancel(&self) -> bool {
        matches!(self.state, ImportDialogState::Running { .. })
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
        }
    }

    pub fn result_folder(&self) -> Option<PathBuf> {
        match &self.state {
            ImportDialogState::Done { done } => result_folder(done.copied, &done.folders),
            _ => None,
        }
    }

    pub fn can_start(&self) -> bool {
        let ImportDialogState::Editing {
            preview: Some(preview),
            error: None,
        } = &self.state
        else {
            return false;
        };
        preview.files_to_copy > 0
    }

    pub fn rescan(&mut self, now: Instant) {
        if matches!(self.state, ImportDialogState::Running { .. }) {
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

#[cfg(test)]
mod tests {
    use super::*;
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
            message: "Välj en källa.".to_string(),
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
                progress: Some(current)
            } if current.files_done == 1
        ));

        let done = ImportDone {
            copied: 2,
            folders: vec![
                ("destination/largest".to_string(), 2),
                ("destination/small".to_string(), 1),
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
        assert_eq!(
            dialog.result_folder(),
            Some(PathBuf::from("destination/largest"))
        );
        assert!(!dialog.cancel());
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
            } if message == STOPPED_MESSAGE
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
            message: "Mappmall: ogiltig".to_string(),
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
            } if message == STOPPED_MESSAGE
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
                message: "Välj ett mål.".to_string(),
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
                progress: Some(progress)
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

    #[test]
    fn result_folder_is_none_without_copied_files() {
        let mut dialog = ImportDialog::new(ImportForm::default());
        dialog.state = ImportDialogState::Done {
            done: ImportDone {
                copied: 0,
                folders: vec![("destination/folder".to_string(), 3)],
                ..ImportDone::default()
            },
        };

        assert_eq!(dialog.result_folder(), None);
    }
}
