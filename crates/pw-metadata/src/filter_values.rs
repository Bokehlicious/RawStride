//! The values ADR 0022's filter reads, taken from a `MetadataRead`.

use crate::read::MetadataRead;
use crate::value::Value;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterValues {
    pub title: Vec<String>,
    pub headline: Vec<String>,
    pub description: Vec<String>,
    pub keywords: Vec<String>,
    pub person_shown: Vec<String>,
    pub creator: Vec<String>,
    pub copyright_notice: Vec<String>,
    pub city: Vec<String>,
    pub country: Vec<String>,
    pub location: Vec<String>,
}

impl FilterValues {
    pub fn from_metadata(read: &MetadataRead) -> Self {
        let texts = |id: &str| {
            let mut values = Vec::new();
            let mut add = |text: &String| {
                if !text.trim().is_empty() {
                    values.push(text.clone());
                }
            };
            match read
                .reconciled
                .metadata
                .get(id)
                .map(|property| &property.value)
            {
                Some(Value::Text(text)) => add(text),
                Some(Value::List(items)) => items.iter().for_each(add),
                Some(Value::LangAlt(alt)) => alt.entries().iter().for_each(|(_, text)| add(text)),
                _ => {}
            }
            values
        };
        Self {
            title: texts("dc:title"),
            headline: texts("photoshop:Headline"),
            description: texts("dc:description"),
            keywords: texts("dc:subject"),
            person_shown: texts("Iptc4xmpExt:PersonInImage"),
            creator: texts("dc:creator"),
            copyright_notice: texts("dc:rights"),
            city: texts("photoshop:City"),
            country: texts("photoshop:Country"),
            location: texts("Iptc4xmpCore:Location"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iim::CodedCharset;
    use crate::reconcile::{FileKind, IimDigest, Readings, reconcile};
    use crate::value::LangAlt;
    use crate::write_plan::FileState;

    fn filter(xmp: Vec<(String, Value)>) -> FilterValues {
        let kind = FileKind::Jpeg;
        FilterValues::from_metadata(&MetadataRead {
            reconciled: reconcile(Readings {
                kind,
                xmp,
                camera_xmp: vec![],
                iim: None,
                exif: Default::default(),
            }),
            file_notices: vec![],
            state: FileState {
                kind,
                iim_present: false,
                iim_charset: CodedCharset::Absent,
                iim_values: vec![],
                digest: IimDigest::Missing,
                exif_present: Default::default(),
                xmp_present: Default::default(),
            },
        })
    }

    #[test]
    fn each_property_fills_only_its_filter_field() {
        let lang = Value::LangAlt(LangAlt::from_entries(vec![(
            "x-default".into(),
            "test".into(),
        )]));
        let text = Value::Text("test".into());
        let list = Value::List(vec!["test".into()]);
        let values = vec![String::from("test")];
        for (id, value, expected) in [
            (
                "dc:title",
                &lang,
                FilterValues {
                    title: values.clone(),
                    ..Default::default()
                },
            ),
            (
                "photoshop:Headline",
                &text,
                FilterValues {
                    headline: values.clone(),
                    ..Default::default()
                },
            ),
            (
                "dc:description",
                &lang,
                FilterValues {
                    description: values.clone(),
                    ..Default::default()
                },
            ),
            (
                "dc:subject",
                &list,
                FilterValues {
                    keywords: values.clone(),
                    ..Default::default()
                },
            ),
            (
                "Iptc4xmpExt:PersonInImage",
                &list,
                FilterValues {
                    person_shown: values.clone(),
                    ..Default::default()
                },
            ),
            (
                "dc:creator",
                &list,
                FilterValues {
                    creator: values.clone(),
                    ..Default::default()
                },
            ),
            (
                "dc:rights",
                &lang,
                FilterValues {
                    copyright_notice: values.clone(),
                    ..Default::default()
                },
            ),
            (
                "photoshop:City",
                &text,
                FilterValues {
                    city: values.clone(),
                    ..Default::default()
                },
            ),
            (
                "photoshop:Country",
                &text,
                FilterValues {
                    country: values.clone(),
                    ..Default::default()
                },
            ),
            (
                "Iptc4xmpCore:Location",
                &text,
                FilterValues {
                    location: values.clone(),
                    ..Default::default()
                },
            ),
        ] {
            assert_eq!(filter(vec![(id.into(), value.clone())]), expected, "{id}");
        }
    }

    #[test]
    fn languages_and_list_items_keep_their_order_and_text() {
        let result = filter(vec![
            (
                "dc:title".into(),
                Value::LangAlt(LangAlt::from_entries(vec![
                    ("sv".into(), " Sol ".into()),
                    ("en".into(), "Sun".into()),
                ])),
            ),
            (
                "dc:subject".into(),
                Value::List(vec!["z".into(), " a".into()]),
            ),
        ]);
        assert_eq!(result.title, [" Sol ", "Sun"]);
        assert_eq!(result.keywords, ["z", " a"]);
    }

    #[test]
    fn empty_text_blank_items_and_absent_properties_are_omitted() {
        let result = filter(vec![
            ("photoshop:City".into(), Value::Text(String::new())),
            (
                "dc:subject".into(),
                Value::List(vec!["   ".into(), "kept".into()]),
            ),
        ]);
        assert_eq!(
            result,
            FilterValues {
                keywords: vec!["kept".into()],
                ..Default::default()
            }
        );
    }
}
