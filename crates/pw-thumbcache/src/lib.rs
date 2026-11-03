//! Thumbnail and preview cache.
//!
//! Responsibilities (planned):
//! - Content-addressed cache of thumbnails and screen-sized previews.
//! - Background generation with priority for what is currently visible.
//! - Size-bounded eviction; cache location configurable (default on NVMe).
