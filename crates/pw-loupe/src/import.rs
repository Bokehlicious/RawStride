use std::path::PathBuf;

use pw_metadata::template::{Mode, Template, TemplateField};
use pw_metadata::value::{LangAlt, Value};
use pw_metadata::{Iptc, LocalizedValue, X_DEFAULT};

use crate::import_text::{folder_template_error_text, name_template_error_text};

/// The editable fields used to prepare an import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportForm {
    pub source: String,
    pub dest: String,
    pub backup_on: bool,
    pub backup: String,
    pub folder: String,
    pub name: String,
    pub job: String,
    pub first_seq: u32,
    pub iptc: IptcForm,
    pub eject: bool,
    /// Copy new images only: leave out the files an earlier import took from the card (the
    /// card's ingest register, ADR 0005 choice 6, T158).
    pub incremental: bool,
}

/// The IPTC fields edited in the import dialog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IptcForm {
    pub creator: String,
    pub copyright: String,
    pub credit: String,
    pub email: String,
    pub phone: String,
    pub url: String,
    /// The mode of each field (ADR 0007 §10, D13).
    pub modes: IptcModes,
}

/// One of the six template fields of the import dialog (ADR 0007 §10, D13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IptcField {
    Creator,
    Copyright,
    Credit,
    Email,
    Phone,
    Url,
}

impl IptcField {
    /// The fields in the order of the dialog.
    pub const ALL: [IptcField; 6] = [
        IptcField::Creator,
        IptcField::Copyright,
        IptcField::Credit,
        IptcField::Email,
        IptcField::Phone,
        IptcField::Url,
    ];
}

/// The mode of each field of the import dialog's template (D13). The default is `Replace` for
/// all six; with an empty text that does nothing, so a form nobody touches writes what the
/// dialog wrote before the modes existed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IptcModes {
    pub creator: Mode,
    pub copyright: Mode,
    pub credit: Mode,
    pub email: Mode,
    pub phone: Mode,
    pub url: Mode,
}

impl Default for IptcModes {
    fn default() -> Self {
        Self {
            creator: Mode::Replace,
            copyright: Mode::Replace,
            credit: Mode::Replace,
            email: Mode::Replace,
            phone: Mode::Replace,
            url: Mode::Replace,
        }
    }
}

impl IptcModes {
    /// The mode of `field`.
    pub fn get(&self, field: IptcField) -> Mode {
        match field {
            IptcField::Creator => self.creator,
            IptcField::Copyright => self.copyright,
            IptcField::Credit => self.credit,
            IptcField::Email => self.email,
            IptcField::Phone => self.phone,
            IptcField::Url => self.url,
        }
    }

    /// The mode of `field`, to change it.
    pub fn get_mut(&mut self, field: IptcField) -> &mut Mode {
        match field {
            IptcField::Creator => &mut self.creator,
            IptcField::Copyright => &mut self.copyright,
            IptcField::Credit => &mut self.credit,
            IptcField::Email => &mut self.email,
            IptcField::Phone => &mut self.phone,
            IptcField::Url => &mut self.url,
        }
    }
}

/// The modes the dialog offers for `field` (ADR 0007 §8, §10; D13): no Clear of the
/// copyright-management fields (Creator, Copyright Notice, Credit Line), no Prefix on a list
/// (Creator).
pub fn allowed_modes(field: IptcField) -> &'static [Mode] {
    match field {
        IptcField::Creator => &[Mode::Keep, Mode::Replace, Mode::Append],
        IptcField::Copyright | IptcField::Credit => {
            &[Mode::Keep, Mode::Replace, Mode::Append, Mode::Prefix]
        }
        IptcField::Email | IptcField::Phone | IptcField::Url => &[
            Mode::Keep,
            Mode::Replace,
            Mode::Append,
            Mode::Prefix,
            Mode::Clear,
        ],
    }
}

/// The structure of the creator's contact details (IPTC Core), as in `Iptc::to_template`.
const CONTACT: &str = "Iptc4xmpCore:CreatorContactInfo";

/// One template field. Replace, Append and Prefix without a value become `Keep` (an empty field
/// never clears and does nothing, as in `Iptc::to_template`); Keep and Clear carry no value.
fn template_field(
    property: &str,
    sub: Option<&str>,
    mode: Mode,
    value: Option<Value>,
) -> TemplateField {
    let (mode, value) = match (mode, value) {
        (Mode::Clear, _) => (Mode::Clear, None),
        (Mode::Keep, _) | (_, None) => (Mode::Keep, None),
        (mode, Some(value)) => (mode, Some(value)),
    };
    TemplateField {
        property: property.to_string(),
        field: sub.map(str::to_string),
        mode,
        value,
    }
}

