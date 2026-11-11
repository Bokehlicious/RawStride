//! Dates and times of IPTC metadata (ADR 0007 §7, §5.3, §6.1).
//!
//! One value type, [`IptcDate`], for IPTC Date Created and the other date properties:
//! - **Truncated dates** are kept as they are: `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, with or without a
//!   time (IPTC §7.10, §14.2). A missing part is `None`, never filled in.
//! - **A time zone is never invented** (MWG §4.2.3.4): a value read without a zone stays zone-less
//!   in every form it is written to. There is no time-zone conversion in this module and the
//!   computer's time zone is never used.
//! - The XMP form (`parse_xmp`, `to_xmp`) writes back exactly the text it read.
//! - The IIM form (`to_iim`, `from_iim`) is Date Created 2:55 (`YYYYMMDD`, `00` for an unknown month
//!   or day) and Time Created 2:60 (`hhmmss±hhmm`), which is only written when the zone is known
//!   (Q3). IIM has no sub-seconds, so they are dropped there.
//! - Exif DateTimeOriginal, SubSecTimeOriginal and OffsetTimeOriginal are only read
//!   (`from_exif`); nothing here writes Exif (ADR 0007 §7: DateTimeOriginal is never changed).

use std::fmt;

/// A date, possibly truncated, with an optional time.
///
/// A day is only present with a month, and a time only with a day. The parsers in this module
/// never produce anything else; the writers skip a part whose parent part is missing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IptcDate {
    pub year: u16,
    pub month: Option<u8>,
    pub day: Option<u8>,
    pub time: Option<Time>,
}

/// A time of day, with the parts that were given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Time {
    pub hour: u8,
    pub minute: u8,
    pub second: Option<u8>,
    /// The digits after the decimal point of the seconds, exactly as read (no rounding). Only
    /// present with `second`.
    pub fraction: Option<String>,
    /// `None` when the zone is not known. Never filled in by this module.
    pub zone: Option<Zone>,
}

/// A known time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// `Z` in the text; written back as `Z` in XMP and as `+0000` in IIM.
    Utc,
    /// Minutes east of UTC (`+01:00` is 60, `-05:30` is -330).
    Offset(i16),
}

/// Why a date or time could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DateError {
    /// No value: an empty text, only spaces, or an Exif "unknown" form with zeros or blanks.
    Empty,
    /// A text that is not a valid date or time in the expected form; holds the text.
    Invalid(String),
}

impl fmt::Display for DateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DateError::Empty => write!(f, "the date is empty"),
            DateError::Invalid(text) => write!(f, "not a valid date or time: \"{text}\""),
        }
    }
}

impl std::error::Error for DateError {}

impl IptcDate {
    /// Reads the XMP form (IPTC §14.2): `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, `YYYY-MM-DDThh:mm`,
    /// `YYYY-MM-DDThh:mm:ss` or `YYYY-MM-DDThh:mm:ss.s…`, each time form with or without `Z` or
    /// `±hh:mm`.
    ///
    /// `-00:00` is refused: it cannot be written back as the same text (`Zone::Offset(0)` is
    /// written `+00:00`), and in RFC 3339 it means that the offset is unknown.
    pub fn parse_xmp(text: &str) -> Result<IptcDate, DateError> {
        if text.trim().is_empty() {
            return Err(DateError::Empty);
        }
        let invalid = || DateError::Invalid(text.to_string());
        if !text.is_ascii() {
            return Err(invalid());
        }
        let (date_text, time_text) = match text.split_once('T') {
            Some((date, time)) => (date, Some(time)),
            None => (text, None),
        };

        let year = number(date_text, 0, 4).ok_or_else(invalid)?;
        let (month, day) = match date_text.len() {
            4 => (None, None),
            7 if separator(date_text, 4, '-') => {
                (Some(number(date_text, 5, 7).ok_or_else(invalid)?), None)
            }
            10 if separator(date_text, 4, '-') && separator(date_text, 7, '-') => (
                Some(number(date_text, 5, 7).ok_or_else(invalid)?),
                Some(number(date_text, 8, 10).ok_or_else(invalid)?),
            ),
            _ => return Err(invalid()),
        };
        let date = checked_date(year, month, day).ok_or_else(invalid)?;

        let time = match time_text {
            None => None,
            // A time needs a full date.
            Some(_) if date.day.is_none() => return Err(invalid()),
            Some(time_text) => Some(parse_xmp_time(time_text).ok_or_else(invalid)?),
        };
        Ok(IptcDate { time, ..date })
    }

