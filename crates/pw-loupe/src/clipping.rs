//! The clipping marks drawn over the image in the preview (ADR 0017 §2.6, U7b, T149): the view
//! core's clipping map turned into the overlay's pixels and texture coordinates, and the rule for
//! when the overlay's texture is made again. Plain Rust, tested without a window (GR11);
//! `main.rs` uploads the pixels and draws the quad.

use std::sync::Arc;

use pw_viewcore::ClippingMap;

/// The colour of a cell with clipped highlights: solid red (U7b-Q4), as RGBA.
pub const HIGHLIGHT_RGBA: [u8; 4] = [255, 0, 0, 255];
/// The colour of a cell in the deepest shadows: solid blue (U7b-Q4), as RGBA.
pub const SHADOW_RGBA: [u8; 4] = [0, 0, 255, 255];
/// A cell without a mark: fully transparent.
pub const CLEAR_RGBA: [u8; 4] = [0, 0, 0, 0];

/// Which of the two overlays the user has on (T148's `clip_highlights` and `clip_shadows`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClipChoice {
    pub highlights: bool,
    pub shadows: bool,
}

impl ClipChoice {
    /// Whether at least one overlay is on.
    pub fn any(self) -> bool {
        self.highlights || self.shadows
    }
}

/// The overlay's pixels for one clipping map and choice.
#[derive(Clone, Debug, PartialEq)]
pub struct Overlay {
    /// Width and height in pixels: one pixel per cell of the map, or per joined cell.
    pub size: [usize; 2],
    /// `size[0] × size[1]` RGBA pixels, row by row; each is [`HIGHLIGHT_RGBA`], [`SHADOW_RGBA`]
    /// or [`CLEAR_RGBA`], so premultiplied and unmultiplied alpha are the same.
    pub rgba: Vec<u8>,
    /// What the map's texture coordinates are multiplied by, for u and v ([`overlay_uv`]): 1.0
    /// unless cells were joined on a side with an odd number of them.
    pub uv_scale: [f32; 2],
}

/// The colour of one cell (U7b-Q4): red where a clipped bit (any of R, G, B) is set and the
/// highlight overlay is on, blue where the deep-shadow bit is set and the shadow overlay is on,
/// transparent elsewhere. A joined cell can hold both; the highlights are drawn then.
pub fn cell_rgba(bits: u8, choice: ClipChoice) -> [u8; 4] {
    let clipped = ClippingMap::CLIPPED_R | ClippingMap::CLIPPED_G | ClippingMap::CLIPPED_B;
    if choice.highlights && bits & clipped != 0 {
        HIGHLIGHT_RGBA
    } else if choice.shadows && bits & ClippingMap::DEEP_SHADOWS != 0 {
        SHADOW_RGBA
    } else {
        CLEAR_RGBA
    }
}

/// One joining of 2 × 2 cells: `width` cells per row become `width / 2` rounded up, and so the
/// rows. A joined cell has every bit that any of its cells has; a cell past an odd side counts
/// as unmarked.
fn join(width: usize, data: &[u8]) -> (usize, Vec<u8>) {
    let any = |cells: &[u8]| cells.iter().fold(0u8, |bits, cell| bits | cell);
    let joined_width = width.div_ceil(2);
    let mut joined = Vec::with_capacity(data.len().div_ceil(4));
    if width == 0 {
        return (0, joined);
    }
    for rows in data.chunks(width.saturating_mul(2)) {
        let (top, bottom) = rows.split_at(width.min(rows.len()));
        let mut bottom = bottom.chunks(2);
        joined.extend(
            top.chunks(2)
                .map(|cells| any(cells) | bottom.next().map_or(0, any)),
        );
    }
    (joined_width, joined)
}

