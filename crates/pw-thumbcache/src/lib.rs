//! Thumbnail extraction, background generation and an in-memory cache.

mod extract;
mod worker;

pub use extract::{ThumbError, choose_preview, choose_scale, orient, thumbnail};
pub use pw_libjpeg::Rgb8;
pub use worker::{ThumbSource, ThumbWorker, ThumbnailEvent, ThumbnailRequest};
