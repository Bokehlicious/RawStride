//! The variables of a metadata text (ADR 0017 U13, part U13b, U13-Q9).
//!
//! A text field or a list entry in the metadata window or in a template can hold **variables**
//! that are turned into each image's own values, so one template gives each image its own date:
//!
//! | Variable | Example | Value |
//! |---|---|---|
//! | `{date}` | `2026-11-22` | the capture date, `YYYY-MM-DD` |
//! | `{year}` | `2026` | four digits |
//! | `{month}` `{day}` | `11`, `22` | two digits each |
//! | `{time}` | `143005` | the capture time, `hhmmss` |
//! | `{make}` `{model}` | `SONY`, `ILCE-1` | the camera; empty when unknown |
//! | `{name}` | `DSC01234` | the image's file name without its extension |
//! | `{folder}` | `2026-11-22 Trip` | the name of the folder the image is in |
//!
//! `{{` gives `{` and `}}` gives `}`. A name is matched after trimming and in any case
//! (`{ Date }` is `{date}`). The text is read as the import's names are read: a `{` starts a name
//! that runs to the next `}`, and a lone `}` is an error. Variables are filled only in texts, Lang
//! Alt texts, list entries and structure fields, never in dates and numbers.
//!
//! The variables are filled when an image is written (T288), so the image gets the value and a
//! template file keeps the variable. The capture time is the embedded Exif DateTimeOriginal until
//! ADR 0020 C2 reads the corrected one.

use std::fmt;
use std::path::Path;

use crate::date::IptcDate;
use crate::value::{LangAlt, StructValue, Value};

/// The values of one image that the variables are filled with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VariableValues {
    pub capture_time: Option<IptcDate>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub name: String,
    pub folder: String,
}

impl VariableValues {
    /// The values of `image`: the capture time from Exif DateTimeOriginal (spaces and NULs
    /// trimmed at both ends; a text that is not a date gives none), make and model trimmed (a
    /// blank one is `None`), the file name without its extension and the name of the folder the
    /// image is in (each empty when the path has none).
    pub fn of(
        image: &Path,
        date_time_original: Option<&str>,
        make: Option<&str>,
        model: Option<&str>,
    ) -> VariableValues {
        let capture_time = date_time_original.and_then(|text| {
            IptcDate::from_exif(text.trim_matches(|c| c == ' ' || c == '\0'), None, None).ok()
        });
        let camera = |text: Option<&str>| {
            text.map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        };
        let name = image
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let folder = image
            .parent()
            .and_then(Path::file_name)
            .map(|folder| folder.to_string_lossy().into_owned())
            .unwrap_or_default();
        VariableValues {
            capture_time,
            make: camera(make),
            model: camera(model),
            name,
            folder,
        }
    }
}

/// Why a text's variables could not be checked or filled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableError {
    /// A `{` without its `}`: the text from that `{`, the brace included, to the end.
    Unclosed(String),
    /// A single `}` that does not close a variable.
    UnexpectedClose,
    /// A name that is not a variable, exactly as written between the braces.
    Unknown(String),
    /// A date variable on an image without a capture time, or with one that lacks the part the
    /// variable needs; the name exactly as written between the braces.
    NoCaptureTime(String),
}

impl fmt::Display for VariableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VariableError::Unclosed(text) => write!(f, "'{text}' is not closed"),
            VariableError::UnexpectedClose => f.write_str("a '}' without its '{'"),
            VariableError::Unknown(name) => write!(f, "unknown variable {{{name}}}"),
            VariableError::NoCaptureTime(name) => {
                write!(f, "the image has no capture time for {{{name}}}")
            }
        }
    }
}

impl std::error::Error for VariableError {}

/// One of the nine variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Variable {
    Date,
    Year,
    Month,
    Day,
    Time,
    Make,
    Model,
    Name,
    Folder,
}

impl Variable {
    /// The variable of a name as written between the braces, trimmed and in any case.
    fn from_name(name: &str) -> Option<Variable> {
        let variable = match name.trim().to_ascii_lowercase().as_str() {
            "date" => Variable::Date,
            "year" => Variable::Year,
            "month" => Variable::Month,
            "day" => Variable::Day,
            "time" => Variable::Time,
            "make" => Variable::Make,
            "model" => Variable::Model,
            "name" => Variable::Name,
            "folder" => Variable::Folder,
            _ => return None,
        };
        Some(variable)
    }

