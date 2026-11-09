//! Star ratings and colour labels in RawStride (ADR 0013, step B1): the key map, how a key changes
//! a rating, and the Swedish rating texts. Plain Rust without I/O (GR11); the store and the writes
//! come in step B2, the drawing in B3.

use pw_metadata::Rating;

/// The status-line message when a rating key is pressed on a file that cannot carry ratings.
pub const UNSUPPORTED_TEXT: &str = "Betyg och etiketter kan inte sparas för den här filtypen.";

/// A rating or label change requested by a keyboard shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RatingKey {
    Stars(u8),
    Label(&'static str),
    ClearLabel,
    Reject,
}

/// Maps rating shortcuts without changing the existing loupe or grid key maps.
pub fn rating_key_action(key: egui::Key, ctrl: bool, shift: bool, alt: bool) -> Option<RatingKey> {
    use egui::Key as K;

    if shift || (ctrl && alt) {
        return None;
    }

    if ctrl {
        return match key {
            K::Num0 => Some(RatingKey::ClearLabel),
            K::Num1 => Some(RatingKey::Label("Red")),
            K::Num2 => Some(RatingKey::Label("Yellow")),
            K::Num3 => Some(RatingKey::Label("Green")),
            K::Num4 => Some(RatingKey::Label("Blue")),
            K::Num5 => Some(RatingKey::Label("Purple")),
            _ => None,
        };
    }

    if alt {
        return match key {
            K::Num0 => Some(RatingKey::Stars(0)),
            K::Num1 => Some(RatingKey::Stars(1)),
            K::Num2 => Some(RatingKey::Stars(2)),
            K::Num3 => Some(RatingKey::Stars(3)),
            K::Num4 => Some(RatingKey::Stars(4)),
            K::Num5 => Some(RatingKey::Stars(5)),
            _ => None,
        };
    }

    match key {
        K::Num0 => Some(RatingKey::Stars(0)),
        K::Num1 => Some(RatingKey::Stars(1)),
        K::Num2 => Some(RatingKey::Stars(2)),
        K::Num3 => Some(RatingKey::Stars(3)),
        K::Num4 => Some(RatingKey::Stars(4)),
        K::Num5 => Some(RatingKey::Stars(5)),
        K::Num6 => Some(RatingKey::Label("Red")),
        K::Num7 => Some(RatingKey::Label("Yellow")),
        K::Num8 => Some(RatingKey::Label("Green")),
        K::Num9 => Some(RatingKey::Label("Blue")),
        K::X => Some(RatingKey::Reject),
        _ => None,
    }
}

/// Applies a rating shortcut while preserving the other half of the rating.
pub fn apply(current: &Rating, key: RatingKey) -> Rating {
    let mut next = current.clone();
    match key {
        RatingKey::Stars(0) => next.stars = None,
        RatingKey::Stars(stars @ 1..=5) => next.stars = Some(stars as i8),
        RatingKey::Stars(_) => {}
        RatingKey::Label(label) => {
            if current.label.as_deref() == Some(label) {
                next.label = None;
            } else {
                next.label = Some(label.to_owned());
            }
        }
        RatingKey::ClearLabel => next.label = None,
        RatingKey::Reject => {
            next.stars = if current.stars == Some(-1) {
                None
            } else {
                Some(-1)
            };
        }
    }
    next
}

/// The Swedish name of an Adobe label name; an unknown label is returned as it is.
pub fn label_name(label: &str) -> &str {
    match label {
        "Red" => "Röd",
        "Yellow" => "Gul",
        "Green" => "Grön",
        "Blue" => "Blå",
        "Purple" => "Lila",
        other => other,
    }
}

