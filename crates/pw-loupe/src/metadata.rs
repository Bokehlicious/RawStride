use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;

use pw_metadata::{MetadataWorker, RequestId};
use pw_metadata::{Rating, SubmitError, WorkerEvent, supports_rating};

#[derive(Clone, Debug)]
enum LoadState {
    Loading,
    Ready,
    Unsupported,
    Error(String),
}

#[derive(Clone, Debug)]
struct ImageMetadata {
    rating: Option<Rating>,
    state: LoadState,
}

#[derive(Default)]
pub(crate) struct MetadataCache {
    images: HashMap<PathBuf, ImageMetadata>,
    load_request: Option<RequestId>,
}

impl MetadataCache {
    /// Keys metadata by absolute path so it matches the paths sent to the view core.
    pub fn begin_load(
        &mut self,
        files: Vec<PathBuf>,
        worker: &mut MetadataWorker,
    ) -> Result<Vec<PathBuf>, String> {
        let cwd =
            std::env::current_dir().map_err(|e| format!("cannot resolve image paths: {e}"))?;
        let files: Vec<PathBuf> = files
            .into_iter()
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    cwd.join(path)
                }
            })
            .collect();
        self.images = files
            .iter()
            .map(|path| {
                let state = if supports_rating(path) {
                    LoadState::Loading
                } else {
                    LoadState::Unsupported
                };
                (
                    path.clone(),
                    ImageMetadata {
                        rating: None,
                        state,
                    },
                )
            })
            .collect();
        self.load_request = None;
        let supported: Vec<PathBuf> = files
            .iter()
            .filter(|path| supports_rating(path))
            .cloned()
            .collect();
        if !supported.is_empty() {
            match worker.read_ratings(supported) {
                Ok(id) => self.load_request = Some(id),
                Err(error) => self.fail_loading(&submit_error(error)),
            }
        }
        Ok(files)
    }

    pub fn poll(&mut self, worker: &MetadataWorker) {
        self.drain(|| worker.try_recv());
    }

    /// Applies every available event. If the worker is gone, nothing still loading can ever
    /// finish, so those entries become errors instead of showing "loading" forever.
    fn drain(&mut self, mut next: impl FnMut() -> Result<WorkerEvent, TryRecvError>) {
        loop {
            match next() {
                Ok(event) => self.apply(event),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.fail_loading(&submit_error(SubmitError::Stopped));
                    return;
                }
            }
        }
    }

    /// Moves every entry that is still loading to the error state.
    fn fail_loading(&mut self, message: &str) {
        self.load_request = None;
        for metadata in self.images.values_mut() {
            if matches!(&metadata.state, LoadState::Loading) {
                metadata.state = LoadState::Error(message.to_owned());
            }
        }
    }

    pub fn notice(&self, path: Option<&Path>) -> Option<String> {
        let metadata = self.images.get(path?)?;
        match &metadata.state {
            LoadState::Loading => Some("Metadata loading…".into()),
            LoadState::Ready => None,
            LoadState::Unsupported => Some("Ratings unsupported for this file".into()),
            LoadState::Error(error) => Some(format!("Metadata error: {error}")),
        }
    }

    pub fn is_loading(&self, path: Option<&Path>) -> bool {
        path.and_then(|path| self.images.get(path))
            .is_some_and(|metadata| matches!(&metadata.state, LoadState::Loading))
    }

    pub fn is_error(&self, path: Option<&Path>) -> bool {
        path.and_then(|path| self.images.get(path))
            .is_some_and(|metadata| matches!(&metadata.state, LoadState::Error(_)))
    }

    pub fn rating_summary(&self, path: Option<&Path>) -> Option<String> {
        let rating = self.images.get(path?)?.rating.as_ref()?;
        let mut summary = match rating.stars {
            Some(-1) => "Rejected".to_string(),
            Some(stars) => format!("{stars} stars"),
            None => "Unrated".to_string(),
        };
        if let Some(label) = &rating.label {
            summary.push_str(&format!(", {label}"));
        }
        Some(format!("Rating: {summary}"))
    }

    fn apply(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::RatingsRead { id, result } if Some(id) == self.load_request => {
                self.load_request = None;
                match result {
                    Ok(ratings) => {
                        for (path, metadata) in &mut self.images {
                            if matches!(&metadata.state, LoadState::Loading) {
                                metadata.rating = ratings.get(path).cloned();
                                metadata.state = LoadState::Ready;
                            }
                        }
                    }
                    Err(error) => self.fail_loading(&error),
                }
            }
            WorkerEvent::RatingWritten {
                path,
                rating,
                result,
                ..
            } => {
                if let Some(metadata) = self.images.get_mut(&path) {
                    match result {
                        Ok(()) => {
                            metadata.rating = (!rating.is_empty()).then_some(rating);
                            metadata.state = LoadState::Ready;
                        }
                        Err(error) => metadata.state = LoadState::Error(error),
                    }
                }
            }
            WorkerEvent::RatingsRead { .. } => {}
        }
    }
}

