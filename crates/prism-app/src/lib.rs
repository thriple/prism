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
mod mesh_files;
mod overlay;
pub mod platform;
mod recovery;
pub mod settings;
mod storage;
mod tabs;
mod transfer;
mod viewport;

pub use app::run;