    /// Writes the XMP form; for a value read with [`IptcDate::parse_xmp`] this is exactly the text
    /// that was read.
    pub fn to_xmp(&self) -> String {
        let mut out = format!("{:04}", self.year);
        let Some(month) = self.month else {
            return out;
        };
        out.push_str(&format!("-{month:02}"));
        let Some(day) = self.day else {
            return out;
        };
        out.push_str(&format!("-{day:02}"));
        let Some(time) = &self.time else {
            return out;
        };
        out.push_str(&format!("T{:02}:{:02}", time.hour, time.minute));
        if let Some(second) = time.second {
            out.push_str(&format!(":{second:02}"));
            if let Some(fraction) = &time.fraction {
                out.push('.');
                out.push_str(fraction);
            }
        }
        match time.zone {
            None => {}
            Some(Zone::Utc) => out.push('Z'),
            Some(Zone::Offset(minutes)) => out.push_str(&offset_text(minutes, ":")),
        }
        out
    }

    /// The IIM form (ADR 0007 §5.3, Q3): Date Created 2:55 as `YYYYMMDD` with `00` for an unknown
    /// month or day, and Time Created 2:60 as `hhmmss±hhmm` **only when there is a time and its
    /// zone is known** (`Utc` as `+0000`), otherwise `None`. The fraction is dropped; missing
    /// seconds are written as `00`.
    pub fn to_iim(&self) -> (String, Option<String>) {
        let month = self.month.unwrap_or(0);
        let day = if self.month.is_some() {
            self.day.unwrap_or(0)
        } else {
            0
        };
        let date = format!("{:04}{month:02}{day:02}", self.year);
        let time = match (&self.time, self.day.is_some() && self.month.is_some()) {
            (Some(time), true) => time.zone.map(|zone| {
                let minutes = match zone {
                    Zone::Utc => 0,
                    Zone::Offset(minutes) => minutes,
                };
                format!(
                    "{:02}{:02}{:02}{}",
                    time.hour,
                    time.minute,
                    time.second.unwrap_or(0),
                    offset_text(minutes, "")
                )
            }),
            _ => None,
        };
        (date, time)
    }

    /// Reads IIM Date Created 2:55 (`YYYYMMDD`, `00` for an unknown month or day) and, if present,
    /// Time Created 2:60 (`hhmmss±hhmm`, or `hhmmss` without an offset, which gives a time
    /// **without a zone**). A day without a month is an error, and so is a time with a date that
    /// has no day.
    pub fn from_iim(date: &str, time: Option<&str>) -> Result<IptcDate, DateError> {
        if date.trim().is_empty() {
            return Err(DateError::Empty);
        }
        let invalid_date = || DateError::Invalid(date.to_string());
        if date.len() != 8 || !date.is_ascii() {
            return Err(invalid_date());
        }
        let year = number(date, 0, 4).ok_or_else(invalid_date)?;
        let month = number(date, 4, 6).ok_or_else(invalid_date)?;
        let day = number(date, 6, 8).ok_or_else(invalid_date)?;
        let (month, day) = match (month, day) {
            (0, 0) => (None, None),
            (0, _) => return Err(invalid_date()),
            (month, 0) => (Some(month), None),
            (month, day) => (Some(month), Some(day)),
        };
        let parsed = checked_date(year, month, day).ok_or_else(invalid_date)?;

        let time = match time {
            None => None,
            Some(text) => {
                let invalid_time = || DateError::Invalid(text.to_string());
                if parsed.day.is_none() {
                    return Err(invalid_time());
                }
                Some(parse_iim_time(text).ok_or_else(invalid_time)?)
            }
        };
        Ok(IptcDate { time, ..parsed })
    }

