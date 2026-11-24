//! The template window's form (ADR 0017 U13, part U13c; U13-Q3, U13-Q4, U13-Q12, U13-Q13,
//! U13-Q15; T291). Pure functions, no egui; T293 draws the window.
//!
//! - **The form** ([`TemplateForm`]): the metadata window's form ([`EditorForm`]) with no image
//!   behind it, so every start text is empty and whatever is in it counts as typed. T293 draws it
//!   with the metadata window's field widgets ([`TemplateForm::form`],
//!   [`TemplateForm::form_mut`]).
//! - **The rows** ([`template_rows`], U13-Q3, U13-Q13): every section and item of the metadata
//!   window's layout in its order, hidden ones included, without Rating and Label and without the
//!   fields that are not editable. Each row with a check box is a [`TemplateItem`]; Creator's
//!   Contact Info and Location Created are a heading over one row per sub-field.
//! - **Check boxes and + boxes** (U13-Q4, U13-Q12): only the checked items are saved; typing in an
//!   item checks it ([`TemplateForm::notice_typing`]). A + box is on a list of plain text values
//!   (`can_add`).
//! - **The template** ([`TemplateForm::template`]) and **Load** ([`TemplateForm::load`]): a
//!   [`TemplateFile`] made from the checked items, and the form filled from one.
//! - **Texts** ([`TemplateText`], [`template_text`] and the functions after it): every text the
//!   template window shows (`docs/tasks/README.md` rule 13).

use std::path::{Path, PathBuf};

use pw_metadata::template_file::{TemplateFile, TemplateFileError, TemplateRead, can_add};
use pw_metadata::value::{StructValue, Value};

use crate::editor::{EditorField, EditorForm, EditorSection, TemplateValues, variable_error_text};
use crate::editor_layout::{EditorItem, EditorLayout};
use crate::info_panel::{Language, property_label};

/// The XMP id of Creator's Contact Info, the structure of the eight contact fields.
const CONTACT_INFO: &str = "Iptc4xmpCore:CreatorContactInfo";

/// The separator of the parts of a sub-field's name (`{property} – {sub-field}`) and of the window
/// title's file name.
const NAME_SEPARATOR: &str = " – ";

/// A row of the template form with a check box (U13-Q4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TemplateItem {
    /// A field that is not a contact field.
    Field(EditorField),
    /// One of the eight contact fields, drawn indented under the heading "Creator's Contact Info".
    ContactField(EditorField),
    /// One of Location Created's seven sub-fields ([`EditorForm::LOCATION_FIELDS`]) by its XMP id,
    /// drawn indented under the heading "Location Created". The form's Location Created list
    /// always has exactly one entry.
    LocationField(&'static str),
    /// Any other structure list by its XMP id, one check box for the whole list.
    List(&'static str),
}

impl TemplateItem {
    /// The XMP id of the property the item puts in the template: the field's,
    /// `Iptc4xmpCore:CreatorContactInfo` for a contact field, `Iptc4xmpExt:LocationCreated` for a
    /// sub-field of Location Created, or the list's.
    pub fn property(self) -> &'static str {
        match self {
            TemplateItem::Field(field) => field.property(),
            TemplateItem::ContactField(_) => CONTACT_INFO,
            TemplateItem::LocationField(_) => EditorForm::LOCATION_CREATED,
            TemplateItem::List(list) => list,
        }
    }

    /// The item's name: the field's name, or [`property_label`] of the sub-field or the list.
    pub fn label(self) -> String {
        match self {
            TemplateItem::Field(field) | TemplateItem::ContactField(field) => field.label(),
            TemplateItem::LocationField(id) | TemplateItem::List(id) => {
                property_label(id, Language::En)
            }
        }
    }

    /// True for an item with a + box (U13-Q12): a field whose property is a list of plain text
    /// values (`can_add`).
    pub fn has_add_box(self) -> bool {
        matches!(self, TemplateItem::Field(field) if can_add(field.property()))
    }
}

/// A row of a section of the template form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemplateRow {
    /// A heading without a check box over the rows of a structure's sub-fields: the property's
    /// XMP id (Creator's Contact Info, Location Created).
    Heading(&'static str),
    /// A row with a check box.
    Item(TemplateItem),
}

impl TemplateRow {
    /// The row's name: [`property_label`] of a heading, [`TemplateItem::label`] of an item.
    pub fn label(self) -> String {
        match self {
            TemplateRow::Heading(property) => property_label(property, Language::En),
            TemplateRow::Item(item) => item.label(),
        }
    }
}

/// The template form's sections and their rows for `layout` (U13-Q3, U13-Q4, U13-Q13): every
/// section and item in the layout's order, **hidden ones included**, without the Rating and Label
/// section (U13-Q10) and without the fields that are not editable (Urgency, Category and
/// Supplemental Categories). Creator's Contact Info is a [`TemplateRow::Heading`] followed by its
/// eight contact fields; Location Created a heading followed by its seven sub-fields. A section's
/// row count, its headings included, is the length of its rows.
pub fn template_rows(layout: &EditorLayout) -> Vec<(EditorSection, Vec<TemplateRow>)> {
    layout
        .sections()
        .into_iter()
        .filter(|(section, _)| *section != EditorSection::RatingLabel)
        .map(|(section, _)| {
            let mut rows = Vec::new();
            for (item, _) in layout.items(section) {
                match item {
                    EditorItem::Field(field) => {
                        if field.editable() {
                            rows.push(TemplateRow::Item(TemplateItem::Field(field)));
                        }
                    }
                    EditorItem::ContactInfo => {
                        rows.push(TemplateRow::Heading(CONTACT_INFO));
                        rows.extend(
                            item.fields()
                                .into_iter()
                                .map(|field| TemplateRow::Item(TemplateItem::ContactField(field))),
                        );
                    }
                    EditorItem::List(list) if list == EditorForm::LOCATION_CREATED => {
                        rows.push(TemplateRow::Heading(EditorForm::LOCATION_CREATED));
                        rows.extend(
                            EditorForm::LOCATION_FIELDS
                                .into_iter()
                                .map(|sub| TemplateRow::Item(TemplateItem::LocationField(sub))),
                        );
                    }
                    EditorItem::List(list) => {
                        rows.push(TemplateRow::Item(TemplateItem::List(list)));
                    }
                }
            }
            (section, rows)
        })
        .collect()
}

/// Every item of the template form, in the default layout's order. The set does not depend on
/// the layout (hidden items are rows too).
fn all_items() -> Vec<TemplateItem> {
    template_rows(&EditorLayout::default())
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .filter_map(|row| match row {
            TemplateRow::Item(item) => Some(item),
            TemplateRow::Heading(_) => None,
        })
        .collect()
}

/// What an item holds, compared to find typing and changes: a field's or sub-field's text; for a
/// structure list the texts of its entries that are not empty and the structures they write.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ItemValue {
    Text(String),
    Entries(Vec<Vec<String>>, Vec<StructValue>),
}

/// The state [`TemplateForm::changed`] compares with.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    values: Vec<ItemValue>,
    checked: Vec<bool>,
    added: Vec<bool>,
}

/// A checked item's error that refuses the template ([`TemplateRefused::FieldErrors`]), so T293
/// shows it under the item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TemplateFieldError {
    /// The item with the error.
    pub item: TemplateItem,
    /// For a structure list ([`TemplateItem::List`]): the entry's index and the sub-field's XMP
    /// id; `None` for a field and a sub-field of Location Created.
    pub entry: Option<(usize, &'static str)>,
    /// The error's text.
    pub text: String,
}

