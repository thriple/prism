//! Many edges rounded at once through the kernel (Fillet Joins step B,
//! brief 32), checked against volumes worked by hand.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::rounds::{NO_SIZE, TWO_SOLIDS, refusal, round_edges};
use prism_ops::{add_circle, add_polygon, add_rect, check, face_at, health, push_pull, vertex_at};
use prism_topo::{EdgeId, Model};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn pull(m: &mut Model, p: DVec3, d: f64) {
    let f = face_at(m, p).expect("a face there");
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(m, f, d * up, false);
}

/// A cube of side `l` on the ground.
fn cube(l: f64) -> Model {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, l, l, 0.0);
    pull(&mut m, v(l / 2.0, l / 2.0, 0.0), l);
    m
}

fn edge(m: &Model, p: DVec3, q: DVec3) -> EdgeId {
    let a = vertex_at(m, p).expect("p");
    let b = vertex_at(m, q).expect("q");
    m.edge_between(a, b).expect("an edge")
}

/// The solid's volume as the kernel measures it (exact), checked against
/// Model Health's sum (which draws curved edges as fine segments).
fn volume(m: &Model) -> f64 {
    let seed = m.faces().next().expect("a face").0;
    let exact = prism_ops::bridge::round_trip(&mut m.clone(), seed).expect("a solid");
    let drawn: f64 = m.faces().map(|(id, _)| health::face_volume(m, id)).sum();
    assert!((exact - drawn).abs() < 1e-3 * exact, "{exact} {drawn}");
    exact
}

fn clean(m: &Model) {
    assert_eq!(check::problems(m), Vec::<String>::new());
    let h = health::check(m);
    assert_eq!((h.watertight, h.leaky), (1, 0), "{h:?}");
    assert!(h.reversed.is_empty(), "{h:?}");
}

#[test]
fn one_edge_filleted_removes_its_corner() {
    let (l, r) = (2.0, 0.3);
    let mut m = cube(l);
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    round_edges(&mut m, &BTreeSet::from([e]), r, true).unwrap();
    clean(&m);
    let want = l * l * l - r * r * (1.0 - PI / 4.0) * l;
    assert!((volume(&m) - want).abs() < 1e-7, "{} {want}", volume(&m));
    // The round is a cylinder face, Default paint, joined softly.
    assert_eq!(
        m.faces()
            .filter(|(_, f)| matches!(f.surface, Surface::Cylinder(_)))
            .count(),
        1
    );
    assert_eq!(m.edges().filter(|(_, e)| e.soft && e.smooth).count(), 2);
}

