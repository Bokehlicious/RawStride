//! The filter of a folder tab (ADR 0014 §2.1–§2.3, §4.1, §5.2): which images a rating and a label
//! let through, the visible list with the keep-current rule, the nearest visible image, and the
//! Swedish texts of the two drop-down lists. Plain Rust, no I/O (GR11).

use pw_metadata::Rating;

use crate::label_name;

/// How a number of stars (`StarFilter::Stars`) is compared (ADR 0014 §2.1, amendment 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StarCondition {
    /// This many stars or more.
    #[default]
    AtLeast,
    /// Exactly this many stars.
    Exactly,
    /// Unrated, or 0 up to this many stars.
    AtMost,
}

/// The star part of a filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StarFilter {
    #[default]
    All,
    /// A number of stars (1–5), compared by the filter's `StarCondition`.
    Stars(u8),
    Unrated,
    Rejected,
    /// Anything except rejected, unrated included.
    NotRejected,
}

/// The label part of a filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LabelFilter {
    #[default]
    All,
    /// One of `pw_metadata::LABELS`.
    Label(&'static str),
    NoLabel,
}

/// The filter of a tab: stars and label, combined with AND. `condition` applies only to
/// `StarFilter::Stars`; it keeps its value while another star choice is made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    pub stars: StarFilter,
    pub condition: StarCondition,
    pub label: LabelFilter,
}

impl Filter {
    /// True when the filter lets anything less than every image through (not all / all). The
    /// condition alone never makes a filter active.
    pub fn is_active(&self) -> bool {
        self.stars != StarFilter::All || self.label != LabelFilter::All
    }
}

/// The value of a rejected image (Lightroom convention).
const REJECTED: i8 = -1;

/// Whether an image with `rating` passes `filter`. No rating and a rating without stars count as
/// unrated.
pub fn matches(filter: &Filter, rating: Option<&Rating>) -> bool {
    let stars = rating.and_then(|rating| rating.stars);
    let label = rating.and_then(|rating| rating.label.as_deref());

    let stars_match = match filter.stars {
        StarFilter::All => true,
        // Clamped to 1–5 so the filter does what its text says (`star_filter_text`).
        StarFilter::Stars(n) => {
            let n = i16::from(n.clamp(1, 5));
            match filter.condition {
                StarCondition::AtLeast => stars.is_some_and(|stars| i16::from(stars) >= n),
                StarCondition::Exactly => stars.is_some_and(|stars| i16::from(stars) == n),
                // Unrated counts as no stars; a reject (−1) is not a number of stars.
                StarCondition::AtMost => {
                    stars.is_none_or(|stars| (0..=n).contains(&i16::from(stars)))
                }
            }
        }
        StarFilter::Unrated => stars.is_none(),
        StarFilter::Rejected => stars == Some(REJECTED),
        StarFilter::NotRejected => stars != Some(REJECTED),
    };
    let label_match = match filter.label {
        LabelFilter::All => true,
        LabelFilter::Label(wanted) => label == Some(wanted),
        LabelFilter::NoLabel => label.is_none(),
    };
    stars_match && label_match
}

/// The indexes `0..count` that match, in order, plus `keep` when it is `< count` (the keep-current
/// rule, ADR 0014 §2.2).
pub fn visible(count: usize, keep: Option<usize>, is_match: impl Fn(usize) -> bool) -> Vec<usize> {
    let keep = keep.filter(|keep| *keep < count);
    (0..count)
        .filter(|index| Some(*index) == keep || is_match(*index))
        .collect()
}

/// `current` when it is visible; otherwise the first visible index after it; otherwise the last
/// visible one; `None` when nothing is visible (ADR 0014 §2.3).
pub fn nearest_visible(visible: &[usize], current: usize) -> Option<usize> {
    if visible.contains(&current) {
        return Some(current);
    }
    visible
        .iter()
        .copied()
        .find(|index| *index > current)
        .or_else(|| visible.last().copied())
}

/// The star choices in the order of the drop-down list (ADR 0014 §5.2, Q8).
pub const STAR_FILTERS: [StarFilter; 9] = [
    StarFilter::All,
    StarFilter::Stars(1),
    StarFilter::Stars(2),
    StarFilter::Stars(3),
    StarFilter::Stars(4),
    StarFilter::Stars(5),
    StarFilter::Unrated,
    StarFilter::Rejected,
    StarFilter::NotRejected,
];