/// Why [`TemplateForm::template`] gives no template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateRefused {
    /// A checked item has an error (a field error, or a sub-field error of a checked list or of a
    /// checked Location Created sub-field), each named.
    FieldErrors(Vec<TemplateFieldError>),
    /// A variable cannot be read (in a part the form does not show, kept from a loaded file): the
    /// variable error's text.
    Variables(String),
    /// Nothing checked has a value.
    Empty,
}

/// The template window's form (ADR 0017 U13c): the form, the state of every check box and + box,
/// and what typing and changes are compared with.
#[derive(Clone, Debug)]
pub struct TemplateForm {
    form: EditorForm,
    /// Every item, in the default layout's order; `checked`, `added` and the values are in the
    /// same order.
    items: Vec<TemplateItem>,
    checked: Vec<bool>,
    added: Vec<bool>,
    /// The values at the last [`TemplateForm::notice_typing`] (or `new`, Clear Form, Load).
    last: Vec<ItemValue>,
    /// The state at `new`, Clear Form, Load and [`TemplateForm::mark_saved`].
    snapshot: Snapshot,
}

impl Default for TemplateForm {
    fn default() -> Self {
        TemplateForm::new()
    }
}

impl TemplateForm {
    /// The empty form: no value, nothing checked, Location Created with one empty entry.
    pub fn new() -> TemplateForm {
        let items = all_items();
        let count = items.len();
        let mut form = TemplateForm {
            form: EditorForm::empty(),
            items,
            checked: vec![false; count],
            added: vec![false; count],
            last: Vec::new(),
            snapshot: Snapshot {
                values: Vec::new(),
                checked: Vec::new(),
                added: Vec::new(),
            },
        };
        form.clear_form();
        form
    }

    /// The form, to be drawn.
    pub fn form(&self) -> &EditorForm {
        &self.form
    }

    /// The form, to be edited by the window's field widgets.
    pub fn form_mut(&mut self) -> &mut EditorForm {
        &mut self.form
    }

    fn index(&self, item: TemplateItem) -> Option<usize> {
        self.items.iter().position(|i| *i == item)
    }

    /// True when `item`'s check box is checked.
    pub fn checked(&self, item: TemplateItem) -> bool {
        self.index(item)
            .and_then(|index| self.checked.get(index))
            .copied()
            .unwrap_or(false)
    }

    /// Checks or unchecks `item`.
    pub fn set_checked(&mut self, item: TemplateItem, checked: bool) {
        if let Some(slot) = self
            .index(item)
            .and_then(|index| self.checked.get_mut(index))
        {
            *slot = checked;
        }
    }

    /// True when `item`'s + box is checked; false for an item without one
    /// ([`TemplateItem::has_add_box`]). It keeps its state while the item is unchecked, but
    /// counts only on a checked item.
    pub fn added(&self, item: TemplateItem) -> bool {
        item.has_add_box()
            && self
                .index(item)
                .and_then(|index| self.added.get(index))
                .copied()
                .unwrap_or(false)
    }

    /// Checks or unchecks `item`'s + box; nothing for an item without one.
    pub fn set_added(&mut self, item: TemplateItem, added: bool) {
        if !item.has_add_box() {
            return;
        }
        if let Some(slot) = self.index(item).and_then(|index| self.added.get_mut(index)) {
            *slot = added;
        }
    }

    /// What each item holds now, in the order of `items`.
    fn values(&self) -> Vec<ItemValue> {
        let values = self.form.template_values();
        self.items
            .iter()
            .map(|item| self.item_value(*item, &values))
            .collect()
    }

    fn item_value(&self, item: TemplateItem, values: &TemplateValues) -> ItemValue {
        match item {
            TemplateItem::Field(field) | TemplateItem::ContactField(field) => {
                ItemValue::Text(self.form.text(field).to_string())
            }
            TemplateItem::LocationField(sub) => ItemValue::Text(
                self.form
                    .list_text(EditorForm::LOCATION_CREATED, 0, sub)
                    .to_string(),
            ),
            TemplateItem::List(list) => {
                let texts = (0..self.form.list_count(list))
                    .map(|entry| {
                        EditorForm::list_fields(list)
                            .iter()
                            .map(|field| self.form.list_text(list, entry, field).to_string())
                            .collect::<Vec<String>>()
                    })
                    .filter(|texts| texts.iter().any(|text| !text.trim().is_empty()))
                    .collect();
                let entries = values
                    .list(list)
                    .map(|values| values.entries.clone())
                    .unwrap_or_default();
                ItemValue::Entries(texts, entries)
            }
        }
    }

    /// Typing checks the field (U13-Q4), called after each frame: every item whose value changed
    /// since the previous call (or `new`, Clear Form or Load) is checked. A field's text counts (a
    /// check box's and a choice list's too), a contact field's or Location Created sub-field's
    /// text, and a structure list's entries without the empty ones (so an empty entry added
    /// checks nothing). It never checks or unchecks a + box and never unchecks an item, so the
    /// user's own click on a check box stays.
    pub fn notice_typing(&mut self) {
        let now = self.values();
        for ((value, last), checked) in now.iter().zip(&self.last).zip(&mut self.checked) {
            if value != last {
                *checked = true;
            }
        }
        self.last = now;
    }

    /// True when a value, a check box or a + box differs from what it was at `new`, Clear Form,
    /// Load or [`TemplateForm::mark_saved`] (U13-Q15); a text typed back to what it was is not a
    /// change. It does not depend on [`TemplateForm::notice_typing`] having run.
    pub fn changed(&self) -> bool {
        self.current() != self.snapshot
    }

    fn current(&self) -> Snapshot {
        Snapshot {
            values: self.values(),
            checked: self.checked.clone(),
            added: self.added.clone(),
        }
    }

    /// Takes the snapshot of [`TemplateForm::changed`] and the values typing is compared with.
    fn settle(&mut self) {
        self.snapshot = self.current();
        self.last = self.snapshot.values.clone();
    }

    /// After a save: [`TemplateForm::changed`] is false until the next change.
    pub fn mark_saved(&mut self) {
        self.snapshot = self.current();
    }

    /// Clear Form: every value emptied, every check box and + box unchecked, Location Created back
    /// to one empty entry; [`TemplateForm::changed`] is then false.
    pub fn clear_form(&mut self) {
        self.form = EditorForm::empty();
        self.form.add_entry(EditorForm::LOCATION_CREATED);
        self.checked.iter_mut().for_each(|checked| *checked = false);
        self.added.iter_mut().for_each(|added| *added = false);
        self.settle();
    }

    /// Load: Clear Form, then the template's fields put in the form as typed; each item that got
    /// a value is checked (a contact field or Location Created sub-field by its own text, a
    /// structure list as a whole, Copyrighted when it is `True` or `False`), and the + box of
    /// each id in the template's `add`. Returns the XMP ids (`property` or `property/sub-field`)
    /// of what could not be put in the form. Afterwards [`TemplateForm::changed`] is false and
    /// the load does not count as typing.
    pub fn load(&mut self, template: &TemplateFile) -> Vec<String> {
        self.clear_form();
        let not_loaded = self.form.put_template_fields(&template.fields);
        let loaded = |property: &str| {
            template.fields.iter().any(|(id, _)| id == property)
                && !not_loaded.iter().any(|id| id == property)
        };
        for (item, checked) in self.items.iter().zip(&mut self.checked) {
            *checked = match *item {
                TemplateItem::Field(field) if field.check_box() => loaded(field.property()),
                TemplateItem::Field(field) | TemplateItem::ContactField(field) => {
                    !self.form.text(field).is_empty()
                }
                TemplateItem::LocationField(sub) => !self
                    .form
                    .list_text(EditorForm::LOCATION_CREATED, 0, sub)
                    .is_empty(),
                TemplateItem::List(list) => loaded(list),
            };
        }
        for (item, added) in self.items.iter().zip(&mut self.added) {
            *added = item.has_add_box() && template.add.iter().any(|id| id == item.property());
        }
        self.settle();
        not_loaded
    }