#[test]
fn one_edge_through_the_kernel_matches_the_old_round() {
    let mut a = cube(2.0);
    let mut b = a.clone();
    let e = edge(&a, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    round_edges(&mut a, &BTreeSet::from([e]), 0.3, true).unwrap();
    assert!(prism_ops::round::round(&mut b, e, 0.3, true));
    assert!((volume(&a) - volume(&b)).abs() < 1e-7);
    assert_eq!(a.face_count(), b.face_count());
    let mut a = cube(2.0);
    let mut b = a.clone();
    round_edges(&mut a, &BTreeSet::from([e]), 0.3, false).unwrap();
    assert!(prism_ops::round::round(&mut b, e, 0.3, false));
    assert!((volume(&a) - volume(&b)).abs() < 1e-7);
}

#[test]
fn a_boxs_four_top_edges_round_with_mitred_corners() {
    let (l, r) = (2.0, 0.3);
    let mut m = cube(l);
    let top = face_at(&m, v(1.0, 1.0, 2.0)).unwrap();
    let edges: BTreeSet<EdgeId> = m.face(top).unwrap().edge_ids().collect();
    assert_eq!(edges.len(), 4);
    round_edges(&mut m, &edges, r, true).unwrap();
    clean(&m);
    let a = r * r * (1.0 - PI / 4.0);
    let want = l * l * l - 4.0 * a * (l - 2.0 * r) - 4.0 * r * r * r / 3.0;
    assert!((volume(&m) - want).abs() < 1e-7, "{} {want}", volume(&m));
    // The top face is still there with its id, smaller.
    assert!(m.face(top).is_some());
    // The mitres are exact curves, not approximations.
    assert!(m.edges().all(|(_, e)| !e.approx));
}

#[test]
fn all_twelve_edges_make_a_rounded_cube() {
    let (l, r) = (2.0, 0.3);
    let mut m = cube(l);
    let edges: BTreeSet<EdgeId> = m.edges().map(|(id, _)| id).collect();
    assert_eq!(edges.len(), 12);
    round_edges(&mut m, &edges, r, true).unwrap();
    clean(&m);
    let k = l - 2.0 * r;
    let want = k * k * k + 6.0 * r * k * k + 3.0 * PI * r * r * k + 4.0 / 3.0 * PI * r * r * r;
    assert!((volume(&m) - want).abs() < 1e-7, "{} {want}", volume(&m));
    assert_eq!(
        m.faces()
            .filter(|(_, f)| matches!(f.surface, Surface::Sphere(_)))
            .count(),
        8,
        "a ball at each corner"
    );
}

#[test]
fn a_box_chamfered_on_its_top_edges() {
    let (l, d) = (2.0, 0.2);
    let mut m = cube(l);
    let top = face_at(&m, v(1.0, 1.0, 2.0)).unwrap();
    let edges: BTreeSet<EdgeId> = m.face(top).unwrap().edge_ids().collect();
    round_edges(&mut m, &edges, d, false).unwrap();
    clean(&m);
    // Four prisms of section d²/2 along the straight runs, and at each
    // corner the two half-runs to the mitre: 2 · ∫ (d - y)² dy = 2d³/3.
    let want = l * l * l - 4.0 * (d * d / 2.0) * (l - 2.0 * d) - 4.0 * 2.0 * d * d * d / 3.0;
    assert!((volume(&m) - want).abs() < 1e-7, "{} {want}", volume(&m));
}

#[test]
fn a_sixty_degree_edge_is_filleted() {
    // An equilateral triangle pulled up; its upright edges meet at 60°.
    let (s, h, r) = (2.0, 1.0, 0.2);
    let mut m = Model::new();
    let c = v(s / 2.0, s * 3f64.sqrt() / 2.0, 0.0);
    add_polygon(&mut m, &[v(0.0, 0.0, 0.0), v(s, 0.0, 0.0), c]);
    pull(&mut m, v(1.0, 0.5, 0.0), h);
    let e = edge(&m, v(0.0, 0.0, 0.0), v(0.0, 0.0, h));
    let before = volume(&m);
    round_edges(&mut m, &BTreeSet::from([e]), r, true).unwrap();
    clean(&m);
    let removed = r * r * (3f64.sqrt() - PI / 3.0) * h;
    assert!(
        (before - volume(&m) - removed).abs() < 1e-7,
        "{}",
        before - volume(&m)
    );
}

#[test]
fn the_top_edge_of_a_cylinder_is_filleted() {
    let (big, h, s) = (1.0, 2.0, 0.2);
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, big);
    pull(&mut m, DVec3::ZERO, h);
    let top = face_at(&m, v(0.0, 0.0, h)).unwrap();
    let edges: BTreeSet<EdgeId> = m.face(top).unwrap().edge_ids().collect();
    let before = volume(&m);
    round_edges(&mut m, &edges, s, true).unwrap();
    clean(&m);
    // Pappus: the corner's cross-section turned about the axis.
    let a = s * s * (1.0 - PI / 4.0);
    let xbar = s * (5.0 / 6.0 - PI / 4.0) / (1.0 - PI / 4.0);
    let removed = 2.0 * PI * (big - xbar) * a;
    assert!(
        (before - volume(&m) - removed).abs() < 1e-7,
        "{} {removed}",
        before - volume(&m)
    );
}

#[test]
fn rounds_are_refused_with_a_reason_and_change_nothing() {
    let mut m = cube(2.0);
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    let before = m.clone();
    assert_eq!(refusal(&m, &BTreeSet::from([e]), 0.0, true), Some(NO_SIZE));
    assert_eq!(
        refusal(&m, &BTreeSet::from([e]), 2.5, true),
        Some(prism_kernel::solid::TOO_LARGE)
    );
    assert!(round_edges(&mut m, &BTreeSet::from([e]), 2.5, true).is_err());
    assert_eq!(m, before);
    // An edge of a loose rectangle beside the cube.
    add_rect(&mut m, 5.0, 0.0, 6.0, 1.0, 0.0);
    let loose = edge(&m, v(5.0, 0.0, 0.0), v(6.0, 0.0, 0.0));
    assert_eq!(
        refusal(&m, &BTreeSet::from([loose]), 0.1, true),
        Some(TWO_SOLIDS)
    );
    assert_eq!(
        refusal(&m, &BTreeSet::from([e, loose]), 0.1, true),
        Some(TWO_SOLIDS)
    );
}
