//! Prism's tools (PLAN 4, `prism-tools`): Select, Line and Rectangle, the
//! measurement box's units, the test scenes, and the viewport session that
//! drives them from a stream of input events. Pure: no window, GPU or clock.

pub mod scenes;
pub mod select;
pub mod session;
pub mod units;

pub use session::{
    Button, EntityInfo, Event, HealthRow, Input, Key, OutlineRow, Recording, Response, Session,
    TagOp,
};
