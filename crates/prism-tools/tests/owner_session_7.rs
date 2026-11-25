//! The owner's session 7 (2026-11-25), a car's interior shell: the side
//! face was pushed across (Ctrl, a new face) to where a wheel well had
//! already been pulled. Its circle's arcs swept onto arcs already there
//! (centres a few 1e-15 m apart), and the weld kept both: zero-width
//! "lens" faces between each pair and the wheel well's strip twice, back
//! to back. `transform::apply_weld` now takes an arc landing on the same
//! piece of the same circle as that edge, and `settle::dedupe_faces`
//! keeps one curved face per place.

fn replay_to(n: usize) -> prism_tools::Session {
    let mut r: prism_tools::Recording =
        serde_json::from_str(include_str!("data/owner-session-7-wheel-wells.json"))
            .expect("recording");
    r.events.truncate(n);
    prism_tools::Session::replay(&r)
}

#[test]
fn the_side_pushed_onto_a_pulled_wheel_well_leaves_one_face_per_place() {
    // Right after the side face's push commits (event 735).
    let s = replay_to(736);
    let m = s.work_model();
    assert_eq!(s.document().undo_name(), Some("Push/Pull"));
    assert_eq!(prism_ops::check::problems(m), Vec::<String>::new());
    // No two arcs join the same two points along the same circle.
    let arcs: Vec<_> = m
        .edges()
        .filter(|(_, e)| matches!(e.curve, prism_geom::Curve::Arc { .. }))
        .collect();
    for (i, (_, a)) in arcs.iter().enumerate() {
        for (_, b) in &arcs[i + 1..] {
            let same_ends = (a.a, a.b) == (b.a, b.b) || (a.a, a.b) == (b.b, b.a);
            if same_ends {
                let (pa, pb) = (m.vertex(a.a), m.vertex(a.b));
                let mid_a = a.curve.point(pa, pb, 0.5);
                let mid_b = b.curve.point(m.vertex(b.a), m.vertex(b.b), 0.5);
                assert!(mid_a.distance(mid_b) > 1e-6, "two copies of one arc");
            }
        }
    }
}
