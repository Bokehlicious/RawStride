//! The focus marking drawn over the image in the preview (ADR 0017 §2.6, U9a, T168): the view
//! core's focus map turned into the overlay's pixels, and the rule for when the overlay's texture
//! is made again. Plain Rust, tested without a window (GR11); `main.rs` uploads the pixels and
//! draws the quad as it does the clipping marks (`clipping.rs`).

use std::sync::Arc;

use pw_viewcore::FocusMap;

use crate::clipping::{self, CLEAR_RGBA, Overlay};

/// The colour of a cell on an edge of high local contrast: solid green (U9-Q3), as RGBA.
pub const FOCUS_RGBA: [u8; 4] = [0, 255, 0, 255];

/// The overlay of `map`, no wider or higher than `max_side` (the GPU's largest texture side): one
/// pixel per cell, [`FOCUS_RGBA`] where [`FocusMap::EDGE`] is set and [`CLEAR_RGBA`] elsewhere. A
/// larger map is made smaller by joining 2 × 2 cells until it fits (a joined cell is an edge when
/// any of its cells is); a side with an odd number of cells then gives a joined grid that reaches
/// past the map by half a joined cell, and `uv_scale` is `w / (2 · ceil(w / 2))` per joining for
/// it, the rule of [`clipping::overlay`]. `None` (nothing is drawn) for a map without cells, a map
/// whose `data` length is not `width × height`, or a `max_side` of 0.
pub fn focus_overlay(map: &FocusMap, max_side: usize) -> Option<Overlay> {
    let mut width = usize::try_from(map.width).ok()?;
    let mut height = usize::try_from(map.height).ok()?;
    if width == 0 || height == 0 || max_side == 0 || width.checked_mul(height)? != map.data.len() {
        return None;
    }
    let mut uv_scale = [1.0f64; 2];
    let mut joined: Option<Vec<u8>> = None;
    while width > max_side || height > max_side {
        let (joined_width, data) = clipping::join(width, joined.as_deref().unwrap_or(&map.data));
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
        rgba.extend_from_slice(if bits & FocusMap::EDGE != 0 {
            &FOCUS_RGBA
        } else {
            &CLEAR_RGBA
        });
    }
    Some(Overlay {
        size: [width, height],
        rgba,
        uv_scale: uv_scale.map(|scale| scale as f32),
    })
}

/// What the kept texture was made from.
struct Kept<T> {
    map: Arc<FocusMap>,
    max_side: usize,
    /// `None` when the map cannot be drawn, so it is not tried again every frame.
    texture: Option<(T, [f32; 2])>,
}

/// The focus marking's one texture (`T`; an egui texture in the program), made once per map: again
/// only when the map (by `Arc::ptr_eq`) or the largest texture side changes, and dropped by
/// [`release`](Self::release) when the marking is not shown.
pub struct FocusCache<T> {
    kept: Option<Kept<T>>,
}

impl<T> std::fmt::Debug for FocusCache<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FocusCache")
            .field("kept", &self.kept.as_ref().map(|kept| &kept.map))
            .finish()
    }
}

impl<T> Default for FocusCache<T> {
    fn default() -> Self {
        FocusCache { kept: None }
    }
}

impl<T> FocusCache<T> {
    /// The texture of `map` and its [`Overlay::uv_scale`]; `make` uploads the pixels and is called
    /// only when the kept texture is not of this map and `max_side`. The texture kept before is
    /// dropped first, so there is one at a time. `None` when the map cannot be drawn
    /// ([`focus_overlay`]).
    pub fn texture(
        &mut self,
        map: &Arc<FocusMap>,
        max_side: usize,
        make: impl FnOnce(Overlay) -> T,
    ) -> Option<(&T, [f32; 2])> {
        let same = self
            .kept
            .as_ref()
            .is_some_and(|kept| Arc::ptr_eq(&kept.map, map) && kept.max_side == max_side);
        if !same {
            self.kept = None;
            let texture = focus_overlay(map, max_side).map(|overlay| {
                let uv_scale = overlay.uv_scale;
                (make(overlay), uv_scale)
            });
            self.kept = Some(Kept {
                map: Arc::clone(map),
                max_side,
                texture,
            });
        }
        let (texture, uv_scale) = self.kept.as_ref()?.texture.as_ref()?;
        Some((texture, *uv_scale))
    }