    /// The template the form holds: the checked items with a value, made by
    /// [`TemplateFile::new`].
    ///
    /// - a checked field with a value gives `(property, value)`, its text as typed (nothing
    ///   escaped); a checked field without a value gives nothing (U13-Q4); a checked Copyrighted
    ///   gives `True` when its box is ticked and `False` when not;
    /// - the checked contact fields with a value give one Creator's Contact Info structure of only
    ///   those sub-fields; the checked Location Created sub-fields with a value one Location
    ///   Created entry of only those;
    /// - a checked structure list gives the entries that write something;
    /// - `add`: the checked fields whose + box is checked.
    ///
    /// Refused when a checked item has an error (each named), when a variable cannot be read
    /// (`TemplateFile::check_variables`, a part the form does not show), or when nothing checked
    /// has a value. An unchecked item's error never refuses it.
    pub fn template(&self) -> Result<TemplateFile, TemplateRefused> {
        let values = self.form.template_values();
        let location = values.list(EditorForm::LOCATION_CREATED);
        let location_entry = location.and_then(|list| list.entries.first());
        let mut errors = Vec::new();
        let mut fields: Vec<(String, Value)> = Vec::new();
        let mut add = Vec::new();
        let mut contact = Vec::new();
        let mut location_fields = Vec::new();
        for (item, checked) in self.items.iter().zip(&self.checked) {
            if !*checked {
                continue;
            }
            let item = *item;
            match item {
                TemplateItem::Field(field) | TemplateItem::ContactField(field) => {
                    match values.field(field) {
                        Some(Err(text)) => errors.push(TemplateFieldError {
                            item,
                            entry: None,
                            text: text.clone(),
                        }),
                        Some(Ok(Some(value))) => match (item, field.struct_field()) {
                            (TemplateItem::ContactField(_), Some(sub)) => {
                                contact.push((sub.to_string(), value.clone()));
                            }
                            _ => fields.push((field.property().to_string(), value.clone())),
                        },
                        Some(Ok(None)) | None => {}
                    }
                    if self.added(item) {
                        add.push(field.property().to_string());
                    }
                }
                TemplateItem::LocationField(sub) => {
                    if let Some(list) = location {
                        errors.extend(
                            list.errors
                                .iter()
                                .filter(|(entry, field, _)| *entry == 0 && *field == sub)
                                .map(|(_, _, text)| TemplateFieldError {
                                    item,
                                    entry: None,
                                    text: text.clone(),
                                }),
                        );
                    }
                    if let Some((id, value)) = location_entry
                        .and_then(|entry| entry.fields.iter().find(|(id, _)| id == sub))
                    {
                        location_fields.push((id.clone(), value.clone()));
                    }
                }
                TemplateItem::List(list) => {
                    let Some(list_values) = values.list(list) else {
                        continue;
                    };
                    errors.extend(list_values.errors.iter().map(|(entry, field, text)| {
                        TemplateFieldError {
                            item,
                            entry: Some((*entry, *field)),
                            text: text.clone(),
                        }
                    }));
                    if !list_values.entries.is_empty() {
                        fields.push((
                            list.to_string(),
                            Value::StructList(list_values.entries.clone()),
                        ));
                    }
                }
            }
        }
        if !errors.is_empty() {
            return Err(TemplateRefused::FieldErrors(errors));
        }
        if !contact.is_empty() {
            fields.push((
                CONTACT_INFO.to_string(),
                Value::Struct(StructValue { fields: contact }),
            ));
        }
        if !location_fields.is_empty() {
            fields.push((
                EditorForm::LOCATION_CREATED.to_string(),
                Value::StructList(vec![StructValue {
                    fields: location_fields,
                }]),
            ));
        }
        let template = TemplateFile::new(fields, add);
        if template.fields.is_empty() {
            return Err(TemplateRefused::Empty);
        }
        template
            .check_variables()
            .map_err(|error| TemplateRefused::Variables(variable_error_text(&error)))?;
        Ok(template)
    }
}

/// How many sections go in the left column of two (U13-Q13), given each section's height in
/// order (any unit): the sections fill the left column first, then the right, at the split where
/// the taller column is lowest. Only the splits 1..=n are considered, so the left column is never
/// empty while there is a section; the first such split on a tie; 0 for no section.
pub fn left_column_sections(heights: &[f32]) -> usize {
    let total: f32 = heights.iter().sum();
    let mut best = 0;
    let mut best_height = f32::INFINITY;
    let mut left = 0.0;
    for (index, height) in heights.iter().enumerate() {
        left += height;
        let taller = left.max(total - left);
        if taller < best_height {
            best_height = taller;
            best = index + 1;
        }
    }
    best
}

/// The folder of the template files (U13-Q2): `<local_app_data>\RawStride\templates`.
pub fn templates_folder(local_app_data: &Path) -> PathBuf {
    local_app_data.join("RawStride").join("templates")
}

/// `path` with a name that ends in `.xmp`: unchanged when it does (any case), otherwise with
/// `.xmp` added to the whole name (`a.txt` → `a.txt.xmp`).
pub fn with_xmp_extension(path: &Path) -> PathBuf {
    let ends_in_xmp = path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().to_lowercase().ends_with(".xmp"));
    if ends_in_xmp {
        return path.to_path_buf();
    }
    let mut name = path.as_os_str().to_owned();
    name.push(".xmp");
    PathBuf::from(name)
}

/// A text of the template window (`docs/tasks/README.md` rule 13), made by [`template_text`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateText {
    /// The window's title; `file` is the name of the file loaded or saved last, if any.
    Title { file: Option<String> },
    /// The button that empties the form.
    ClearForm,
    /// The button that loads a template file.
    Load,
    /// The button that saves the form as a template file.
    Save,
    /// The button that closes the window.
    Close,
    /// The button of a window opened from the import dialog that hands the form to that import
    /// without saving a file (ADR 0017 U13-Q21, T298).
    UseForImport,
    /// The hover text of an item's check box.
    CheckHover,
    /// The label of a + box.
    AddLabel,
    /// The hover text of a + box.
    AddHover,
    /// The name of the Save and Load dialogs' file type.
    FileType,
    /// After a load.
    Loaded { file: String },
    /// The fields of a loaded file that were not loaded, by name.
    NotLoaded { names: Vec<String> },
    /// A value of a loaded file that could not be read: its name and the text read.
    Unreadable { name: String, text: String },
    /// A load that failed, with its reason.
    LoadFailed { file: String, reason: String },
    /// A load answered by a template reader that has stopped.
    LoadStopped { file: String },
    /// After a save.
    Saved { file: String },
    /// A save refused: a checked field has an error.
    NotSavedFieldError,
    /// A save refused: a variable cannot be read; the variable error's text.
    NotSavedVariables { text: String },
    /// A save refused: no checked field has a value.
    NotSavedEmpty,
    /// A save refused: the folder's path holds a `%` (U13-Q1 amended).
    NotSavedPercent { folder: String },
    /// A save that failed, with its reason.
    NotSaved { reason: String },
    /// A save answered by a template reader that has stopped.
    NotSavedStopped,
    /// A save without ExifTool's settings file for templates, with the reason.
    NotSavedNoConfig { reason: String },
    /// The question before a Load or Close loses changes (U13-Q15).
    DiscardQuestion,
    /// The button that discards the changes.
    Discard,
    /// The button that cancels a question.
    Cancel,
    /// The question before a save over a file that exists.
    ReplaceQuestion { file: String },
    /// The button that replaces the file.
    Replace,
}

