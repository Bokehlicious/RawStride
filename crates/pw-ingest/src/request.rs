use std::path::PathBuf;

use crate::{Options, Template};

/// Validates the selected source folder.
pub fn validate_source(source: &str) -> Result<PathBuf, String> {
    let source = source.trim();
    if source.is_empty() {
        return Err("Välj en källa.".into());
    }
    let path = PathBuf::from(source);
    if !path.is_absolute() {
        return Err(format!("Källan måste vara en fullständig sökväg: {source}"));
    }
    if !path.is_dir() {
        return Err(format!("Källan {source} finns inte."));
    }
    Ok(path)
}

/// Validates an import request and creates the options used for planning and copying.
pub fn validate_options(
    source: &str,
    dest: &str,
    backup: Option<&str>,
    folder: &str,
    name: &str,
    job: &str,
    first_seq: u32,
) -> Result<Options, String> {
    let source = validate_source(source)?;
    let dest = dest.trim();
    if dest.is_empty() {
        return Err("Välj ett mål.".into());
    }
    let dest = PathBuf::from(dest);
    let backup = backup
        .map(str::trim)
        .filter(|backup| !backup.is_empty())
        .map(PathBuf::from);

    let source_resolved = crate::resolve(&source, "Källan")?;
    let dest_resolved = crate::resolve(&dest, "Målet")?;
    let backup_resolved = backup
        .as_deref()
        .map(|path| crate::resolve(path, "Backupmålet"))
        .transpose()?;

    for (what, path) in [
        ("Målet", Some(&dest_resolved)),
        ("Backupmålet", backup_resolved.as_ref()),
    ] {
        if let Some(path) = path
            && (crate::inside(path, &source_resolved) || crate::inside(&source_resolved, path))
        {
            return Err(format!("{what} får inte ligga på eller över källan."));
        }
    }
    if let Some(backup) = &backup_resolved
        && (crate::inside(backup, &dest_resolved) || crate::inside(&dest_resolved, backup))
    {
        return Err("Backupmålet måste vara en annan mapp än målet.".into());
    }

    Ok(Options {
        dest,
        backup,
        folder: Template::parse(folder).map_err(|e| format!("Mappmall: {e}"))?,
        name: Template::parse(name).map_err(|e| format!("Filnamnsmall: {e}"))?,
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

    #[test]
    fn validates_source_and_preserves_error_messages() {
        assert_eq!(validate_source("  "), Err("Välj en källa.".into()));
        assert_eq!(
            validate_source("relative"),
            Err("Källan måste vara en fullständig sökväg: relative".into())
        );

        let missing_parent = source_folder();
        let missing = missing_parent.join("missing");
        assert_eq!(
            validate_source(&missing.to_string_lossy()),
            Err(format!("Källan {} finns inte.", missing.display()))
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
        let options = validate_options(
            &source.to_string_lossy(),
            &format!(" {} ", dest.display()),
            Some(" "),
            "2026/{date}",
            "{name}_{seq:4}",
            "  jobb  ",
            7,
        );
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
            validate_options(
                &source.to_string_lossy(),
                &nested_dest.to_string_lossy(),
                None,
                "",
                "{name}",
                "",
                1,
            )
            .err(),
            Some("Målet får inte ligga på eller över källan.".into())
        );

        let dest = source.with_file_name("pw-ingest-request-dest");
        let backup = dest.join("backup");
        assert_eq!(
            validate_options(
                &source.to_string_lossy(),
                &dest.to_string_lossy(),
                Some(&backup.to_string_lossy()),
                "",
                "{name}",
                "",
                1,
            )
            .err(),
            Some("Backupmålet måste vara en annan mapp än målet.".into())
        );
        assert!(
            validate_options(
                &source.to_string_lossy(),
                &dest.to_string_lossy(),
                None,
                "{invalid}",
                "{name}",
                "",
                1,
            )
            .is_err()
        );
        let _ = std::fs::remove_dir_all(source);
    }

    #[test]
    fn rejects_relative_destination_with_existing_message() {
        let source = source_folder();
        assert_eq!(
            validate_options(
                &source.to_string_lossy(),
                "relative",
                None,
                "",
                "{name}",
                "",
                1
            )
            .err(),
            Some("Målet måste vara en fullständig sökväg: relative".into())
        );
        let _ = std::fs::remove_dir_all(source);
    }
}
