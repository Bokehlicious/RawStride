use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::TryRecvError;

use pw_loupe::{RatingStore, UNSUPPORTED_TEXT, status_text};
use pw_metadata::{MetadataWorker, RequestId, SubmitError, WorkerEvent, supports_rating};

#[derive(Clone, Debug)]
enum LoadState {
    Loading,
    Ready,
    Unsupported,
    Error(String),
}

#[derive(Clone, Debug)]
struct ImageMetadata {
    state: LoadState,
}

#[derive(Default)]
pub(crate) struct MetadataCache {
    images: HashMap<PathBuf, ImageMetadata>,
    load_request: Option<RequestId>,
    ratings: RatingStore,
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
                (path.clone(), ImageMetadata { state })
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

    pub fn poll(&mut self, worker: &mut MetadataWorker) -> Vec<String> {
        let mut failures = Vec::new();
        self.drain(|| worker.try_recv(), &mut failures);
        failures.extend(self.ratings.pump(worker));
        failures
    }

    pub fn prune(&mut self, keep: impl Fn(&Path) -> bool) {
        self.ratings.prune(keep);
    }

    /// Applies every available event. If the worker is gone, no outstanding operation can
    /// finish, so loading entries and writes become errors instead of remaining pending.
    fn drain(
        &mut self,
        mut next: impl FnMut() -> Result<WorkerEvent, TryRecvError>,
        failures: &mut Vec<String>,
    ) {
        loop {
            match next() {
                Ok(event) => self.apply(event, failures),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.fail_loading(&submit_error(SubmitError::Stopped));
                    failures.extend(self.ratings.fail_sent());
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
            LoadState::Loading => Some("Läser metadata…".into()),
            LoadState::Ready => None,
            LoadState::Unsupported => Some(UNSUPPORTED_TEXT.into()),
            LoadState::Error(error) => Some(format!("Metadatafel: {error}")),
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
        let path = path?;
        self.images.get(path)?;
        self.ratings
            .contains(path)
            .then(|| status_text(self.ratings.shown(path)))
    }

    fn apply(&mut self, event: WorkerEvent, failures: &mut Vec<String>) {
        match event {
            WorkerEvent::RatingsRead { id, result } if Some(id) == self.load_request => {
                self.load_request = None;
                match result {
                    Ok(ratings) => {
                        let paths: Vec<PathBuf> = self
                            .images
                            .iter()
                            .filter(|(_, metadata)| matches!(&metadata.state, LoadState::Loading))
                            .map(|(path, _)| path.clone())
                            .collect();
                        self.ratings.read(id, &ratings, &paths);
                        for metadata in self.images.values_mut() {
                            if matches!(&metadata.state, LoadState::Loading) {
                                metadata.state = LoadState::Ready;
                            }
                        }
                    }
                    Err(error) => self.fail_loading(&error),
                }
            }
            WorkerEvent::RatingWritten {
                id, path, result, ..
            } => {
                if let Some(message) = self.ratings.written(id, &path, result) {
                    failures.push(message);
                }
            }
            WorkerEvent::RatingsRead { .. } => {}
        }
    }
}

fn submit_error(error: SubmitError) -> String {
    match error {
        SubmitError::QueueFull => "metadatakön är full".into(),
        SubmitError::Stopped => "metadataarbetaren har stannat".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_metadata::Rating;
    use std::collections::HashMap;

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
            Some("Läser metadata…")
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
        assert_eq!(
            cache.notice(Some(&files[0])).as_deref(),
            Some(UNSUPPORTED_TEXT)
        );
    }

    #[test]
    fn disconnected_worker_turns_loading_into_a_swedish_error() {
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
        let mut failures = Vec::new();
        cache.drain(|| Err(TryRecvError::Disconnected), &mut failures);
        assert!(failures.is_empty());
        assert!(!cache.is_loading(Some(&files[0])));
        assert!(cache.is_error(Some(&files[0])));
        assert_eq!(
            cache.notice(Some(&files[0])).as_deref(),
            Some("Metadatafel: metadataarbetaren har stannat")
        );
        assert_eq!(
            cache.notice(Some(&files[1])).as_deref(),
            Some(UNSUPPORTED_TEXT),
            "entries that were not loading keep their state"
        );
    }

    #[test]
    fn ratings_survive_loading_another_folder() {
        let program =
            std::env::temp_dir().join(format!("pw-loupe-preserved-rating-{}", std::process::id()));
        let mut worker = match MetadataWorker::start_with(&program.to_string_lossy(), &[]) {
            Ok(worker) => worker,
            Err(error) => panic!("worker thread should start: {error}"),
        };
        let mut cache = MetadataCache::default();
        let old_path = std::env::temp_dir().join("pw-loupe-preserved.ARW");
        if let Err(error) = cache.begin_load(vec![old_path.clone()], &mut worker) {
            panic!("current directory should be available: {error}");
        }
        if let Err(error) = cache.ratings.set(&old_path, pw_loupe::RatingKey::Stars(3)) {
            panic!("supported rating path should be accepted: {error}");
        }
        if let Err(error) = cache.begin_load(vec![PathBuf::from("notes.txt")], &mut worker) {
            panic!("current directory should be available: {error}");
        }

        assert_eq!(
            cache
                .ratings
                .shown(&old_path)
                .and_then(|rating| rating.stars),
            Some(3)
        );
    }

    #[test]
    fn rating_summary_uses_the_swedish_rating_text() {
        let program =
            std::env::temp_dir().join(format!("pw-loupe-rating-summary-{}", std::process::id()));
        let mut worker = match MetadataWorker::start_with(&program.to_string_lossy(), &[]) {
            Ok(worker) => worker,
            Err(error) => panic!("worker thread should start: {error}"),
        };
        let mut cache = MetadataCache::default();
        let path = std::env::temp_dir().join("pw-loupe-summary.ARW");
        let files = match cache.begin_load(vec![path.clone()], &mut worker) {
            Ok(files) => files,
            Err(error) => panic!("current directory should be available: {error}"),
        };
        let Some(id) = cache.load_request else {
            panic!("supported image should queue a metadata read");
        };
        let mut failures = Vec::new();
        cache.apply(
            WorkerEvent::RatingsRead {
                id,
                result: Ok(HashMap::from([(
                    files[0].clone(),
                    Rating {
                        stars: Some(3),
                        label: Some("Red".to_owned()),
                    },
                )])),
            },
            &mut failures,
        );

        assert!(failures.is_empty());
        assert_eq!(cache.notice(Some(&files[0])), None);
        assert_eq!(
            cache.rating_summary(Some(&files[0])).as_deref(),
            Some("Betyg: 3 stjärnor, Röd")
        );
    }
}
