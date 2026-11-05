//! Prism's application shell: a window, a GPU surface and a debug overlay.
//!
//! The same code runs natively (Windows, macOS) and on the web.

pub mod actions;
mod app;
pub mod chrome;
pub mod frame_stats;
mod gpu;
pub mod input_log;
mod overlay;
pub mod platform;
pub mod settings;
mod storage;

pub use app::run;