impl IptcForm {
    /// The text of `field`.
    pub fn text(&self, field: IptcField) -> &str {
        match field {
            IptcField::Creator => &self.creator,
            IptcField::Copyright => &self.copyright,
            IptcField::Credit => &self.credit,
            IptcField::Email => &self.email,
            IptcField::Phone => &self.phone,
            IptcField::Url => &self.url,
        }
    }

    /// The text of `field`, to change it.
    pub fn text_mut(&mut self, field: IptcField) -> &mut String {
        match field {
            IptcField::Creator => &mut self.creator,
            IptcField::Copyright => &mut self.copyright,
            IptcField::Credit => &mut self.credit,
            IptcField::Email => &mut self.email,
            IptcField::Phone => &mut self.phone,
            IptcField::Url => &mut self.url,
        }
    }

    /// The template of the import (ADR 0007 §10; D13, D6): the same fields in the same order as
    /// [`Iptc::to_template`], each with its mode and its trimmed text. Creator is `dc:creator` as
    /// a list of one name, Copyright the `x-default` of `dc:rights`, Credit `photoshop:Credit`,
    /// e-mail, phone and URL fields of `Iptc4xmpCore:CreatorContactInfo`; `dc:title` and
    /// `dc:description`, which the form does not have, are `Keep`. An empty field never clears
    /// (§10): Replace, Append or Prefix with an empty text is `Keep` without a value, so the
    /// default form gives exactly the template of `Iptc::to_template`. Clear has no value.
    /// `xmpRights:Marked` is `True` with `Replace` when Copyright has a text and the mode Replace,
    /// Append or Prefix (D6), and `Keep` otherwise.
    pub fn to_template(&self) -> Template {
        let trimmed = |text: &str| Some(text.trim().to_string()).filter(|text| !text.is_empty());
        let field = |property: &str, sub: Option<&str>, which: IptcField| {
            let value = trimmed(self.text(which)).map(|text| match which {
                IptcField::Creator => Value::List(vec![text]),
                IptcField::Copyright => {
                    Value::LangAlt(LangAlt::from_entries(vec![(X_DEFAULT.to_string(), text)]))
                }
                IptcField::Credit | IptcField::Email | IptcField::Phone | IptcField::Url => {
                    Value::Text(text)
                }
            });
            template_field(property, sub, self.modes.get(which), value)
        };
        let copyright = field("dc:rights", None, IptcField::Copyright);
        let marked = match copyright.mode {
            Mode::Replace | Mode::Append | Mode::Prefix => Some(Value::Text("True".to_string())),
            Mode::Keep | Mode::Clear => None,
        };
        Template {
            fields: vec![
                field("dc:creator", None, IptcField::Creator),
                template_field("dc:title", None, Mode::Keep, None),
                template_field("dc:description", None, Mode::Keep, None),
                copyright,
                field("photoshop:Credit", None, IptcField::Credit),
                field(CONTACT, Some("Iptc4xmpCore:CiEmailWork"), IptcField::Email),
                field(CONTACT, Some("Iptc4xmpCore:CiTelWork"), IptcField::Phone),
                field(CONTACT, Some("Iptc4xmpCore:CiUrlWork"), IptcField::Url),
                template_field("xmpRights:Marked", None, Mode::Replace, marked),
            ],
        }
    }

    /// True when no field would do anything: each is `Keep`, or Replace, Append or Prefix with
    /// an empty text.
    pub fn template_is_empty(&self) -> bool {
        self.to_template()
            .fields
            .iter()
            .all(|field| field.mode == Mode::Keep)
    }

    /// The fields in `Clear`, in the order of the dialog.
    pub fn cleared_fields(&self) -> Vec<IptcField> {
        IptcField::ALL
            .into_iter()
            .filter(|field| self.modes.get(*field) == Mode::Clear)
            .collect()
    }

