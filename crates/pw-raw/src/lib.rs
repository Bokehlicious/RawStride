//! RAW decoding for PhotoWorkflow.
//!
//! Responsibilities (planned):
//! - Fast path: locate and return embedded JPEG previews without a full demosaic –
//!   this is what culling speed depends on. Development formats: Sony ARW (A1, A7R IVA),
//!   Panasonic RW2 and DNG; long term, most cameras supported by LibRaw.
//! - Slow path: full decode through LibRaw (FFI, statically linked via vcpkg).
//! - Orientation and basic capture info needed before metadata is loaded.
//!
//! All `unsafe` FFI lives in a single private module with documented invariants.
