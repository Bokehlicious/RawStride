use std::path::PathBuf;

use crate::{Options, Template};

/// Named input for validating an ingest request.
#[derive(Debug)]
pub struct OptionsInput<'a> {
    pub source: &'a str,
    pub dest: &'a str,
    pub backup: Option<&'a str>,
    pub folder: &'a str,
    pub name: &'a str,
    pub job: &'a str,
    pub first_seq: u32,
}

/// Validates the selected source folder.
pub fn validate_source(source: &str) -> Result<PathBuf, String> {
    let source = source.trim();
    if source.is_empty() {
        return Err("Choose a source.".into());
    }
    let path = PathBuf::from(source);
    if !path.is_absolute() {
        return Err(format!("The source must be a full path: {source}."));
    }
    if !path.is_dir() {
        return Err(format!("The source {source} does not exist."));
    }
    Ok(path)
}

/// Validates an import request and creates the options used for planning and copying.
pub fn validate_options(input: OptionsInput<'_>) -> Result<Options, String> {
    let OptionsInput {
        source,
        dest,
        backup,
        folder,
        name,
        job,
        first_seq,
    } = input;
    let source = validate_source(source)?;
    let dest = dest.trim();
    if dest.is_empty() {
        return Err("Choose a destination.".into());
    }
    let dest = PathBuf::from(dest);
    let backup = backup
        .map(str::trim)
        .filter(|backup| !backup.is_empty())
        .map(PathBuf::from);

    let source_resolved = crate::resolve(&source, "The source")?;
    let dest_resolved = crate::resolve(&dest, "The destination")?;
    let backup_resolved = backup
        .as_deref()
        .map(|path| crate::resolve(path, "The backup destination"))
        .transpose()?;

    for (what, path) in [
        ("The destination", Some(&dest_resolved)),
        ("The backup destination", backup_resolved.as_ref()),
    ] {
        if let Some(path) = path
            && (crate::inside(path, &source_resolved) || crate::inside(&source_resolved, path))
        {
            return Err(format!("{what} must not be on or above the source."));
        }
    }
    if let Some(backup) = &backup_resolved
        && (crate::inside(backup, &dest_resolved) || crate::inside(&dest_resolved, backup))
    {
        return Err(
            "The backup destination must be a different folder from the destination.".into(),
        );
    }

    Ok(Options {
        dest,
        backup,
        folder: Template::parse(folder).map_err(|e| format!("Folder Template: {e}."))?,
        name: Template::parse(name).map_err(|e| format!("File Name Template: {e}."))?,
        job: job.trim().to_string(),
        first_seq,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_TEST_SOURCE: AtomicUsize = AtomicUsize::new(0);

    fn source_folder() -> PathBuf {
        let id = NEXT_TEST_SOURCE.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("pw-ingest-request-{}-{id}", std::process::id()));
        if let Err(error) = std::fs::create_dir_all(&path) {
            panic!("could not create test source {}: {error}", path.display());
        }
        path
    }

    fn input<'a>(
        source: &'a str,
        dest: &'a str,
        backup: Option<&'a str>,
        folder: &'a str,
        name: &'a str,
    ) -> OptionsInput<'a> {
        OptionsInput {
            source,
            dest,
            backup,
            folder,
            name,
            job: " jobb ",
            first_seq: 7,
        }
    }

    #[test]
    fn validates_source_and_preserves_error_messages() {
        assert_eq!(validate_source("  "), Err("Choose a source.".into()));
        assert_eq!(
            validate_source("relative"),
            Err("The source must be a full path: relative.".into())
        );

        let missing_parent = source_folder();
        let missing = missing_parent.join("missing");
        assert_eq!(
            validate_source(&missing.to_string_lossy()),
            Err(format!("The source {} does not exist.", missing.display()))
        );
        let _ = std::fs::remove_dir_all(missing_parent);

        let source = source_folder();
        assert_eq!(
            validate_source(&format!(" {} ", source.display())),
            Ok(source.clone())
        );
        let _ = std::fs::remove_dir_all(source);
    }

    #[test]
    fn validates_paths_and_builds_options() {
        let source = source_folder();
        let dest = source.with_file_name("pw-ingest-request-dest");
        let source_text = source.to_string_lossy();
        let dest_text = format!(" {} ", dest.display());
        let options = validate_options(input(
            &source_text,
            &dest_text,
            Some(" "),
            "2026/{date}",
            "{name}_{seq:4}",
        ));
        let options = match options {
            Ok(options) => options,
            Err(error) => panic!("{error}"),
        };
        assert_eq!(options.dest, dest);
        assert!(options.backup.is_none());
        assert_eq!(options.job, "jobb");
        assert_eq!(options.first_seq, 7);
        let _ = std::fs::remove_dir_all(source);
    }

    #[test]
    fn rejects_overlapping_destinations_and_invalid_templates() {
        let source = source_folder();
        let nested_dest = source.join("destination");
        assert_eq!(
            validate_options(input(
                &source.to_string_lossy(),
                &nested_dest.to_string_lossy(),
                None,
                "",
                "{name}",
            ))
            .err(),
            Some("The destination must not be on or above the source.".into())
        );

        let dest = source.with_file_name("pw-ingest-request-dest");
        let backup = dest.join("backup");
        assert_eq!(
            validate_options(input(
                &source.to_string_lossy(),
                &dest.to_string_lossy(),
                Some(&backup.to_string_lossy()),
                "",
                "{name}",
            ))
            .err(),
            Some("The backup destination must be a different folder from the destination.".into())
        );
        let _ = std::fs::remove_dir_all(source);
    }

    #[test]
    fn rejects_relative_destination_with_existing_message() {
        let source = source_folder();
        assert_eq!(
            validate_options(input(
                &source.to_string_lossy(),
                "relative",
                None,
                "",
                "{name}"
            ))
            .err(),
            Some("The destination must be a full path: relative.".into())
        );
        let _ = std::fs::remove_dir_all(source);
    }

    #[test]
    fn rejects_empty_destination_with_existing_message() {
        let source = source_folder();
        assert_eq!(
            validate_options(input(&source.to_string_lossy(), "  ", None, "", "{name}",)).err(),
            Some("Choose a destination.".into())
        );
        let _ = std::fs::remove_dir_all(source);
    }

    #[test]
    fn rejects_source_inside_destination_with_existing_message() {
        let base = source_folder();
        let source = base.join("source");
        if let Err(error) = std::fs::create_dir_all(&source) {
            panic!("could not create test source {}: {error}", source.display());
        }
        assert_eq!(
            validate_options(input(
                &source.to_string_lossy(),
                &base.to_string_lossy(),
                None,
                "",
                "{name}",
            ))
            .err(),
            Some("The destination must not be on or above the source.".into())
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn invalid_templates_keep_their_field_specific_error_prefixes() {
        let source = source_folder();
        let dest = source.with_file_name("pw-ingest-request-dest");
        let source_text = source.to_string_lossy();
        let dest_text = dest.to_string_lossy();

        let folder_error =
            validate_options(input(&source_text, &dest_text, None, "{invalid}", "{name}"))
                .err()
                .unwrap_or_default();
        assert!(
            folder_error.starts_with("Folder Template: "),
            "{folder_error}"
        );

        let name_error = validate_options(input(&source_text, &dest_text, None, "", "{invalid}"))
            .err()
            .unwrap_or_default();
        assert!(
            name_error.starts_with("File Name Template: "),
            "{name_error}"
        );

        let _ = std::fs::remove_dir_all(source);
    }

    #[test]
    fn accepts_a_separate_backup_folder() {
        let base = source_folder();
        let source = base.join("source");
        let dest = base.join("destination");
        let backup = base.join("backup");
        if let Err(error) = std::fs::create_dir_all(&source) {
            panic!("could not create test source {}: {error}", source.display());
        }
        let options = validate_options(input(
            &source.to_string_lossy(),
            &dest.to_string_lossy(),
            Some(&backup.to_string_lossy()),
            "2026/{date}",
            "{name}",
        ));
        let options = match options {
            Ok(options) => options,
            Err(error) => panic!("{error}"),
        };
        assert_eq!(options.dest, dest);
        assert_eq!(options.backup, Some(backup));

        let _ = std::fs::remove_dir_all(base);
    }
}
