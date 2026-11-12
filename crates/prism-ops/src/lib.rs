//! Prism's modelling operations, on a [`prism_topo::Model`]:
//!
//! - [`sticky`]: points and segments that snap, split and close faces, and
//!   coplanar faces that never overlap (PLAN 1.9).
//! - [`pushpull`]: Push/Pull of a face.
//! - [`transform`]: move, rotate and scale, keeping faces planar, then weld.
//! - [`select`]: erase, duplicate, and what is connected.
//! - [`check`]: the rules every model must satisfy, for tests.
//!
//! All of it is pure: no clock, no randomness, ordered iteration.

pub mod approx;
pub mod check;
pub mod crossing;
pub mod cut;
pub mod fillet;
pub mod follow;
pub mod health;
pub mod intersect;
mod meet;
pub mod offset;
pub mod pushpull;
pub mod radius;
pub mod reach;
mod regions;
pub mod round;
pub mod select;
pub mod settle;
mod shell;
pub mod solid;
pub mod sticky;
pub mod transform;

pub use pushpull::push_pull;
pub use select::{Selection, connected, duplicate, remove};
pub use sticky::{
    add_arc, add_circle, add_pie, add_polygon, add_rect, add_segment, add_spline, face_at,
    insert_point, vertex_at,
};
