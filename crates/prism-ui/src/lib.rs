//! Prism's theme layer: the Motion Kit's dark Soft glass look, rebuilt in Rust
//! on egui (`docs/UI-STYLE.md`). Every panel and control in the app uses it.
//!
//! - [`theme`]: colours, fonts, sizes and depths. The only place they are defined.
//! - [`spring`]: the kit's springs, pure and deterministic.
//! - [`emboss`]: the four-shadow raised and pressed surfaces.
//! - [`aurora`]: the still aurora behind glass cutouts.
//! - [`widgets`]: soft button, toggle, slider, dropdown, panel and groove.

pub mod anim;
pub mod aurora;
pub mod emboss;
pub mod fonts;
pub mod glass;
pub mod spring;
pub mod theme;
pub mod widgets;

pub use anim::{MotionSettings, motion, set_motion};
pub use aurora::Aurora;
