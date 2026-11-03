//! Shared types, error handling and configuration used by every PhotoWorkflow crate.
//!
//! Responsibilities (planned):
//! - Common identifiers (asset id, volume id, session id)
//! - The workspace-wide error type
//! - Application configuration and well-known paths
//!
//! This crate must stay dependency-light: no I/O, no FFI, no async runtime.