/// The overlay of `map` for `choice`, no wider or higher than `max_side` (the GPU's largest
/// texture side). A larger map is made smaller by joining 2 × 2 cells until it fits; a side with
/// an odd number of cells then gives a joined grid that reaches past the map by half a joined
/// cell, and `uv_scale` is `w / (2 · ceil(w / 2))` per joining for it, so the marks stay in
/// place. `None` (nothing is drawn) for a map without cells, a map whose `data` length is not
/// `width × height`, or a `max_side` of 0.
pub fn overlay(map: &ClippingMap, choice: ClipChoice, max_side: usize) -> Option<Overlay> {
    let mut width = usize::try_from(map.width).ok()?;
    let mut height = usize::try_from(map.height).ok()?;
    if width == 0 || height == 0 || max_side == 0 || width.checked_mul(height)? != map.data.len() {
        return None;
    }
    let mut uv_scale = [1.0f64; 2];
    let mut joined: Option<Vec<u8>> = None;
    while width > max_side || height > max_side {
        let (joined_width, data) = join(width, joined.as_deref().unwrap_or(&map.data));
        let joined_height = height.div_ceil(2);
        uv_scale[0] *= width as f64 / (2.0 * joined_width as f64);
        uv_scale[1] *= height as f64 / (2.0 * joined_height as f64);
        width = joined_width;
        height = joined_height;
        joined = Some(data);
    }
    let cells = joined.as_deref().unwrap_or(&map.data);
    let mut rgba = Vec::with_capacity(cells.len().saturating_mul(4));
    for &bits in cells {
        rgba.extend_from_slice(&cell_rgba(bits, choice));
    }
    Some(Overlay {
        size: [width, height],
        rgba,
        uv_scale: uv_scale.map(|scale| scale as f32),
    })
}

/// The overlay's texture coordinates at the corners of the drawn image: the map's (T147's, in
/// which 1.0 is the map's right or bottom edge) scaled for the joined cells ([`Overlay::uv_scale`]).
pub fn overlay_uv(uv: [[f32; 2]; 4], uv_scale: [f32; 2]) -> [[f32; 2]; 4] {
    uv.map(|[u, v]| [u * uv_scale[0], v * uv_scale[1]])
}

/// What the kept texture was made from.
struct Kept<T> {
    map: Arc<ClippingMap>,
    choice: ClipChoice,
    max_side: usize,
    /// `None` when the map cannot be drawn, so it is not tried again every frame.
    texture: Option<(T, [f32; 2])>,
}

/// The overlay's one texture (`T`; an egui texture in the program), made once per map and choice
/// (T149 point 3): again only when the map (by `Arc::ptr_eq`), the choice or the largest texture
/// side changes, and dropped by [`release`](Self::release) when no overlay is shown.
pub struct OverlayCache<T> {
    kept: Option<Kept<T>>,
}

impl<T> std::fmt::Debug for OverlayCache<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayCache")
            .field("kept", &self.kept.as_ref().map(|kept| &kept.map))
            .finish()
    }
}

impl<T> Default for OverlayCache<T> {
    fn default() -> Self {
        OverlayCache { kept: None }
    }
}

impl<T> OverlayCache<T> {
    /// The texture of `map` for `choice` and its [`Overlay::uv_scale`]; `make` uploads the pixels
    /// and is called only when the kept texture is not of this map, choice and `max_side`. The
    /// texture kept before is dropped first, so there is one at a time. `None` when the map
    /// cannot be drawn ([`overlay`]).
    pub fn texture(
        &mut self,
        map: &Arc<ClippingMap>,
        choice: ClipChoice,
        max_side: usize,
        make: impl FnOnce(Overlay) -> T,
    ) -> Option<(&T, [f32; 2])> {
        let same = self.kept.as_ref().is_some_and(|kept| {
            Arc::ptr_eq(&kept.map, map) && kept.choice == choice && kept.max_side == max_side
        });
        if !same {
            self.kept = None;
            let texture = overlay(map, choice, max_side).map(|overlay| {
                let uv_scale = overlay.uv_scale;
                (make(overlay), uv_scale)
            });
            self.kept = Some(Kept {
                map: Arc::clone(map),
                choice,
                max_side,
                texture,
            });
        }
        let (texture, uv_scale) = self.kept.as_ref()?.texture.as_ref()?;
        Some((texture, *uv_scale))
    }

    /// Drops the texture: no overlay is shown.
    pub fn release(&mut self) {
        self.kept = None;
    }

