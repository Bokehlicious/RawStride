use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Counts planned files by destination folder, ordered by descending count and then folder name.
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
}
