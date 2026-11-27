//! Where faces cross, and Intersect Faces (DECISIONS #29).

use prism_math::DVec3;
use prism_ops::intersect::{Flat, all_crossings, crossings, intersect_faces};
use prism_ops::{check, face_at};
use prism_topo::Model;

fn ok(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

/// A loop of segments through `pts` (a face once closed).
fn poly(m: &mut Model, pts: &[DVec3]) {
    for i in 0..pts.len() {
        prism_ops::add_segment(m, pts[i], pts[(i + 1) % pts.len()]);
    }
}

fn flat_at(m: &Model, p: DVec3) -> Flat {
    Flat::of(m, face_at(m, p).expect("a face there")).unwrap()
}

#[test]
fn two_crossing_squares_cross_once_at_every_scale() {
    for k in [1e-3, 1.0, 1e3] {
        let v = |x: f64, y: f64, z: f64| DVec3::new(x, y, z) * k;
        let mut m = Model::new();
        poly(
            &mut m,
            &[v(0., 0., 0.), v(4., 0., 0.), v(4., 4., 0.), v(0., 4., 0.)],
        );
        poly(
            &mut m,
            &[v(1., 2., -1.), v(3., 2., -1.), v(3., 2., 1.), v(1., 2., 1.)],
        );
        let tol = m.tol();
        let s = crossings(
            &flat_at(&m, v(0.5, 0.5, 0.)),
            &flat_at(&m, v(2., 2., 0.5)),
            &tol,
        );
        assert_eq!(s.len(), 1, "at {k}");
        let (a, b) = s[0];
        let (a, b) = if a.x < b.x { (a, b) } else { (b, a) };
        assert!(a.distance(v(1., 2., 0.)) < 1e-9 * k && b.distance(v(3., 2., 0.)) < 1e-9 * k);
    }
}

#[test]
fn faces_sharing_an_edge_and_parallel_faces_do_not_cross() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    let f = face_at(&m, DVec3::new(2.0, 2.0, 0.0)).unwrap();
    prism_ops::push_pull(&mut m, f, 2.0, false);
    // A box: every pair of faces shares an edge or is parallel.
    assert!(all_crossings(&m, None).is_empty());
}

#[test]
fn a_face_with_a_hole_crosses_twice() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 6.0, 6.0, 0.0);
    prism_ops::add_rect(&mut m, 2.0, 2.0, 4.0, 4.0, 0.0);
    let ring = flat_at(&m, DVec3::new(1.0, 1.0, 0.0));
    assert_eq!(ring.holes.len(), 1);
    let mut w = Model::new();
    poly(
        &mut w,
        &[
            DVec3::new(-1.0, 3.0, -1.0),
            DVec3::new(7.0, 3.0, -1.0),
            DVec3::new(7.0, 3.0, 1.0),
            DVec3::new(-1.0, 3.0, 1.0),
        ],
    );
    let wall = flat_at(&w, DVec3::new(3.0, 3.0, 0.5));
    assert_eq!(crossings(&ring, &wall, &m.tol()).len(), 2);
}

#[test]
fn a_wall_standing_on_a_floor_crosses_along_its_foot() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    let mut w = Model::new();
    poly(
        &mut w,
        &[
            DVec3::new(1.0, 2.0, 0.0),
            DVec3::new(3.0, 2.0, 0.0),
            DVec3::new(3.0, 2.0, 1.0),
            DVec3::new(1.0, 2.0, 1.0),
        ],
    );
    let s = crossings(
        &flat_at(&m, DVec3::new(0.5, 0.5, 0.0)),
        &flat_at(&w, DVec3::new(2.0, 2.0, 0.5)),
        &m.tol(),
    );
    assert_eq!(s.len(), 1);
}

/// The owner's example: a box passing through another. Move does not cut
/// (only Push/Pull keeps solids whole, DECISIONS #31), so Intersect Faces
/// puts the outline on both faces it passes through.
#[test]
fn intersect_faces_cuts_where_a_moved_bar_passes_through() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    let f = face_at(&m, DVec3::new(2.0, 2.0, 0.0)).unwrap();
    let up = m.face(f).unwrap().plane().unwrap().normal.z > 0.0;
    prism_ops::push_pull(&mut m, f, if up { 2.0 } else { -2.0 }, false);
    m = through_box(m);
    ok(&m);
    let crossing = all_crossings(&m, None);
    assert_eq!(
        crossing.len(),
        8,
        "the bar's four walls cross the front and the back"
    );
    let before = m.face_count();
    assert_eq!(intersect_faces(&mut m, None), 8);
    ok(&m);
    assert!(m.face_count() > before);
    // The back face (y = 4) now has a 2 x 1 opening as a face of its own.
    let opening = face_at(&m, DVec3::new(2.0, 4.0, 1.0)).expect("the opening");
    assert!((m.face_area(opening) - 2.0).abs() < 1e-9);
    assert!(all_crossings(&m, None).is_empty(), "nothing left to cut");
}

/// A 2 x 1 bar from y = -1 to 6 through the 4 x 4 x 2 box, made away from
/// it and moved in (a move only merges faces on one plane).
pub fn through_box(mut m: Model) -> Model {
    let before: std::collections::BTreeSet<_> = m.vertices().map(|(id, _)| id).collect();
    prism_ops::add_rect(&mut m, 21.0, -1.0, 23.0, 6.0, 0.5);
    let bar = face_at(&m, DVec3::new(22.0, 2.0, 0.5)).unwrap();
    let up = m.face(bar).unwrap().plane().unwrap().normal.z > 0.0;
    prism_ops::push_pull(&mut m, bar, if up { 1.0 } else { -1.0 }, false);
    let vs = m
        .vertices()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    prism_ops::transform::translate(&mut m, &vs, DVec3::new(-20.0, 0.0, 0.0));
    m
}
