//! Folder and file-name templates, e.g. `{year}/{date} {job}` and `{date}_{seq:4}`.
//!
//! Variables (case-insensitive):
//!
//! | Variable | Example | Meaning |
//! |---|---|---|
//! | `{year}` `{year2}` | 2026, 26 | capture year |
//! | `{month}` `{day}` | 11, 04 | two digits |
//! | `{hour}` `{minute}` `{second}` | 06, 28, 51 | two digits |
//! | `{date}` | 2026-11-04 | `{year}-{month}-{day}` |
//! | `{time}` | 062851 | `{hour}{minute}{second}` |
//! | `{make}` `{model}` | SONY, ILCE-1 | camera (empty if unknown) |
//! | `{name}` | TN005956 | original file name without extension |
//! | `{seq}` `{seq:N}` | 7, 0007 | running number in this ingest, zero-padded to N digits |
//! | `{job}` | Bröllop Andersson | free text given for the ingest |
//!
//! `/` in a folder template creates sub-folders. Characters that Windows does not allow in names
//! are replaced by `_` in variable values. `{{` and `}}` write literal braces.

use std::fmt;

/// Capture date and time, as recorded by the camera (local camera time, no time zone).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CaptureTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl CaptureTime {
    /// Parses EXIF `YYYY:MM:DD HH:MM:SS` (also accepts `-` as date separator).
    pub fn parse_exif(s: &str) -> Option<CaptureTime> {
        let s = s.trim();
        let b = s.as_bytes();
        if b.len() < 19 {
            return None;
        }
        let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<u16>().ok();
        let t = CaptureTime {
            year: num(0..4)?,
            month: num(5..7)? as u8,
            day: num(8..10)? as u8,
            hour: num(11..13)? as u8,
            minute: num(14..16)? as u8,
            second: num(17..19)? as u8,
        };
        let valid = t.year >= 1900
            && (1..=12).contains(&t.month)
            && (1..=31).contains(&t.day)
            && t.hour < 24
            && t.minute < 60
            && t.second < 61;
        valid.then_some(t)
    }

    /// From seconds since the Unix epoch (UTC; used when a file has no EXIF date).
    pub fn from_unix(secs: i64) -> CaptureTime {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        // Civil-from-days (Howard Hinnant).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        CaptureTime {
            year: year.clamp(0, 9999) as u16,
            month: month as u8,
            day: day as u8,
            hour: (rem / 3600) as u8,
            minute: (rem / 60 % 60) as u8,
            second: (rem % 60) as u8,
        }
    }
}

/// Values available to a template for one image.
#[derive(Debug, Clone, Default)]
pub struct Vars<'a> {
    pub time: Option<CaptureTime>,
    pub make: Option<&'a str>,
    pub model: Option<&'a str>,
    pub name: &'a str,
    pub seq: u32,
    pub job: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Year,
    Year2,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Date,
    Time,
    Make,
    Model,
    Name,
    Seq(usize),
    Job,
}

/// A parsed template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    parts: Vec<Part>,
}

/// A template that could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateError(pub String);

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TemplateError {}

