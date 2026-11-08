use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// Returns true if `a` is `b` or lies below it, ignoring case and separator differences.
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

/// Validates an absolute path and resolves it, including paths below existing parents.
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

/// Returns the drive letter at the start of `source`, if present.
pub fn drive_of(source: &Path) -> Option<String> {
    let s = source.to_string_lossy();
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic() => {
            Some(format!("{}:", letter.to_ascii_uppercase()))
        }
        _ => None,
    }
}

/// Summarizes the number of plan items in each destination folder.
pub fn folder_summary(items: &[&crate::PlanItem], dest: &Path) -> Vec<(String, usize)> {
    let mut folders: BTreeMap<PathBuf, usize> = BTreeMap::new();
    for item in items {
        let path = dest.join(&item.rel);
        if let Some(parent) = path.parent() {
            *folders.entry(parent.to_path_buf()).or_default() += 1;
        }
    }
    let mut folders: Vec<(String, usize)> = folders
        .into_iter()
        .map(|(path, count)| (path.to_string_lossy().into_owned(), count))
        .collect();
    folders.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    folders
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlanItem;
    use std::time::SystemTime;

    fn item() -> PlanItem {
        PlanItem {
            source: PathBuf::from("DSC00001.ARW"),
            size: 1,
            modified: SystemTime::UNIX_EPOCH,
            rel: PathBuf::from("2026/DSC00001.ARW"),
            to_dest: false,
            to_backup: false,
            renamed_for_conflict: false,
        }
    }

    fn folder_item(rel: &str) -> PlanItem {
        PlanItem {
            rel: PathBuf::from(rel),
            ..item()
        }
    }

    #[test]
    fn folder_summary_sorts_by_count_then_name() {
        let dest = Path::new("destination");
        let items = [
            folder_item("z/one.arw"),
            folder_item("z/two.arw"),
            folder_item("a/one.arw"),
            folder_item("a/two.arw"),
            folder_item("m/one.arw"),
        ];
        let summary = folder_summary(&items.iter().collect::<Vec<_>>(), dest);

        assert_eq!(
            summary,
            vec![
                (dest.join("a").to_string_lossy().into_owned(), 2),
                (dest.join("z").to_string_lossy().into_owned(), 2),
                (dest.join("m").to_string_lossy().into_owned(), 1),
            ]
        );
    }

    #[test]
    fn folder_summary_counts_items_in_separate_folders() {
        let dest = Path::new("destination");
        let items = [
            folder_item("2026/01/one.arw"),
            folder_item("2026/02/two.arw"),
        ];
        let summary = folder_summary(&items.iter().collect::<Vec<_>>(), dest);

        assert_eq!(
            summary,
            vec![
                (dest.join("2026/01").to_string_lossy().into_owned(), 1),
                (dest.join("2026/02").to_string_lossy().into_owned(), 1),
            ]
        );
    }

    #[test]
    fn folder_summary_counts_only_items_passed_to_it() {
        let dest = Path::new("destination");
        let items = [
            folder_item("2026/01/one.arw"),
            folder_item("2026/01/two.arw"),
            folder_item("2026/02/three.arw"),
        ];
        let selected = [&items[0], &items[2]];

        let summary = folder_summary(&selected, dest);

        assert_eq!(
            summary,
            vec![
                (dest.join("2026/01").to_string_lossy().into_owned(), 1),
                (dest.join("2026/02").to_string_lossy().into_owned(), 1),
            ]
        );
    }

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

    #[test]
    fn drive_letter_is_found_only_at_the_start() {
        assert_eq!(drive_of(Path::new(r"e:\DCIM")).as_deref(), Some("E:"));
        assert_eq!(drive_of(Path::new("F:")).as_deref(), Some("F:"));
        assert_eq!(drive_of(Path::new(r"\\server\share")), None);
        assert_eq!(drive_of(Path::new("/media/card")), None);
        assert_eq!(drive_of(Path::new("1:")), None);
        assert_eq!(drive_of(Path::new("")), None);
    }
}