/// The text of a star choice. `Stars` outside 1–5 gives the text of the nearest valid value.
pub fn star_filter_text(filter: StarFilter) -> &'static str {
    match filter {
        StarFilter::All => "Alla betyg",
        StarFilter::Stars(0..=1) => "1 stjärna",
        StarFilter::Stars(2) => "2 stjärnor",
        StarFilter::Stars(3) => "3 stjärnor",
        StarFilter::Stars(4) => "4 stjärnor",
        StarFilter::Stars(_) => "5 stjärnor",
        StarFilter::Unrated => "Ej betygsatt",
        StarFilter::Rejected => "Avvisade",
        StarFilter::NotRejected => "Alla utom avvisade",
    }
}

/// The star conditions in the order of their drop-down list (ADR 0014 §5.2, amendment 1).
pub const STAR_CONDITIONS: [StarCondition; 3] = [
    StarCondition::AtLeast,
    StarCondition::Exactly,
    StarCondition::AtMost,
];

/// The text of a star condition.
pub fn star_condition_text(condition: StarCondition) -> &'static str {
    match condition {
        StarCondition::AtLeast => "≥",
        StarCondition::Exactly => "=",
        StarCondition::AtMost => "≤",
    }
}

/// The label choices in the order of the drop-down list (ADR 0014 §5.2): all, the five labels in
/// `pw_metadata::LABELS` order, no label.
pub const LABEL_FILTERS: [LabelFilter; 7] = [
    LabelFilter::All,
    LabelFilter::Label("Red"),
    LabelFilter::Label("Yellow"),
    LabelFilter::Label("Green"),
    LabelFilter::Label("Blue"),
    LabelFilter::Label("Purple"),
    LabelFilter::NoLabel,
];

