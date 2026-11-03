//! The SQLite catalog.
//!
//! Responsibilities (planned):
//! - Schema and forward-only migrations.
//! - Assets, folders/volumes, ratings, labels, tags, ingest sessions.
//! - WAL mode, a single writer task and pooled readers.
//!
//! The catalog is an index: XMP sidecars remain the portable source of truth
//! for user-facing metadata.