impl Template {
    pub fn parse(src: &str) -> Result<Template, TemplateError> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut chars = src.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    text.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    text.push('}');
                }
                '}' => return Err(TemplateError("unexpected '}'".into())),
                '{' => {
                    let mut name = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some(c) => name.push(c),
                            None => return Err(TemplateError(format!("'{{{name}' is not closed"))),
                        }
                    }
                    if !text.is_empty() {
                        parts.push(Part::Text(std::mem::take(&mut text)));
                    }
                    parts.push(Self::variable(&name)?);
                }
                c => text.push(c),
            }
        }
        if !text.is_empty() {
            parts.push(Part::Text(text));
        }
        Ok(Template { parts })
    }

    fn variable(name: &str) -> Result<Part, TemplateError> {
        let lower = name.trim().to_ascii_lowercase();
        let (base, arg) = match lower.split_once(':') {
            Some((b, a)) => (b, Some(a)),
            None => (lower.as_str(), None),
        };
        let part = match (base, arg) {
            ("year", None) => Part::Year,
            ("year2", None) => Part::Year2,
            ("month", None) => Part::Month,
            ("day", None) => Part::Day,
            ("hour", None) => Part::Hour,
            ("minute", None) => Part::Minute,
            ("second", None) => Part::Second,
            ("date", None) => Part::Date,
            ("time", None) => Part::Time,
            ("make", None) => Part::Make,
            ("model", None) => Part::Model,
            ("name", None) => Part::Name,
            ("job", None) => Part::Job,
            ("seq", None) => Part::Seq(0),
            ("seq", Some(n)) => match n.parse::<usize>() {
                Ok(w) if w <= 9 => Part::Seq(w),
                _ => return Err(TemplateError(format!("{{seq:{n}}}: width must be 0–9"))),
            },
            _ => return Err(TemplateError(format!("unknown variable {{{name}}}"))),
        };
        Ok(part)
    }

    /// True if the template uses the running number (then names are unique by construction).
    pub fn uses_seq(&self) -> bool {
        self.parts.iter().any(|p| matches!(p, Part::Seq(_)))
    }

    /// Renders the template. Variable values are made safe for file names; literal text is kept
    /// (so `/` in the template still separates folders).
    pub fn render(&self, v: &Vars<'_>) -> String {
        let t = v.time.unwrap_or(CaptureTime {
            year: 0,
            month: 0,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
        });
        let mut out = String::new();
        for p in &self.parts {
            let value = match p {
                Part::Text(s) => {
                    out.push_str(s);
                    continue;
                }
                Part::Year => format!("{:04}", t.year),
                Part::Year2 => format!("{:02}", t.year % 100),
                Part::Month => format!("{:02}", t.month),
                Part::Day => format!("{:02}", t.day),
                Part::Hour => format!("{:02}", t.hour),
                Part::Minute => format!("{:02}", t.minute),
                Part::Second => format!("{:02}", t.second),
                Part::Date => format!("{:04}-{:02}-{:02}", t.year, t.month, t.day),
                Part::Time => format!("{:02}{:02}{:02}", t.hour, t.minute, t.second),
                Part::Make => v.make.unwrap_or("").to_string(),
                Part::Model => v.model.unwrap_or("").to_string(),
                Part::Name => v.name.to_string(),
                Part::Seq(w) => format!("{:0w$}", v.seq, w = *w),
                Part::Job => v.job.to_string(),
            };
            out.push_str(&sanitize(&value));
        }
        out
    }
}

/// Replaces characters Windows does not allow in file names, and control characters.
pub fn sanitize(s: &str) -> String {
    s.trim()
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

/// Cleans one rendered path segment: trims spaces and trailing dots (Windows drops them),
/// and avoids reserved device names such as `CON` or `COM1`.
pub fn clean_segment(seg: &str) -> String {
    let s = seg.trim().trim_end_matches(['.', ' ']).to_string();
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if reserved { format!("_{s}") } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vars<'static> {
        Vars {
            time: CaptureTime::parse_exif("2026:10:26 06:28:51"),
            make: Some("SONY"),
            model: Some("ILCE-7RM4A"),
            name: "TN005956",
            seq: 7,
            job: "Bröllop: Andersson",
        }
    }

    #[test]
    fn renders_all_variables() {
        let t = Template::parse(
            "{year}/{date} {job}/{YEAR2}{month}{day}_{time}_{model}_{make}_{name}_{seq:4}_{seq}",
        )
        .unwrap();
        assert_eq!(
            t.render(&vars()),
            "2026/2026-10-26 Bröllop_ Andersson/261026_062851_ILCE-7RM4A_SONY_TN005956_0007_7"
        );
        assert!(t.uses_seq());
        assert_eq!(Template::parse("{{x}}").unwrap().render(&vars()), "{x}");
    }

    #[test]
    fn rejects_bad_templates() {
        assert!(Template::parse("{nope}").is_err());
        assert!(Template::parse("{year").is_err());
        assert!(Template::parse("a}b").is_err());
        assert!(Template::parse("{seq:12}").is_err());
        assert!(!Template::parse("{name}").unwrap().uses_seq());
    }

    #[test]
    fn exif_dates() {
        assert_eq!(CaptureTime::parse_exif("2026:13:01 00:00:00"), None);
        assert_eq!(CaptureTime::parse_exif("0000:00:00 00:00:00"), None);
        assert_eq!(CaptureTime::parse_exif("    "), None);
        assert!(CaptureTime::parse_exif("2026-11-04 10:00:00").is_some());
        let t = CaptureTime::from_unix(1_793_766_956); // 2026-11-04 04:35:56 UTC
        assert_eq!((t.year, t.month, t.day), (2026, 11, 4));
        assert_eq!((t.hour, t.minute, t.second), (4, 35, 56));
        let t = CaptureTime::from_unix(951_782_400); // 2000-02-29
        assert_eq!((t.year, t.month, t.day), (2000, 2, 29));
    }

    #[test]
    fn segments_are_windows_safe() {
        assert_eq!(sanitize("a/b:c*"), "a_b_c_");
        assert_eq!(clean_segment("Job. "), "Job");
        assert_eq!(clean_segment("CON"), "_CON");
        assert_eq!(clean_segment("com1.txt"), "_com1.txt");
        assert_eq!(clean_segment("COMPANY"), "COMPANY");
    }
}
