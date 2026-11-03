//! RAW decoding for PhotoWorkflow.
//!
//! Responsibilities (planned):
//! - Fast path: locate and return the embedded JPEG previews in Sony ARW files
//!   (A1, A7R IV) without a full demosaic – this is what culling speed depends on.
//! - Slow path: full decode through LibRaw (FFI, statically linked via vcpkg).
//! - Orientation and basic capture info needed before metadata is loaded.
//!
//! All `unsafe` FFI lives in a single private module with documented invariants.
