//! Prism's numeric basics: f64 vectors, the one relative tolerance, exact
//! predicates, planes and 2D projection.
//!
//! Nothing here reads a clock or anything outside its arguments, so results
//! are the same on every run and every platform.

pub mod plane;
pub mod predicates;
pub mod sun;
pub mod tol;
pub mod units;

pub use glam::{DAffine3, DMat3, DMat4, DQuat, DVec2, DVec3};
pub use plane::{Plane, newell};
pub use tol::Tol;
pub use units::{DisplayUnits, LengthUnit};