    /// The value for one image; `None` when a date variable lacks the part of the capture time
    /// it needs.
    fn value(self, values: &VariableValues) -> Option<String> {
        let date = values.capture_time.as_ref();
        match self {
            Variable::Date => {
                let date = date?;
                Some(format!(
                    "{:04}-{:02}-{:02}",
                    date.year, date.month?, date.day?
                ))
            }
            Variable::Year => Some(format!("{:04}", date?.year)),
            Variable::Month => Some(format!("{:02}", date?.month?)),
            Variable::Day => Some(format!("{:02}", date?.day?)),
            Variable::Time => {
                let time = date?.time.as_ref()?;
                Some(format!(
                    "{:02}{:02}{:02}",
                    time.hour, time.minute, time.second?
                ))
            }
            Variable::Make => Some(values.make.clone().unwrap_or_default()),
            Variable::Model => Some(values.model.clone().unwrap_or_default()),
            Variable::Name => Some(values.name.clone()),
            Variable::Folder => Some(values.folder.clone()),
        }
    }
}

/// Reads `text` as the import's names are read; with `values` the variables are filled, without
/// them only the syntax and the names are checked (and the result is not used). The first error
/// in the order of the text is returned.
fn walk(text: &str, values: Option<&VariableValues>) -> Result<String, VariableError> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        match c {
            '{' if chars.peek().map(|&(_, next)| next) == Some('{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek().map(|&(_, next)| next) == Some('}') => {
                chars.next();
                out.push('}');
            }
            '}' => return Err(VariableError::UnexpectedClose),
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some((_, '}')) => break,
                        Some((_, c)) => name.push(c),
                        None => {
                            let rest = text.get(start..).unwrap_or_default();
                            return Err(VariableError::Unclosed(rest.to_string()));
                        }
                    }
                }
                let variable = Variable::from_name(&name)
                    .ok_or_else(|| VariableError::Unknown(name.clone()))?;
                if let Some(values) = values {
                    let value = variable
                        .value(values)
                        .ok_or_else(|| VariableError::NoCaptureTime(name.clone()))?;
                    out.push_str(&value);
                }
            }
            c => out.push(c),
        }
    }
    Ok(out)
}

/// The first error of `text` by syntax and names, without any image (so never
/// [`VariableError::NoCaptureTime`]).
pub fn check(text: &str) -> Result<(), VariableError> {
    walk(text, None).map(|_| ())
}

/// `text` with each variable replaced by its value for one image and `{{` and `}}` by one brace;
/// the rest is kept exactly. The first error in the order of the text is returned and nothing is
/// filled.
pub fn fill(text: &str, values: &VariableValues) -> Result<String, VariableError> {
    walk(text, Some(values))
}

/// `value` with `f` applied to every text that holds variables, in order: a text, every language
/// of a Lang Alt, every list entry and every structure field at any depth; dates and numbers are
/// kept as they are.
fn map_texts(
    value: &Value,
    f: &mut dyn FnMut(&str) -> Result<String, VariableError>,
) -> Result<Value, VariableError> {
    let mapped = match value {
        Value::Text(text) => Value::Text(f(text)?),
        Value::LangAlt(lang_alt) => {
            let entries = lang_alt
                .entries()
                .iter()
                .map(|(language, text)| Ok((language.clone(), f(text)?)))
                .collect::<Result<Vec<_>, VariableError>>()?;
            Value::LangAlt(LangAlt::from_entries(entries))
        }
        Value::List(entries) => Value::List(
            entries
                .iter()
                .map(|entry| f(entry))
                .collect::<Result<Vec<_>, VariableError>>()?,
        ),
        Value::Date(_) | Value::Number(_) => value.clone(),
        Value::Struct(structure) => Value::Struct(map_struct(structure, f)?),
        Value::StructList(entries) => Value::StructList(
            entries
                .iter()
                .map(|entry| map_struct(entry, f))
                .collect::<Result<Vec<_>, VariableError>>()?,
        ),
    };
    Ok(mapped)
}

/// [`map_texts`] on every field of a structure, in order.
fn map_struct(
    structure: &StructValue,
    f: &mut dyn FnMut(&str) -> Result<String, VariableError>,
) -> Result<StructValue, VariableError> {
    let fields = structure
        .fields
        .iter()
        .map(|(id, value)| Ok((id.clone(), map_texts(value, f)?)))
        .collect::<Result<Vec<_>, VariableError>>()?;
    Ok(StructValue { fields })
}