/// The text of `text`.
pub fn template_text(text: &TemplateText) -> String {
    match text {
        TemplateText::Title { file: None } => "Metadata Template".to_string(),
        TemplateText::Title { file: Some(file) } => format!("Metadata Template – {file}"),
        TemplateText::ClearForm => "Clear Form".to_string(),
        TemplateText::Load => "Load…".to_string(),
        TemplateText::Save => "Save…".to_string(),
        TemplateText::Close => "Close".to_string(),
        TemplateText::UseForImport => "Use for This Import".to_string(),
        TemplateText::CheckHover => "Use this field in the template.".to_string(),
        TemplateText::AddLabel => "+".to_string(),
        TemplateText::AddHover => {
            "Add the template's entries after the image's own instead of replacing the list."
                .to_string()
        }
        TemplateText::FileType => "Metadata Template".to_string(),
        TemplateText::Loaded { file } => format!("Loaded {file}."),
        TemplateText::NotLoaded { names } => {
            let list = names.join(", ");
            match names.len() {
                1 => format!("1 field of the file was not loaded: {list}."),
                n => format!("{n} fields of the file were not loaded: {list}."),
            }
        }
        TemplateText::Unreadable { name, text } => format!("{name} could not be read ({text})."),
        TemplateText::LoadFailed { file, reason } => format!("{file} was not loaded: {reason}"),
        TemplateText::LoadStopped { file } => {
            format!("{file} was not loaded: the template reader has stopped")
        }
        TemplateText::Saved { file } => format!("Saved {file}."),
        TemplateText::NotSavedFieldError => "Not saved: a checked field has an error.".to_string(),
        TemplateText::NotSavedVariables { text } => format!("Not saved: {text}"),
        TemplateText::NotSavedEmpty => "Not saved: no checked field has a value.".to_string(),
        TemplateText::NotSavedPercent { folder } => format!(
            "Not saved: a template cannot be saved in a folder whose path holds a % sign \
             ({folder})."
        ),
        TemplateText::NotSaved { reason } => format!("Not saved: {reason}"),
        TemplateText::NotSavedStopped => "Not saved: the template reader has stopped".to_string(),
        TemplateText::NotSavedNoConfig { reason } => {
            format!("Not saved: ExifTool cannot use its settings file for templates ({reason}).")
        }
        TemplateText::DiscardQuestion => "Discard the changes to the template?".to_string(),
        TemplateText::Discard => "Discard".to_string(),
        TemplateText::Cancel => "Cancel".to_string(),
        TemplateText::ReplaceQuestion { file } => format!("{file} already exists. Replace it?"),
        TemplateText::Replace => "Replace".to_string(),
    }
}

/// The name of a property path (`property` or `property/sub-field/…`): each part by
/// [`property_label`], joined by ` – `.
pub fn property_path_name(path: &str) -> String {
    path.split('/')
        .map(|part| property_label(part, Language::En))
        .collect::<Vec<String>>()
        .join(NAME_SEPARATOR)
}

/// The status line's text of a refused template ([`TemplateForm::template`]).
pub fn refused_text(refused: &TemplateRefused) -> String {
    template_text(&match refused {
        TemplateRefused::FieldErrors(_) => TemplateText::NotSavedFieldError,
        TemplateRefused::Variables(text) => TemplateText::NotSavedVariables { text: text.clone() },
        TemplateRefused::Empty => TemplateText::NotSavedEmpty,
    })
}

/// The status line's text of a save that failed: a folder whose path holds a `%` has its own
/// text, any other error its reason.
pub fn save_failed_text(error: &TemplateFileError) -> String {
    template_text(&match error {
        TemplateFileError::PercentInFolder(folder) => TemplateText::NotSavedPercent {
            folder: folder.display().to_string(),
        },
        other => TemplateText::NotSaved {
            reason: other.to_string(),
        },
    })
}

/// The status line's text of a load of `file` that failed with `error`.
pub fn load_failed_text(file: &str, error: &TemplateFileError) -> String {
    template_text(&TemplateText::LoadFailed {
        file: file.to_string(),
        reason: error.to_string(),
    })
}

