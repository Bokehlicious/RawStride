//! Image presentation in egui (ADR 0009 §4.1, step S4; GUI_REQUIREMENTS GR3, GR4).
//!
//! `pw-viewcore` decides what to show; this crate turns its finished views into egui textures and
//! draws the current one. Nothing is decoded here and nothing waits: an upload is a copy into egui's
//! texture manager, and the GPU work happens in eframe's renderer (glow).
//!
//! Once per frame:
//!
//! 1. [`Textures::sync`] uploads the views the core has finished (within the texture budget, which
//!    the core keeps) and releases the textures the core has dropped;
//! 2. [`Textures::paint`] draws what the core's [`Frame`] says: one textured quad, oriented by its
//!    per-corner texture coordinates.
//!
//! The geometry is plain functions ([`mesh`], [`view_px`]), so it is tested without a GPU (GR11).

use std::collections::HashMap;
use std::time::Instant;

use egui::epaint::Vertex;
use egui::{
    Color32, ColorImage, Mesh, Pos2, Rect, Shape, TextureFilter, TextureId, TextureOptions,
};
use pw_viewcore::{Backend, Draw, Frame, ViewCore, ViewKind, ViewOut};

/// Size of the image area in physical pixels, for [`ViewCore::frame`].
pub fn view_px(rect: Rect, pixels_per_point: f32) -> [u32; 2] {
    let ppp = sane_ppp(pixels_per_point);
    let side = |v: f32| -> u32 {
        let px = (v * ppp).round();
        if px.is_finite() && px >= 1.0 {
            px.min(u32::MAX as f32) as u32
        } else {
            1
        }
    };
    [side(rect.width()), side(rect.height())]
}

fn sane_ppp(ppp: f32) -> f32 {
    if ppp.is_finite() && ppp > 0.0 {
        ppp
    } else {
        1.0
    }
}

/// Whether a view is uploaded with mipmaps. The half-size view may be drawn reduced (during a zoom
/// gesture below 50 %, before the reduced view is made); mipmaps keep that free of aliasing. The
/// other views, the reduced and the fine view below 50 % included (ADR 0027 W1, W2), are made for
/// the screen's resolution and are drawn at about 1:1 or magnified.
pub fn mipmapped(kind: ViewKind) -> bool {
    kind == ViewKind::Half
}

/// The texture options for a view. Magnified pixels of the views at 50 % to 100 % and above stay
/// sharp (nearest-neighbour above 100 %, "real pixels"); the half-size, the reduced and the fine
/// view are magnified only as a zoom gesture's brief stand-in, so they are filtered linearly,
/// which is not blocky (ADR 0027 W1-Q9, W2). Reduced pixels are filtered linearly. Mipmaps for the
/// half-size view (egui_glow supports them; glow is the renderer, GUI_REQUIREMENTS §7).
pub fn texture_options(kind: ViewKind) -> TextureOptions {
    let mut o = TextureOptions::LINEAR;
    o.magnification = if matches!(kind, ViewKind::Half | ViewKind::Reduced | ViewKind::Fine) {
        TextureFilter::Linear
    } else {
        TextureFilter::Nearest
    };
    o.minification = TextureFilter::Linear;
    o.mipmap_mode = if mipmapped(kind) {
        Some(TextureFilter::Linear)
    } else {
        None
    };
    o
}

/// The texture memory of a `w × h` view: RGBA, plus a third for the mipmap chain.
pub fn texture_bytes(w: u32, h: u32, kind: ViewKind) -> u64 {
    let base = u64::from(w) * u64::from(h) * 4;
    if mipmapped(kind) {
        base + base.div_ceil(3)
    } else {
        base
    }
}

/// The quad for `draw` in an image area whose top-left corner is `origin` (points), at
/// `pixels_per_point`. The core works in physical pixels; egui draws in points.
pub fn mesh(draw: &Draw, origin: Pos2, pixels_per_point: f32, texture: TextureId) -> Mesh {
    let ppp = sane_ppp(pixels_per_point);
    let [x0, y0, x1, y1] = draw.dest.map(|v| v / ppp);
    let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    let mut mesh = Mesh::with_texture(texture);
    for ((x, y), uv) in corners.into_iter().zip(draw.uv) {
        mesh.vertices.push(Vertex {
            pos: Pos2::new(origin.x + x, origin.y + y),
            uv: Pos2::new(uv[0], uv[1]),
            color: Color32::WHITE,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    mesh
}

/// What one [`Textures::sync`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Synced {
    pub uploaded: usize,
    /// Views the core no longer wanted (an old folder, a free-zoom level passed by).
    pub declined: usize,
    /// Views larger than the GPU allows.
    pub rejected: usize,
    pub released: usize,
    /// Time spent converting and handing views to egui, in milliseconds.
    pub upload_ms: f64,
}

/// The textures of the core's resident views.
pub struct Textures {
    map: HashMap<u64, egui::TextureHandle>,
}

impl std::fmt::Debug for Textures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Textures")
            .field("count", &self.map.len())
            .finish()
    }
}

impl Default for Textures {
    fn default() -> Self {
        Self::new()
    }
}

impl Textures {
    pub fn new() -> Textures {
        Textures {
            map: HashMap::new(),
        }
    }

    /// Number of textures held.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The egui texture of a core texture id.
    pub fn texture(&self, id: u64) -> Option<TextureId> {
        self.map.get(&id).map(|t| t.id())
    }

