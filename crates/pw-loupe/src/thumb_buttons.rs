//! The four buttons on a thumbnail in the contact sheet (ADR 0017 §2.8, addition U15, T245):
//! Rotate Left in the upper left corner of the cell's image square, Rotate Right in the upper
//! right, Metadata… in the lower left and Preview in the lower right, shown while the pointer is
//! over the cell. Here are where they are, which are drawn and what Preview opens with, as plain
//! values; `main.rs` draws them and does the clicks.

use egui::{Pos2, Rect, Vec2};

use crate::shell::Command;

/// A button on a thumbnail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThumbButton {
    RotateLeft,
    RotateRight,
    Metadata,
    Preview,
}

/// The four buttons, in the order they are drawn.
pub const THUMB_BUTTONS: [ThumbButton; 4] = [
    ThumbButton::RotateLeft,
    ThumbButton::RotateRight,
    ThumbButton::Metadata,
    ThumbButton::Preview,
];

/// A button's disc's diameter in points, the same at every thumbnail size (U15-Q6).
pub const BUTTON_SIZE: f32 = 20.0;
/// From the image square's edges to a disc's edge, in points.
pub const BUTTON_INSET: f32 = 4.0;

/// What the commands of the buttons can do for the cell's image, by the rules that grey their
/// menu items (U15-Q5): a command that cannot run draws no button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonFacts {
    /// Rotate Left and Rotate Right.
    pub rotate: bool,
    /// Metadata…: the file's kind has metadata support.
    pub metadata: bool,
    /// Preview.
    pub preview: bool,
}

impl ThumbButton {
    /// The command the button runs, the same as its menu item's.
    pub fn command(self) -> Command {
        match self {
            Self::RotateLeft => Command::RotateLeft,
            Self::RotateRight => Command::RotateRight,
            Self::Metadata => Command::Metadata,
            Self::Preview => Command::ToggleView,
        }
    }

    /// Whether the button is drawn.
    pub fn shown(self, facts: &ButtonFacts) -> bool {
        match self {
            Self::RotateLeft | Self::RotateRight => facts.rotate,
            Self::Metadata => facts.metadata,
            Self::Preview => facts.preview,
        }
    }
}

/// The bounding square of `button`'s disc in its corner of `image`, the cell's square image area
/// (not the fitted thumbnail), [`BUTTON_INSET`] in from both edges. The square is the button's
/// whole hit area; only the drawing is round.
pub fn button_rect(image: Rect, button: ThumbButton) -> Rect {
    let left = image.left() + BUTTON_INSET;
    let right = image.right() - BUTTON_INSET - BUTTON_SIZE;
    let top = image.top() + BUTTON_INSET;
    let bottom = image.bottom() - BUTTON_INSET - BUTTON_SIZE;
    let min = match button {
        ThumbButton::RotateLeft => Pos2::new(left, top),
        ThumbButton::RotateRight => Pos2::new(right, top),
        ThumbButton::Metadata => Pos2::new(left, bottom),
        ThumbButton::Preview => Pos2::new(right, bottom),
    };
    Rect::from_min_size(min, Vec2::splat(BUTTON_SIZE))
}

/// The button whose [`button_rect`] holds `pointer`, else `None`.
pub fn button_at(image: Rect, pointer: Pos2) -> Option<ThumbButton> {
    THUMB_BUTTONS
        .into_iter()
        .find(|&button| button_rect(image, button).contains(pointer))
}

