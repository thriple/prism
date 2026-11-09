//! Prism's application shell: a window, a GPU surface and a debug overlay.
//!
//! The same code runs natively (Windows, macOS) and on the web.

pub mod actions;
mod app;
pub mod chrome;
mod doc_files;
mod files;
pub mod frame_stats;
mod gpu;
pub mod input_log;
pub mod mcp;
// The web build has one page per Prism; only some of this is used there.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
mod instance;
mod mesh_files;
mod overlay;
pub mod platform;
mod recovery;
pub mod settings;
mod storage;
mod tabs;
pub mod toolbars;
mod transfer;
pub mod update;
mod viewport;

pub use app::run;