    /// Converts the plain-text form fields to the metadata template.
    pub fn to_iptc(&self) -> Iptc {
        Iptc {
            creator: self.creator.clone(),
            copyright: LocalizedValue::x_default(self.copyright.clone()),
            title: LocalizedValue::default(),
            description: LocalizedValue::default(),
            credit: self.credit.clone(),
            email: self.email.clone(),
            phone: self.phone.clone(),
            url: self.url.clone(),
        }
    }
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
            job: String::new(),
            first_seq: 1,
            iptc: IptcForm::default(),
            eject: false,
            incremental: true,
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
            message: folder_template_error_text(&error.to_string()),
        })?;
        pw_ingest::Template::parse(&self.name).map_err(|error| FieldError {
            field: Field::Name,
            message: name_template_error_text(&error.to_string()),
        })?;

        let mut options = pw_ingest::validate_options(pw_ingest::OptionsInput {
            source: &self.source,
            dest: &self.dest,
            backup: None,
            folder: &self.folder,
            name: &self.name,
            job: &self.job,
            first_seq: self.first_seq,
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
                job: &self.job,
                first_seq: self.first_seq,
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
    /// The files left out because an earlier import took them (T158).
    pub earlier: usize,
    /// Why earlier imports cannot be recognised or the card cannot be marked, if so (T158).
    pub register_note: Option<String>,
}

impl Preview {
    /// Summarizes shots and the corresponding plan without touching files; `earlier` files were
    /// left out because an earlier import took them, and `register_note` says why earlier
    /// imports cannot be recognised or the card cannot be marked.
    pub fn new(
        shots: &[pw_ingest::Shot],
        plan: &pw_ingest::Plan,
        earlier: usize,
        register_note: Option<String>,
    ) -> Self {
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
            earlier,
            register_note,
        }
    }
}

