//! Model Health checks and fixes (PLAN 1.10).

use prism_math::DVec3;
use prism_ops::health::{self, check};
use prism_topo::Model;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn cube(at: DVec3, s: f64) -> Model {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, at.x, at.y, at.x + s, at.y + s, at.z);
    let f = prism_ops::face_at(&m, at + v(s / 2.0, s / 2.0, 0.0)).expect("base");
    let up = m.face(f).unwrap().plane().unwrap().normal.z > 0.0;
    prism_ops::push_pull(&mut m, f, if up { s } else { -s }, false);
    m
}

#[test]
fn a_clean_box_has_no_problems() {
    let r = check(&cube(v(0.0, 0.0, 0.0), 2.0));
    assert_eq!(r.watertight, 1);
    assert_eq!(r.leaky, 0);
    for (name, f) in [
        ("duplicates", &r.duplicates),
        ("near misses", &r.near_misses),
        ("reversed", &r.reversed),
        ("open", &r.open_edges),
        ("non-manifold", &r.non_manifold),
        ("interior", &r.interior),
        ("degenerate", &r.degenerate),
        ("stray", &r.stray),
        ("crossing", &r.crossing),
        ("tiny", &r.tiny),
    ] {
        assert!(f.is_empty(), "{name}: {f:?}");
    }
    assert!(!r.far_from_origin);
}

#[test]
fn a_flipped_face_is_found_and_orient_turns_it_back() {
    let mut m = cube(v(0.0, 0.0, 0.0), 2.0);
    let top = prism_ops::face_at(&m, v(1.0, 1.0, 2.0)).unwrap();
    health::reverse(&mut m, &[top].into());
    assert_eq!(check(&m).reversed.faces, [top].into());
    assert_eq!(health::orient(&mut m, None), 1);
    assert!(check(&m).reversed.is_empty());
    assert!(
        m.face(top).unwrap().plane().unwrap().normal.z > 0.9,
        "the top faces up again"
    );
}

#[test]
fn an_inside_out_box_is_turned_to_face_out() {
    let mut m = cube(v(0.0, 0.0, 0.0), 2.0);
    let all = m.faces().map(|(id, _)| id).collect();
    health::reverse(&mut m, &all);
    assert_eq!(check(&m).reversed.faces.len(), 6, "every face faces in");
    health::orient(&mut m, None);
    assert!(check(&m).reversed.is_empty());
    let top = prism_ops::face_at(&m, v(1.0, 1.0, 2.0)).unwrap();
    assert!(m.face(top).unwrap().plane().unwrap().normal.z > 0.9);
}

#[test]
fn an_open_box_has_open_edges_and_is_not_watertight() {
    let mut m = cube(v(0.0, 0.0, 0.0), 2.0);
    let top = prism_ops::face_at(&m, v(1.0, 1.0, 2.0)).unwrap();
    m.remove_face(top);
    let r = check(&m);
    assert_eq!(r.open_edges.edges.len(), 4);
    assert_eq!((r.watertight, r.leaky), (0, 1));
}

#[test]
fn a_wall_inside_a_box_is_interior_and_clean_envelope_removes_it() {
    let mut m = cube(v(0.0, 0.0, 0.0), 2.0);
    // A wall across the middle (lines across the top, side and bottom make
    // a face inside).
    for (a, b) in [
        (v(1.0, 0.0, 0.0), v(1.0, 0.0, 2.0)),
        (v(1.0, 0.0, 2.0), v(1.0, 2.0, 2.0)),
        (v(1.0, 2.0, 2.0), v(1.0, 2.0, 0.0)),
        (v(1.0, 2.0, 0.0), v(1.0, 0.0, 0.0)),
    ] {
        prism_ops::add_segment(&mut m, a, b);
    }
    let wall = prism_ops::face_at(&m, v(1.0, 1.0, 1.0)).expect("the wall");
    let r = check(&m);
    assert!(r.interior.faces.contains(&wall), "{:?}", r.interior);
    assert_eq!(health::clean_envelope(&mut m, None), 1);
    assert!(prism_ops::face_at(&m, v(1.0, 1.0, 1.0)).is_none());
    assert!(check(&m).interior.is_empty());
    // The outside stays (now split in two along the old wall's edges).
    assert!(prism_ops::face_at(&m, v(0.5, 1.0, 2.0)).is_some());
}