    /// Whether a map's texture (or the note that it cannot be drawn) is kept.
    pub fn is_kept(&self) -> bool {
        self.kept.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: u8 = ClippingMap::CLIPPED_R;
    const G: u8 = ClippingMap::CLIPPED_G;
    const B: u8 = ClippingMap::CLIPPED_B;
    const DARK: u8 = ClippingMap::DEEP_SHADOWS;

    const BOTH: ClipChoice = ClipChoice {
        highlights: true,
        shadows: true,
    };
    const HIGHLIGHTS: ClipChoice = ClipChoice {
        highlights: true,
        shadows: false,
    };
    const SHADOWS: ClipChoice = ClipChoice {
        highlights: false,
        shadows: true,
    };
    const NEITHER: ClipChoice = ClipChoice {
        highlights: false,
        shadows: false,
    };

    fn map(width: u32, height: u32, data: Vec<u8>) -> ClippingMap {
        ClippingMap {
            cell: 2,
            width,
            height,
            data,
        }
    }

    /// The overlay's pixels, one `[u8; 4]` per cell.
    fn pixels(overlay: &Overlay) -> Vec<[u8; 4]> {
        overlay.rgba.as_chunks::<4>().0.to_vec()
    }

    #[test]
    fn a_highlight_bit_is_red_and_a_shadow_bit_blue_only_when_its_overlay_is_on() {
        // R, G, B alone, two channels, nothing, the deep shadows
        let m = map(3, 2, vec![R, G, B, R | B, 0, DARK]);
        let (red, blue, clear) = (HIGHLIGHT_RGBA, SHADOW_RGBA, CLEAR_RGBA);
        assert_eq!(red, [255, 0, 0, 255]);
        assert_eq!(blue, [0, 0, 255, 255]);
        assert_eq!(clear, [0, 0, 0, 0]);

        let both = overlay(&m, BOTH, 16).unwrap();
        assert_eq!(both.size, [3, 2]);
        assert_eq!(both.uv_scale, [1.0, 1.0]);
        assert_eq!(pixels(&both), [red, red, red, red, clear, blue]);

        let highlights = overlay(&m, HIGHLIGHTS, 16).unwrap();
        assert_eq!(pixels(&highlights), [red, red, red, red, clear, clear]);

        let shadows = overlay(&m, SHADOWS, 16).unwrap();
        assert_eq!(pixels(&shadows), [clear, clear, clear, clear, clear, blue]);

        let neither = overlay(&m, NEITHER, 16).unwrap();
        assert_eq!(pixels(&neither), [clear; 6]);
        assert!(BOTH.any() && HIGHLIGHTS.any() && SHADOWS.any() && !NEITHER.any());
    }

    #[test]
    fn a_map_that_fits_is_not_joined() {
        let m = map(4, 3, vec![0; 12]);
        let o = overlay(&m, BOTH, 4).unwrap();
        assert_eq!(o.size, [4, 3]);
        assert_eq!(o.uv_scale, [1.0, 1.0]);
        assert_eq!(o.rgba.len(), 4 * 3 * 4);
    }

    #[test]
    fn joining_marks_a_joined_cell_when_any_of_its_cells_is_marked() {
        #[rustfmt::skip]
        let m = map(4, 4, vec![
            R, 0,   0, 0,
            0, 0,   0, G,
            0, 0,   0, 0,
            DARK, 0, 0, 0,
        ]);
        let (red, blue, clear) = (HIGHLIGHT_RGBA, SHADOW_RGBA, CLEAR_RGBA);
        // 4 × 4 does not fit a side of 2: joined once
        let o = overlay(&m, BOTH, 2).unwrap();
        assert_eq!(o.size, [2, 2]);
        assert_eq!(o.uv_scale, [1.0, 1.0]);
        assert_eq!(pixels(&o), [red, red, blue, clear]);
        // only the overlay that is on
        let o = overlay(&m, SHADOWS, 2).unwrap();
        assert_eq!(pixels(&o), [clear, clear, blue, clear]);
        // a side of 1: joined twice, and the highlights are drawn over the shadows
        let o = overlay(&m, BOTH, 1).unwrap();
        assert_eq!(o.size, [1, 1]);
        assert_eq!(pixels(&o), [red]);
        let o = overlay(&m, SHADOWS, 1).unwrap();
        assert_eq!(pixels(&o), [blue]);
    }

    #[test]
    fn joining_fits_the_largest_side_also_when_only_one_side_is_too_long() {
        // 9 × 2 on a side of 4: 5 × 1 after one joining, 3 × 1 after two
        let mut data = vec![0; 18];
        data[8] = B; // the last cell of the first row
        data[9] = DARK; // the first cell of the second row
        let m = map(9, 2, data);
        let o = overlay(&m, BOTH, 4).unwrap();
        assert_eq!(o.size, [3, 1]);
        assert!(o.size.iter().all(|&side| side <= 4));
        assert_eq!(
            pixels(&o),
            [SHADOW_RGBA, CLEAR_RGBA, HIGHLIGHT_RGBA],
            "the marks stay at their ends"
        );
        for max_side in 1..=9 {
            let o = overlay(&m, BOTH, max_side).unwrap();
            assert!(o.size.iter().all(|&side| side <= max_side), "{max_side}");
            assert_eq!(o.rgba.len(), o.size[0] * o.size[1] * 4, "{max_side}");
        }
    }

    /// Where a texture coordinate lies in the original map, in cells from its left or top edge:
    /// the overlay has `side` pixels of `joined` cells each.
    fn cells(coordinate: f32, side: usize, joined: usize) -> f32 {
        coordinate * side as f32 * joined as f32
    }

    #[test]
    fn a_corner_of_an_odd_map_stays_on_the_same_image_point_after_joining() {
        // 5 × 3 cells: before joining the corner (1, 1) is 5 and 3 cells from the top left
        let m = map(5, 3, vec![0; 15]);
        let corners = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let whole = overlay(&m, BOTH, 8).unwrap();
        assert_eq!(overlay_uv(corners, whole.uv_scale), corners);

        // joined once: 3 × 2 joined cells, which reach to 6 and 4 cells
        let once = overlay(&m, BOTH, 4).unwrap();
        assert_eq!(once.size, [3, 2]);
        assert!((once.uv_scale[0] - 5.0 / 6.0).abs() < 1e-6);
        assert!((once.uv_scale[1] - 3.0 / 4.0).abs() < 1e-6);
        let uv = overlay_uv(corners, once.uv_scale);
        assert_eq!(uv[0], [0.0, 0.0]);
        assert!((cells(uv[2][0], 3, 2) - 5.0).abs() < 1e-5);
        assert!((cells(uv[2][1], 2, 2) - 3.0).abs() < 1e-5);
        assert!((cells(uv[1][0], 3, 2) - 5.0).abs() < 1e-5);
        assert!((cells(uv[3][1], 2, 2) - 3.0).abs() < 1e-5);

        // joined twice: 2 × 1 cells of 4 × 4, which reach to 8 and 4 cells
        let twice = overlay(&m, BOTH, 2).unwrap();
        assert_eq!(twice.size, [2, 1]);
        let uv = overlay_uv(corners, twice.uv_scale);
        assert!((cells(uv[2][0], 2, 4) - 5.0).abs() < 1e-5);
        assert!((cells(uv[2][1], 1, 4) - 3.0).abs() < 1e-5);

        // a point inside the map, and a turned image (u and v swapped between the corners)
        let turned = [[0.4, 1.0], [0.4, 0.0], [1.0, 0.0], [1.0, 1.0]];
        let uv = overlay_uv(turned, once.uv_scale);
        assert!((cells(uv[0][0], 3, 2) - 2.0).abs() < 1e-5);
        assert!((cells(uv[0][1], 2, 2) - 3.0).abs() < 1e-5);
        assert!((cells(uv[2][0], 3, 2) - 5.0).abs() < 1e-5);
        assert_eq!(uv[2][1], 0.0);
    }

    #[test]
    fn an_even_side_keeps_its_coordinates_when_joined() {
        let m = map(8, 6, vec![0; 48]);
        let o = overlay(&m, BOTH, 4).unwrap();
        assert_eq!(o.size, [4, 3]);
        assert_eq!(o.uv_scale, [1.0, 1.0]);
    }

    #[test]
    fn a_map_that_cannot_be_drawn_gives_no_overlay() {
        // the data is shorter or longer than width × height
        assert_eq!(overlay(&map(3, 2, vec![R; 5]), BOTH, 16), None);
        assert_eq!(overlay(&map(3, 2, vec![R; 7]), BOTH, 16), None);
        assert_eq!(overlay(&map(3, 2, Vec::new()), BOTH, 1), None);
        // no cells, or no texture side
        assert_eq!(overlay(&map(0, 2, Vec::new()), BOTH, 16), None);
        assert_eq!(overlay(&map(2, 0, Vec::new()), BOTH, 16), None);
        assert_eq!(overlay(&map(2, 2, vec![R; 4]), BOTH, 0), None);
        // a size whose product does not fit is not multiplied blindly
        assert_eq!(
            overlay(&map(u32::MAX, u32::MAX, vec![R; 4]), BOTH, 16),
            None
        );
    }

    #[test]
    fn the_texture_is_made_again_only_for_another_map_or_choice() {
        let made = std::cell::Cell::new(0);
        let mut cache = OverlayCache::<Vec<[u8; 4]>>::default();
        let mut texture = |map: &Arc<ClippingMap>, choice: ClipChoice, max_side: usize| {
            cache
                .texture(map, choice, max_side, |overlay| {
                    made.set(made.get() + 1);
                    pixels(&overlay)
                })
                .map(|(texture, uv_scale)| (texture.clone(), uv_scale))
        };

        let first = Arc::new(map(2, 1, vec![R, DARK]));
        let both = Some((vec![HIGHLIGHT_RGBA, SHADOW_RGBA], [1.0, 1.0]));
        assert_eq!(texture(&first, BOTH, 16), both);
        assert_eq!(made.get(), 1);
        // the same map and choice, frame after frame: not made again
        for _ in 0..3 {
            assert_eq!(texture(&first, BOTH, 16), both);
        }
        // another `Arc` handle of the same map is the same map
        assert_eq!(texture(&Arc::clone(&first), BOTH, 16), both);
        assert_eq!(made.get(), 1);
        // another choice
        let highlights = Some((vec![HIGHLIGHT_RGBA, CLEAR_RGBA], [1.0, 1.0]));
        assert_eq!(texture(&first, HIGHLIGHTS, 16), highlights);
        assert_eq!(texture(&first, HIGHLIGHTS, 16), highlights);
        assert_eq!(made.get(), 2);
        // an equal map in another allocation is another map (the view core made a new one)
        let second = Arc::new(map(2, 1, vec![R, DARK]));
        assert_eq!(texture(&second, HIGHLIGHTS, 16), highlights);
        assert_eq!(texture(&second, HIGHLIGHTS, 16), highlights);
        assert_eq!(made.get(), 3);
        // another largest side joins the cells
        let joined = Some((vec![HIGHLIGHT_RGBA], [1.0, 0.5]));
        assert_eq!(texture(&second, HIGHLIGHTS, 1), joined);
        assert_eq!(texture(&second, HIGHLIGHTS, 1), joined);
        assert_eq!(made.get(), 4);
    }

    #[test]
    fn a_released_texture_is_made_again_when_an_overlay_is_shown() {
        let m = Arc::new(map(1, 1, vec![DARK]));
        let mut cache = OverlayCache::<u32>::default();
        assert!(!cache.is_kept());
        // kept between the frames: the second call does not make it
        assert_eq!(
            cache.texture(&m, SHADOWS, 16, |_| 1),
            Some((&1, [1.0, 1.0]))
        );
        assert_eq!(
            cache.texture(&m, SHADOWS, 16, |_| 2),
            Some((&1, [1.0, 1.0]))
        );
        assert!(cache.is_kept());
        cache.release();
        assert!(!cache.is_kept());
        assert_eq!(
            cache.texture(&m, SHADOWS, 16, |_| 3),
            Some((&3, [1.0, 1.0]))
        );
    }

    #[test]
    fn a_map_that_cannot_be_drawn_is_tried_once() {
        let broken = Arc::new(map(3, 2, vec![R; 5]));
        let mut cache = OverlayCache::<()>::default();
        let mut tried = 0;
        for _ in 0..3 {
            let texture = cache.texture(&broken, BOTH, 16, |_| tried += 1);
            assert!(texture.is_none());
        }
        assert_eq!(tried, 0, "nothing is uploaded for it");
        assert!(cache.is_kept(), "the note that it cannot be drawn is kept");
        // a good map after it is drawn
        let good = Arc::new(map(1, 1, vec![G]));
        assert!(cache.texture(&good, BOTH, 16, |_| tried += 1).is_some());
        assert_eq!(tried, 1);
    }
}
