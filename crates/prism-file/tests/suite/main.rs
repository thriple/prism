//! Every integration test of this crate in one program: one build and
//! link instead of one per file (owner, 2026-11-27: the test builds took
//! too long). Each file is a module, as it was a program.

mod drawing;
mod files;
mod golden;
mod mesh;
mod shell_hull;
mod shell_truck;
mod step;
mod structured;
mod thicken_hull;
mod trimmed;
