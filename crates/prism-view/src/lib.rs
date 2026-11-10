//! The viewport's pure parts (PLAN 4): the camera, display triangulation,
//! the visible scene that drawing and picking share, screen-space picking,
//! the Select tool's clicks and the viewport's colours.
//!
//! No GPU, window or clock: the inference engine and the tools build on
//! these and stay testable and replayable. `prism-render` draws with them.

pub mod camera;
pub mod clicks;
pub mod overlay;
pub mod pick;
pub mod scene;
pub mod style;
pub mod tess;
pub mod turned;
pub mod view_style;

pub use camera::{Camera, Motion, View};
pub use pick::{Hit, Picked};
pub use scene::Scene;
pub use style::Style;
pub use view_style::ViewStyle;
