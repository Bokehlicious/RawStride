//! The texts of ratings and labels (T116, rule 13): the names of the labels, the status-line text
//! of a rating, the stars under a thumbnail, the messages of a rating that cannot be saved, and the
//! texts of the filter's drop-down lists. Moved here from `ratings.rs` and `filter.rs` (T116),
//! translated to English (T117).

use pw_metadata::Rating;

use crate::filter::{LabelChoice, LabelFilter, StarCondition, StarFilter};

/// The status-line message when a rating key is pressed on a file that cannot carry ratings.
pub const UNSUPPORTED_TEXT: &str = "Ratings and labels cannot be saved for this file type.";

/// The message when the path of an image to rate cannot be made absolute.
pub fn path_resolution_text(error: &str) -> String {
    format!("Could not look up the image's path: {error}")
}

/// The message for one failed write, as returned by `pump`, `written` and `fail_sent`.
pub(crate) fn failure_message(name: &str, error: &str) -> String {
    format!("Could not save the rating for {name}: {error}")
}

/// The status-line text for more than one failed write since the last key (ADR 0014 Q6): the
/// count, then the last failure's file name and error.
pub fn failures_text(count: usize, name: &str, error: &str) -> String {
    if count == 1 {
        format!("Could not save the rating for 1 image. Last: {name}: {error}")
    } else {
        format!("Could not save the rating for {count} images. Last: {name}: {error}")
    }
}

/// The reason of a failed rating write when the metadata worker has stopped.
pub fn worker_stopped_text() -> &'static str {
    "metadata writing has stopped"
}

/// The shown name of an Adobe label name: the stored English name (`Red`, `Yellow`, `Green`,
/// `Blue`, `Purple`) as it is; an unknown label is returned as it is too.
pub fn label_name(label: &str) -> &str {
    label
}

/// A number of stars: "1 star" or "{n} stars" (glossary §2: singular and plural in one function).
/// Used by the status line and the metadata window's buttons (T124).
pub fn star_count_text(stars: u8) -> String {
    if stars == 1 {
        "1 star".to_owned()
    } else {
        format!("{stars} stars")
    }
}

/// The status-line text of a rating, for example "Rating: 3 stars, Red".
pub fn status_text(rating: Option<&Rating>) -> String {
    let stars = rating.and_then(|rating| rating.stars);
    let mut text = match stars {
        Some(-1) => "Rating: Rejected".to_owned(),
        Some(stars @ 1..) => format!("Rating: {}", star_count_text(stars.unsigned_abs())),
        _ => "Rating: Unrated".to_owned(),
    };
    if let Some(label) = rating.and_then(|rating| rating.label.as_deref()) {
        text.push_str(", ");
        text.push_str(label_name(label));
    }
    text
}

/// The stars drawn under a thumbnail: "★★★••" for three stars, "✕" for rejected, nothing when
/// unrated.
pub fn star_glyphs(stars: Option<i8>) -> String {
    match stars {
        Some(-1) => "✕".to_owned(),
        Some(stars @ 1..=5) => {
            let filled = usize::from(stars.unsigned_abs());
            format!("{}{}", "★".repeat(filled), "•".repeat(5 - filled))
        }
        _ => String::new(),
    }
}

/// The text of a star choice. `Stars` outside 1–5 gives the text of the nearest valid value.
pub fn star_filter_text(filter: StarFilter) -> &'static str {
    match filter {
        StarFilter::All => "All ratings",
        StarFilter::Stars(0..=1) => "1 star",
        StarFilter::Stars(2) => "2 stars",
        StarFilter::Stars(3) => "3 stars",
        StarFilter::Stars(4) => "4 stars",
        StarFilter::Stars(_) => "5 stars",
        StarFilter::Unrated => "Unrated",
        StarFilter::Rejected => "Rejected",
        StarFilter::NotRejected => "All but rejected",
    }
}

/// The text of a star condition.
pub fn star_condition_text(condition: StarCondition) -> &'static str {
    match condition {
        StarCondition::AtLeast => "≥",
        StarCondition::Exactly => "=",
        StarCondition::AtMost => "≤",
    }
}

/// The text of a label choice: the label name of `label_name`, or "No label".
pub fn label_choice_text(choice: LabelChoice) -> &'static str {
    match choice {
        LabelChoice::Label(label) => label_name(label),
        LabelChoice::NoLabel => "No label",
    }
}