/// "Only the marked" for the preview the Preview button opens (U15-Q4): on when the image is
/// marked and at least two images are, else off.
pub fn preview_only_marked(marked_count: usize, image_marked: bool) -> bool {
    image_marked && marked_count >= 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{MAX_THUMBNAIL_SIZE, MIN_THUMBNAIL_SIZE};

    /// The label row under a cell's image square: `GRID_LABEL_HEIGHT` in `main.rs`, which the
    /// library cannot see; `draw_grid` takes it from the cell's height for the image square.
    const LABEL_ROW: f32 = 18.0;

    fn square(x: f32, y: f32, side: f32) -> Rect {
        Rect::from_min_size(Pos2::new(x, y), Vec2::splat(side))
    }

    fn image_side(thumbnail_size: u32) -> f32 {
        f32::from(u16::try_from(thumbnail_size).unwrap()) - LABEL_ROW
    }

    #[test]
    fn each_button_sits_in_its_corner_inset_from_both_edges() {
        for image in [square(0.0, 0.0, 200.0), square(312.5, 47.0, 150.0)] {
            let upper_left = button_rect(image, ThumbButton::RotateLeft);
            assert_eq!(upper_left.left() - image.left(), BUTTON_INSET);
            assert_eq!(upper_left.top() - image.top(), BUTTON_INSET);
            let upper_right = button_rect(image, ThumbButton::RotateRight);
            assert_eq!(image.right() - upper_right.right(), BUTTON_INSET);
            assert_eq!(upper_right.top() - image.top(), BUTTON_INSET);
            let lower_left = button_rect(image, ThumbButton::Metadata);
            assert_eq!(lower_left.left() - image.left(), BUTTON_INSET);
            assert_eq!(image.bottom() - lower_left.bottom(), BUTTON_INSET);
            let lower_right = button_rect(image, ThumbButton::Preview);
            assert_eq!(image.right() - lower_right.right(), BUTTON_INSET);
            assert_eq!(image.bottom() - lower_right.bottom(), BUTTON_INSET);
        }
    }

    #[test]
    fn the_buttons_are_the_same_size_at_every_thumbnail_size() {
        for size in [MIN_THUMBNAIL_SIZE, 160, 300, MAX_THUMBNAIL_SIZE] {
            let image = square(10.0, 20.0, image_side(size));
            for button in THUMB_BUTTONS {
                let rect = button_rect(image, button);
                assert_eq!(rect.size(), Vec2::splat(BUTTON_SIZE), "{size} {button:?}");
            }
        }
    }

    #[test]
    fn the_buttons_do_not_overlap_at_the_smallest_and_the_largest_size() {
        // The smallest: a 78-point square leaves 30 points between two discs.
        assert_eq!(image_side(MIN_THUMBNAIL_SIZE), 78.0);
        for size in [MIN_THUMBNAIL_SIZE, MAX_THUMBNAIL_SIZE] {
            let image = square(0.0, 0.0, image_side(size));
            for (i, &a) in THUMB_BUTTONS.iter().enumerate() {
                for &b in THUMB_BUTTONS.iter().skip(i + 1) {
                    let ra = button_rect(image, a);
                    let rb = button_rect(image, b);
                    let gap_x = (rb.left() - ra.right()).max(ra.left() - rb.right());
                    let gap_y = (rb.top() - ra.bottom()).max(ra.top() - rb.bottom());
                    assert!(
                        gap_x.max(gap_y) >= BUTTON_INSET,
                        "{size}: {a:?} and {b:?} are {gap_x}, {gap_y} apart"
                    );
                }
                assert!(image.contains_rect(button_rect(image, a)), "{size} {a:?}");
            }
        }
        let image = square(0.0, 0.0, image_side(MIN_THUMBNAIL_SIZE));
        let gap = button_rect(image, ThumbButton::RotateRight).left()
            - button_rect(image, ThumbButton::RotateLeft).right();
        assert_eq!(gap, 30.0);
    }

    #[test]
    fn button_at_finds_the_button_under_the_pointer() {
        let image = square(100.0, 50.0, 150.0);
        for button in THUMB_BUTTONS {
            let rect = button_rect(image, button);
            assert_eq!(button_at(image, rect.center()), Some(button), "{button:?}");
            // A corner of the square, just inside: the square is the hit area.
            let corner = rect.min + Vec2::splat(0.5);
            assert_eq!(button_at(image, corner), Some(button), "{button:?}");
            let corner = rect.max - Vec2::splat(0.5);
            assert_eq!(button_at(image, corner), Some(button), "{button:?}");
        }
        assert_eq!(button_at(image, image.center()), None);
        // Just outside a button's square.
        let upper_left = button_rect(image, ThumbButton::RotateLeft);
        assert_eq!(
            button_at(
                image,
                Pos2::new(upper_left.right() + 0.5, upper_left.center().y)
            ),
            None
        );
        assert_eq!(
            button_at(
                image,
                Pos2::new(upper_left.center().x, upper_left.bottom() + 0.5)
            ),
            None
        );
        let lower_right = button_rect(image, ThumbButton::Preview);
        assert_eq!(
            button_at(
                image,
                Pos2::new(lower_right.left() - 0.5, lower_right.center().y)
            ),
            None
        );
    }

    #[test]
    fn each_button_runs_its_menu_items_command() {
        assert_eq!(ThumbButton::RotateLeft.command(), Command::RotateLeft);
        assert_eq!(ThumbButton::RotateRight.command(), Command::RotateRight);
        assert_eq!(ThumbButton::Metadata.command(), Command::Metadata);
        assert_eq!(ThumbButton::Preview.command(), Command::ToggleView);
    }

    #[test]
    fn a_button_is_shown_by_its_fact() {
        let none = ButtonFacts {
            rotate: false,
            metadata: false,
            preview: false,
        };
        for button in THUMB_BUTTONS {
            assert!(!button.shown(&none), "{button:?}");
        }
        let rotate = ButtonFacts {
            rotate: true,
            ..none
        };
        let metadata = ButtonFacts {
            metadata: true,
            ..none
        };
        let preview = ButtonFacts {
            preview: true,
            ..none
        };
        let shown = |facts: &ButtonFacts| -> Vec<ThumbButton> {
            THUMB_BUTTONS
                .into_iter()
                .filter(|button| button.shown(facts))
                .collect()
        };
        assert_eq!(
            shown(&rotate),
            [ThumbButton::RotateLeft, ThumbButton::RotateRight]
        );
        assert_eq!(shown(&metadata), [ThumbButton::Metadata]);
        assert_eq!(shown(&preview), [ThumbButton::Preview]);
        let all = ButtonFacts {
            rotate: true,
            metadata: true,
            preview: true,
        };
        assert_eq!(shown(&all), THUMB_BUTTONS);
    }

    #[test]
    fn the_preview_opens_with_only_the_marked_when_two_or_more_and_the_image_is_marked() {
        assert!(!preview_only_marked(0, false));
        assert!(!preview_only_marked(1, true));
        assert!(!preview_only_marked(2, false));
        assert!(preview_only_marked(2, true));
        assert!(preview_only_marked(5, true));
    }
}
