//! Prism's viewport renderer (PLAN 4, `prism-render`): draws the scene with
//! wgpu. The pure parts it draws from live in `prism-view` and are
//! re-exported here.

pub mod gpu;

pub use prism_view::{Camera, Hit, Motion, Picked, Scene, Style, View};
pub use prism_view::{camera, clicks, pick, scene, style, tess};
