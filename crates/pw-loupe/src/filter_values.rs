//! ADR 0022 S2: the values a tab keeps for the filter.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::meta_filter::{Field, Group, ImageValues, Value};
use crate::sort::CaptureTime;
use pw_metadata::FilterValues;

/// The images in one IPTC read for the filter (ADR 0022 S2-Q2, as amended 2026-11-25).
pub const FILTER_READ_CHUNK: usize = 10;

/// The Exif fields of one image that the filter uses, from the sort's own read (S2-Q3).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExifSummary {
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub date_time_original: Option<String>,
    pub exposure_time: Option<(u32, u32)>,
    pub f_number: Option<f64>,
    pub focal_length: Option<f64>,
    pub iso: Option<u32>,
}

/// Keeps the filter fields from the sort's inspection of one image.
pub fn exif_summary(info: &pw_raw::ImageInfo) -> ExifSummary {
    ExifSummary {
        make: info.make.clone(),
        model: info.model.clone(),
        lens: info.lens.clone(),
        date_time_original: info.date_time_original.clone(),
        exposure_time: info.exposure_time.as_ref().map(|t| (t.num, t.den)),
        f_number: info.f_number,
        focal_length: info.focal_length,
        iso: info.iso,
    }
}

fn capture_seconds(stored: &str) -> Option<i64> {
    CaptureTime::parse(stored)?;
    let head = stored.trim().get(..19)?;
    let digits = |range: std::ops::Range<usize>| head.get(range)?.parse::<i64>().ok();
    let year = digits(0..4)?;
    let month = digits(5..7)?;
    let day = digits(8..10)?;
    let hour = digits(11..13)?;
    let minute = digits(14..16)?;
    let second = digits(17..19)?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let last_day = match month {
        2 => 28 + i64::from(leap),
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return None,
    };
    if !(1..=last_day).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    // Count from March so the leap day is at the end of its year.
    let year = year - i64::from(month <= 2);
    let march_month = if month <= 2 { month + 9 } else { month - 3 };
    let days = year * 365 + year.div_euclid(4) - year.div_euclid(100)
        + year.div_euclid(400)
        + (153 * march_month + 2) / 5
        + day
        - 1
        - 719_468;
    Some(days * 86400 + hour * 3600 + minute * 60 + second)
}

/// Converts available Exif fields and the file extension to filter values.
pub fn exif_values(path: &Path, summary: Option<&ExifSummary>) -> Vec<(Field, Value)> {
    let mut values = Vec::new();
    let kind = if pw_metadata::supports_sidecar(path) {
        Some("RAW")
    } else {
        match path
            .extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("jpg" | "jpeg") => Some("JPEG"),
            Some("tif" | "tiff") => Some("TIFF"),
            Some("dng") => Some("DNG"),
            _ => None,
        }
    };
    if let Some(kind) = kind {
        values.push((Field::FileType, Value::Text(kind.into())));
    }
    let Some(s) = summary else { return values };
    let make = s.make.as_deref().unwrap_or_default().trim();
    let model = s.model.as_deref().unwrap_or_default().trim();
    let camera = if model.is_empty() {
        make.to_owned()
    } else if model.to_lowercase().starts_with(&make.to_lowercase()) {
        model.to_owned()
    } else {
        format!("{make} {model}")
    };
    if !camera.is_empty() {
        values.push((Field::Camera, Value::Text(camera)));
    }
    if let Some(lens) = s.lens.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        values.push((Field::Lens, Value::Text(lens.into())));
    }
    for (field, number) in [
        (Field::FocalLength, s.focal_length),
        (Field::Aperture, s.f_number),
        (Field::Iso, s.iso.map(f64::from)),
    ] {
        if let Some(n) = number.filter(|n| n.is_finite() && *n > 0.0) {
            values.push((field, Value::Number(n)));
        }
    }
    if let Some((num, den)) = s.exposure_time.filter(|(num, den)| *num != 0 && *den != 0) {
        values.push((
            Field::ExposureTime,
            Value::Number(f64::from(num) / f64::from(den)),
        ));
    }
    if let Some(seconds) = s.date_time_original.as_deref().and_then(capture_seconds) {
        values.push((Field::CaptureDate, Value::Date(seconds)));
    }
    values
}