/// The text of a label choice: the Swedish label name of `label_name`.
pub fn label_filter_text(filter: LabelFilter) -> &'static str {
    match filter {
        LabelFilter::All => "Alla etiketter",
        LabelFilter::Label(label) => label_name(label),
        LabelFilter::NoLabel => "Ingen etikett",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rating(stars: Option<i8>, label: Option<&str>) -> Rating {
        Rating {
            stars,
            label: label.map(str::to_owned),
        }
    }

    fn stars_only(stars: StarFilter) -> Filter {
        Filter {
            stars,
            condition: StarCondition::default(),
            label: LabelFilter::All,
        }
    }

    fn label_only(label: LabelFilter) -> Filter {
        Filter {
            stars: StarFilter::All,
            condition: StarCondition::default(),
            label,
        }
    }

    fn with_condition(stars: StarFilter, condition: StarCondition) -> Filter {
        Filter {
            stars,
            condition,
            label: LabelFilter::All,
        }
    }

    /// The star values of the task: unrated, 1, 3, 5, rejected.
    const STAR_VALUES: [Option<i8>; 5] = [None, Some(1), Some(3), Some(5), Some(-1)];

    /// The expected result of each star choice for `STAR_VALUES`, in `STAR_FILTERS` order.
    const STAR_EXPECTED: [[bool; 5]; 9] = [
        // All
        [true, true, true, true, true],
        // ≥ 1
        [false, true, true, true, false],
        // ≥ 2
        [false, false, true, true, false],
        // ≥ 3
        [false, false, true, true, false],
        // ≥ 4
        [false, false, false, true, false],
        // 5
        [false, false, false, true, false],
        // Unrated
        [true, false, false, false, false],
        // Rejected
        [false, false, false, false, true],
        // NotRejected
        [true, true, true, true, false],
    ];

    #[test]
    fn every_star_choice_against_unrated_1_3_5_and_rejected_with_and_without_a_label() {
        for (filter, expected) in STAR_FILTERS.iter().zip(STAR_EXPECTED.iter()) {
            for (stars, want) in STAR_VALUES.iter().zip(expected.iter()) {
                for label in [None, Some("Red")] {
                    let rating = rating(*stars, label);
                    assert_eq!(
                        matches(&stars_only(*filter), Some(&rating)),
                        *want,
                        "{filter:?} against stars {stars:?}, label {label:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn no_rating_counts_as_unrated() {
        for (filter, expected) in STAR_FILTERS.iter().zip(STAR_EXPECTED.iter()) {
            assert_eq!(
                matches(&stars_only(*filter), None),
                expected[0],
                "{filter:?} against no rating"
            );
        }
        assert!(matches(&label_only(LabelFilter::NoLabel), None));
        assert!(!matches(&label_only(LabelFilter::Label("Red")), None));
    }

    #[test]
    fn at_least_one_never_matches_a_reject() {
        for label in [None, Some("Red"), Some("Purple")] {
            let rejected = rating(Some(-1), label);
            assert!(!matches(&stars_only(StarFilter::Stars(1)), Some(&rejected)));
        }
    }

    #[test]
    fn at_least_outside_one_to_five_matches_like_the_nearest_valid_value() {
        for (outside, valid) in [(0, 1), (6, 5), (u8::MAX, 5)] {
            for stars in STAR_VALUES {
                for label in [None, Some("Red")] {
                    let rating = rating(stars, label);
                    assert_eq!(
                        matches(&stars_only(StarFilter::Stars(outside)), Some(&rating)),
                        matches(&stars_only(StarFilter::Stars(valid)), Some(&rating)),
                        "Stars({outside}) against stars {stars:?}, label {label:?}"
                    );
                }
            }
            assert_eq!(
                matches(&stars_only(StarFilter::Stars(outside)), None),
                matches(&stars_only(StarFilter::Stars(valid)), None),
                "Stars({outside}) against no rating"
            );
        }
        // A reject still never matches.
        assert!(!matches(
            &stars_only(StarFilter::Stars(0)),
            Some(&rating(Some(-1), None))
        ));
    }

    #[test]
    fn not_rejected_matches_unrated() {
        assert!(matches(&stars_only(StarFilter::NotRejected), None));
        assert!(matches(
            &stars_only(StarFilter::NotRejected),
            Some(&rating(None, None))
        ));
        assert!(matches(
            &stars_only(StarFilter::NotRejected),
            Some(&rating(None, Some("Blue")))
        ));
    }

    #[test]
    fn every_label_choice_against_no_label_red_and_purple() {
        let labels = [None, Some("Red"), Some("Purple")];
        // Expected results for `labels`, in `LABEL_FILTERS` order.
        let expected: [[bool; 3]; 7] = [
            [true, true, true],    // All
            [false, true, false],  // Red
            [false, false, false], // Yellow
            [false, false, false], // Green
            [false, false, false], // Blue
            [false, false, true],  // Purple
            [true, false, false],  // NoLabel
        ];
        for (filter, expected) in LABEL_FILTERS.iter().zip(expected.iter()) {
            for (label, want) in labels.iter().zip(expected.iter()) {
                for stars in [None, Some(3)] {
                    let rating = rating(stars, *label);
                    assert_eq!(
                        matches(&label_only(*filter), Some(&rating)),
                        *want,
                        "{filter:?} against label {label:?}, stars {stars:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn stars_and_label_combine_with_and() {
        let filter = Filter {
            stars: StarFilter::Stars(3),
            condition: StarCondition::default(),
            label: LabelFilter::Label("Red"),
        };
        assert!(matches(&filter, Some(&rating(Some(3), Some("Red")))));
        assert!(matches(&filter, Some(&rating(Some(5), Some("Red")))));
        assert!(!matches(&filter, Some(&rating(Some(1), Some("Red")))));
        assert!(!matches(&filter, Some(&rating(Some(5), Some("Purple")))));
        assert!(!matches(&filter, Some(&rating(Some(5), None))));
        assert!(!matches(&filter, None));

        let filter = Filter {
            stars: StarFilter::Unrated,
            condition: StarCondition::default(),
            label: LabelFilter::NoLabel,
        };
        assert!(matches(&filter, None));
        assert!(!matches(&filter, Some(&rating(None, Some("Red")))));
        assert!(!matches(&filter, Some(&rating(Some(1), None))));
    }

    #[test]
    fn the_default_filter_is_all_all_and_not_active() {
        let filter = Filter::default();
        assert_eq!(filter.stars, StarFilter::All);
        assert_eq!(filter.label, LabelFilter::All);
        assert!(!filter.is_active());
        assert!(stars_only(StarFilter::Unrated).is_active());
        assert!(label_only(LabelFilter::NoLabel).is_active());
    }

    #[test]
    fn visible_without_keep_lists_the_matches_in_order() {
        assert_eq!(visible(6, None, |index| index % 2 == 1), vec![1, 3, 5]);
        assert_eq!(visible(6, None, |_| false), Vec::<usize>::new());
        assert_eq!(visible(0, None, |_| true), Vec::<usize>::new());
    }

    #[test]
    fn visible_with_keep_adds_the_kept_index_in_order() {
        assert_eq!(
            visible(6, Some(2), |index| index % 2 == 1),
            vec![1, 2, 3, 5]
        );
        assert_eq!(visible(6, Some(3), |index| index % 2 == 1), vec![1, 3, 5]);
        assert_eq!(visible(6, Some(0), |_| false), vec![0]);
    }

    #[test]
    fn visible_ignores_keep_out_of_range() {
        assert_eq!(visible(6, Some(6), |index| index % 2 == 1), vec![1, 3, 5]);
        assert_eq!(visible(6, Some(usize::MAX), |_| false), Vec::<usize>::new());
        assert_eq!(visible(0, Some(0), |_| true), Vec::<usize>::new());
    }

    #[test]
    fn nearest_visible_keeps_a_visible_current() {
        assert_eq!(nearest_visible(&[1, 3, 5], 3), Some(3));
    }

    #[test]
    fn nearest_visible_takes_the_following_index() {
        assert_eq!(nearest_visible(&[1, 3, 5], 2), Some(3));
        assert_eq!(nearest_visible(&[1, 3, 5], 0), Some(1));
    }

    #[test]
    fn nearest_visible_takes_the_last_when_only_earlier_ones_are_visible() {
        assert_eq!(nearest_visible(&[1, 3, 5], 6), Some(5));
        assert_eq!(nearest_visible(&[1, 3, 5], usize::MAX), Some(5));
    }

    #[test]
    fn nearest_visible_is_none_when_nothing_is_visible() {
        assert_eq!(nearest_visible(&[], 0), None);
        assert_eq!(nearest_visible(&[], 4), None);
    }

    #[test]
    fn star_filter_list_and_texts() {
        assert_eq!(STAR_FILTERS.len(), 9);
        let texts: Vec<&str> = STAR_FILTERS.iter().map(|f| star_filter_text(*f)).collect();
        assert_eq!(
            texts,
            vec![
                "Alla betyg",
                "1 stjärna",
                "2 stjärnor",
                "3 stjärnor",
                "4 stjärnor",
                "5 stjärnor",
                "Ej betygsatt",
                "Avvisade",
                "Alla utom avvisade",
            ]
        );
    }

    #[test]
    fn star_filter_text_outside_one_to_five_uses_the_nearest_valid_value() {
        assert_eq!(star_filter_text(StarFilter::Stars(0)), "1 stjärna");
        assert_eq!(star_filter_text(StarFilter::Stars(6)), "5 stjärnor");
        assert_eq!(star_filter_text(StarFilter::Stars(u8::MAX)), "5 stjärnor");
    }

    /// Unrated, 1–5 stars and rejected.
    const CONDITION_VALUES: [Option<i8>; 7] =
        [None, Some(1), Some(2), Some(3), Some(4), Some(5), Some(-1)];

    #[test]
    fn each_condition_with_one_to_five_stars_against_unrated_1_to_5_and_rejected() {
        for n in 1..=5u8 {
            let n_i8 = i8::try_from(n).unwrap_or(i8::MAX);
            for stars in CONDITION_VALUES {
                let rejected = stars == Some(-1);
                let at_least = !rejected && stars.is_some_and(|s| s >= n_i8);
                let exactly = !rejected && stars == Some(n_i8);
                // ≤ n includes unrated; a reject never matches.
                let at_most = !rejected && stars.is_none_or(|s| s <= n_i8);
                for (condition, want) in [
                    (StarCondition::AtLeast, at_least),
                    (StarCondition::Exactly, exactly),
                    (StarCondition::AtMost, at_most),
                ] {
                    let filter = with_condition(StarFilter::Stars(n), condition);
                    for label in [None, Some("Red")] {
                        assert_eq!(
                            matches(&filter, Some(&rating(stars, label))),
                            want,
                            "{condition:?} {n} against stars {stars:?}, label {label:?}"
                        );
                    }
                    if stars.is_none() {
                        assert_eq!(
                            matches(&filter, None),
                            want,
                            "{condition:?} {n} against no rating"
                        );
                    }
                }
            }
        }
        // Spot checks of the rule above.
        let at_most_2 = with_condition(StarFilter::Stars(2), StarCondition::AtMost);
        assert!(matches(&at_most_2, None));
        assert!(matches(&at_most_2, Some(&rating(Some(0), None))));
        assert!(matches(&at_most_2, Some(&rating(Some(2), None))));
        assert!(!matches(&at_most_2, Some(&rating(Some(3), None))));
        assert!(!matches(&at_most_2, Some(&rating(Some(-1), None))));
        let exactly_3 = with_condition(StarFilter::Stars(3), StarCondition::Exactly);
        assert!(matches(&exactly_3, Some(&rating(Some(3), None))));
        assert!(!matches(&exactly_3, Some(&rating(Some(4), None))));
        assert!(!matches(&exactly_3, None));
    }

    #[test]
    fn exactly_and_at_most_outside_one_to_five_match_like_the_nearest_valid_value() {
        let values = [
            None,
            Some(0),
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(-1),
        ];
        for condition in [StarCondition::Exactly, StarCondition::AtMost] {
            for (outside, valid) in [(0, 1), (6, 5), (9, 5), (u8::MAX, 5)] {
                let outside_filter = with_condition(StarFilter::Stars(outside), condition);
                let valid_filter = with_condition(StarFilter::Stars(valid), condition);
                for stars in values {
                    assert_eq!(
                        matches(&outside_filter, Some(&rating(stars, None))),
                        matches(&valid_filter, Some(&rating(stars, None))),
                        "{condition:?} {outside} against stars {stars:?}"
                    );
                }
                assert_eq!(
                    matches(&outside_filter, None),
                    matches(&valid_filter, None),
                    "{condition:?} {outside} against no rating"
                );
            }
        }
        // = 0 as = 1, ≤ 9 as ≤ 5.
        let exactly_0 = with_condition(StarFilter::Stars(0), StarCondition::Exactly);
        assert!(matches(&exactly_0, Some(&rating(Some(1), None))));
        assert!(!matches(&exactly_0, Some(&rating(Some(0), None))));
        let at_most_9 = with_condition(StarFilter::Stars(9), StarCondition::AtMost);
        assert!(matches(&at_most_9, Some(&rating(Some(5), None))));
        assert!(matches(&at_most_9, None));
        assert!(!matches(&at_most_9, Some(&rating(Some(-1), None))));
    }

    #[test]
    fn the_condition_is_ignored_for_all_unrated_rejected_and_not_rejected() {
        let values = [None, Some(0), Some(1), Some(3), Some(5), Some(-1)];
        for stars in [
            StarFilter::All,
            StarFilter::Unrated,
            StarFilter::Rejected,
            StarFilter::NotRejected,
        ] {
            for condition in STAR_CONDITIONS {
                let filter = with_condition(stars, condition);
                for value in values {
                    assert_eq!(
                        matches(&filter, Some(&rating(value, None))),
                        matches(&stars_only(stars), Some(&rating(value, None))),
                        "{stars:?} with {condition:?} against stars {value:?}"
                    );
                }
                assert_eq!(
                    matches(&filter, None),
                    matches(&stars_only(stars), None),
                    "{stars:?} with {condition:?} against no rating"
                );
            }
        }
    }

    #[test]
    fn the_condition_alone_never_makes_a_filter_active() {
        for condition in STAR_CONDITIONS {
            assert!(
                !with_condition(StarFilter::All, condition).is_active(),
                "All/All with {condition:?}"
            );
            for n in 1..=5 {
                assert!(
                    with_condition(StarFilter::Stars(n), condition).is_active(),
                    "Stars({n}) with {condition:?}"
                );
            }
        }
    }

    #[test]
    fn star_condition_list_texts_and_default() {
        assert_eq!(
            STAR_CONDITIONS,
            [
                StarCondition::AtLeast,
                StarCondition::Exactly,
                StarCondition::AtMost
            ]
        );
        let texts: Vec<&str> = STAR_CONDITIONS
            .iter()
            .map(|c| star_condition_text(*c))
            .collect();
        assert_eq!(texts, vec!["≥", "=", "≤"]);
        assert_eq!(StarCondition::default(), StarCondition::AtLeast);
        assert_eq!(Filter::default().condition, StarCondition::AtLeast);
    }

    #[test]
    fn label_filter_list_and_texts() {
        assert_eq!(LABEL_FILTERS.len(), 7);
        let labels: Vec<&str> = LABEL_FILTERS
            .iter()
            .filter_map(|filter| match filter {
                LabelFilter::Label(label) => Some(*label),
                _ => None,
            })
            .collect();
        assert_eq!(labels, pw_metadata::LABELS);
        assert_eq!(LABEL_FILTERS.first(), Some(&LabelFilter::All));
        assert_eq!(LABEL_FILTERS.last(), Some(&LabelFilter::NoLabel));

        let texts: Vec<&str> = LABEL_FILTERS
            .iter()
            .map(|f| label_filter_text(*f))
            .collect();
        assert_eq!(
            texts,
            vec![
                "Alla etiketter",
                "Röd",
                "Gul",
                "Grön",
                "Blå",
                "Lila",
                "Ingen etikett",
            ]
        );
    }
}