#[test]
fn stray_edges_degenerate_tiny_and_crossing() {
    let mut m = cube(v(0.0, 0.0, 0.0), 2.0);
    // A stray line.
    prism_ops::add_segment(&mut m, v(5.0, 0.0, 0.0), v(6.0, 0.0, 0.0));
    let r = check(&m);
    assert_eq!(r.stray.edges.len(), 1);
    assert_eq!(health::remove_stray(&mut m), 1);
    assert!(check(&m).stray.is_empty());
    // A bar moved through the box: faces cross.
    let mut bar = cube(v(0.5, 0.5, 0.5), 1.0);
    let vs = bar.vertices().map(|(id, _)| id).collect();
    prism_ops::transform::translate(&mut bar, &vs, v(1.0, 0.0, 0.0));
    m.append(&bar);
    assert!(!check(&m).crossing.is_empty(), "the bar crosses the box");
    // A tiny piece far smaller than the model.
    let mut t = Model::new();
    let s = m.tol().dist * 2.0;
    prism_ops::add_segment(&mut t, v(9.0, 9.0, 0.0), v(9.0 + s, 9.0, 0.0));
    m.append(&t);
    assert!(!check(&m).tiny.is_empty());
}

#[test]
fn the_export_note_says_when_a_model_is_not_watertight() {
    let closed = cube(v(0.0, 0.0, 0.0), 2.0);
    assert_eq!(health::export_note(&closed, true), None);
    let mut open = closed.clone();
    let top = prism_ops::face_at(&open, v(1.0, 1.0, 2.0)).unwrap();
    open.remove_face(top);
    let note = health::export_note(&open, true).expect("a note");
    assert!(
        note.starts_with("4 open edges: the model is not watertight"),
        "{note}"
    );
    assert!(
        !health::export_note(&open, false)
            .unwrap()
            .contains("watertight")
    );
}

/// The owner's session (2026-11-17): a straight edge beside a curve
/// between the same two points left a lens of a face 0.65 mm wide, and
/// Model Health said nothing. It is a sliver now, and Merge erases the
/// straight edge so the face beside it takes the curve.
#[test]
fn a_lens_beside_a_curve_is_a_sliver_and_merges_away() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 2.0, 0.0);
    // From (4, 0) to (4, 2), bowing out 0.1 mm.
    let bow = prism_geom::Nurbs::open(
        &[v(4.0, 0.0, 0.0), v(4.0002, 1.0, 0.0), v(4.0, 2.0, 0.0)],
        2,
    )
    .unwrap();
    prism_ops::add_spline(&mut m, &bow);
    assert_eq!(m.face_count(), 2, "the rectangle and the lens");
    // As in the owner's model, the lens faces the other way.
    let normal = |m: &Model, small: bool| {
        let mut fs: Vec<_> = m
            .faces()
            .map(|(id, f)| (m.face_area(id), id, f.plane().unwrap().normal))
            .collect();
        fs.sort_by(|a, b| a.0.total_cmp(&b.0));
        if small {
            (fs[0].1, fs[0].2)
        } else {
            (fs[1].1, fs[1].2)
        }
    };
    let (lens, n) = normal(&m, true);
    if n.dot(normal(&m, false).1) > 0.0 {
        health::reverse(&mut m, &[lens].into());
    }
    let r = check(&m);
    assert_eq!(r.slivers.faces.len(), 1, "{r:?}");
    assert!(r.degenerate.is_empty());
    let (turn, edges) = health::sliver_merge(&m);
    assert_eq!(turn, [lens].into(), "turned to match first");
    health::reverse(&mut m, &turn);
    assert_eq!(edges.len(), 1);
    let e = *edges.iter().next().unwrap();
    assert!(
        matches!(m.edge(e).unwrap().curve, prism_geom::Curve::Line),
        "the straight edge"
    );
    prism_ops::select::remove(
        &mut m,
        &prism_ops::Selection {
            edges,
            ..Default::default()
        },
    );
    assert_eq!(m.face_count(), 1, "merged");
    assert!(check(&m).slivers.is_empty());
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
    let (id, _) = m.faces().next().unwrap();
    let want = 8.0 + 2.0 / 3.0 * 2.0 * 0.0001;
    assert!((m.face_area(id) - want).abs() < 1e-9, "{}", m.face_area(id));
}

#[test]
fn a_thin_strip_that_is_not_a_slip_is_no_sliver() {
    // A 1 cm strip 1 m long beside a 4 m box: thin, but not that thin.
    let mut m = cube(v(0.0, 0.0, 0.0), 4.0);
    prism_ops::add_rect(&mut m, 5.0, 0.0, 6.0, 0.01, 0.0);
    assert!(check(&m).slivers.is_empty());
}