/// [`fill`] on every text of `value`: a text, every language of a Lang Alt (its languages kept, in
/// order), every list entry and every field of a structure or of each entry of a list of
/// structures, at any depth. Dates and numbers are returned as they are. Nothing is cleaned: an
/// entry that is empty after filling stays.
pub fn fill_value(value: &Value, values: &VariableValues) -> Result<Value, VariableError> {
    map_texts(value, &mut |text| fill(text, values))
}

/// [`check`] on every text [`fill_value`] would fill, in the same order, so the first error is the
/// one `fill_value` would give (never [`VariableError::NoCaptureTime`]).
pub fn check_value(value: &Value) -> Result<(), VariableError> {
    map_texts(value, &mut |text| walk(text, None)).map(|_| ())
}

/// `text` with every `{` doubled to `{{` and every `}` to `}}`, so [`fill`] gives `text` back for
/// any image.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '{' || c == '}' {
            out.push(c);
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::date::Time;

    fn image() -> PathBuf {
        Path::new(r"D:\Photos")
            .join("2026-11-22 Trip")
            .join("DSC01234.ARW")
    }

    fn values() -> VariableValues {
        VariableValues::of(
            &image(),
            Some("2026:11:22 14:30:05"),
            Some("SONY"),
            Some("ILCE-1"),
        )
    }

    fn no_capture_time() -> VariableValues {
        VariableValues::of(&image(), None, Some("SONY"), None)
    }

    fn unknown(name: &str) -> Result<String, VariableError> {
        Err(VariableError::Unknown(name.to_string()))
    }

    fn text(text: &str) -> Value {
        Value::Text(text.to_string())
    }

    fn location_shown(city: &str) -> Value {
        Value::StructList(vec![StructValue {
            fields: vec![
                ("Iptc4xmpExt:City".to_string(), text(city)),
                (
                    "exif:GPSLatitude".to_string(),
                    Value::Number("59.3293".to_string()),
                ),
            ],
        }])
    }

    #[test]
    fn every_variable_is_filled() {
        let values = values();
        for (variable, expected) in [
            ("{date}", "2026-11-22"),
            ("{year}", "2026"),
            ("{month}", "11"),
            ("{day}", "22"),
            ("{time}", "143005"),
            ("{make}", "SONY"),
            ("{model}", "ILCE-1"),
            ("{name}", "DSC01234"),
            ("{folder}", "2026-11-22 Trip"),
        ] {
            assert_eq!(
                fill(variable, &values),
                Ok(expected.to_string()),
                "{variable}"
            );
            assert_eq!(check(variable), Ok(()), "{variable}");
        }
        assert_eq!(
            fill("{name} from {folder}", &values),
            Ok("DSC01234 from 2026-11-22 Trip".to_string())
        );
        assert_eq!(
            fill(" a/b: {date}T{time} ", &values),
            Ok(" a/b: 2026-11-22T143005 ".to_string())
        );
        assert_eq!(fill("", &values), Ok(String::new()));
    }

    #[test]
    fn a_name_is_trimmed_and_in_any_case_and_braces_are_doubled() {
        let values = values();
        assert_eq!(fill("{ DATE }", &values), Ok("2026-11-22".to_string()));
        assert_eq!(fill("{Make}", &values), Ok("SONY".to_string()));
        assert_eq!(fill("{{date}}", &values), Ok("{date}".to_string()));
        assert_eq!(fill("a}}b", &values), Ok("a}b".to_string()));
        assert_eq!(fill("{{{date}}}", &values), Ok("{2026-11-22}".to_string()));
    }

    #[test]
    fn syntax_errors_and_unknown_names() {
        let values = values();
        assert_eq!(fill("{year2}", &values), unknown("year2"));
        assert_eq!(fill("{job}", &values), unknown("job"));
        assert_eq!(fill("{ Job }", &values), unknown(" Job "));
        assert_eq!(fill("{seq:4}", &values), unknown("seq:4"));
        assert_eq!(fill("{}", &values), unknown(""));
        assert_eq!(fill("{a{b}", &values), unknown("a{b"));
        assert_eq!(
            fill("x {date", &values),
            Err(VariableError::Unclosed("{date".to_string()))
        );
        assert_eq!(fill("a}b", &values), Err(VariableError::UnexpectedClose));
        assert_eq!(
            fill("{date}}", &values),
            Err(VariableError::UnexpectedClose)
        );
    }

    #[test]
    fn display_texts() {
        assert_eq!(
            VariableError::Unclosed("{date".to_string()).to_string(),
            "'{date' is not closed"
        );
        assert_eq!(
            VariableError::UnexpectedClose.to_string(),
            "a '}' without its '{'"
        );
        assert_eq!(
            VariableError::Unknown(" Job ".to_string()).to_string(),
            "unknown variable { Job }"
        );
        assert_eq!(
            VariableError::NoCaptureTime("date".to_string()).to_string(),
            "the image has no capture time for {date}"
        );
    }

    #[test]
    fn a_capture_time_without_seconds_has_no_time() {
        let values = VariableValues {
            capture_time: Some(IptcDate {
                year: 2026,
                month: Some(11),
                day: Some(22),
                time: Some(Time {
                    hour: 14,
                    minute: 30,
                    second: None,
                    fraction: None,
                    zone: None,
                }),
            }),
            ..VariableValues::default()
        };
        assert_eq!(
            fill("{time}", &values),
            Err(VariableError::NoCaptureTime("time".to_string()))
        );
        assert_eq!(fill("{date}", &values), Ok("2026-11-22".to_string()));
        let year_only = VariableValues {
            capture_time: Some(IptcDate {
                year: 2026,
                month: None,
                day: None,
                time: None,
            }),
            ..VariableValues::default()
        };
        assert_eq!(fill("{year}", &year_only), Ok("2026".to_string()));
        for name in ["date", "month", "day", "time"] {
            assert_eq!(
                fill(&format!("{{{name}}}"), &year_only),
                Err(VariableError::NoCaptureTime(name.to_string()))
            );
        }
    }

    #[test]
    fn the_first_error_is_returned() {
        let values = values();
        assert_eq!(fill("{job} {date", &values), unknown("job"));
        assert_eq!(fill("{date {job}", &values), unknown("date {job"));
        assert_eq!(
            fill("a}b {job}", &values),
            Err(VariableError::UnexpectedClose)
        );
        let none = no_capture_time();
        assert_eq!(
            fill("{Date} {job}", &none),
            Err(VariableError::NoCaptureTime("Date".to_string()))
        );
        assert_eq!(fill("{job} {date}", &none), unknown("job"));
    }

    #[test]
    fn check_gives_the_same_errors_without_values() {
        assert_eq!(
            check("{year2}"),
            Err(VariableError::Unknown("year2".into()))
        );
        assert_eq!(check("{job}"), Err(VariableError::Unknown("job".into())));
        assert_eq!(
            check("{seq:4}"),
            Err(VariableError::Unknown("seq:4".into()))
        );
        assert_eq!(check("{}"), Err(VariableError::Unknown(String::new())));
        assert_eq!(check("{a{b}"), Err(VariableError::Unknown("a{b".into())));
        assert_eq!(check("{date"), Err(VariableError::Unclosed("{date".into())));
        assert_eq!(check("a}b"), Err(VariableError::UnexpectedClose));
        assert_eq!(check("{date}}"), Err(VariableError::UnexpectedClose));
        assert_eq!(
            check("{job} {date"),
            Err(VariableError::Unknown("job".into()))
        );
        assert_eq!(check("plain text"), Ok(()));
        assert_eq!(check("{{"), Ok(()));
        assert_eq!(check("{date} {time}"), Ok(()));
    }

    #[test]
    fn date_variables_need_a_capture_time() {
        let none = no_capture_time();
        for name in ["date", "year", "month", "day", "time"] {
            assert_eq!(
                fill(&format!("{{{name}}}"), &none),
                Err(VariableError::NoCaptureTime(name.to_string()))
            );
        }
        assert_eq!(
            fill("{make} {name}", &none),
            Ok("SONY DSC01234".to_string())
        );
        assert_eq!(fill("[{model}]", &none), Ok("[]".to_string()));
        assert_eq!(
            fill("[{make}]", &VariableValues::default()),
            Ok("[]".to_string())
        );
    }

    #[test]
    fn values_of_an_image() {
        let path = image();
        for blank in [
            "    :  :     :  :  ",
            "0000:00:00 00:00:00",
            "not a date",
            "",
        ] {
            assert_eq!(
                VariableValues::of(&path, Some(blank), None, None).capture_time,
                None,
                "{blank:?}"
            );
        }
        let padded = VariableValues::of(&path, Some(" 2026:11:22 14:30:05\0"), None, None);
        assert_eq!(padded.capture_time, values().capture_time);
        assert!(values().capture_time.is_some());

        let camera = VariableValues::of(&path, None, Some(" SONY "), Some("   "));
        assert_eq!(camera.make.as_deref(), Some("SONY"));
        assert_eq!(camera.model, None);
        assert_eq!(VariableValues::of(&path, None, None, None).make, None);

        let bare = VariableValues::of(Path::new("x.jpg"), None, None, None);
        assert_eq!(bare.folder, "");
        assert_eq!(bare.name, "x");
        #[cfg(windows)]
        assert_eq!(
            VariableValues::of(Path::new(r"D:\x.jpg"), None, None, None).folder,
            ""
        );
        let dotted = VariableValues::of(&Path::new("photos").join("a.b.jpg"), None, None, None);
        assert_eq!(dotted.name, "a.b");
        assert_eq!(dotted.folder, "photos");
    }

    #[test]
    fn fill_value_fills_every_text() {
        let values = values();
        let lang_alt = Value::LangAlt(LangAlt::from_entries(vec![
            ("x-default".to_string(), "{date}".to_string()),
            ("sv".to_string(), "{name} i {folder}".to_string()),
        ]));
        assert_eq!(
            fill_value(&lang_alt, &values),
            Ok(Value::LangAlt(LangAlt::from_entries(vec![
                ("x-default".to_string(), "2026-11-22".to_string()),
                ("sv".to_string(), "DSC01234 i 2026-11-22 Trip".to_string()),
            ])))
        );
        let list = Value::List(vec!["{year}".to_string(), "fixed".to_string()]);
        assert_eq!(
            fill_value(&list, &values),
            Ok(Value::List(vec!["2026".to_string(), "fixed".to_string()]))
        );
        assert_eq!(
            fill_value(&location_shown("{folder}"), &values),
            Ok(location_shown("2026-11-22 Trip"))
        );
        let nested = Value::Struct(StructValue {
            fields: vec![("a".to_string(), location_shown("{make}"))],
        });
        assert_eq!(
            fill_value(&nested, &values),
            Ok(Value::Struct(StructValue {
                fields: vec![("a".to_string(), location_shown("SONY"))],
            }))
        );
        let date = Value::Date(IptcDate {
            year: 2026,
            month: None,
            day: None,
            time: None,
        });
        assert_eq!(fill_value(&date, &values), Ok(date.clone()));
        let number = Value::Number("{date}".to_string());
        assert_eq!(fill_value(&number, &values), Ok(number.clone()));

        let empty = Value::List(vec!["{make}".to_string(), "b".to_string()]);
        assert_eq!(
            fill_value(&empty, &VariableValues::default()),
            Ok(Value::List(vec![String::new(), "b".to_string()]))
        );
        assert_eq!(
            fill_value(&text("{job}"), &values),
            Err(VariableError::Unknown("job".to_string()))
        );
    }

    #[test]
    fn check_value_checks_every_text_fill_value_fills() {
        assert_eq!(check_value(&location_shown("{folder}")), Ok(()));
        assert_eq!(check_value(&Value::Number("{job}".to_string())), Ok(()));
        assert_eq!(check_value(&text("{date}")), Ok(()));
        assert_eq!(
            check_value(&location_shown("{job}")),
            Err(VariableError::Unknown("job".to_string()))
        );
        assert_eq!(
            check_value(&Value::List(vec!["ok".to_string(), "{job}".to_string()])),
            Err(VariableError::Unknown("job".to_string()))
        );
        let two = Value::StructList(vec![
            StructValue {
                fields: vec![("a".to_string(), text("{first}"))],
            },
            StructValue {
                fields: vec![("a".to_string(), text("x}y"))],
            },
        ]);
        let first = Err(VariableError::Unknown("first".to_string()));
        assert_eq!(check_value(&two), first);
        assert_eq!(fill_value(&two, &values()).map(|_| ()), first);
    }

    #[test]
    fn escaped_text_comes_back_through_fill() {
        let values = values();
        for original in ["{x}", "a}b", "{{", "{date", "plain", ""] {
            let escaped = escape(original);
            assert_eq!(check(&escaped), Ok(()), "{original}");
            assert_eq!(
                fill(&escaped, &values),
                Ok(original.to_string()),
                "{original}"
            );
            assert_eq!(
                fill(&escaped, &VariableValues::default()),
                Ok(original.to_string()),
                "{original}"
            );
        }
        assert_eq!(escape("{x}"), "{{x}}");
    }
}
