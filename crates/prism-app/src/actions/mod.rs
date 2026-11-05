//! Actions: one registry, and everything generated from it (menus, dock,
//! context menu, command search, hotkeys, Planned Features).
//!
//! This module is plain logic with no window or GPU, so it is tested directly.

pub mod esc;
pub mod keymap;
pub mod registry;
pub mod search;
mod table;
pub mod table_types;

pub use registry::{Action, Context, Registry, Run};