/// The text of the closed label list (ADR 0014 Q10): "All labels" for the empty set, the one
/// name, two names joined with ", ", and "{n} labels" from three.
pub fn label_filter_text(filter: &LabelFilter) -> String {
    let names: Vec<&str> = filter.choices().map(label_choice_text).collect();
    match names.len() {
        0 => "All labels".to_owned(),
        1 | 2 => names.join(", "),
        n => format!("{n} labels"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::STAR_FILTERS;

    fn rating(stars: Option<i8>, label: Option<&str>) -> Rating {
        Rating {
            stars,
            label: label.map(str::to_owned),
        }
    }

    #[test]
    fn star_filter_text_outside_one_to_five_uses_the_nearest_valid_value() {
        assert_eq!(star_filter_text(StarFilter::Stars(0)), "1 star");
        assert_eq!(star_filter_text(StarFilter::Stars(6)), "5 stars");
        assert_eq!(star_filter_text(StarFilter::Stars(u8::MAX)), "5 stars");
    }

    #[test]
    fn english_label_names() {
        for (label, name) in [
            ("Red", "Red"),
            ("Yellow", "Yellow"),
            ("Green", "Green"),
            ("Blue", "Blue"),
            ("Purple", "Purple"),
            ("Orange", "Orange"),
            ("", ""),
        ] {
            assert_eq!(label_name(label), name);
        }
        for label in pw_metadata::LABELS {
            assert_eq!(label_name(label), *label, "{label} is shown as stored");
        }
    }

    #[test]
    fn status_texts() {
        for (value, text) in [
            (None, "Rating: Unrated"),
            (Some(rating(None, None)), "Rating: Unrated"),
            (Some(rating(Some(-1), None)), "Rating: Rejected"),
            (Some(rating(Some(1), None)), "Rating: 1 star"),
            (Some(rating(Some(3), Some("Red"))), "Rating: 3 stars, Red"),
            (Some(rating(Some(5), None)), "Rating: 5 stars"),
            (
                Some(rating(Some(-1), Some("Yellow"))),
                "Rating: Rejected, Yellow",
            ),
            (Some(rating(None, Some("Blue"))), "Rating: Unrated, Blue"),
            (
                Some(rating(None, Some("Orange"))),
                "Rating: Unrated, Orange",
            ),
        ] {
            assert_eq!(status_text(value.as_ref()), text);
        }
    }

    #[test]
    fn star_glyphs_for_each_value() {
        assert_eq!(star_glyphs(None), "");
        assert_eq!(star_glyphs(Some(-1)), "✕");
        assert_eq!(star_glyphs(Some(1)), "★••••");
        assert_eq!(star_glyphs(Some(3)), "★★★••");
        assert_eq!(star_glyphs(Some(5)), "★★★★★");
        // Values that cannot come from the keys or `read_ratings` draw nothing instead of panicking.
        assert_eq!(star_glyphs(Some(0)), "");
        assert_eq!(star_glyphs(Some(6)), "");
        assert_eq!(star_glyphs(Some(-2)), "");
    }

    #[test]
    fn unsupported_text_is_english() {
        assert_eq!(
            UNSUPPORTED_TEXT,
            "Ratings and labels cannot be saved for this file type."
        );
    }

    /// One example of each moved text, as it was before T116.
    #[test]
    fn every_moved_text_keeps_its_wording() {
        assert_eq!(
            path_resolution_text("nekad"),
            "Could not look up the image's path: nekad"
        );
        assert_eq!(
            failure_message("A.ARW", "fel"),
            "Could not save the rating for A.ARW: fel"
        );
        assert_eq!(
            failures_text(3, "C.ARW", "fel C"),
            "Could not save the rating for 3 images. Last: C.ARW: fel C"
        );
        assert_eq!(
            failures_text(1, "C.ARW", "fel C"),
            "Could not save the rating for 1 image. Last: C.ARW: fel C"
        );
        assert_eq!(worker_stopped_text(), "metadata writing has stopped");
        let texts: Vec<&str> = STAR_FILTERS.iter().map(|f| star_filter_text(*f)).collect();
        assert_eq!(texts[0], "All ratings");
        assert_eq!(star_condition_text(StarCondition::AtLeast), "≥");
        assert_eq!(label_choice_text(LabelChoice::NoLabel), "No label");
        assert_eq!(label_filter_text(&LabelFilter::all()), "All labels");
    }
}
