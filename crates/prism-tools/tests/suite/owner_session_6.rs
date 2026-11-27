//! The owner's session 6 (2026-11-24), building a door: an arc drawn with
//! both ends snapped onto the door panel's own bottom edge (a half circle,
//! bulging away from the panel) made a small flap glued to that edge,
//! which now had three faces on it. Pulling the flap up to the panel's own
//! height, so its new edge landed exactly on the panel's own top corners
//! (`transform::weld_new`), came out with a wall that used an edge far
//! from it, not on its own surface: silently corrupted geometry, read by
//! the owner as "inside out" faces, after the pull itself seemed not to
//! reach that high at all. `prism_ops::pushpull::push_pull` now checks its
//! own bookkeeping before returning and backs out rather than hand back a
//! broken model (`crates/prism-ops/tests/pushpull_feedback.rs` has the
//! minimal repro); this replays the owner's actual recording end to end.

#[test]
fn the_recorded_pull_never_leaves_a_face_using_an_edge_off_its_surface() {
    let mut r: prism_tools::Recording =
        serde_json::from_str(include_str!("../data/owner-session-6-arc-extrude.json"))
            .expect("recording");
    // Right after the push/pull's second click commits it (event 789), and
    // well before the owner's later undos take it back out.
    r.events.truncate(791);
    let s = prism_tools::Session::replay(&r);
    let m = s.work_model();
    assert_eq!(
        prism_ops::check::problems(m),
        Vec::<String>::new(),
        "model is clean right after the recorded pull commits"
    );
}