    /// Drops the texture: the marking is not shown.
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

    const EDGE: u8 = FocusMap::EDGE;

    fn map(width: u32, height: u32, data: Vec<u8>) -> FocusMap {
        FocusMap {
            cell: FocusMap::HALF_CELL,
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
    fn an_edge_cell_is_green_and_the_others_transparent() {
        assert_eq!(FOCUS_RGBA, [0, 255, 0, 255]);
        assert_eq!(CLEAR_RGBA, [0, 0, 0, 0]);
        let m = map(3, 2, vec![EDGE, 0, 0, 0, EDGE, EDGE]);
        let o = focus_overlay(&m, 16).unwrap();
        assert_eq!(o.size, [3, 2]);
        assert_eq!(o.uv_scale, [1.0, 1.0]);
        let (green, clear) = (FOCUS_RGBA, CLEAR_RGBA);
        assert_eq!(pixels(&o), [green, clear, clear, clear, green, green]);
    }

    #[test]
    fn joining_marks_a_joined_cell_when_any_of_its_cells_is_an_edge_and_fits_the_side() {
        #[rustfmt::skip]
        let m = map(4, 4, vec![
            EDGE, 0,   0, 0,
            0,    0,   0, 0,
            0,    0,   0, 0,
            0,    0,   0, EDGE,
        ]);
        let (green, clear) = (FOCUS_RGBA, CLEAR_RGBA);
        // 4 × 4 does not fit a side of 2: joined once
        let o = focus_overlay(&m, 2).unwrap();
        assert_eq!(o.size, [2, 2]);
        assert_eq!(o.uv_scale, [1.0, 1.0]);
        assert_eq!(pixels(&o), [green, clear, clear, green]);
        // a side of 1: joined twice
        let o = focus_overlay(&m, 1).unwrap();
        assert_eq!(o.size, [1, 1]);
        assert_eq!(pixels(&o), [green]);
        // an empty map stays empty when joined
        let o = focus_overlay(&map(4, 4, vec![0; 16]), 1).unwrap();
        assert_eq!(pixels(&o), [clear]);
        // every largest side is kept to, also when only one side is too long
        let mut data = vec![0; 18];
        data[8] = EDGE; // the last cell of the first row
        let long = map(9, 2, data);
        for max_side in 1..=9 {
            let o = focus_overlay(&long, max_side).unwrap();
            assert!(o.size.iter().all(|&side| side <= max_side), "{max_side}");
            assert_eq!(o.rgba.len(), o.size[0] * o.size[1] * 4, "{max_side}");
        }
        let o = focus_overlay(&long, 4).unwrap();
        assert_eq!(o.size, [3, 1]);
        assert_eq!(
            pixels(&o),
            [clear, clear, green],
            "the mark stays at its end"
        );
    }

    #[test]
    fn an_odd_number_of_cells_gives_the_uv_scale_that_keeps_the_marks_in_place() {
        // 5 × 3 cells joined once: 3 × 2 joined cells, which reach to 6 and 4 cells
        let m = map(5, 3, vec![0; 15]);
        let once = focus_overlay(&m, 4).unwrap();
        assert_eq!(once.size, [3, 2]);
        assert!((once.uv_scale[0] - 5.0 / 6.0).abs() < 1e-6);
        assert!((once.uv_scale[1] - 3.0 / 4.0).abs() < 1e-6);
        // joined twice: 2 × 1 joined cells of 4 × 4, which reach to 8 and 4 cells
        let twice = focus_overlay(&m, 2).unwrap();
        assert_eq!(twice.size, [2, 1]);
        assert!((twice.uv_scale[0] - 5.0 / 8.0).abs() < 1e-6);
        assert!((twice.uv_scale[1] - 3.0 / 4.0).abs() < 1e-6);
        // the right bottom corner of the drawn image stays on the map's
        let corners = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let uv = clipping::overlay_uv(corners, once.uv_scale);
        assert!((uv[2][0] * 3.0 * 2.0 - 5.0).abs() < 1e-5);
        assert!((uv[2][1] * 2.0 * 2.0 - 3.0).abs() < 1e-5);
        // an even side keeps its coordinates
        let even = focus_overlay(&map(8, 6, vec![0; 48]), 4).unwrap();
        assert_eq!(even.size, [4, 3]);
        assert_eq!(even.uv_scale, [1.0, 1.0]);
    }

    #[test]
    fn a_map_that_cannot_be_drawn_gives_no_overlay() {
        // the data is shorter or longer than width × height
        assert_eq!(focus_overlay(&map(3, 2, vec![EDGE; 5]), 16), None);
        assert_eq!(focus_overlay(&map(3, 2, vec![EDGE; 7]), 16), None);
        // no cells, or no texture side
        assert_eq!(focus_overlay(&map(0, 2, Vec::new()), 16), None);
        assert_eq!(focus_overlay(&map(2, 0, Vec::new()), 16), None);
        assert_eq!(focus_overlay(&map(2, 2, vec![EDGE; 4]), 0), None);
        // a size whose product does not fit is not multiplied blindly
        assert_eq!(
            focus_overlay(&map(u32::MAX, u32::MAX, vec![EDGE; 4]), 16),
            None
        );
    }

    #[test]
    fn the_texture_is_made_again_only_for_another_map_or_side() {
        let made = std::cell::Cell::new(0);
        let mut cache = FocusCache::<Vec<[u8; 4]>>::default();
        let mut texture = |map: &Arc<FocusMap>, max_side: usize| {
            cache
                .texture(map, max_side, |overlay| {
                    made.set(made.get() + 1);
                    pixels(&overlay)
                })
                .map(|(texture, uv_scale)| (texture.clone(), uv_scale))
        };

        let first = Arc::new(map(2, 1, vec![EDGE, 0]));
        let shown = Some((vec![FOCUS_RGBA, CLEAR_RGBA], [1.0, 1.0]));
        assert_eq!(texture(&first, 16), shown);
        assert_eq!(made.get(), 1);
        // the same map and side, frame after frame, also through another `Arc` handle
        for _ in 0..3 {
            assert_eq!(texture(&first, 16), shown);
        }
        assert_eq!(texture(&Arc::clone(&first), 16), shown);
        assert_eq!(made.get(), 1);
        // an equal map in another allocation is a new map (the view core made a new one)
        let second = Arc::new(map(2, 1, vec![EDGE, 0]));
        assert_eq!(texture(&second, 16), shown);
        assert_eq!(texture(&second, 16), shown);
        assert_eq!(made.get(), 2);
        // another largest side joins the cells
        let joined = Some((vec![FOCUS_RGBA], [1.0, 0.5]));
        assert_eq!(texture(&second, 1), joined);
        assert_eq!(texture(&second, 1), joined);
        assert_eq!(made.get(), 3);
    }

    #[test]
    fn a_released_texture_is_made_again_and_a_broken_map_is_tried_once() {
        let m = Arc::new(map(1, 1, vec![EDGE]));
        let mut cache = FocusCache::<u32>::default();
        assert!(!cache.is_kept());
        assert_eq!(cache.texture(&m, 16, |_| 1), Some((&1, [1.0, 1.0])));
        assert_eq!(cache.texture(&m, 16, |_| 2), Some((&1, [1.0, 1.0])));
        assert!(cache.is_kept());
        cache.release();
        assert!(!cache.is_kept());
        assert_eq!(cache.texture(&m, 16, |_| 3), Some((&3, [1.0, 1.0])));

        let broken = Arc::new(map(3, 2, vec![EDGE; 5]));
        let mut tried = 0;
        for _ in 0..3 {
            assert!(
                cache
                    .texture(&broken, 16, |_| {
                        tried += 1;
                        4
                    })
                    .is_none()
            );
        }
        assert_eq!(tried, 0, "nothing is uploaded for it");
        assert!(cache.is_kept(), "the note that it cannot be drawn is kept");
    }
}