    /// Reads Exif DateTimeOriginal (`YYYY:MM:DD hh:mm:ss`) with SubSecTimeOriginal (digits, kept as
    /// the fraction) and OffsetTimeOriginal (`±hh:mm`, the zone). **Without an offset the result
    /// has no zone** (ADR 0007 §7). An empty value, only spaces, or the "unknown" forms with zeros
    /// or blanks (`0000:00:00 00:00:00`, `    :  :     :  :  `) give [`DateError::Empty`].
    ///
    /// A SubSecTimeOriginal or OffsetTimeOriginal that is empty or blank (Exif pads unknown values
    /// with spaces) counts as not given.
    pub fn from_exif(
        date_time: &str,
        subsec: Option<&str>,
        offset: Option<&str>,
    ) -> Result<IptcDate, DateError> {
        if date_time.chars().all(|c| c == ' ' || c == '0' || c == ':') {
            return Err(DateError::Empty);
        }
        let invalid = || DateError::Invalid(date_time.to_string());
        if date_time.len() != 19
            || !date_time.is_ascii()
            || !separator(date_time, 4, ':')
            || !separator(date_time, 7, ':')
            || !separator(date_time, 10, ' ')
            || !separator(date_time, 13, ':')
            || !separator(date_time, 16, ':')
        {
            return Err(invalid());
        }
        let year = number(date_time, 0, 4).ok_or_else(invalid)?;
        let month = number(date_time, 5, 7).ok_or_else(invalid)?;
        let day = number(date_time, 8, 10).ok_or_else(invalid)?;
        let date = checked_date(year, Some(month), Some(day)).ok_or_else(invalid)?;
        let hour = number(date_time, 11, 13).ok_or_else(invalid)?;
        let minute = number(date_time, 14, 16).ok_or_else(invalid)?;
        let second = number(date_time, 17, 19).ok_or_else(invalid)?;
        if !clock_in_range(hour, minute, Some(second)) {
            return Err(invalid());
        }

        let fraction = match subsec.map(|s| s.trim_matches(' ')) {
            None | Some("") => None,
            Some(digits) if digits.bytes().all(|b| b.is_ascii_digit()) => Some(digits.to_string()),
            Some(other) => return Err(DateError::Invalid(other.to_string())),
        };
        let zone = match offset {
            None => None,
            Some(text) if text.chars().all(|c| c == ' ' || c == ':') => None,
            Some(text) => {
                Some(parse_offset(text, true).ok_or_else(|| DateError::Invalid(text.to_string()))?)
            }
        };

        Ok(IptcDate {
            time: Some(Time {
                hour,
                minute,
                second: Some(second),
                fraction,
                zone,
            }),
            ..date
        })
    }
}

/// The time part of the XMP form: `hh:mm`, `hh:mm:ss` or `hh:mm:ss.s…`, then nothing, `Z` or
/// `±hh:mm`.
fn parse_xmp_time(text: &str) -> Option<Time> {
    let (clock, zone) = if let Some(clock) = text.strip_suffix('Z') {
        (clock, Some(Zone::Utc))
    } else if let Some(at) = text.rfind(['+', '-']) {
        let (clock, offset) = text.split_at(at);
        (clock, Some(parse_offset(offset, true)?))
    } else {
        (text, None)
    };

    let hour = number(clock, 0, 2)?;
    if !separator(clock, 2, ':') {
        return None;
    }
    let minute = number(clock, 3, 5)?;
    let (second, fraction) = match clock.len() {
        5 => (None, None),
        8 if separator(clock, 5, ':') => (Some(number(clock, 6, 8)?), None),
        len if len > 9 && separator(clock, 5, ':') && separator(clock, 8, '.') => {
            let digits = clock.get(9..)?;
            if !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            (Some(number(clock, 6, 8)?), Some(digits.to_string()))
        }
        _ => return None,
    };
    if !clock_in_range(hour, minute, second) {
        return None;
    }
    Some(Time {
        hour,
        minute,
        second,
        fraction,
        zone,
    })
}

