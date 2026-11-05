//! Prism's application shell: a window, a GPU surface and a debug overlay.
//!
//! The same code runs natively (Windows, macOS) and on the web.

mod app;
pub mod frame_stats;
mod gpu;
pub mod input_log;
mod overlay;
mod platform;
pub mod settings;

pub use app::run;
