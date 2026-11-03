//! Ingest from memory cards.
//!
//! Responsibilities (planned):
//! - Detect removable volumes (CFexpress A / SD) and enumerate DCIM content.
//! - Copy to a primary and optional backup destination, with checksum verification.
//! - Rename by template, apply IPTC templates, register files in the catalog.
//! - Never modify or delete files on the source card.
