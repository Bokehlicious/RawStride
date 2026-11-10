//! Pixel helpers for the TIFF and RAW views. The TIFF decoder, the sample conversion to 8-bit RGB
//! (ADR 0015 D1) and the target size `fit` and area-averaging `downscale` (ADR 0015 D3) live in the
//! shared crate `pw-tiff`; they are re-exported here so the views keep their `imaging::` paths.

pub use pw_tiff::{Rgb8, downscale, fit};
