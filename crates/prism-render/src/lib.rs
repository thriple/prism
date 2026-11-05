//! Prism's viewport (PLAN 4, `prism-render`).
//!
//! The pure parts (camera, display triangulation, the visible scene,
//! picking, the Select tool's clicks and the viewport style) use no GPU and
//! no clock, and are tested on their own. `gpu` draws with wgpu.

pub mod camera;
pub mod clicks;
pub mod gpu;
pub mod pick;
pub mod scene;
pub mod style;
pub mod tess;

pub use camera::{Camera, Motion, View};
pub use pick::{Hit, Picked};
pub use scene::Scene;
pub use style::Style;
