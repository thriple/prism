//! Every integration test of this crate in one program: one build and
//! link instead of one per file (owner, 2026-11-27: the test builds took
//! too long). Each file is a module, as it was a program.

mod arc_rules;
mod booleans;
mod clipboard;
mod closing;
mod dimensions;
mod group_axes;
mod groups;
mod images;
mod joints;
mod live_features;
mod materials;
mod model_axes;
mod round_edges;
mod sections;
mod shell;
mod text;
mod tiny_pushpull;
mod undo_everything;
mod work_planes;