/// Chooses the destination folders to open after an import, each in its own tab (T163): none when
/// nothing was copied (ADR 0010 Q2), else every folder the import copied into, in their order.
pub fn result_folders(copied: usize, new_folders: &[PathBuf]) -> Vec<PathBuf> {
    if copied == 0 {
        return Vec::new();
    }
    new_folders.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_text::clear_confirmation_text;
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
                job: String::new(),
                first_seq: 1,
                iptc: IptcForm::default(),
                eject: false,
                incremental: true,
            }
        );
    }

    #[test]
    fn the_form_copies_new_images_only_by_default() {
        assert!(ImportForm::default().incremental);
    }

    #[test]
    fn iptc_form_maps_copyright_to_x_default_and_empty_form_is_empty() {
        let empty = IptcForm::default().to_iptc();
        assert!(empty.is_empty());

        let iptc = IptcForm {
            creator: "Fotograf".to_string(),
            copyright: "© 2026 Namn".to_string(),
            credit: "Byrå".to_string(),
            email: "namn@example.com".to_string(),
            phone: "+46 123".to_string(),
            url: "https://example.com".to_string(),
            ..IptcForm::default()
        }
        .to_iptc();
        assert_eq!(iptc.creator, "Fotograf");
        assert_eq!(iptc.copyright.default_text(), "© 2026 Namn");
        assert!(iptc.title.is_empty());
        assert!(iptc.description.is_empty());
        assert_eq!(iptc.credit, "Byrå");
        assert_eq!(iptc.email, "namn@example.com");
        assert_eq!(iptc.phone, "+46 123");
        assert_eq!(iptc.url, "https://example.com");
    }

    fn filled_form() -> IptcForm {
        IptcForm {
            creator: " Fotograf ".to_string(),
            copyright: "© 2026 Namn".to_string(),
            credit: "Byrå".to_string(),
            email: "namn@example.com".to_string(),
            phone: "+46 123".to_string(),
            url: " https://example.com ".to_string(),
            ..IptcForm::default()
        }
    }

    fn template_field_of<'a>(
        template: &'a Template,
        property: &str,
        sub: Option<&str>,
    ) -> &'a TemplateField {
        template
            .fields
            .iter()
            .find(|field| field.property == property && field.field.as_deref() == sub)
            .unwrap_or_else(|| panic!("{property} {sub:?} missing in {template:?}"))
    }

    fn text(text: &str) -> Option<Value> {
        Some(Value::Text(text.to_string()))
    }

    #[test]
    fn default_modes_give_the_template_of_to_iptc() {
        assert_eq!(IptcForm::default().modes, IptcModes::default());
        for field in IptcField::ALL {
            assert_eq!(IptcForm::default().modes.get(field), Mode::Replace);
        }
        // All six filled.
        let form = filled_form();
        assert_eq!(form.to_template(), form.to_iptc().to_template());
        // None filled.
        let empty = IptcForm::default();
        assert_eq!(empty.to_template(), empty.to_iptc().to_template());
        // Only white space.
        let blank = IptcForm {
            creator: "  ".to_string(),
            copyright: " \t".to_string(),
            url: "\r\n".to_string(),
            ..IptcForm::default()
        };
        assert_eq!(blank.to_template(), blank.to_iptc().to_template());
    }

    #[test]
    fn to_template_uses_each_fields_mode() {
        let mut form = filled_form();
        form.modes.creator = Mode::Append;
        form.modes.credit = Mode::Prefix;
        form.modes.email = Mode::Clear;
        form.modes.copyright = Mode::Keep;
        let template = form.to_template();

        assert_eq!(
            template_field_of(&template, "dc:creator", None),
            &TemplateField {
                property: "dc:creator".to_string(),
                field: None,
                mode: Mode::Append,
                value: Some(Value::List(vec!["Fotograf".to_string()])),
            }
        );
        assert_eq!(
            template_field_of(&template, "photoshop:Credit", None),
            &TemplateField {
                property: "photoshop:Credit".to_string(),
                field: None,
                mode: Mode::Prefix,
                value: text("Byrå"),
            }
        );
        assert_eq!(
            template_field_of(&template, CONTACT, Some("Iptc4xmpCore:CiEmailWork")),
            &TemplateField {
                property: CONTACT.to_string(),
                field: Some("Iptc4xmpCore:CiEmailWork".to_string()),
                mode: Mode::Clear,
                value: None,
            }
        );
        // Keep on a filled Copyright: nothing written, so no xmpRights:Marked either.
        let rights = template_field_of(&template, "dc:rights", None);
        assert_eq!((rights.mode, &rights.value), (Mode::Keep, &None));
        let marked = template_field_of(&template, "xmpRights:Marked", None);
        assert_eq!((marked.mode, &marked.value), (Mode::Keep, &None));
        // The other fields keep their Replace.
        let url = template_field_of(&template, CONTACT, Some("Iptc4xmpCore:CiUrlWork"));
        assert_eq!(
            (url.mode, &url.value),
            (Mode::Replace, &text("https://example.com"))
        );
    }

    #[test]
    fn copyright_marks_the_file_with_replace_append_and_prefix() {
        for mode in [Mode::Replace, Mode::Append, Mode::Prefix] {
            let mut form = filled_form();
            form.modes.copyright = mode;
            let template = form.to_template();
            let rights = template_field_of(&template, "dc:rights", None);
            assert_eq!(rights.mode, mode);
            assert_eq!(
                rights.value,
                Some(Value::LangAlt(LangAlt::from_entries(vec![(
                    "x-default".to_string(),
                    "© 2026 Namn".to_string()
                )])))
            );
            let marked = template_field_of(&template, "xmpRights:Marked", None);
            assert_eq!(
                (marked.mode, &marked.value),
                (Mode::Replace, &text("True")),
                "{mode:?}"
            );
        }
        // Append without a text does nothing, so no mark.
        let mut form = IptcForm::default();
        form.modes.copyright = Mode::Append;
        let template = form.to_template();
        let marked = template_field_of(&template, "xmpRights:Marked", None);
        assert_eq!((marked.mode, &marked.value), (Mode::Keep, &None));
    }

    #[test]
    fn allowed_modes_of_each_field() {
        assert_eq!(
            allowed_modes(IptcField::Creator),
            [Mode::Keep, Mode::Replace, Mode::Append]
        );
        for field in [IptcField::Copyright, IptcField::Credit] {
            assert_eq!(
                allowed_modes(field),
                [Mode::Keep, Mode::Replace, Mode::Append, Mode::Prefix],
                "{field:?}"
            );
        }
        for field in [IptcField::Email, IptcField::Phone, IptcField::Url] {
            assert_eq!(
                allowed_modes(field),
                [
                    Mode::Keep,
                    Mode::Replace,
                    Mode::Append,
                    Mode::Prefix,
                    Mode::Clear
                ],
                "{field:?}"
            );
        }
        // The default mode is offered for every field.
        for field in IptcField::ALL {
            assert!(allowed_modes(field).contains(&Mode::Replace), "{field:?}");
        }
    }

    #[test]
    fn modes_and_texts_are_reached_by_field() {
        let mut form = IptcForm::default();
        for (index, field) in IptcField::ALL.into_iter().enumerate() {
            *form.text_mut(field) = format!("text {index}");
            *form.modes.get_mut(field) = Mode::Keep;
        }
        assert_eq!(
            [
                &form.creator,
                &form.copyright,
                &form.credit,
                &form.email,
                &form.phone,
                &form.url
            ],
            ["text 0", "text 1", "text 2", "text 3", "text 4", "text 5"]
        );
        for (index, field) in IptcField::ALL.into_iter().enumerate() {
            assert_eq!(form.text(field), format!("text {index}"));
            assert_eq!(form.modes.get(field), Mode::Keep);
        }
    }

    #[test]
    fn template_is_empty_when_no_field_does_anything() {
        let mut all_keep = filled_form();
        for field in IptcField::ALL {
            *all_keep.modes.get_mut(field) = Mode::Keep;
        }
        assert!(all_keep.template_is_empty());

        assert!(IptcForm::default().template_is_empty());
        let mut joins_without_text = IptcForm::default();
        joins_without_text.modes.creator = Mode::Append;
        joins_without_text.modes.credit = Mode::Prefix;
        assert!(joins_without_text.template_is_empty());

        let mut one_clear = IptcForm::default();
        one_clear.modes.phone = Mode::Clear;
        assert!(!one_clear.template_is_empty());

        assert!(!filled_form().template_is_empty());
    }

    #[test]
    fn cleared_fields_and_their_confirmation_text() {
        assert_eq!(
            IptcForm::default().cleared_fields(),
            Vec::<IptcField>::new()
        );

        let mut form = filled_form();
        form.modes.url = Mode::Clear;
        form.modes.email = Mode::Clear;
        let cleared = form.cleared_fields();
        assert_eq!(cleared, [IptcField::Email, IptcField::Url]);
        assert_eq!(
            clear_confirmation_text(&cleared),
            "The template clears these fields in every imported image:\nEmail\nWeb URL"
        );
    }

    #[test]
    fn empty_source_has_source_field_error() {
        let error = ImportForm::default().validate().unwrap_err();
        assert_eq!(error.field, Field::Source);
        assert_eq!(error.message, "Choose a source.");
    }

    #[test]
    fn empty_destination_has_destination_field_error() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.dest.clear();

        let error = form.validate().unwrap_err();
        assert_eq!(error.field, Field::Dest);
        assert_eq!(error.message, "Choose a destination.");
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
                .contains("must not be on or above the source.")
        );
    }

    #[test]
    fn bad_folder_template_has_folder_field_error() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.folder = "{invalid}".to_string();

        let error = form.validate().unwrap_err();
        assert_eq!(error.field, Field::Folder);
        assert!(error.message.starts_with("Folder Template: "));
    }

    #[test]
    fn bad_name_template_has_name_field_error() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.name = "{invalid}".to_string();

        let error = form.validate().unwrap_err();
        assert_eq!(error.field, Field::Name);
        assert!(error.message.starts_with("File Name Template: "));
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
    fn job_and_first_seq_are_passed_to_options() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.job = "jobb".to_string();
        form.first_seq = 42;

        let validated = form.validate().unwrap();

        assert_eq!(validated.options.job, "jobb");
        assert_eq!(validated.options.first_seq, 42);
    }

    #[test]
    fn job_is_trimmed_before_it_reaches_options() {
        let dirs = TempDirs::new();
        let mut form = valid_form(&dirs);
        form.job = "  jobb  ".to_string();

        let validated = form.validate().unwrap();

        assert_eq!(validated.options.job, "jobb");
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
            "The backup destination must be a different folder from the destination."
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

        let preview = Preview::new(&shots, &plan, 0, None);
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
    fn preview_passes_earlier_and_the_register_note_through() {
        let plan = pw_ingest::Plan { items: Vec::new() };
        let preview = Preview::new(&[], &plan, 7, Some("note".to_string()));
        assert_eq!(preview.earlier, 7);
        assert_eq!(preview.register_note.as_deref(), Some("note"));
        assert_eq!(preview.shots, 0);
    }

    #[test]
    fn result_folders_are_empty_without_copied_files_and_every_new_folder_otherwise() {
        let folders = vec![
            PathBuf::from("destination/2026-11-15"),
            PathBuf::from("destination/2026-11-16"),
        ];
        assert_eq!(result_folders(0, &folders), Vec::<PathBuf>::new());
        assert_eq!(result_folders(2, &folders), folders);
        assert_eq!(result_folders(1, &[]), Vec::<PathBuf>::new());
    }
}