/// The IIM time 2:60: `hhmmss±hhmm`, or `hhmmss` without a zone.
fn parse_iim_time(text: &str) -> Option<Time> {
    if !text.is_ascii() {
        return None;
    }
    let zone = match text.len() {
        6 => None,
        11 => Some(parse_offset(text.get(6..)?, false)?),
        _ => return None,
    };
    let hour = number(text, 0, 2)?;
    let minute = number(text, 2, 4)?;
    let second = number(text, 4, 6)?;
    if !clock_in_range(hour, minute, Some(second)) {
        return None;
    }
    Some(Time {
        hour,
        minute,
        second: Some(second),
        fraction: None,
        zone,
    })
}

/// `±hh:mm` (with a colon) or `±hhmm` (without), hours 0–23 and minutes 0–59. A negative zero
/// offset is refused (see [`IptcDate::parse_xmp`]).
fn parse_offset(text: &str, colon: bool) -> Option<Zone> {
    if !text.is_ascii() {
        return None;
    }
    let negative = match text.get(..1)? {
        "+" => false,
        "-" => true,
        _ => return None,
    };
    let (hours, minutes) = if colon {
        if text.len() != 6 || !separator(text, 3, ':') {
            return None;
        }
        (number::<u8>(text, 1, 3)?, number::<u8>(text, 4, 6)?)
    } else {
        if text.len() != 5 {
            return None;
        }
        (number::<u8>(text, 1, 3)?, number::<u8>(text, 3, 5)?)
    };
    if hours > 23 || minutes > 59 {
        return None;
    }
    let total = i16::from(hours) * 60 + i16::from(minutes);
    if negative && total == 0 {
        return None;
    }
    Some(Zone::Offset(if negative { -total } else { total }))
}

/// `+hh:mm` / `-hh:mm` with `separator` between hours and minutes.
fn offset_text(minutes: i16, separator: &str) -> String {
    let sign = if minutes < 0 { '-' } else { '+' };
    let abs = minutes.unsigned_abs();
    format!("{sign}{:02}{separator}{:02}", abs / 60, abs % 60)
}

/// The ASCII digits in `text[start..end]` as a number; `None` if anything else is there.
fn number<T: TryFrom<u32>>(text: &str, start: usize, end: usize) -> Option<T> {
    let part = text.get(start..end)?;
    if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value = part
        .bytes()
        .fold(0u32, |acc, b| acc * 10 + u32::from(b - b'0'));
    T::try_from(value).ok()
}

/// Whether the byte at `index` is `expected`.
fn separator(text: &str, index: usize, expected: char) -> bool {
    text.as_bytes().get(index).copied() == u8::try_from(expected).ok()
}

