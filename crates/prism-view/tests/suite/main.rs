//! Every integration test of this crate in one program: one build and
//! link instead of one per file (owner, 2026-11-27: the test builds took
//! too long). Each file is a module, as it was a program.

mod corner;
mod curved;
mod halves;
mod inside;
mod outline;
mod placed;
mod reversed;
mod textures;
mod trimmed;