    /// Uploads finished views and releases dropped ones. `max_side` is the GPU's largest texture
    /// side (`ctx.input(|i| i.max_texture_side)`).
    pub fn sync<B: Backend>(
        &mut self,
        ctx: &egui::Context,
        core: &ViewCore<B>,
        max_side: usize,
    ) -> Synced {
        let mut s = Synced::default();
        for view in core.take_views() {
            self.upload(ctx, core, &view, max_side, &mut s);
        }
        for id in core.take_dropped() {
            if self.map.remove(&id).is_some() {
                s.released += 1;
            }
        }
        s
    }

    fn upload<B: Backend>(
        &mut self,
        ctx: &egui::Context,
        core: &ViewCore<B>,
        view: &ViewOut,
        max_side: usize,
        s: &mut Synced,
    ) {
        let (w, h) = (view.image.width as usize, view.image.height as usize);
        if w == 0 || h == 0 || w > max_side || h > max_side || !view.image.is_consistent() {
            core.reject(
                view,
                &format!("the view ({w}×{h}) is larger than the GPU allows ({max_side})"),
            );
            s.rejected += 1;
            return;
        }
        let bytes = texture_bytes(view.image.width, view.image.height, view.kind);
        let Some(id) = core.accept(view, bytes) else {
            s.declined += 1;
            return;
        };
        let t = Instant::now();
        let image = ColorImage::from_rgb([w, h], &view.image.data);
        let name = format!("view-{id}-{}-{}", view.pos, view.kind.name());
        let handle = ctx.load_texture(name, image, texture_options(view.kind));
        s.upload_ms += t.elapsed().as_secs_f64() * 1000.0;
        self.map.insert(id, handle);
        s.uploaded += 1;
    }

    /// Draws the frame's view into `rect` (the image area). Returns whether something was drawn.
    /// The background, the status line and the indicator are the caller's.
    pub fn paint(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        pixels_per_point: f32,
        frame: &Frame,
    ) -> bool {
        let Some(draw) = frame.draw else {
            return false;
        };
        let Some(texture) = self.texture(draw.id) else {
            return false;
        };
        painter.add(Shape::mesh(mesh(
            &draw,
            rect.min,
            pixels_per_point,
            texture,
        )));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw() -> Draw {
        Draw {
            id: 7,
            dest: [100.0, 50.0, 1100.0, 750.0],
            uv: [[1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]],
            nearest: false,
        }
    }

    #[test]
    fn view_size_is_in_physical_pixels() {
        let r = Rect::from_min_size(Pos2::new(0.0, 0.0), egui::vec2(960.0, 540.0));
        assert_eq!(view_px(r, 2.0), [1920, 1080]);
        assert_eq!(view_px(r, 1.25), [1200, 675]);
        assert_eq!(view_px(r, f32::NAN), [960, 540]);
        let empty = Rect::from_min_size(Pos2::new(0.0, 0.0), egui::vec2(0.0, -5.0));
        assert_eq!(view_px(empty, 1.0), [1, 1]);
    }

    #[test]
    fn the_quad_is_placed_in_points_with_the_corners_uvs() {
        let m = mesh(&draw(), Pos2::new(10.0, 20.0), 2.0, TextureId::Managed(3));
        assert_eq!(m.texture_id, TextureId::Managed(3));
        assert_eq!(m.indices, vec![0, 1, 2, 0, 2, 3]);
        let pos: Vec<(f32, f32)> = m.vertices.iter().map(|v| (v.pos.x, v.pos.y)).collect();
        assert_eq!(
            pos,
            vec![(60.0, 45.0), (560.0, 45.0), (560.0, 395.0), (60.0, 395.0)]
        );
        let uv: Vec<[f32; 2]> = m.vertices.iter().map(|v| [v.uv.x, v.uv.y]).collect();
        assert_eq!(uv.as_slice(), draw().uv.as_slice());
        assert!(m.vertices.iter().all(|v| v.color == Color32::WHITE));
    }

    #[test]
    fn magnified_pixels_stay_sharp_and_the_half_view_is_mipmapped() {
        for kind in [
            ViewKind::Half,
            ViewKind::Zoom,
            ViewKind::Ring,
            ViewKind::Scaled,
            ViewKind::Reduced,
            ViewKind::Fine,
        ] {
            let o = texture_options(kind);
            // the half-size, the reduced and the fine view are magnified only as a gesture's
            // stand-in (ADR 0027 W1-Q9, W2): linearly; the others keep sharp pixels
            let magnification =
                if matches!(kind, ViewKind::Half | ViewKind::Reduced | ViewKind::Fine) {
                    TextureFilter::Linear
                } else {
                    TextureFilter::Nearest
                };
            assert_eq!(o.magnification, magnification, "{kind:?}");
            assert_eq!(o.minification, TextureFilter::Linear);
            assert_eq!(o.mipmap_mode.is_some(), kind == ViewKind::Half);
        }
        assert!(!mipmapped(ViewKind::Reduced));
        assert!(!mipmapped(ViewKind::Fine));
        assert_eq!(texture_bytes(100, 100, ViewKind::Reduced), 40_000);
        assert_eq!(texture_bytes(100, 100, ViewKind::Fine), 40_000);
        assert_eq!(texture_bytes(100, 100, ViewKind::Zoom), 40_000);
        assert_eq!(texture_bytes(100, 100, ViewKind::Half), 53_334);
    }
}