/// Converts the worker's reconciled IPTC fields to the filter model.
pub fn iptc_values(v: &FilterValues) -> Vec<(Field, Value)> {
    [
        (Field::Title, &v.title),
        (Field::Headline, &v.headline),
        (Field::Description, &v.description),
        (Field::Keywords, &v.keywords),
        (Field::PersonShown, &v.person_shown),
        (Field::Creator, &v.creator),
        (Field::CopyrightNotice, &v.copyright_notice),
        (Field::City, &v.city),
        (Field::Country, &v.country),
        (Field::Location, &v.location),
    ]
    .into_iter()
    .filter_map(|(field, items)| {
        let value = match items.as_slice() {
            [] => return None,
            [one] if !matches!(field, Field::Keywords | Field::PersonShown | Field::Creator) => {
                Value::Text(one.clone())
            }
            _ => Value::List(items.clone()),
        };
        Some((field, value))
    })
    .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum ReadState {
    #[default]
    NotAsked,
    InFlight {
        retry: bool,
    },
    FailedOnce,
    Unreadable,
}

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    image: ImageValues,
    state: ReadState,
}

fn named_image(path: &Path) -> ImageValues {
    ImageValues::new(&path.file_name().unwrap_or_default().to_string_lossy())
}

/// The filter values of one tab's images and the state of their IPTC reads, by the tab's paths.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TabValues {
    entries: HashMap<PathBuf, Entry>,
}