/// The lines after a load: what was not loaded, from the read's `skipped` and `dropped` and
/// `not_loaded` ([`TemplateForm::load`]'s list) together, by name, in one line (none when nothing
/// was left out); then one line for each value that could not be read.
pub fn not_loaded_lines(read: &TemplateRead, not_loaded: &[String]) -> Vec<String> {
    let names: Vec<String> = read
        .skipped
        .iter()
        .chain(&read.dropped)
        .chain(not_loaded)
        .map(|path| property_path_name(path))
        .collect();
    let mut lines = Vec::new();
    if !names.is_empty() {
        lines.push(template_text(&TemplateText::NotLoaded { names }));
    }
    lines.extend(read.unreadable.iter().map(|(path, text)| {
        template_text(&TemplateText::Unreadable {
            name: property_path_name(path),
            text: text.clone(),
        })
    }));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::BTreeSet;

    use pw_metadata::properties;
    use pw_metadata::template_file::in_template;
    use pw_metadata::value::LangAlt;
    use pw_metadata::variables::VariableError;

    const LOCATION_SHOWN: &str = "Iptc4xmpExt:LocationShown";
    const LC: &str = "Iptc4xmpExt:LocationCreated";
    const CITY: &str = "Iptc4xmpExt:City";
    const COUNTRY: &str = "Iptc4xmpExt:CountryName";
    const LOCATION_NAME: &str = "Iptc4xmpExt:LocationName";
    const EMAIL: &str = "Iptc4xmpCore:CiEmailWork";
    const CONTACT_CITY: &str = "Iptc4xmpCore:CiAdrCity";

    fn text(s: &str) -> Value {
        Value::Text(s.to_string())
    }

    fn entry(fields: &[(&str, Value)]) -> StructValue {
        StructValue {
            fields: fields
                .iter()
                .map(|(id, value)| (id.to_string(), value.clone()))
                .collect(),
        }
    }

    fn type_into(form: &mut TemplateForm, field: EditorField, value: &str) {
        *form.form_mut().text_mut(field).unwrap() = value.to_string();
    }

    fn type_list(form: &mut TemplateForm, list: &str, index: usize, field: &str, value: &str) {
        *form.form_mut().list_text_mut(list, index, field).unwrap() = value.to_string();
    }

    fn checked_items(form: &TemplateForm) -> Vec<TemplateItem> {
        all_items()
            .into_iter()
            .filter(|item| form.checked(*item))
            .collect()
    }

    fn field_of(template: &TemplateFile, property: &str) -> Option<Value> {
        template
            .fields
            .iter()
            .find(|(id, _)| id == property)
            .map(|(_, value)| value.clone())
    }

    const HEADLINE: TemplateItem = TemplateItem::Field(EditorField::Headline);
    const KEYWORDS: TemplateItem = TemplateItem::Field(EditorField::Keywords);
    const SHOWN: TemplateItem = TemplateItem::List(LOCATION_SHOWN);

    // --- The rows ---

    #[test]
    fn the_rows_of_the_default_layout() {
        let rows = template_rows(&EditorLayout::default());
        let sections: Vec<EditorSection> = rows.iter().map(|(section, _)| *section).collect();
        assert_eq!(
            sections,
            [
                EditorSection::Content,
                EditorSection::CreatorRights,
                EditorSection::TimePlace,
                EditorSection::Assignment,
                EditorSection::Licence,
            ]
        );
        let all: Vec<TemplateRow> = rows.iter().flat_map(|(_, rows)| rows.clone()).collect();
        assert!(all.contains(&TemplateRow::Item(HEADLINE)));
        assert!(all.contains(&TemplateRow::Item(KEYWORDS)));
        assert!(all.contains(&TemplateRow::Item(SHOWN)));
        assert!(!all.contains(&TemplateRow::Item(TemplateItem::Field(
            EditorField::Urgency
        ))));
        assert!(!all.contains(&TemplateRow::Item(TemplateItem::List(LC))));

        // The contact fields under their heading, in order.
        let contact = all
            .iter()
            .position(|row| *row == TemplateRow::Heading(CONTACT_INFO))
            .unwrap();
        let contact_fields: Vec<TemplateRow> = EditorField::ALL
            .into_iter()
            .filter(|field| field.struct_field().is_some())
            .map(|field| TemplateRow::Item(TemplateItem::ContactField(field)))
            .collect();
        assert_eq!(contact_fields.len(), 8);
        assert_eq!(all[contact + 1..contact + 9], contact_fields[..]);

        // Location Created's seven sub-fields under their heading.
        let location = all
            .iter()
            .position(|row| *row == TemplateRow::Heading(LC))
            .unwrap();
        let location_fields: Vec<TemplateRow> = EditorForm::LOCATION_FIELDS
            .into_iter()
            .map(|sub| TemplateRow::Item(TemplateItem::LocationField(sub)))
            .collect();
        assert_eq!(all[location + 1..location + 8], location_fields[..]);
        assert_eq!(
            all[location + 8],
            TemplateRow::Item(TemplateItem::List(LOCATION_SHOWN))
        );

        // The heading counts in its section's rows.
        let (_, creator_rows) = &rows[1];
        assert!(creator_rows.contains(&TemplateRow::Heading(CONTACT_INFO)));
        assert_eq!(
            TemplateRow::Heading(CONTACT_INFO).label(),
            property_label(CONTACT_INFO, Language::En)
        );
    }

    #[test]
    fn hidden_sections_and_items_are_rows_in_the_layout_order() {
        let mut layout = EditorLayout::default();
        layout.set_section_shown(EditorSection::Assignment, false);
        layout.set_item_shown(EditorItem::Field(EditorField::Headline), false);
        layout.move_section_up(EditorSection::TimePlace);
        let rows = template_rows(&layout);
        let sections: Vec<EditorSection> = rows.iter().map(|(section, _)| *section).collect();
        assert_eq!(
            sections,
            [
                EditorSection::Content,
                EditorSection::TimePlace,
                EditorSection::CreatorRights,
                EditorSection::Assignment,
                EditorSection::Licence,
            ]
        );
        let (_, content) = &rows[0];
        assert_eq!(
            content[..2],
            [
                TemplateRow::Item(TemplateItem::Field(EditorField::Title)),
                TemplateRow::Item(HEADLINE),
            ]
        );
        let (_, assignment) = &rows[3];
        assert!(assignment.contains(&TemplateRow::Item(TemplateItem::Field(
            EditorField::Instructions
        ))));

        // A moved item keeps its place among the rows.
        let mut layout = EditorLayout::default();
        layout.move_item_up(EditorItem::Field(EditorField::Headline));
        let rows = template_rows(&layout);
        assert_eq!(
            rows[0].1[..2],
            [
                TemplateRow::Item(HEADLINE),
                TemplateRow::Item(TemplateItem::Field(EditorField::Title)),
            ]
        );
    }

    /// The template's fields equal the window's (T284): both ways, 50 properties.
    #[test]
    fn the_template_fields_equal_the_window_fields() {
        let window: BTreeSet<&str> = template_rows(&EditorLayout::default())
            .into_iter()
            .flat_map(|(_, rows)| rows)
            .filter_map(|row| match row {
                TemplateRow::Item(item) => Some(item.property()),
                TemplateRow::Heading(_) => None,
            })
            .collect();
        let table: BTreeSet<&str> = properties::properties()
            .map(|def| def.xmp_id)
            .filter(|id| in_template(id))
            .collect();
        assert_eq!(window, table);
        assert_eq!(window.len(), 50);
    }

    // --- Typing ---

    #[test]
    fn typing_in_headline_checks_headline_only() {
        let mut form = TemplateForm::new();
        type_into(&mut form, EditorField::Headline, "News");
        form.notice_typing();
        assert_eq!(checked_items(&form), [HEADLINE]);
    }

    #[test]
    fn a_location_shown_entry_with_a_city_checks_location_shown() {
        let mut form = TemplateForm::new();
        form.form_mut().add_entry(LOCATION_SHOWN);
        form.notice_typing();
        assert_eq!(checked_items(&form), []);
        type_list(&mut form, LOCATION_SHOWN, 0, CITY, "Lund");
        form.notice_typing();
        assert_eq!(checked_items(&form), [SHOWN]);
    }

    #[test]
    fn a_choice_and_copyrighted_check_their_fields() {
        let mut form = TemplateForm::new();
        form.form_mut()
            .set_choice(EditorField::ModelReleaseStatus, "MR-NON");
        form.form_mut().set_checked(EditorField::Copyrighted, true);
        form.notice_typing();
        assert_eq!(
            checked_items(&form),
            [
                TemplateItem::Field(EditorField::Copyrighted),
                TemplateItem::Field(EditorField::ModelReleaseStatus),
            ]
        );
    }

    #[test]
    fn a_field_unchecked_after_typing_stays_unchecked() {
        let mut form = TemplateForm::new();
        type_into(&mut form, EditorField::Headline, "News");
        form.notice_typing();
        form.set_checked(HEADLINE, false);
        form.notice_typing();
        assert!(!form.checked(HEADLINE));
    }

    #[test]
    fn typing_never_checks_a_plus_box() {
        let mut form = TemplateForm::new();
        type_into(&mut form, EditorField::Keywords, "a, b");
        form.notice_typing();
        assert!(form.checked(KEYWORDS));
        assert!(!form.added(KEYWORDS));
    }

    #[test]
    fn typing_in_a_location_created_sub_field_checks_it_only() {
        let mut form = TemplateForm::new();
        type_list(&mut form, LC, 0, CITY, "Lund");
        form.notice_typing();
        assert_eq!(checked_items(&form), [TemplateItem::LocationField(CITY)]);
    }

    #[test]
    fn a_plus_box_only_on_a_list_of_plain_texts() {
        let mut form = TemplateForm::new();
        assert!(KEYWORDS.has_add_box());
        assert!(!HEADLINE.has_add_box());
        assert!(!SHOWN.has_add_box());
        form.set_added(HEADLINE, true);
        form.set_added(SHOWN, true);
        assert!(!form.added(HEADLINE));
        assert!(!form.added(SHOWN));
        form.set_added(KEYWORDS, true);
        assert!(form.added(KEYWORDS));
        // Kept while the field is unchecked.
        form.set_checked(KEYWORDS, true);
        form.set_checked(KEYWORDS, false);
        assert!(form.added(KEYWORDS));
    }

    // --- The template ---

    #[test]
    fn a_checked_headline_gives_its_text_unescaped() {
        let mut form = TemplateForm::new();
        type_into(&mut form, EditorField::Headline, "{date} {name}");
        type_into(&mut form, EditorField::Title, "Not checked");
        form.notice_typing();
        form.set_checked(TemplateItem::Field(EditorField::Title), false);
        form.set_checked(TemplateItem::Field(EditorField::Description), true);
        let template = form.template().unwrap();
        assert_eq!(
            template.fields,
            [("photoshop:Headline".to_string(), text("{date} {name}"))]
        );
        assert_eq!(template.add, Vec::<String>::new());
    }

    #[test]
    fn keywords_with_plus_are_in_fields_and_add() {
        let mut form = TemplateForm::new();
        type_into(&mut form, EditorField::Keywords, "a, b");
        form.notice_typing();
        form.set_added(KEYWORDS, true);
        let template = form.template().unwrap();
        assert_eq!(
            field_of(&template, "dc:subject"),
            Some(Value::List(vec!["a".to_string(), "b".to_string()]))
        );
        assert_eq!(template.add, ["dc:subject"]);
    }

    #[test]
    fn the_plus_box_of_an_unchecked_keywords_is_not_in_add() {
        let mut form = TemplateForm::new();
        type_into(&mut form, EditorField::Keywords, "a, b");
        type_into(&mut form, EditorField::Headline, "News");
        form.notice_typing();
        form.set_added(KEYWORDS, true);
        form.set_checked(KEYWORDS, false);
        let template = form.template().unwrap();
        assert_eq!(field_of(&template, "dc:subject"), None);
        assert_eq!(template.add, Vec::<String>::new());
    }

    #[test]
    fn a_checked_copyrighted_gives_true_or_false() {
        let copyrighted = TemplateItem::Field(EditorField::Copyrighted);
        let mut form = TemplateForm::new();
        form.set_checked(copyrighted, true);
        let template = form.template().unwrap();
        assert_eq!(field_of(&template, "xmpRights:Marked"), Some(text("False")));
        form.form_mut().set_checked(EditorField::Copyrighted, true);
        let template = form.template().unwrap();
        assert_eq!(field_of(&template, "xmpRights:Marked"), Some(text("True")));
    }

    #[test]
    fn the_checked_contact_fields_give_one_structure() {
        let mut form = TemplateForm::new();
        type_into(&mut form, EditorField::Email, "a@example.com");
        type_into(&mut form, EditorField::ContactCity, "Lund");
        type_into(&mut form, EditorField::Phone, "123");
        form.notice_typing();
        form.set_checked(TemplateItem::ContactField(EditorField::Phone), false);
        let template = form.template().unwrap();
        let expected = TemplateFile::new(
            vec![(
                CONTACT_INFO.to_string(),
                Value::Struct(entry(&[
                    (EMAIL, text("a@example.com")),
                    (CONTACT_CITY, text("Lund")),
                ])),
            )],
            Vec::new(),
        );
        assert_eq!(template, expected);
        assert_eq!(template.fields.len(), 1);
    }

    #[test]
    fn the_checked_location_created_sub_fields_give_one_entry() {
        let mut form = TemplateForm::new();
        type_list(&mut form, LC, 0, CITY, "Lund");
        type_list(&mut form, LC, 0, COUNTRY, "Sweden");
        type_list(&mut form, LC, 0, "Iptc4xmpExt:Sublocation", "Centre");
        form.notice_typing();
        form.set_checked(
            TemplateItem::LocationField("Iptc4xmpExt:Sublocation"),
            false,
        );
        let template = form.template().unwrap();
        assert_eq!(
            template.fields,
            [(
                LC.to_string(),
                Value::StructList(vec![entry(&[
                    (CITY, text("Lund")),
                    (COUNTRY, text("Sweden")),
                ])])
            )]
        );
    }

    #[test]
    fn a_checked_location_shown_gives_both_entries() {
        let mut form = TemplateForm::new();
        form.form_mut().add_entry(LOCATION_SHOWN);
        form.form_mut().add_entry(LOCATION_SHOWN);
        form.form_mut().add_entry(LOCATION_SHOWN);
        type_list(&mut form, LOCATION_SHOWN, 0, CITY, "Lund");
        type_list(&mut form, LOCATION_SHOWN, 2, CITY, "Malmö");
        form.notice_typing();
        let template = form.template().unwrap();
        assert_eq!(
            field_of(&template, LOCATION_SHOWN),
            Some(Value::StructList(vec![
                entry(&[(CITY, text("Lund"))]),
                entry(&[(CITY, text("Malmö"))]),
            ]))
        );
    }

    #[test]
    fn a_checked_unknown_variable_refuses_the_template() {
        let mut form = TemplateForm::new();
        type_into(&mut form, EditorField::Headline, "{job}");
        form.notice_typing();
        let unknown = variable_error_text(&VariableError::Unknown("job".to_string()));
        assert_eq!(
            form.template(),
            Err(TemplateRefused::FieldErrors(vec![TemplateFieldError {
                item: HEADLINE,
                entry: None,
                text: unknown,
            }]))
        );

        // Unchecked, with a checked Title: not refused.
        form.set_checked(HEADLINE, false);
        type_into(&mut form, EditorField::Title, "A title");
        form.notice_typing();
        let template = form.template().unwrap();
        assert_eq!(field_of(&template, "photoshop:Headline"), None);
        assert!(field_of(&template, "dc:title").is_some());
    }

    #[test]
    fn a_checked_wrong_date_refuses_the_template() {
        let mut form = TemplateForm::new();
        type_into(&mut form, EditorField::DateCreated, "abc");
        form.notice_typing();
        let Err(TemplateRefused::FieldErrors(errors)) = form.template() else {
            panic!("expected field errors");
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].item,
            TemplateItem::Field(EditorField::DateCreated)
        );
        assert!(errors[0].text.starts_with("Write the date as YYYY"));
    }

    #[test]
    fn a_list_error_is_named_by_entry_and_sub_field() {
        let mut form = TemplateForm::new();
        form.form_mut().add_entry(LOCATION_SHOWN);
        form.form_mut().add_entry(LOCATION_SHOWN);
        type_list(&mut form, LOCATION_SHOWN, 1, CITY, "{job}");
        form.notice_typing();
        let Err(TemplateRefused::FieldErrors(errors)) = form.template() else {
            panic!("expected field errors");
        };
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].item, SHOWN);
        assert_eq!(errors[0].entry, Some((1, CITY)));
    }

    #[test]
    fn nothing_checked_is_refused() {
        let form = TemplateForm::new();
        assert_eq!(form.template(), Err(TemplateRefused::Empty));
        assert_eq!(
            refused_text(&TemplateRefused::Empty),
            "Not saved: no checked field has a value."
        );
        // A checked field without a value is not saved.
        let mut form = TemplateForm::new();
        form.set_checked(HEADLINE, true);
        assert_eq!(form.template(), Err(TemplateRefused::Empty));
    }

    // --- Load ---

    fn loaded_template() -> TemplateFile {
        TemplateFile::new(
            vec![
                ("photoshop:Headline".to_string(), text("{date}")),
                (
                    "dc:subject".to_string(),
                    Value::List(vec!["a".to_string(), "b".to_string()]),
                ),
                (
                    CONTACT_INFO.to_string(),
                    Value::Struct(entry(&[(EMAIL, text("a@example.com"))])),
                ),
                (
                    LC.to_string(),
                    Value::StructList(vec![entry(&[(CITY, text("Lund"))])]),
                ),
                (
                    LOCATION_SHOWN.to_string(),
                    Value::StructList(vec![entry(&[(CITY, text("Malmö"))])]),
                ),
            ],
            vec!["dc:subject".to_string()],
        )
    }

    #[test]
    fn load_fills_and_checks_the_form() {
        let template = loaded_template();
        assert_eq!(template.fields.len(), 5);
        let mut form = TemplateForm::new();
        type_into(&mut form, EditorField::Title, "Gone");
        form.notice_typing();
        let not_loaded = form.load(&template);
        assert_eq!(not_loaded, Vec::<String>::new());
        assert_eq!(form.form().text(EditorField::Title), "");
        assert_eq!(form.form().text(EditorField::Headline), "{date}");
        assert_eq!(form.form().text(EditorField::Keywords), "a, b");
        assert_eq!(form.form().text(EditorField::Email), "a@example.com");
        assert_eq!(form.form().list_text(LC, 0, CITY), "Lund");
        assert_eq!(form.form().list_count(LC), 1);
        assert_eq!(form.form().list_text(LOCATION_SHOWN, 0, CITY), "Malmö");
        assert_eq!(
            checked_items(&form),
            [
                HEADLINE,
                KEYWORDS,
                TemplateItem::ContactField(EditorField::Email),
                TemplateItem::LocationField(CITY),
                SHOWN,
            ]
        );
        assert!(form.added(KEYWORDS));
        assert!(!form.changed());
        form.notice_typing();
        assert_eq!(checked_items(&form).len(), 5);
        assert_eq!(form.template(), Ok(template));
    }

    #[test]
    fn a_location_name_of_location_created_is_not_loaded() {
        let template = TemplateFile::new(
            vec![(
                LC.to_string(),
                Value::StructList(vec![entry(&[
                    (CITY, text("Lund")),
                    (
                        LOCATION_NAME,
                        Value::LangAlt(LangAlt::from_entries(vec![(
                            "x-default".to_string(),
                            "Home".to_string(),
                        )])),
                    ),
                ])]),
            )],
            Vec::new(),
        );
        let mut form = TemplateForm::new();
        assert_eq!(form.load(&template), [format!("{LC}/{LOCATION_NAME}")]);
        assert_eq!(
            form.template().unwrap().fields,
            [(
                LC.to_string(),
                Value::StructList(vec![entry(&[(CITY, text("Lund"))])])
            )]
        );
    }

    fn shown_with_name(name: &str) -> TemplateFile {
        TemplateFile::new(
            vec![(
                LOCATION_SHOWN.to_string(),
                Value::StructList(vec![entry(&[
                    (CITY, text("Lund")),
                    (
                        LOCATION_NAME,
                        Value::LangAlt(LangAlt::from_entries(vec![(
                            "x-default".to_string(),
                            name.to_string(),
                        )])),
                    ),
                ])]),
            )],
            Vec::new(),
        )
    }

    #[test]
    fn a_hidden_sub_field_of_a_loaded_entry_is_kept() {
        let template = shown_with_name("{name}");
        assert_eq!(template.fields.len(), 1);
        let mut form = TemplateForm::new();
        assert_eq!(form.load(&template), Vec::<String>::new());
        assert_eq!(form.template(), Ok(template));

        let template = shown_with_name("{job}");
        form.load(&template);
        assert_eq!(
            form.template(),
            Err(TemplateRefused::Variables(variable_error_text(
                &VariableError::Unknown("job".to_string())
            )))
        );
    }

    #[test]
    fn copyrighted_loads_true_false_or_not_at_all() {
        let copyrighted = TemplateItem::Field(EditorField::Copyrighted);
        let marked = |value: &str| TemplateFile {
            fields: vec![("xmpRights:Marked".to_string(), text(value))],
            add: Vec::new(),
        };
        let mut form = TemplateForm::new();
        assert_eq!(form.load(&marked("False")), Vec::<String>::new());
        assert!(form.checked(copyrighted));
        assert!(!form.form().checked(EditorField::Copyrighted));
        assert_eq!(
            field_of(&form.template().unwrap(), "xmpRights:Marked"),
            Some(text("False"))
        );

        assert_eq!(form.load(&marked("True")), Vec::<String>::new());
        assert!(form.checked(copyrighted));
        assert!(form.form().checked(EditorField::Copyrighted));

        assert_eq!(form.load(&marked("Maybe")), ["xmpRights:Marked"]);
        assert!(!form.checked(copyrighted));
        assert!(!form.changed());
    }

    #[test]
    fn a_property_the_form_has_no_field_for_is_not_loaded() {
        let template = TemplateFile {
            fields: vec![
                ("photoshop:Urgency".to_string(), text("5")),
                ("photoshop:Headline".to_string(), text("News")),
            ],
            add: Vec::new(),
        };
        let mut form = TemplateForm::new();
        assert_eq!(form.load(&template), ["photoshop:Urgency"]);
        assert_eq!(checked_items(&form), [HEADLINE]);
    }

    // --- Changed and Clear Form ---

    #[test]
    fn a_text_typed_back_is_not_a_change() {
        let mut form = TemplateForm::new();
        form.load(&loaded_template());
        type_into(&mut form, EditorField::Headline, "{date} x");
        assert!(form.changed());
        type_into(&mut form, EditorField::Headline, "{date}");
        assert!(!form.changed());
    }

    #[test]
    fn changed_after_typing_clear_form_and_save() {
        let mut form = TemplateForm::new();
        assert!(!form.changed());
        type_into(&mut form, EditorField::Headline, "News");
        assert!(form.changed());
        form.mark_saved();
        assert!(!form.changed());
        form.set_checked(HEADLINE, true);
        assert!(form.changed());
        form.mark_saved();
        form.set_added(KEYWORDS, true);
        assert!(form.changed());
        form.clear_form();
        assert!(!form.changed());
    }

    #[test]
    fn clear_form_empties_and_unchecks_everything() {
        let mut form = TemplateForm::new();
        form.load(&loaded_template());
        form.form_mut().add_entry(LC);
        form.clear_form();
        assert_eq!(checked_items(&form), []);
        assert!(!form.added(KEYWORDS));
        assert_eq!(form.form().text(EditorField::Headline), "");
        assert_eq!(form.form().list_count(LOCATION_SHOWN), 0);
        assert_eq!(form.form().list_count(LC), 1);
        assert_eq!(form.form().list_text(LC, 0, CITY), "");
        assert!(!form.changed());
        // Clear Form does not count as typing.
        form.notice_typing();
        assert_eq!(checked_items(&form), []);
    }

    // --- Columns and paths ---

    #[test]
    fn the_column_split() {
        assert_eq!(left_column_sections(&[1.0]), 1);
        assert_eq!(left_column_sections(&[3.0, 1.0, 1.0, 1.0]), 1);
        assert_eq!(left_column_sections(&[1.0, 1.0, 1.0, 1.0]), 2);
        assert_eq!(left_column_sections(&[5.0, 1.0]), 1);
        assert_eq!(left_column_sections(&[2.0, 0.0]), 1);
        assert_eq!(left_column_sections(&[]), 0);
    }

    #[test]
    fn the_templates_folder_and_the_xmp_extension() {
        assert_eq!(
            templates_folder(Path::new(r"D:\Data")),
            PathBuf::from(r"D:\Data\RawStride\templates")
        );
        assert_eq!(
            with_xmp_extension(Path::new(r"C:\t\a.xmp")),
            PathBuf::from(r"C:\t\a.xmp")
        );
        assert_eq!(
            with_xmp_extension(Path::new(r"C:\t\a.XMP")),
            PathBuf::from(r"C:\t\a.XMP")
        );
        assert_eq!(
            with_xmp_extension(Path::new(r"C:\t\a")),
            PathBuf::from(r"C:\t\a.xmp")
        );
        assert_eq!(
            with_xmp_extension(Path::new(r"C:\t\a.txt")),
            PathBuf::from(r"C:\t\a.txt.xmp")
        );
    }

    // --- Texts ---

    #[test]
    fn the_use_for_this_import_button_text() {
        assert_eq!(
            template_text(&TemplateText::UseForImport),
            "Use for This Import"
        );
    }

    #[test]
    fn the_texts() {
        let cases = [
            (TemplateText::Title { file: None }, "Metadata Template"),
            (
                TemplateText::Title {
                    file: Some("press.xmp".to_string()),
                },
                "Metadata Template – press.xmp",
            ),
            (TemplateText::ClearForm, "Clear Form"),
            (TemplateText::Load, "Load…"),
            (TemplateText::Save, "Save…"),
            (TemplateText::Close, "Close"),
            (TemplateText::CheckHover, "Use this field in the template."),
            (TemplateText::AddLabel, "+"),
            (
                TemplateText::AddHover,
                "Add the template's entries after the image's own instead of replacing the list.",
            ),
            (TemplateText::FileType, "Metadata Template"),
            (
                TemplateText::NotLoaded {
                    names: vec!["Rating".to_string()],
                },
                "1 field of the file was not loaded: Rating.",
            ),
            (
                TemplateText::NotLoaded {
                    names: vec!["Rating".to_string(), "Label".to_string()],
                },
                "2 fields of the file were not loaded: Rating, Label.",
            ),
            (
                TemplateText::Loaded {
                    file: "press.xmp".to_string(),
                },
                "Loaded press.xmp.",
            ),
            (
                TemplateText::Unreadable {
                    name: "Date Created".to_string(),
                    text: "2026-13".to_string(),
                },
                "Date Created could not be read (2026-13).",
            ),
            (
                TemplateText::LoadFailed {
                    file: "press.xmp".to_string(),
                    reason: "it is broken".to_string(),
                },
                "press.xmp was not loaded: it is broken",
            ),
            (
                TemplateText::LoadStopped {
                    file: "press.xmp".to_string(),
                },
                "press.xmp was not loaded: the template reader has stopped",
            ),
            (
                TemplateText::Saved {
                    file: "press.xmp".to_string(),
                },
                "Saved press.xmp.",
            ),
            (
                TemplateText::NotSavedFieldError,
                "Not saved: a checked field has an error.",
            ),
            (
                TemplateText::NotSavedVariables {
                    text: "A { is not closed. Write {{ for a brace.".to_string(),
                },
                "Not saved: A { is not closed. Write {{ for a brace.",
            ),
            (
                TemplateText::NotSavedEmpty,
                "Not saved: no checked field has a value.",
            ),
            (
                TemplateText::NotSavedPercent {
                    folder: r"C:\100%".to_string(),
                },
                r"Not saved: a template cannot be saved in a folder whose path holds a % sign (C:\100%).",
            ),
            (
                TemplateText::NotSaved {
                    reason: "disk full".to_string(),
                },
                "Not saved: disk full",
            ),
            (
                TemplateText::NotSavedStopped,
                "Not saved: the template reader has stopped",
            ),
            (
                TemplateText::NotSavedNoConfig {
                    reason: "access denied".to_string(),
                },
                "Not saved: ExifTool cannot use its settings file for templates (access denied).",
            ),
            (
                TemplateText::DiscardQuestion,
                "Discard the changes to the template?",
            ),
            (TemplateText::Discard, "Discard"),
            (TemplateText::Cancel, "Cancel"),
            (
                TemplateText::ReplaceQuestion {
                    file: "press.txt.xmp".to_string(),
                },
                "press.txt.xmp already exists. Replace it?",
            ),
            (TemplateText::Replace, "Replace"),
        ];
        for (text, expected) in cases {
            assert_eq!(template_text(&text), expected, "{text:?}");
        }
    }

    #[test]
    fn the_refused_and_failed_texts() {
        assert_eq!(
            refused_text(&TemplateRefused::FieldErrors(Vec::new())),
            "Not saved: a checked field has an error."
        );
        assert_eq!(
            refused_text(&TemplateRefused::Variables("X.".to_string())),
            "Not saved: X."
        );
        assert_eq!(
            save_failed_text(&TemplateFileError::PercentInFolder(PathBuf::from(
                r"C:\100%"
            ))),
            r"Not saved: a template cannot be saved in a folder whose path holds a % sign (C:\100%)."
        );
        assert_eq!(
            save_failed_text(&TemplateFileError::Empty),
            "Not saved: the template has no field to save"
        );
        assert_eq!(
            load_failed_text(
                "a.xmp",
                &TemplateFileError::NotFound(PathBuf::from(r"C:\t\a.xmp"))
            ),
            r"a.xmp was not loaded: C:\t\a.xmp does not exist"
        );
    }

    #[test]
    fn the_not_loaded_lines() {
        let read = |skipped: &[&str], unreadable: &[(&str, &str)]| TemplateRead {
            template: TemplateFile::default(),
            skipped: skipped.iter().map(|s| s.to_string()).collect(),
            dropped: Vec::new(),
            unreadable: unreadable
                .iter()
                .map(|(p, t)| (p.to_string(), t.to_string()))
                .collect(),
        };
        assert_eq!(not_loaded_lines(&read(&[], &[]), &[]), Vec::<String>::new());
        assert_eq!(
            not_loaded_lines(&read(&["xmp:Rating"], &[]), &[]),
            [format!(
                "1 field of the file was not loaded: {}.",
                property_label("xmp:Rating", Language::En)
            )]
        );
        let two = TemplateRead {
            dropped: vec!["dc:title".to_string()],
            ..read(&["xmp:Rating"], &[])
        };
        assert_eq!(
            not_loaded_lines(&two, &[format!("{LC}/{LOCATION_NAME}")]),
            [format!(
                "3 fields of the file were not loaded: {}, Title, {} – {}.",
                property_label("xmp:Rating", Language::En),
                property_label(LC, Language::En),
                property_label(LOCATION_NAME, Language::En),
            )]
        );
        assert_eq!(
            not_loaded_lines(
                &read(
                    &[],
                    &[
                        ("photoshop:DateCreated", "2026-13"),
                        ("Iptc4xmpExt:ArtworkOrObject/Iptc4xmpExt:AODateCreated", "x"),
                    ]
                ),
                &[]
            ),
            [
                format!(
                    "{} could not be read (2026-13).",
                    property_label("photoshop:DateCreated", Language::En)
                ),
                format!(
                    "{} – {} could not be read (x).",
                    property_label("Iptc4xmpExt:ArtworkOrObject", Language::En),
                    property_label("Iptc4xmpExt:AODateCreated", Language::En)
                ),
            ]
        );
    }
}
