use std::path::Path;
#[cfg(not(windows))]
use std::path::PathBuf;

fn has_dcim(path: &Path) -> bool {
    path.join("DCIM").is_dir()
}

/// Returns drives or mounted volumes that contain a `DCIM` folder.
pub fn ingest_volumes() -> Vec<String> {
    let mut out = Vec::new();
    #[cfg(windows)]
    for c in b'A'..=b'Z' {
        let root = format!("{}:\\", c as char);
        if has_dcim(Path::new(&root)) {
            out.push(root);
        }
    }
    #[cfg(not(windows))]
    for base in ["/Volumes", "/media", "/run/media"] {
        let mut stack = vec![PathBuf::from(base)];
        for _ in 0..2 {
            let mut next = Vec::new();
            for dir in stack {
                for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                    let path = entry.path();
                    if has_dcim(&path) {
                        out.push(path.to_string_lossy().into_owned());
                    } else if path.is_dir() {
                        next.push(path);
                    }
                }
            }
            stack = next;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn identifies_volumes_with_a_dcim_directory() {
        let id = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("pw-ingest-volume-{}-{id}", std::process::id()));
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
}