impl TabValues {
    fn entry(&mut self, path: &Path) -> &mut Entry {
        self.entries
            .entry(path.to_owned())
            .or_insert_with(|| Entry {
                image: named_image(path),
                state: ReadState::NotAsked,
            })
    }
    /// Drops all groups and pending reads on a fresh listing.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    /// Keeps only paths still listed in the tab.
    pub fn retain(&mut self, files: &[PathBuf]) {
        if self.entries.is_empty() {
            return;
        }
        let listed: HashSet<_> = files.iter().collect();
        self.entries.retain(|p, _| listed.contains(p));
    }
    /// Returns the groups kept for a listed image.
    pub fn values(&self, path: &Path) -> Option<&ImageValues> {
        self.entries.get(path).map(|e| &e.image)
    }
    /// Records an inspection, including a failed inspection's file type.
    pub fn record_exif(&mut self, path: &Path, summary: Option<&ExifSummary>) {
        self.entry(path).image.set_exif(exif_values(path, summary));
    }
    /// Selects the next eligible paths in display order. A retry travels alone.
    pub fn iptc_chunk(
        &self,
        order: &[PathBuf],
        write_pending: &dyn Fn(&Path) -> bool,
    ) -> Vec<PathBuf> {
        let mut chunk = Vec::new();
        for path in order {
            let entry = self.entries.get(path);
            if entry.is_some_and(|e| e.image.is_read(Group::Iptc)) {
                continue;
            }
            match entry.map(|e| e.state).unwrap_or_default() {
                ReadState::FailedOnce if chunk.is_empty() && !write_pending(path) => {
                    return vec![path.clone()];
                }
                ReadState::NotAsked if !write_pending(path) => chunk.push(path.clone()),
                _ => continue,
            }
            if chunk.len() == FILTER_READ_CHUNK {
                break;
            }
        }
        chunk
    }
    /// Marks a submitted chunk, remembering whether this request is a retry.
    pub fn mark_in_flight(&mut self, paths: &[PathBuf]) {
        for path in paths {
            let e = self.entry(path);
            e.state = ReadState::InFlight {
                retry: e.state == ReadState::FailedOnce,
            };
        }
    }
    /// Ends attempts for paths refused by a stopped worker.
    pub fn mark_unreadable(&mut self, paths: &[PathBuf]) {
        for path in paths {
            self.entry(path).state = ReadState::Unreadable;
        }
    }
    /// Releases unanswered paths when a chunk finishes.
    pub fn end_in_flight(&mut self, paths: &[PathBuf]) {
        for path in paths {
            if let Some(e) = self.entries.get_mut(path)
                && let ReadState::InFlight { retry } = e.state
            {
                e.state = if retry {
                    ReadState::FailedOnce
                } else {
                    ReadState::NotAsked
                };
            }
        }
    }
    /// Accepts only outstanding reads, retrying a failed path once.
    pub fn record_iptc(&mut self, path: &Path, result: Result<&FilterValues, &str>) {
        let Some(e) = self.entries.get_mut(path) else {
            return;
        };
        let ReadState::InFlight { retry } = e.state else {
            return;
        };
        e.state = match result {
            Ok(values) => {
                e.image.set_iptc(iptc_values(values));
                ReadState::NotAsked
            }
            Err(_) if retry => ReadState::Unreadable,
            Err(_) => ReadState::FailedOnce,
        };
    }
    /// Invalidates IPTC after a write while keeping the image name and Exif group.
    pub fn forget_iptc(&mut self, path: &Path) {
        let Some(e) = self.entries.get_mut(path) else {
            return;
        };
        let mut image = named_image(path);
        if e.image.is_read(Group::Exif) {
            image.set_exif(
                Field::ALL
                    .into_iter()
                    .filter(|f| f.group() == Group::Exif)
                    .filter_map(|f| Some((f, e.image.value(f)?.clone())))
                    .collect(),
            );
        }
        e.image = image;
        e.state = ReadState::NotAsked;
    }
    /// Lists images awaiting inspection in the caller's order.
    pub fn exif_wanted(&self, order: &[PathBuf]) -> Vec<PathBuf> {
        order
            .iter()
            .filter(|p| !self.values(p).is_some_and(|v| v.is_read(Group::Exif)))
            .cloned()
            .collect()
    }
    /// Counts read, listed and unreadable images for one group.
    pub fn counts(&self, group: Group, files: &[PathBuf]) -> (usize, usize, usize) {
        let entries = files.iter().filter_map(|p| self.entries.get(p));
        let read = entries.clone().filter(|e| e.image.is_read(group)).count();
        let unreadable = entries
            .filter(|e| group == Group::Iptc && e.state == ReadState::Unreadable)
            .count();
        (read, files.len(), unreadable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_type_survives_a_failed_inspection() {
        for (path, kind) in [
            ("a.ARW", "RAW"),
            ("a.dng", "DNG"),
            ("a.JPG", "JPEG"),
            ("a.jpeg", "JPEG"),
            ("a.tif", "TIFF"),
            ("a.TIFF", "TIFF"),
        ] {
            assert_eq!(
                exif_values(Path::new(path), None),
                [(Field::FileType, Value::Text(kind.into()))]
            );
        }
        assert!(exif_values(Path::new("a.png"), None).is_empty());
    }

    #[test]
    fn camera_combines_trimmed_make_and_model() {
        for (make, model, want) in [
            (Some("Canon"), Some("Canon EOS R5"), Some("Canon EOS R5")),
            (Some(" Canon "), Some(" canon EOS "), Some("canon EOS")),
            (Some("SONY"), Some("ILCE-7RM5"), Some("SONY ILCE-7RM5")),
            (Some("  "), Some("X100V"), Some("X100V")),
            (Some("Fuji"), None, Some("Fuji")),
            (None, None, None),
        ] {
            let s = ExifSummary {
                make: make.map(String::from),
                model: model.map(String::from),
                ..Default::default()
            };
            let values = exif_values(Path::new("a"), Some(&s));
            assert_eq!(
                values,
                want.map(|v| (Field::Camera, Value::Text(v.into())))
                    .into_iter()
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn measurements_require_positive_finite_values() {
        let mut info = pw_raw::ImageInfo {
            lens: Some("  Zoom ".into()),
            f_number: Some(2.8),
            exposure_time: Some(pw_raw::ExposureTime { num: 1, den: 250 }),
            focal_length: Some(35.0),
            iso: Some(0),
            ..Default::default()
        };
        let values: HashMap<_, _> = exif_values(Path::new("a"), Some(&exif_summary(&info)))
            .into_iter()
            .collect();
        assert_eq!(values[&Field::Aperture], Value::Number(2.8));
        assert_eq!(values[&Field::FocalLength], Value::Number(35.0));
        assert_eq!(values[&Field::ExposureTime], Value::Number(0.004));
        assert_eq!(values[&Field::Lens], Value::Text("Zoom".into()));
        assert!(!values.contains_key(&Field::Iso));
        info.f_number = Some(f64::NAN);
        info.focal_length = Some(f64::INFINITY);
        info.lens = Some(" ".into());
        for (num, den) in [(0, 250), (1, 0)] {
            info.exposure_time = Some(pw_raw::ExposureTime { num, den });
            assert!(exif_values(Path::new("a"), Some(&exif_summary(&info))).is_empty());
        }
    }

    #[test]
    fn capture_dates_validate_calendar_and_clock() {
        let date = |text: &str| {
            let s = ExifSummary {
                date_time_original: Some(text.into()),
                ..Default::default()
            };
            exif_values(Path::new("a"), Some(&s))
                .into_iter()
                .find(|(f, _)| *f == Field::CaptureDate)
                .map(|(_, v)| v)
        };
        assert_eq!(
            date("2024:03:01 12:00:00"),
            Some(Value::Date(1_709_294_400))
        );
        assert_eq!(
            date("2024:03:01 12:01:00"),
            Some(Value::Date(1_709_294_460))
        );
        assert_eq!(date("1970:01:01 00:00:00"), Some(Value::Date(0)));
        assert_eq!(date("1969:12:31 23:59:59"), Some(Value::Date(-1)));
        for bad in [
            "2024:02:30 12:00:00",
            "0000:00:00 00:00:00",
            "1900:02:29 00:00:00",
            "2024:13:01 00:00:00",
            "2024:01:01 24:00:00",
            "2024:01:01 00:60:00",
            "2024:01:01 00:00:60",
        ] {
            assert_eq!(date(bad), None);
        }
    }

    #[test]
    fn iptc_preserves_list_fields_and_multiple_text_values() {
        let mut v = FilterValues {
            title: vec!["Title".into()],
            keywords: vec!["keyword".into()],
            ..Default::default()
        };
        assert_eq!(
            iptc_values(&v),
            [
                (Field::Title, Value::Text("Title".into())),
                (Field::Keywords, Value::List(v.keywords.clone()))
            ]
        );
        v.title.push("Other".into());
        assert!(iptc_values(&v).contains(&(Field::Title, Value::List(v.title.clone()))));
    }

    #[test]
    fn chunks_skip_ineligible_files_and_retry_failures_alone() {
        let paths: Vec<_> = (0..18)
            .rev()
            .map(|i| PathBuf::from(i.to_string()))
            .collect();
        let mut tab = TabValues::default();
        assert_eq!(tab.counts(Group::Iptc, &paths), (0, 18, 0));
        tab.mark_in_flight(&paths[..3]);
        assert!(tab.values(&paths[0]).is_some());
        tab.record_iptc(&paths[1], Ok(&FilterValues::default()));
        tab.record_iptc(&paths[2], Err("failed"));
        tab.mark_unreadable(&paths[3..4]);
        let pending = |path: &Path| path == paths[4];
        assert_eq!(tab.iptc_chunk(&paths, &pending), paths[2..3]);
        tab.mark_in_flight(&paths[2..3]);
        tab.record_iptc(&paths[2], Err("failed again"));
        assert_eq!(
            tab.iptc_chunk(&paths, &pending),
            paths[5..5 + FILTER_READ_CHUNK]
        );
        assert_eq!(tab.counts(Group::Iptc, &paths), (1, 18, 2));
        assert_eq!(tab.counts(Group::Exif, &paths), (0, 18, 0));
        tab.end_in_flight(&paths);
        tab.mark_in_flight(&paths[6..7]);
        tab.record_iptc(&paths[6], Err("failed"));
        let next = tab.iptc_chunk(&paths, &pending);
        assert_eq!(next.first(), paths.first());
        assert!(!next.contains(&paths[6]));
    }

    #[test]
    fn stale_answers_and_listing_changes_preserve_group_state() {
        let paths = [PathBuf::from("first.jpg"), PathBuf::from("second.jpg")];
        let mut tab = TabValues::default();
        tab.mark_in_flight(&paths);
        tab.clear();
        tab.record_iptc(&paths[0], Ok(&FilterValues::default()));
        assert!(tab.values(&paths[0]).is_none());
        tab.record_exif(&paths[0], None);
        tab.mark_in_flight(&paths);
        tab.record_iptc(&paths[0], Ok(&FilterValues::default()));
        assert_eq!(tab.counts(Group::Iptc, &paths), (1, 2, 0));
        tab.forget_iptc(&paths[0]);
        assert_eq!(tab.iptc_chunk(&paths, &|_| false), paths[..1]);
        let image = tab.values(&paths[0]).unwrap();
        assert_eq!(
            image.value(Field::FileType),
            Some(&Value::Text("JPEG".into()))
        );
        assert!(!image.is_read(Group::Iptc));
        assert_eq!(tab.exif_wanted(&paths), paths[1..]);
        tab.retain(&paths[..1]);
        assert!(tab.values(&paths[1]).is_none());
        tab.clear();
        assert_eq!(tab, TabValues::default());
    }
}
