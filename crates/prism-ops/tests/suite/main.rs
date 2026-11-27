//! Every integration test of this crate in one program: one build and
//! link instead of one per file (owner, 2026-11-27: the test builds took
//! too long). Each file is a module, as it was a program.

mod bands;
// The STEP test helpers, shared by two modules.
#[path = "../common/step.rs"]
mod common;

mod boolean;
mod booleans_curved;
mod bridge;
mod cage;
mod check_rules;
mod circles;
mod convert;
mod curved;
mod cut;
mod deform;
mod draft;
mod follow;
mod follow_arcs;
mod follow_fallback;
mod follow_overlap;
mod freeform_cross;
mod golden;
mod health;
mod hole;
mod intersect;
mod loft;
mod offset;
mod offset_refusals;
mod operations;
mod owner_wall;
mod primitives;
mod properties;
mod pull_beside_nurbs;
mod purity;
mod pushpull_feedback;
mod round;
mod rounds;
mod rule_order;
mod shelling;
mod sizes_curved;
mod solids;
mod splines;
mod step;
mod sticky;
mod sticky_curved;
mod stops_curved;
mod sweep;
mod taper;
mod texture;
mod thresholds;
mod trim;
mod trimmed;
mod validate;
mod weld;
mod wheel_wells;
