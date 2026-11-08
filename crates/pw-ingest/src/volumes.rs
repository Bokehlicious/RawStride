use std::path::Path;
use std::path::PathBuf;

fn has_dcim(path: &Path) -> bool {
    path.join("DCIM").is_dir()
}

fn with_dcim(roots: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    roots.into_iter().filter(|root| has_dcim(root)).collect()
}

/// Returns drives or mounted volumes that contain a `DCIM` folder.
pub fn card_volumes() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        with_dcim((b'A'..=b'Z').map(|c| PathBuf::from(format!("{}:\\", c as char))))
    }

    #[cfg(not(windows))]
    {
        let mut found = Vec::new();
        for base in ["/Volumes", "/media", "/run/media"] {
            let mut stack = vec![PathBuf::from(base)];
            for _ in 0..2 {
                let mut candidates = Vec::new();
                for dir in stack {
                    for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            candidates.push(path);
                        }
                    }
                }
                found.extend(with_dcim(candidates.iter().cloned()));
                stack = candidates
                    .into_iter()
                    .filter(|root| !has_dcim(root))
                    .collect();
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

    fn test_root() -> PathBuf {
        let id = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("pw-ingest-volume-{}-{id}", std::process::id()))
    }

    #[test]
    fn identifies_volumes_with_a_dcim_directory() {
        let root = test_root();
        if let Err(error) = std::fs::create_dir_all(&root) {
            panic!("could not create test volume {}: {error}", root.display());
        }
        assert!(!has_dcim(&root));
        if let Err(error) = std::fs::create_dir(root.join("DCIM")) {
            panic!("could not create DCIM test folder: {error}");
        }
        assert!(has_dcim(&root));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn with_dcim_returns_matching_roots_and_skips_missing_or_unmatched_roots() {
        let root = test_root();
        let matching = root.join("matching");
        let unmatched = root.join("unmatched");
        let missing = root.join("missing");
        if let Err(error) = std::fs::create_dir_all(matching.join("DCIM")) {
            panic!("could not create DCIM test folder: {error}");
        }
        if let Err(error) = std::fs::create_dir_all(&unmatched) {
            panic!(
                "could not create test volume {}: {error}",
                unmatched.display()
            );
        }

        assert_eq!(
            with_dcim([matching.clone(), unmatched.clone(), missing.clone()]),
            vec![matching.clone()]
        );

        let _ = std::fs::remove_dir_all(root);
    }
}
