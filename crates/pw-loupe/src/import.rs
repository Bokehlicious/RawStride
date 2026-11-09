use std::path::PathBuf;

/// The editable fields used to prepare an import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportForm {
    pub source: String,
    pub dest: String,
    pub backup_on: bool,
    pub backup: String,
    pub folder: String,
    pub name: String,
}

impl Default for ImportForm {
    fn default() -> Self {
        Self {
            source: String::new(),
            dest: String::new(),
            backup_on: false,
            backup: String::new(),
            folder: "{year}/{date} {job}".to_string(),
            name: "{name}".to_string(),
        }
    }
}

/// The form field associated with a validation error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Source,
    Dest,
    Backup,
    Folder,
    Name,
}

/// A validation error associated with one import form field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldError {
    pub field: Field,
    pub message: String,
}

/// A validated source and the options used for planning and copying.
#[derive(Clone, Debug)]
pub struct Validated {
    pub source: PathBuf,
    pub options: pw_ingest::Options,
}

impl ImportForm {
    /// Validates the form using the shared ingest rules and messages.
    pub fn validate(&self) -> Result<Validated, FieldError> {
        let source = pw_ingest::validate_source(&self.source).map_err(|message| FieldError {
            field: Field::Source,
            message,
        })?;

        pw_ingest::Template::parse(&self.folder).map_err(|error| FieldError {
            field: Field::Folder,
            message: format!("Mappmall: {error}"),
        })?;
        pw_ingest::Template::parse(&self.name).map_err(|error| FieldError {
            field: Field::Name,
            message: format!("Filnamnsmall: {error}"),
        })?;

        let mut options = pw_ingest::validate_options(pw_ingest::OptionsInput {
            source: &self.source,
            dest: &self.dest,
            backup: None,
            folder: &self.folder,
            name: &self.name,
            job: "",
            first_seq: 1,
        })
        .map_err(|message| FieldError {
            field: Field::Dest,
            message,
        })?;

        if let Some(backup) = self
            .backup_on
            .then(|| self.backup.trim())
            .filter(|backup| !backup.is_empty())
        {
            options = pw_ingest::validate_options(pw_ingest::OptionsInput {
                source: &self.source,
                dest: &self.dest,
                backup: Some(backup),
                folder: &self.folder,
                name: &self.name,
                job: "",
                first_seq: 1,
            })
            .map_err(|message| FieldError {
                field: Field::Backup,
                message,
            })?;
        }

        Ok(Validated { source, options })
    }
}

/// The action represented by a planned item in the preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemAction {
    Copy,
    Renamed,
    Present,
    BackupOnly,
}

/// One of the first planned items shown in the preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewLine {
    pub source_name: String,
    pub rel: String,
    pub action: ItemAction,
}

/// A summary of an import plan suitable for displaying in the form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub shots: usize,
    pub files_to_copy: usize,
    pub bytes_to_copy: u64,
    pub present: usize,
    pub renamed: usize,
    pub no_exif: usize,
    pub items_total: usize,
    pub lines: Vec<PreviewLine>,
}

impl Preview {
    /// Summarizes shots and the corresponding plan without touching files.
    pub fn new(shots: &[pw_ingest::Shot], plan: &pw_ingest::Plan) -> Self {
        Self {
            shots: shots.len(),
            files_to_copy: plan.files_to_copy(),
            bytes_to_copy: plan.bytes_to_copy(),
            present: plan.already_present(),
            renamed: plan
                .items
                .iter()
                .filter(|item| item.renamed_for_conflict && (item.to_dest || item.to_backup))
                .count(),
            no_exif: shots.iter().filter(|shot| !shot.time_from_exif).count(),
            items_total: plan.items.len(),
            lines: plan
                .items
                .iter()
                .take(20)
                .map(|item| PreviewLine {
                    source_name: item
                        .source
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    rel: item.rel.to_string_lossy().into_owned(),
                    action: match (item.to_dest, item.to_backup) {
                        (false, false) => ItemAction::Present,
                        (true, _) if item.renamed_for_conflict => ItemAction::Renamed,
                        (true, _) => ItemAction::Copy,
                        (false, true) => ItemAction::BackupOnly,
                    },
                })
                .collect(),
        }
    }
}