fn submit_error(error: SubmitError) -> String {
    match error {
        SubmitError::QueueFull => "metadata queue is full".into(),
        SubmitError::Stopped => "metadata worker has stopped".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_cache_uses_absolute_paths_and_starts_loading() {
        let program =
            std::env::temp_dir().join(format!("pw-loupe-missing-exiftool-{}", std::process::id()));
        let mut worker = match MetadataWorker::start_with(&program.to_string_lossy(), &[]) {
            Ok(worker) => worker,
            Err(error) => panic!("worker thread should start: {error}"),
        };
        let mut cache = MetadataCache::default();
        let files = match cache.begin_load(vec![PathBuf::from("photo.ARW")], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        assert!(files[0].is_absolute());
        assert!(cache.is_loading(Some(&files[0])));
        assert!(!cache.is_error(Some(&files[0])));
        assert_eq!(
            cache.notice(Some(&files[0])).as_deref(),
            Some("Metadata loading…")
        );
    }

    #[test]
    fn metadata_state_queries_are_false_for_unsupported_files_and_missing_paths() {
        let program =
            std::env::temp_dir().join(format!("pw-loupe-unsupported-{}", std::process::id()));
        let mut worker = match MetadataWorker::start_with(&program.to_string_lossy(), &[]) {
            Ok(worker) => worker,
            Err(error) => panic!("worker thread should start: {error}"),
        };
        let mut cache = MetadataCache::default();
        let files = match cache.begin_load(vec![PathBuf::from("notes.txt")], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };

        assert!(!cache.is_loading(Some(&files[0])));
        assert!(!cache.is_error(Some(&files[0])));
        assert!(!cache.is_loading(None));
        assert!(!cache.is_error(None));
    }

    #[test]
    fn disconnected_worker_turns_loading_into_an_error() {
        let program =
            std::env::temp_dir().join(format!("pw-loupe-stopped-worker-{}", std::process::id()));
        let mut worker = match MetadataWorker::start_with(&program.to_string_lossy(), &[]) {
            Ok(worker) => worker,
            Err(error) => panic!("worker thread should start: {error}"),
        };
        let mut cache = MetadataCache::default();
        let files = match cache.begin_load(
            vec![PathBuf::from("photo.ARW"), PathBuf::from("notes.txt")],
            &mut worker,
        ) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        cache.drain(|| Err(TryRecvError::Disconnected));
        assert!(!cache.is_loading(Some(&files[0])));
        assert!(cache.is_error(Some(&files[0])));
        assert_eq!(
            cache.notice(Some(&files[0])).as_deref(),
            Some("Metadata error: metadata worker has stopped")
        );
        assert_eq!(
            cache.notice(Some(&files[1])).as_deref(),
            Some("Ratings unsupported for this file"),
            "entries that were not loading keep their state"
        );
    }
}
