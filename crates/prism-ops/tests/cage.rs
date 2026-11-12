//! Cage editing (Freeform NURBS part 3).

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_geom::Nurbs;
use prism_math::{DAffine3, DVec3};
use prism_ops::cage::{self, Cage};
use prism_ops::{add_segment, add_spline, check};
use prism_topo::Model;

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn wave() -> Nurbs {
    Nurbs::open(
        &[
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 2.0, 0.0),
            DVec3::new(3.0, -2.0, 0.0),
            DVec3::new(4.0, 0.0, 0.0),
        ],
        3,
    )
    .unwrap()
}

fn ring() -> Nurbs {
    Nurbs::closed(
        &[
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(4.0, 3.0, 0.0),
            DVec3::new(0.0, 3.0, 0.0),
        ],
        3,
    )
    .unwrap()
}

fn shift(d: DVec3) -> DAffine3 {
    DAffine3::from_translation(d)
}

fn solid() -> Model {
    let mut m = Model::new();
    add_spline(&mut m, &ring());
    let face = m.faces().next().unwrap().0;
    prism_ops::push_pull(&mut m, face, 2.0, false);
    m
}

fn wall(m: &Model) -> Cage {
    Cage::Surface(
        m.faces()
            .find(|(_, f)| f.surface.patch().is_some())
            .unwrap()
            .0,
    )
}

fn volume(m: &Model) -> f64 {
    m.faces()
        .map(|(id, _)| prism_ops::health::face_volume(m, id))
        .sum()
}

#[test]
fn moving_a_control_point_reshapes_a_curve_and_its_end_takes_lines_along() {
    let mut m = Model::new();
    add_spline(&mut m, &wave());
    add_segment(&mut m, DVec3::new(4.0, 0.0, 0.0), DVec3::new(6.0, 0.0, 0.0));
    let e = m.edges().find(|(_, e)| e.curve.is_spline()).unwrap().0;
    let c = Cage::Curve(e);
    assert_eq!(cage::points(&m, c).len(), 4);
    assert_eq!(cage::lines(&m, c).len(), 3);
    // A middle point: the ends stay.
    cage::move_points(
        &mut m,
        c,
        &BTreeSet::from([1]),
        &shift(DVec3::new(0.0, 1.0, 2.0)),
    )
    .unwrap();
    clean(&m);
    assert_eq!(cage::points(&m, c)[1], DVec3::new(1.0, 3.0, 2.0));
    assert!(m.vertices().any(|(_, p)| p == DVec3::ZERO));
    // The last point: the curve's end moves, and the line joined to it.
    cage::move_points(
        &mut m,
        c,
        &BTreeSet::from([3]),
        &shift(DVec3::new(0.0, 1.0, 0.0)),
    )
    .unwrap();
    clean(&m);
    assert!(m.vertices().any(|(_, p)| p == DVec3::new(4.0, 1.0, 0.0)));
    assert!(!m.vertices().any(|(_, p)| p == DVec3::new(4.0, 0.0, 0.0)));
    assert_eq!(m.edge_count(), 2);
}

#[test]
fn a_flat_face_follows_in_its_plane_and_refuses_to_bend() {
    let mut m = Model::new();
    add_spline(&mut m, &ring());
    let e = m.edges().next().unwrap().0;
    let c = Cage::Curve(e);
    let before = m.face_area(m.faces().next().unwrap().0);
    cage::move_points(
        &mut m,
        c,
        &BTreeSet::from([2]),
        &shift(DVec3::new(2.0, 2.0, 0.0)),
    )
    .unwrap();
    clean(&m);
    assert!(m.face_area(m.faces().next().unwrap().0) > before);
    let was = m.clone();
    assert_eq!(
        cage::move_points(&mut m, c, &BTreeSet::from([2]), &shift(DVec3::Z)),
        Err(cage::FLAT_BESIDE)
    );
    assert_eq!(m, was, "a refused edit changes nothing");
    // All of it lifted: the face moves to a new plane.
    let all: BTreeSet<usize> = (0..4).collect();
    cage::move_points(&mut m, c, &all, &shift(DVec3::Z)).unwrap();
    clean(&m);
    assert!(m.vertices().all(|(_, p)| (p.z - 1.0).abs() < 1e-12));
}

#[test]
fn a_surface_cage_moves_both_walls_their_sides_and_the_flat_ends() {
    let mut m = solid();
    let c = wall(&m);
    let (_, faces) = cage::members(&m, c);
    assert_eq!(faces.len(), 2, "both walls are on the surface");
    let pts = cage::points(&m, c);
    assert_eq!(pts.len(), 8, "four round by two up");
    let v0 = volume(&m);
    // The top row (every second point) scaled about its middle: the top
    // face grows in its plane.
    let top: BTreeSet<usize> = (0..8).filter(|k| k % 2 == 1).collect();
    let centre = DVec3::new(2.0, 1.5, 2.0);
    let grow = shift(centre) * DAffine3::from_scale(DVec3::new(1.5, 1.5, 1.0)) * shift(-centre);
    cage::move_points(&mut m, c, &top, &grow).unwrap();
    clean(&m);
    assert!(volume(&m) > v0 * 1.2, "{} {v0}", volume(&m));
    // Lifted whole, the top face moves to its new plane.
    cage::move_points(&mut m, c, &top, &shift(DVec3::Z)).unwrap();
    clean(&m);
    let (_, hi) = m.bbox().unwrap();
    assert!((hi.z - 3.0).abs() < 1e-9);
    // One top point lifted alone would bend the flat top.
    assert_eq!(
        cage::move_points(&mut m, c, &BTreeSet::from([1]), &shift(DVec3::Z)),
        Err(cage::FLAT_BESIDE)
    );
    // Without the top, it may.
    let top_face = m
        .faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.normal.z > 0.5))
        .unwrap()
        .0;
    m.remove_face(top_face);
    cage::move_points(&mut m, c, &BTreeSet::from([1]), &shift(DVec3::Z)).unwrap();
    clean(&m);
    // The base curve is now the side of a face: its own cage is refused.
    let base = m
        .edges()
        .find(|(_, e)| {
            e.curve.is_spline() && m.vertex(e.a).z.abs() < 1e-9 && m.vertex(e.b).z.abs() < 1e-9
        })
        .unwrap()
        .0;
    assert_eq!(
        cage::move_points(
            &mut m,
            Cage::Curve(base),
            &BTreeSet::from([0]),
            &shift(DVec3::X)
        ),
        Err(cage::SIDE_OF_FACE)
    );
}

#[test]
fn adding_control_points_keeps_the_shape() {
    let mut m = Model::new();
    add_spline(&mut m, &wave());
    let e = m.edges().next().unwrap().0;
    let c = Cage::Curve(e);
    let sample = |m: &Model| -> Vec<DVec3> {
        let x = m.edge(e).unwrap();
        (0..=10)
            .map(|i| x.curve.point(m.vertex(x.a), m.vertex(x.b), i as f64 / 10.0))
            .collect()
    };
    let before = sample(&m);
    cage::add_points(&mut m, c).unwrap();
    assert_eq!(cage::points(&m, c).len(), 5);
    for (a, b) in before.iter().zip(sample(&m)) {
        assert!(a.distance(b) < 1e-12);
    }
    clean(&m);
    // A closed surface gets a row along its open direction only.
    let mut s = solid();
    let c = wall(&s);
    let v = volume(&s);
    cage::add_points(&mut s, c).unwrap();
    assert_eq!(cage::points(&s, c).len(), 12);
    assert!((volume(&s) - v).abs() < 1e-9);
    clean(&s);
}