/// Chooses the destination folder to open after an import.
pub fn result_folder(copied: usize, folders: &[(String, usize)]) -> Option<PathBuf> {
    if copied == 0 {
        return None;
    }
    folders.first().map(|(folder, _)| PathBuf::from(folder))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::SystemTime;

    static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

    struct TempDirs {
        root: PathBuf,
        source: PathBuf,
    }

    impl TempDirs {
        fn new() -> Self {
            let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let root =
                std::env::temp_dir().join(format!("pw-loupe-import-{}-{id}", std::process::id()));
            let source = root.join("source");
            std::fs::create_dir_all(&source)
                .unwrap_or_else(|error| panic!("could not create {}: {error}", source.display()));
            Self { root, source }
        }
    }

    impl Drop for TempDirs {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn valid_form(dirs: &TempDirs) -> ImportForm {
        ImportForm {
            source: dirs.source.to_string_lossy().into_owned(),
            dest: dirs.root.join("destination").to_string_lossy().into_owned(),
            ..ImportForm::default()
        }
    }

    fn plan_item(
        index: usize,
        to_dest: bool,
        to_backup: bool,
        renamed: bool,
    ) -> pw_ingest::PlanItem {
        pw_ingest::PlanItem {
            source: PathBuf::from(format!("source/image-{index}.raw")),
            size: 10,
            modified: SystemTime::UNIX_EPOCH,
            rel: PathBuf::from(format!("folder/image-{index}.raw")),
            to_dest,
            to_backup,
            renamed_for_conflict: renamed,
        }
    }

    #[test]
    fn form_defaults_match_the_ingest_templates() {
        assert_eq!(
            ImportForm::default(),
            ImportForm {
                source: String::new(),
                dest: String::new(),
                backup_on: false,
                backup: String::new(),
                folder: "{year}/{date} {job}".to_string(),
                name: "{name}".to_string(),
            }
        );
    }

    #[test]
    fn empty_source_has_source_field_error() {
        let error = ImportForm::default().validate().unwrap_err();
        assert_eq!(error.field, Field::Source);
        assert_eq!(error.message, "Välj en källa.");
    }

    #[test]
    fn empty_destination_has_destination_field_error() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.dest.clear();

        let error = form.validate().unwrap_err();
        assert_eq!(error.field, Field::Dest);
        assert_eq!(error.message, "Välj ett mål.");
    }

    #[test]
    fn destination_inside_source_has_destination_field_error() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.dest = dirs
            .source
            .join("destination")
            .to_string_lossy()
            .into_owned();

        let error = form.validate().unwrap_err();
        assert_eq!(error.field, Field::Dest);
        assert!(
            error
                .message
                .contains("får inte ligga på eller över källan.")
        );
    }

    #[test]
    fn bad_folder_template_has_folder_field_error() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.folder = "{invalid}".to_string();

        let error = form.validate().unwrap_err();
        assert_eq!(error.field, Field::Folder);
        assert!(error.message.starts_with("Mappmall: "));
    }

    #[test]
    fn bad_name_template_has_name_field_error() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.name = "{invalid}".to_string();

        let error = form.validate().unwrap_err();
        assert_eq!(error.field, Field::Name);
        assert!(error.message.starts_with("Filnamnsmall: "));
    }

    #[test]
    fn valid_form_produces_source_destination_and_templates() {
        let dirs = TempDirs::new();
        let form = valid_form(&dirs);

        let validated = form.validate().unwrap();
        assert_eq!(validated.source, dirs.source);
        assert_eq!(validated.options.dest, dirs.root.join("destination"));
        assert_eq!(
            validated.options.folder,
            pw_ingest::Template::parse(&form.folder).unwrap()
        );
        assert_eq!(
            validated.options.name,
            pw_ingest::Template::parse(&form.name).unwrap()
        );
        assert!(validated.options.backup.is_none());
        assert_eq!(validated.options.job, "");
        assert_eq!(validated.options.first_seq, 1);
    }

    #[test]
    fn backup_is_ignored_when_disabled_even_if_a_folder_is_entered() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.backup = dirs.root.join("backup").to_string_lossy().into_owned();

        let validated = form.validate().unwrap();
        assert!(validated.options.backup.is_none());
    }

    #[test]
    fn enabled_backup_folder_is_trimmed_and_passed_to_options() {
        let dirs = TempDirs::new();
        let backup = dirs.root.join("backup");
        let mut form = valid_form(&dirs);
        form.backup_on = true;
        form.backup = format!("  {}  ", backup.display());

        let validated = form.validate().unwrap();
        assert_eq!(validated.options.backup, Some(backup));
    }

    #[test]
    fn enabled_empty_backup_is_none() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.backup_on = true;
        form.backup = "  ".to_string();

        let validated = form.validate().unwrap();
        assert!(validated.options.backup.is_none());
    }

    #[test]
    fn backup_inside_source_has_backup_field_error() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.backup_on = true;
        form.backup = dirs.source.join("backup").to_string_lossy().into_owned();

        let error = form.validate().unwrap_err();
        assert_eq!(error.field, Field::Backup);
    }

    #[test]
    fn backup_equal_to_destination_has_backup_field_error() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.backup_on = true;
        form.backup = form.dest.clone();

        let error = form.validate().unwrap_err();
        assert_eq!(error.field, Field::Backup);
        assert_eq!(
            error.message,
            "Backupmålet måste vara en annan mapp än målet."
        );
    }

    #[test]
    fn invalid_destination_is_reported_before_invalid_backup() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.dest = dirs
            .source
            .join("destination")
            .to_string_lossy()
            .into_owned();
        form.backup_on = true;
        form.backup = dirs.source.join("backup").to_string_lossy().into_owned();

        let error = form.validate().unwrap_err();
        assert_eq!(error.field, Field::Dest);
    }

    #[test]
    fn preview_counts_items_and_shows_first_twenty_actions() {
        let shots = vec![
            pw_ingest::Shot {
                stem: "with-exif".to_string(),
                files: Vec::new(),
                time: None,
                time_from_exif: true,
                make: None,
                model: None,
            },
            pw_ingest::Shot {
                stem: "without-exif".to_string(),
                files: Vec::new(),
                time: None,
                time_from_exif: false,
                make: None,
                model: None,
            },
        ];
        let mut items = vec![
            plan_item(0, true, false, false),
            plan_item(1, true, false, true),
            plan_item(2, false, false, true),
            plan_item(3, false, true, false),
        ];
        items.extend((4..22).map(|index| plan_item(index, true, false, false)));
        let plan = pw_ingest::Plan { items };

        let preview = Preview::new(&shots, &plan);
        assert_eq!(preview.shots, 2);
        assert_eq!(preview.files_to_copy, 21);
        assert_eq!(preview.bytes_to_copy, 210);
        assert_eq!(preview.present, 1);
        assert_eq!(preview.renamed, 1);
        assert_eq!(preview.no_exif, 1);
        assert_eq!(preview.items_total, 22);
        assert_eq!(preview.lines.len(), 20);
        assert_eq!(
            preview
                .lines
                .iter()
                .map(|line| line.action)
                .take(4)
                .collect::<Vec<_>>(),
            vec![
                ItemAction::Copy,
                ItemAction::Renamed,
                ItemAction::Present,
                ItemAction::BackupOnly
            ]
        );
        assert_eq!(preview.lines[0].source_name, "image-0.raw");
        assert_eq!(
            preview.lines[0].rel,
            PathBuf::from("folder/image-0.raw").to_string_lossy()
        );
    }

    #[test]
    fn result_folder_is_none_without_copied_files_and_uses_first_folder_otherwise() {
        let folders = vec![
            ("destination/largest".to_string(), 3),
            ("destination/other".to_string(), 1),
        ];
        assert_eq!(result_folder(0, &folders), None);
        assert_eq!(
            result_folder(1, &folders),
            Some(PathBuf::from("destination/largest"))
        );
        assert_eq!(result_folder(1, &[]), None);
    }
}
