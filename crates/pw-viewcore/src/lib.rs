//! The view core of the egui loupe (ADR 0009, built on the P7 prototype; ADR 0008 §2 and §12).
//!
//! - **View model:** the current image, the zoom level (fit, or a free scale from half of fit up to
//!   800 %), and the position as a relative point of the upright image (V6).
//! - **Views:** fit and small levels from a half-size render; the exact visible region at 100 % (the
//!   zoom target, V7); a ring of twice that for panning; and free levels between 50 % and 100 % as
//!   a region scaled down on the worker. A view is never drawn upscaled below 100 % (V2).
//! - **Plan (strategy p5):** the current view first; after a 150 ms dwell the other view, then +1
//!   and +2 in the direction of travel, the ring, and −1 behind. Free levels are not prefetched.
//! - **Budgets (V8):** LibRaw sessions under the core budget and finished views under the texture
//!   budget, counted separately (ADR 0008 §18). Prefetch starts only if it fits; the current image
//!   and the view on screen are never evicted.
//! - **Swap rule (V2):** the previous view stays until the new one is complete; the neutral
//!   indicator after 1 s. **Damaged files (G3)** show an error, never an earlier image.
//!
//! - **Display mode (G4):** RAW (LibRaw, PPG) or JPEG (the largest embedded JPEG), switched only by
//!   the user. Zoom and position stay; 100 % is 100 % of the source shown. A plain JPEG file and a
//!   TIFF file (decoded once with `pw-tiff` under the core budget, ADR 0015) are shown as
//!   themselves in both modes. A file without an embedded JPEG says so in JPEG mode, and a
//!   damaged RAW keeps its intact embedded JPEG (G3).
//!
//! No GUI types, no files listed here and no FFI outside the `native` feature: the decoders are a
//! [`Backend`], and the presentation (`pw-render`, `pw-loupe`) uploads and draws.

mod backend;
mod core;
pub mod geometry;
#[cfg(feature = "native")]
pub mod native;
pub mod scale;
mod state;

pub use crate::backend::{
    Backend, DecodeError, ImageStatistics, Rgb, Source, Statistics, StatisticsOrigin,
};
pub use crate::core::{Releaser, ViewCore};
pub use crate::geometry::{ImageSize, Rect};
pub use crate::state::{
    Config, Draw, Frame, HALF_DENSITY, MAX_SCALE, Record, Resident, Stats, Status, ViewKind,
    ViewOut, ZOOM_STEP, Zoom, fnv, is_plain_jpeg, is_shown_as_itself,
};
