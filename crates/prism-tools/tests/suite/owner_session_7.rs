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
        serde_json::from_str(include_str!("../data/owner-session-7-wheel-wells.json"))
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

#[test]
fn the_pushed_side_merges_with_the_wheel_well_it_lands_on() {
    let s = replay_to(736);
    let r = prism_ops::health::check(s.work_model());
    assert!(
        r.crossing.faces.is_empty(),
        "crossing: {:?}",
        r.crossing.faces
    );
}

/// Hard lines inside one smooth curved surface: two curved faces meeting
/// with the same normal along a line that is not soft. Hover (and a click)
/// takes a smooth surface only across soft lines, so such a line lit part
/// of a wheel well (owner, 2026-11-25).
fn hard_inside_smooth(m: &prism_topo::Model) -> Vec<prism_topo::EdgeId> {
    let edge_faces = m.edge_faces();
    edge_faces
        .iter()
        .filter(|(e, fs)| {
            let (Some(edge), [a, b]) = (m.edge(**e), fs.as_slice()) else {
                return false;
            };
            let (Some(fa), Some(fb)) = (m.face(*a), m.face(*b)) else {
                return false;
            };
            let mid = edge.curve.point(m.vertex(edge.a), m.vertex(edge.b), 0.5);
            !edge.soft
                && fa.is_curved()
                && fb.is_curved()
                && fa.surface.normal_at(mid).dot(fb.surface.normal_at(mid)) > 1.0 - 1e-9
        })
        .map(|(e, _)| *e)
        .collect()
}

#[test]
fn the_wheel_wells_stay_one_smooth_surface_through_both_pushes() {
    // The well's back pushed deeper (event 309): new curved walls under
    // the old, parted from them by the old outline's arcs, which stayed
    // hard. Then the side pushed across onto the wells (event 735): its
    // lines were split along the wells' soft top seams, and the split
    // copied the hard look onto them.
    for n in [310, 736] {
        let s = replay_to(n);
        let m = s.work_model();
        assert_eq!(hard_inside_smooth(m), Vec::new(), "after event {}", n - 1);
    }
    let s = replay_to(310);
    let m = s.work_model();
    let well = prism_ops::select::surface(m, prism_topo::FaceId(17));
    assert_eq!(well.len(), 4, "the whole well lights: {well:?}");
}
