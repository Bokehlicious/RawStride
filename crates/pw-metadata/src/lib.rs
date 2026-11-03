//! Metadata handling for PhotoWorkflow.
//!
//! Responsibilities (planned):
//! - A persistent ExifTool process pool (`-stay_open` mode) – never one process per file.
//! - Reading EXIF/IPTC/XMP into typed structures.
//! - Writing ratings, colour labels, captions and keywords to XMP sidecars
//!   (and optionally embedded), compatible with common photo tools.
//! - IPTC templates / stationery pads applied at ingest.
