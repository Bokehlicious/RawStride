use std::path::{Component, Path, PathBuf};

/// Returns whether resolved path `a` is equal to or below resolved path `b`.
///
/// Both paths are expected to have gone through [`resolve`], which canonicalises existing path
/// components so equivalent spellings (including case and separator differences) compare
/// consistently. The component boundary prevents a path such as `DCIM2` from being treated as
/// inside `DCIM`.
pub fn inside(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        p.to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_lowercase()
    };
    let (a, b) = (norm(a), norm(b));
    a == b || a.starts_with(&format!("{b}/"))
}

/// Validates an absolute path and canonicalises its existing components.
///
/// Canonicalisation makes equivalent paths compare consistently, including Windows extended
/// `\\?\` paths and paths reached through `subst` drives. The final folder need not exist yet:
/// this function canonicalises the nearest existing parent and appends the remaining components.
/// Parent-directory components are rejected rather than being resolved.
pub fn resolve(p: &Path, what: &str) -> Result<PathBuf, String> {
    if !p.is_absolute() {
        return Err(format!(
            "{what} måste vara en fullständig sökväg: {}",
            p.display()
        ));
    }
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!("{what} får inte innehålla \"..\": {}", p.display()));
    }
    let mut existing = p;
    let mut missing = Vec::new();
    loop {
        if let Ok(canonical) = existing.canonicalize() {
            let mut out = canonical;
            out.extend(missing.iter().rev());
            return Ok(out);
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_os_string());
                existing = parent;
            }
            // Nothing exists, not even the drive: keep the path as given.
            _ => return Ok(p.to_path_buf()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_compares_whole_components_ignoring_case_and_separators() {
        let p = Path::new;
        assert!(inside(p(r"E:\DCIM\100MSDCF"), p("e:/dcim")));
        assert!(inside(p(r"E:\DCIM\"), p(r"E:\DCIM")));
        assert!(inside(p("/a/b"), p("/a/b")));
        assert!(!inside(p(r"E:\DCIM2"), p(r"E:\DCIM")), "prefix of a name");
        assert!(
            !inside(p(r"E:\"), p(r"E:\DCIM")),
            "parent is not inside child"
        );
    }

    #[test]
    fn resolve_rejects_relative_and_parent_paths() {
        assert!(resolve(Path::new("Foto"), "Målet").is_err());
        let up = std::env::temp_dir().join("a").join("..").join("b");
        assert!(resolve(&up, "Målet").is_err());
    }

    #[test]
    fn resolve_keeps_missing_folders_below_an_existing_parent() {
        let base = std::env::temp_dir();
        let wanted = base.join("pw-ingest-resolve-missing").join("2026");
        let resolved = match resolve(&wanted, "Målet") {
            Ok(p) => p,
            Err(e) => panic!("{e}"),
        };
        let canonical_base = match base.canonicalize() {
            Ok(p) => p,
            Err(e) => panic!("{e}"),
        };
        assert_eq!(
            resolved,
            canonical_base
                .join("pw-ingest-resolve-missing")
                .join("2026")
        );
    }

    #[test]
    fn resolve_detects_the_same_folder_spelled_differently() {
        let base = std::env::temp_dir().join(format!("pw-ingest-same-{}", std::process::id()));
        if let Err(e) = std::fs::create_dir_all(base.join("card")) {
            panic!("{e}");
        }
        let a = resolve(&base.join("card"), "Källan");
        let b = resolve(&base.join(".").join("card"), "Målet");
        match (a, b) {
            (Ok(a), Ok(b)) => assert!(inside(&a, &b) && inside(&b, &a)),
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
