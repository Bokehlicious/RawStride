# T77. ADR 0007 I2: dates and times — truncated dates, no invented time zone — Done

- **Can start now.** Independent of T76 and T79.
- **Crates:** `pw-metadata` only. Nothing uses the module yet.
- **Files:** new `crates/pw-metadata/src/date.rs`; `crates/pw-metadata/src/lib.rs` (the `mod`
  line only); `docs/test-inventory.md`.
- **Why:** ADR 0007 §7 (truncated dates, a zone is never invented, DateTimeOriginal is only read),
  §5.3 (IIM 2:55 and 2:60, Q3), §6.1 (Date Created from Exif).
- **What:** `pub mod date` with:
  1. **Types** (`Clone`, `Debug`, `PartialEq`, `Eq`):
     - `IptcDate { year: u16, month: Option<u8>, day: Option<u8>, time: Option<Time> }`; a day only
       with a month, a time only with a day;
     - `Time { hour: u8, minute: u8, second: Option<u8>, fraction: Option<String>, zone: Option<Zone> }`;
       `fraction` holds the digits after the decimal point exactly as read (no rounding);
     - `Zone { Utc, Offset(i16) }` (minutes east of UTC); `Utc` is a `Z` in the text and is written
       back as `Z`.
  2. **XMP form** (IPTC §14.2): `IptcDate::parse_xmp(&str) -> Result<IptcDate, DateError>` accepts
     `YYYY`, `YYYY-MM`, `YYYY-MM-DD`, `YYYY-MM-DDThh:mm`, `YYYY-MM-DDThh:mm:ss`,
     `YYYY-MM-DDThh:mm:ss.s…`, each time form with or without `Z` or `±hh:mm`. Ranges are checked
     (month 1–12; the day exists in that month, leap years included; hour 0–23; minute and second
     0–59; offset hours 0–23 and minutes 0–59). Anything else is an error, never a panic.
     `to_xmp(&self) -> String` writes the same form back: a valid input gives exactly the same text.
  3. **IIM form** (§5.3, Q3):
     - `to_iim(&self) -> (String, Option<String>)`: 2:55 as `YYYYMMDD` with `00` for an unknown
       month or day (`19180000`); 2:60 as `hhmmss±hhmm` **only when there is a time and its zone is
       known** (`Utc` → `+0000`), otherwise `None`; the fraction is dropped; missing seconds are
       written as `00`;
     - `IptcDate::from_iim(date: &str, time: Option<&str>) -> Result<IptcDate, DateError>`: `00`
       gives `None` for month or day (a day without a month is an error); a time `hhmmss±hhmm` gives
       a zone, a time `hhmmss` without an offset is accepted **without a zone**; a time with a date
       that has no day is an error.
  4. **Exif** (§6.1, §7): `IptcDate::from_exif(date_time: &str, subsec: Option<&str>,
     offset: Option<&str>) -> Result<IptcDate, DateError>` for DateTimeOriginal
     `YYYY:MM:DD hh:mm:ss`, SubSecTimeOriginal (digits → `fraction`) and OffsetTimeOriginal
     (`±hh:mm` → zone). **Without an offset the result has no zone.** An empty value, only spaces,
     or the "unknown" forms with zeros or blanks (`0000:00:00 00:00:00`,
     `    :  :     :  :  `) give `DateError::Empty`. No function in this module writes Exif.
  5. **`DateError`** (`Empty`, `Invalid(String)` with the text) with `Display` (English) and
     `std::error::Error`.
  6. No time-zone conversion anywhere, no use of the computer's time zone, no new dependency.
- **Rules:** rules 3–7 and 10–12. No `Cargo.toml` change.
- **Tests to add** (`#[cfg(test)] mod tests` in `date.rs`):
  - each XMP form of point 2 parses and round-trips exactly, including `2026-11-04T10:15:00+01:00`,
    `2026-11-04T10:15:00Z`, `2026-11-04T10:15:00.123-05:30` and the zone-less
    `2026-11-04T10:15:00`;
  - invalid: `2026-13`, `2026-02-30`, `2023-02-29`, `2026-11-04T24:00`, `2026-11-04T10:60`,
    `2026-11-04T10:15+25:00`, `2026-11-04 10:15`, `26-11-04`, `""`; `2024-02-29` is valid;
  - IIM: `1918` → `("19180000", None)`; `2026-11` → `("20261100", None)`;
    `2026-11-04T10:15:30.5+01:00` → `("20261104", Some("101530+0100"))`;
    `2026-11-04T10:15` (no zone) → `("20261104", None)`; `…T10:15Z` → `Some("101500+0000")`;
  - `from_iim`: `19180000` → year only; `20261104` + `101530+0100` → time with zone;
    `20261104` + `101530` → time without zone; `20260004` and `20261100` + a time are errors;
  - Exif: `2026:11:04 10:15:30` + `123` + `+01:00` → `2026-11-04T10:15:30.123+01:00`; without
    offset → `2026-11-04T10:15:30` (no zone); the empty and unknown forms → `Empty`;
  - a zone is never added: a zone-less value stays zone-less through `to_xmp`, `to_iim` and back
    through `from_iim`.
- **Done when:** `.\tools\check.ps1` is all OK; `docs/test-inventory.md` is updated;
  `git diff --stat` shows only the files above.
