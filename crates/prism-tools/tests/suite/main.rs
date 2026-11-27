//! Every integration test of this crate in one program: one build and
//! link instead of one per file (owner, 2026-11-27: the test builds took
//! too long). Each file is a module, as it was a program.

mod approx;
mod curved;
mod fillet_session;
mod joints;
mod new_loop;
mod owner_cylinder;
mod owner_session_6;
mod owner_session_7;
mod replay_time;
mod session;
mod solids;
mod timing;
mod turned;
mod probe30;
