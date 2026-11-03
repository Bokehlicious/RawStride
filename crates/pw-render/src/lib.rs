//! GPU-accelerated image presentation.
//!
//! Responsibilities (planned):
//! - Texture upload and display of previews via wgpu (DX12 / Vulkan on Intel Arc).
//! - Zoom/pan at 100 % with pre-fetched neighbours for instant culling.
//! - Colour management for the display (ICC profile from the OS).
//!
//! How rendering integrates with the Tauri webview is an open design question,
//! tracked in docs/adr.