/// The date if the month is 1–12 and the day exists in that month (leap years included).
fn checked_date(year: u16, month: Option<u8>, day: Option<u8>) -> Option<IptcDate> {
    if let Some(month) = month {
        if !(1..=12).contains(&month) {
            return None;
        }
        if let Some(day) = day
            && (day < 1 || day > days_in_month(year, month))
        {
            return None;
        }
    } else if day.is_some() {
        return None;
    }
    Some(IptcDate {
        year,
        month,
        day,
        time: None,
    })
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn clock_in_range(hour: u8, minute: u8, second: Option<u8>) -> bool {
    hour <= 23 && minute <= 59 && second.is_none_or(|s| s <= 59)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xmp(text: &str) -> IptcDate {
        match IptcDate::parse_xmp(text) {
            Ok(date) => date,
            Err(e) => panic!("{text:?} should parse: {e}"),
        }
    }

    fn date(year: u16, month: Option<u8>, day: Option<u8>, time: Option<Time>) -> IptcDate {
        IptcDate {
            year,
            month,
            day,
            time,
        }
    }

    fn time(
        hour: u8,
        minute: u8,
        second: Option<u8>,
        fraction: Option<&str>,
        zone: Option<Zone>,
    ) -> Time {
        Time {
            hour,
            minute,
            second,
            fraction: fraction.map(str::to_string),
            zone,
        }
    }

    #[test]
    fn every_xmp_form_parses_and_round_trips_exactly() {
        let cases = [
            "2026",
            "2026-11",
            "2026-11-04",
            "2026-11-04T10:15",
            "2026-11-04T10:15Z",
            "2026-11-04T10:15+01:00",
            "2026-11-04T10:15:00",
            "2026-11-04T10:15:00Z",
            "2026-11-04T10:15:00+01:00",
            "2026-11-04T10:15:00.123",
            "2026-11-04T10:15:00.123Z",
            "2026-11-04T10:15:00.123-05:30",
            "2026-11-04T10:15:00.5+14:00",
            "2026-11-04T10:15:00+00:00",
            "2026-11-04T23:59:59.000100-23:59",
            "2024-02-29",
            "0001-01-01T00:00",
        ];
        for text in cases {
            assert_eq!(xmp(text).to_xmp(), text, "round trip of {text:?}");
        }
    }

    #[test]
    fn xmp_values_are_read_into_the_right_parts() {
        assert_eq!(xmp("1918"), date(1918, None, None, None));
        assert_eq!(xmp("2026-11"), date(2026, Some(11), None, None));
        assert_eq!(xmp("2026-11-04"), date(2026, Some(11), Some(4), None));
        assert_eq!(
            xmp("2026-11-04T10:15:00+01:00"),
            date(
                2026,
                Some(11),
                Some(4),
                Some(time(10, 15, Some(0), None, Some(Zone::Offset(60))))
            )
        );
        assert_eq!(
            xmp("2026-11-04T10:15:00Z"),
            date(
                2026,
                Some(11),
                Some(4),
                Some(time(10, 15, Some(0), None, Some(Zone::Utc)))
            )
        );
        assert_eq!(
            xmp("2026-11-04T10:15:00.123-05:30"),
            date(
                2026,
                Some(11),
                Some(4),
                Some(time(10, 15, Some(0), Some("123"), Some(Zone::Offset(-330))))
            )
        );
        assert_eq!(
            xmp("2026-11-04T10:15:00"),
            date(
                2026,
                Some(11),
                Some(4),
                Some(time(10, 15, Some(0), None, None))
            )
        );
        assert_eq!(
            xmp("2026-11-04T10:15"),
            date(
                2026,
                Some(11),
                Some(4),
                Some(time(10, 15, None, None, None))
            )
        );
        // The fraction is kept as written, trailing zeros included.
        assert_eq!(
            xmp("2026-11-04T10:15:00.1230")
                .time
                .and_then(|t| t.fraction),
            Some("1230".to_string())
        );
    }

    #[test]
    fn invalid_xmp_values_are_errors() {
        for text in [
            "2026-13",
            "2026-00",
            "2026-02-30",
            "2023-02-29",
            "1900-02-29",
            "2026-04-31",
            "2026-11-00",
            "2026-11-04T24:00",
            "2026-11-04T10:60",
            "2026-11-04T10:15:60",
            "2026-11-04T10:15+25:00",
            "2026-11-04T10:15+01:60",
            "2026-11-04T10:15-00:00",
            "2026-11-04T10:15+0100",
            "2026-11-04T10:15+01",
            "2026-11-04 10:15",
            "26-11-04",
            "2026-1-04",
            "2026-11-4",
            "2026-11T10:15",
            "2026T10:15",
            "2026-11-04T",
            "2026-11-04T10",
            "2026-11-04T10:15:00.",
            "2026-11-04T10:15:00.12a",
            "2026-11-04T10:15.5",
            "2026-11-04T10:15:00ZZ",
            "2026-11-04T10:15:00 ",
            " 2026",
            "2026-11-04Z",
            "２０２６",
            "+2026",
        ] {
            match IptcDate::parse_xmp(text) {
                Err(DateError::Invalid(t)) => assert_eq!(t, text),
                other => panic!("{text:?} should be invalid, got {other:?}"),
            }
        }
        assert_eq!(IptcDate::parse_xmp(""), Err(DateError::Empty));
        assert_eq!(IptcDate::parse_xmp("   "), Err(DateError::Empty));
        assert_eq!(xmp("2024-02-29"), date(2024, Some(2), Some(29), None));
        assert_eq!(xmp("2000-02-29"), date(2000, Some(2), Some(29), None));
    }

    #[test]
    fn iim_form_of_truncated_and_full_dates() {
        assert_eq!(xmp("1918").to_iim(), ("19180000".to_string(), None));
        assert_eq!(xmp("2026-11").to_iim(), ("20261100".to_string(), None));
        assert_eq!(xmp("2026-11-04").to_iim(), ("20261104".to_string(), None));
        assert_eq!(
            xmp("2026-11-04T10:15:30.5+01:00").to_iim(),
            ("20261104".to_string(), Some("101530+0100".to_string()))
        );
        assert_eq!(
            xmp("2026-11-04T10:15").to_iim(),
            ("20261104".to_string(), None)
        );
        assert_eq!(
            xmp("2026-11-04T10:15Z").to_iim(),
            ("20261104".to_string(), Some("101500+0000".to_string()))
        );
        assert_eq!(
            xmp("2026-11-04T10:15:00.123-05:30").to_iim(),
            ("20261104".to_string(), Some("101500-0530".to_string()))
        );
        assert_eq!(
            xmp("2026-11-04T10:15:30+00:00").to_iim(),
            ("20261104".to_string(), Some("101530+0000".to_string()))
        );
    }

    #[test]
    fn iim_values_are_read() {
        assert_eq!(
            IptcDate::from_iim("19180000", None),
            Ok(date(1918, None, None, None))
        );
        assert_eq!(
            IptcDate::from_iim("20261100", None),
            Ok(date(2026, Some(11), None, None))
        );
        assert_eq!(
            IptcDate::from_iim("20261104", None),
            Ok(date(2026, Some(11), Some(4), None))
        );
        assert_eq!(
            IptcDate::from_iim("20261104", Some("101530+0100")),
            Ok(date(
                2026,
                Some(11),
                Some(4),
                Some(time(10, 15, Some(30), None, Some(Zone::Offset(60))))
            ))
        );
        assert_eq!(
            IptcDate::from_iim("20261104", Some("101530-0530")),
            Ok(date(
                2026,
                Some(11),
                Some(4),
                Some(time(10, 15, Some(30), None, Some(Zone::Offset(-330))))
            ))
        );
        assert_eq!(
            IptcDate::from_iim("20261104", Some("101530")),
            Ok(date(
                2026,
                Some(11),
                Some(4),
                Some(time(10, 15, Some(30), None, None))
            ))
        );
    }

    #[test]
    fn invalid_iim_values_are_errors() {
        for (d, t) in [
            ("20260004", None),
            ("20261100", Some("101530")),
            ("20261100", Some("101530+0100")),
            ("19180000", Some("101530")),
            ("20261304", None),
            ("20260230", None),
            ("2026110", None),
            ("202611040", None),
            ("2026-11-04", None),
            ("20261104", Some("1015")),
            ("20261104", Some("246000")),
            ("20261104", Some("101560")),
            ("20261104", Some("101530+01:00")),
            ("20261104", Some("101530+2500")),
            ("20261104", Some("101530-0000")),
            ("20261104", Some("")),
        ] {
            assert!(
                matches!(IptcDate::from_iim(d, t), Err(DateError::Invalid(_))),
                "{d:?} {t:?} should be invalid"
            );
        }
        assert_eq!(IptcDate::from_iim("", None), Err(DateError::Empty));
    }

    #[test]
    fn exif_values_are_read() {
        assert_eq!(
            IptcDate::from_exif("2026:11:04 10:15:30", Some("123"), Some("+01:00"))
                .map(|d| d.to_xmp()),
            Ok("2026-11-04T10:15:30.123+01:00".to_string())
        );
        let zone_less = IptcDate::from_exif("2026:11:04 10:15:30", None, None);
        assert_eq!(
            zone_less,
            Ok(date(
                2026,
                Some(11),
                Some(4),
                Some(time(10, 15, Some(30), None, None))
            ))
        );
        assert_eq!(
            zone_less.map(|d| d.to_xmp()),
            Ok("2026-11-04T10:15:30".to_string())
        );
        // Exif pads unknown sub-seconds and offsets with blanks.
        assert_eq!(
            IptcDate::from_exif("2026:11:04 10:15:30", Some("   "), Some("   :  "))
                .map(|d| d.to_xmp()),
            Ok("2026-11-04T10:15:30".to_string())
        );
        assert_eq!(
            IptcDate::from_exif("2026:11:04 10:15:30", Some("05  "), Some("-05:30"))
                .map(|d| d.to_xmp()),
            Ok("2026-11-04T10:15:30.05-05:30".to_string())
        );
    }

    #[test]
    fn empty_and_unknown_exif_values_are_empty() {
        for text in [
            "",
            " ",
            "                   ",
            "0000:00:00 00:00:00",
            "    :  :     :  :  ",
        ] {
            assert_eq!(
                IptcDate::from_exif(text, Some("123"), Some("+01:00")),
                Err(DateError::Empty),
                "{text:?}"
            );
        }
    }

    #[test]
    fn invalid_exif_values_are_errors() {
        for (dt, sub, off) in [
            ("2026-11-04 10:15:30", None, None),
            ("2026:11:04T10:15:30", None, None),
            ("2026:11:04 10:15", None, None),
            ("2026:13:04 10:15:30", None, None),
            ("2026:00:00 00:00:00", None, None),
            ("2026:11:04 24:00:00", None, None),
            ("2026:11:04 10:15:30", Some("12a"), None),
            ("2026:11:04 10:15:30", None, Some("+0100")),
            ("2026:11:04 10:15:30", None, Some("+24:00")),
            ("2026:11:04 10:15:30", None, Some("Z")),
        ] {
            assert!(
                matches!(
                    IptcDate::from_exif(dt, sub, off),
                    Err(DateError::Invalid(_))
                ),
                "{dt:?} {sub:?} {off:?} should be invalid"
            );
        }
    }

    #[test]
    fn a_zone_is_never_added() {
        for text in [
            "2026-11-04T10:15",
            "2026-11-04T10:15:00",
            "2026-11-04T10:15:00.5",
        ] {
            let value = xmp(text);
            assert_eq!(value.time.as_ref().and_then(|t| t.zone), None);
            assert_eq!(value.to_xmp(), text);
            let (iim_date, iim_time) = value.to_iim();
            assert_eq!(iim_time, None, "no 2:60 without a zone for {text:?}");
            let back = IptcDate::from_iim(&iim_date, iim_time.as_deref());
            assert_eq!(back, Ok(date(2026, Some(11), Some(4), None)));
            assert_eq!(back.map(|d| d.to_xmp()), Ok("2026-11-04".to_string()));
        }
        // A zone-less IIM time stays zone-less in XMP.
        assert_eq!(
            IptcDate::from_iim("20261104", Some("101530")).map(|d| d.to_xmp()),
            Ok("2026-11-04T10:15:30".to_string())
        );
        // A known zone survives the IIM round trip.
        let (d, t) = xmp("2026-11-04T10:15:30+01:00").to_iim();
        assert_eq!(
            IptcDate::from_iim(&d, t.as_deref()).map(|d| d.to_xmp()),
            Ok("2026-11-04T10:15:30+01:00".to_string())
        );
    }

    #[test]
    fn writers_do_not_panic_on_values_built_by_hand() {
        // A day without a month and a time without a day are skipped, an offset at the end of the
        // `i16` range does not overflow.
        let odd = date(
            2026,
            None,
            Some(4),
            Some(time(10, 15, None, None, Some(Zone::Offset(i16::MIN)))),
        );
        assert_eq!(odd.to_xmp(), "2026");
        assert_eq!(odd.to_iim(), ("20260000".to_string(), None));
        let far = date(
            2026,
            Some(11),
            Some(4),
            Some(time(10, 15, None, None, Some(Zone::Offset(i16::MIN)))),
        );
        assert_eq!(far.to_xmp(), "2026-11-04T10:15-546:08");
    }

    #[test]
    fn error_texts() {
        assert_eq!(DateError::Empty.to_string(), "the date is empty");
        assert_eq!(
            DateError::Invalid("2026-13".to_string()).to_string(),
            "not a valid date or time: \"2026-13\""
        );
        let boxed: Box<dyn std::error::Error> = Box::new(DateError::Empty);
        assert_eq!(boxed.to_string(), "the date is empty");
    }
}