/// The status-line text of a rating, for example "Betyg: 3 stjärnor, Röd".
pub fn status_text(rating: Option<&Rating>) -> String {
    let stars = rating.and_then(|rating| rating.stars);
    let mut text = match stars {
        Some(-1) => "Betyg: Avvisad".to_owned(),
        Some(1) => "Betyg: 1 stjärna".to_owned(),
        Some(stars) if stars > 1 => format!("Betyg: {stars} stjärnor"),
        _ => "Betyg: Ej betygsatt".to_owned(),
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

#[cfg(test)]
mod tests {
    use super::{
        RatingKey, UNSUPPORTED_TEXT, apply, label_name, rating_key_action, star_glyphs, status_text,
    };
    use egui::Key as K;
    use pw_metadata::Rating;

    const DIGITS: [K; 10] = [
        K::Num0,
        K::Num1,
        K::Num2,
        K::Num3,
        K::Num4,
        K::Num5,
        K::Num6,
        K::Num7,
        K::Num8,
        K::Num9,
    ];

    fn rating(stars: Option<i8>, label: Option<&str>) -> Rating {
        Rating {
            stars,
            label: label.map(str::to_owned),
        }
    }

    #[test]
    fn rating_shortcuts_match_the_accepted_key_map() {
        for (key, expected) in [
            (K::Num0, RatingKey::Stars(0)),
            (K::Num1, RatingKey::Stars(1)),
            (K::Num2, RatingKey::Stars(2)),
            (K::Num3, RatingKey::Stars(3)),
            (K::Num4, RatingKey::Stars(4)),
            (K::Num5, RatingKey::Stars(5)),
            (K::Num6, RatingKey::Label("Red")),
            (K::Num7, RatingKey::Label("Yellow")),
            (K::Num8, RatingKey::Label("Green")),
            (K::Num9, RatingKey::Label("Blue")),
            (K::X, RatingKey::Reject),
        ] {
            assert_eq!(rating_key_action(key, false, false, false), Some(expected));
        }

        for (key, expected) in [
            (K::Num0, RatingKey::Stars(0)),
            (K::Num1, RatingKey::Stars(1)),
            (K::Num2, RatingKey::Stars(2)),
            (K::Num3, RatingKey::Stars(3)),
            (K::Num4, RatingKey::Stars(4)),
            (K::Num5, RatingKey::Stars(5)),
        ] {
            assert_eq!(rating_key_action(key, false, false, true), Some(expected));
        }

        for (key, expected) in [
            (K::Num0, RatingKey::ClearLabel),
            (K::Num1, RatingKey::Label("Red")),
            (K::Num2, RatingKey::Label("Yellow")),
            (K::Num3, RatingKey::Label("Green")),
            (K::Num4, RatingKey::Label("Blue")),
            (K::Num5, RatingKey::Label("Purple")),
        ] {
            assert_eq!(rating_key_action(key, true, false, false), Some(expected));
        }
    }

    #[test]
    fn shift_and_ctrl_alt_rating_keys_are_ignored_and_existing_maps_are_unchanged() {
        for key in [
            K::Num0,
            K::Num1,
            K::Num2,
            K::Num3,
            K::Num4,
            K::Num5,
            K::Num6,
            K::Num7,
            K::Num8,
            K::Num9,
            K::X,
        ] {
            assert_eq!(rating_key_action(key, false, true, false), None, "{key:?}");
            assert_eq!(rating_key_action(key, true, false, true), None, "{key:?}");
        }

        assert_eq!(crate::key_action(K::Num1, false, false), None);
        assert_eq!(crate::grid_key_action(K::Num1, false, false), None);
    }

    #[test]
    fn applying_rating_keys_preserves_and_toggles_the_other_value() {
        let current = Rating {
            stars: Some(3),
            label: Some("Red".to_owned()),
        };
        assert_eq!(
            apply(&current, RatingKey::Stars(0)),
            Rating {
                stars: None,
                label: Some("Red".to_owned())
            }
        );
        assert_eq!(
            apply(&current, RatingKey::Stars(5)),
            Rating {
                stars: Some(5),
                label: Some("Red".to_owned())
            }
        );
        assert_eq!(
            apply(&current, RatingKey::Label("Red")),
            Rating {
                stars: Some(3),
                label: None
            }
        );
        assert_eq!(
            apply(&current, RatingKey::Label("Blue")),
            Rating {
                stars: Some(3),
                label: Some("Blue".to_owned())
            }
        );
        assert_eq!(
            apply(&current, RatingKey::ClearLabel),
            Rating {
                stars: Some(3),
                label: None
            }
        );
        assert_eq!(
            apply(&current, RatingKey::Reject),
            Rating {
                stars: Some(-1),
                label: Some("Red".to_owned())
            }
        );
        assert_eq!(
            apply(
                &Rating {
                    stars: Some(-1),
                    label: None
                },
                RatingKey::Reject
            ),
            Rating::default()
        );
    }

    #[test]
    fn modifiers_limit_the_rating_keys() {
        for key in [K::Num6, K::Num7, K::Num8, K::Num9, K::X] {
            assert_eq!(
                rating_key_action(key, false, false, true),
                None,
                "Alt+{key:?}"
            );
            assert_eq!(
                rating_key_action(key, true, false, false),
                None,
                "Ctrl+{key:?}"
            );
        }
        for key in [K::Num0, K::Num5, K::X] {
            assert_eq!(
                rating_key_action(key, true, true, false),
                None,
                "Ctrl+Shift+{key:?}"
            );
            assert_eq!(
                rating_key_action(key, false, true, true),
                None,
                "Alt+Shift+{key:?}"
            );
        }
        for (ctrl, alt) in [(false, false), (true, false), (false, true)] {
            assert_eq!(rating_key_action(K::Q, ctrl, false, alt), None);
            assert_eq!(rating_key_action(K::Enter, ctrl, false, alt), None);
        }
    }

    #[test]
    fn digits_and_x_keep_no_meaning_in_the_loupe_and_grid_key_maps() {
        for key in DIGITS.into_iter().chain([K::X]) {
            assert_eq!(crate::key_action(key, false, false), None, "{key:?}");
            assert_eq!(crate::grid_key_action(key, false, false), None, "{key:?}");
            assert_eq!(crate::loupe_key_action(key, false, false), None, "{key:?}");
        }
    }

    #[test]
    fn zero_clears_a_reject_and_stars_replace_it() {
        let rejected = rating(Some(-1), Some("Green"));
        assert_eq!(
            apply(&rejected, RatingKey::Stars(0)),
            rating(None, Some("Green"))
        );
        assert_eq!(
            apply(&rejected, RatingKey::Stars(4)),
            rating(Some(4), Some("Green"))
        );
        assert_eq!(
            apply(&Rating::default(), RatingKey::Stars(0)),
            Rating::default()
        );
    }

    #[test]
    fn labels_toggle_on_an_image_without_a_label_or_with_stars_only() {
        let starred = rating(Some(2), None);
        let labelled = apply(&starred, RatingKey::Label("Purple"));
        assert_eq!(labelled, rating(Some(2), Some("Purple")));
        assert_eq!(apply(&labelled, RatingKey::Label("Purple")), starred);
        assert_eq!(apply(&starred, RatingKey::ClearLabel), starred);
    }

    #[test]
    fn swedish_label_names() {
        for (label, name) in [
            ("Red", "Röd"),
            ("Yellow", "Gul"),
            ("Green", "Grön"),
            ("Blue", "Blå"),
            ("Purple", "Lila"),
            ("Orange", "Orange"),
            ("", ""),
        ] {
            assert_eq!(label_name(label), name);
        }
        for label in pw_metadata::LABELS {
            assert_ne!(label_name(label), *label, "{label} has a Swedish name");
        }
    }

    #[test]
    fn status_texts() {
        for (value, text) in [
            (None, "Betyg: Ej betygsatt"),
            (Some(rating(None, None)), "Betyg: Ej betygsatt"),
            (Some(rating(Some(-1), None)), "Betyg: Avvisad"),
            (Some(rating(Some(1), None)), "Betyg: 1 stjärna"),
            (Some(rating(Some(3), Some("Red"))), "Betyg: 3 stjärnor, Röd"),
            (Some(rating(Some(5), None)), "Betyg: 5 stjärnor"),
            (
                Some(rating(Some(-1), Some("Yellow"))),
                "Betyg: Avvisad, Gul",
            ),
            (Some(rating(None, Some("Blue"))), "Betyg: Ej betygsatt, Blå"),
            (
                Some(rating(None, Some("Orange"))),
                "Betyg: Ej betygsatt, Orange",
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
    fn unsupported_text_is_swedish() {
        assert_eq!(
            UNSUPPORTED_TEXT,
            "Betyg och etiketter kan inte sparas för den här filtypen."
        );
    }
}
