//! Many edges rounded at once through the kernel (Fillet Joins step B,
//! brief 32), checked against volumes worked by hand.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::rounds::{NO_SIZE, refusal, round_edges};
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
        Some(prism_ops::rounds::ONE_FACE)
    );
    assert_eq!(
        refusal(&m, &BTreeSet::from([e, loose]), 0.1, true),
        Some(prism_ops::rounds::ONE_FACE)
    );
}

/// A line drawn on an end face, and a point where a side edge was split,
/// are not corners of the solid's shape: a round runs past them (owner's
/// session 22: a round stopped at the first of them). The line is drawn
/// back on afterwards, as far as it still lies on the solid.
#[test]
fn a_round_runs_past_lines_drawn_on_the_faces_at_its_ends() {
    let (l, r) = (2.0, 0.3);
    let mut m = cube(l);
    // A line up the end face y = 0, 0.05 from the edge to round.
    prism_ops::add_segment(&mut m, v(1.95, 0.0, 0.0), v(1.95, 0.0, 2.0));
    // The upright edge at that end split 0.03 below the top.
    let tol = m.tol();
    prism_ops::insert_point(&mut m, v(2.0, 0.0, 1.97), &tol);
    let faces = m.face_count();
    let e = edge(&m, v(2.0, 0.0, 2.0), v(2.0, 2.0, 2.0));
    round_edges(&mut m, &BTreeSet::from([e]), r, true).unwrap();
    clean(&m);
    let want = l * l * l - r * r * (1.0 - PI / 4.0) * l;
    assert!((volume(&m) - want).abs() < 1e-7, "{} {want}", volume(&m));
    // The line is back, from the ground up to the round, and nothing of
    // it is left in the air.
    let up: Vec<(DVec3, DVec3)> = m
        .edges()
        .map(|(_, e)| (m.vertex(e.a), m.vertex(e.b)))
        .filter(|(a, b)| {
            (a.x - 1.95).abs() < 1e-9
                && (b.x - 1.95).abs() < 1e-9
                && a.y.abs() < 1e-9
                && b.y.abs() < 1e-9
        })
        .collect();
    assert!(!up.is_empty(), "the line is back");
    let top = up.iter().map(|(a, b)| a.z.max(b.z)).fold(0.0, f64::max);
    let on_round = 1.7 + (r * r - 0.25f64 * 0.25).sqrt();
    assert!(
        (top - on_round).abs() < 1e-6,
        "ends on the round: {top} {on_round}"
    );
    assert_eq!(health::check(&m).stray.edges.len(), 0);
    // The cube's six, the end face still in two by the line, and the round.
    assert_eq!((faces, m.face_count()), (7, 8));
}

/// A box 2 long whose top edge is bevelled by a face at `deg` degrees
/// to the top (the profile in y, z pulled along x), and the edge between
/// the top and the bevel.
fn bevelled(deg: f64) -> (Model, EdgeId) {
    let mut m = Model::new();
    let drop = deg.to_radians().tan();
    add_polygon(
        &mut m,
        &[
            v(0.0, 0.0, 0.0),
            v(0.0, 2.0, 0.0),
            v(0.0, 2.0, 1.0),
            v(0.0, 1.0, 1.0),
            v(0.0, 0.0, 1.0 - drop),
        ],
    );
    let f = face_at(&m, v(0.0, 1.0, 0.5)).expect("the profile");
    let out = m.face(f).unwrap().plane().unwrap().normal.x;
    push_pull(&mut m, f, 2.0 * out, false);
    let e = edge(&m, v(0.0, 1.0, 1.0), v(2.0, 1.0, 1.0));
    (m, e)
}

/// The kernel refuses a small round on a shallow edge as too large (at
/// 12 degrees every size; at 45 degrees, 5 mm): Prism's own round makes
/// it, a cylinder tangent to both faces (owner's Xby4, 2026-11-26).
#[test]
fn a_shallow_edge_takes_a_small_round() {
    for (deg, r) in [
        (12.0_f64, 0.005),
        (12.0, 0.02),
        (12.0, 0.2),
        (45.0, 0.005),
        (27.0, 0.05),
    ] {
        let (mut m, e) = bevelled(deg);
        let whole = volume(&m);
        round_edges(&mut m, &[e].into(), r, true).unwrap_or_else(|w| panic!("{deg} {r}: {w}"));
        clean(&m);
        // What a fillet takes off: between the two tangent lines and the
        // arc, r²(tan(φ/2) − φ/2) for a turn of φ, along the 2 m edge.
        let phi = deg.to_radians();
        let cut = r * r * ((phi / 2.0).tan() - phi / 2.0) * 2.0;
        assert!(
            (whole - volume(&m) - cut).abs() < 1e-9 + 1e-6 * cut,
            "{deg} {r}: {} {cut}",
            whole - volume(&m)
        );
    }
}

/// A line drawn on a flat face has no corner to round, and says so.
#[test]
fn a_line_on_a_flat_face_is_not_rounded() {
    let mut m = cube(2.0);
    prism_ops::add_segment(&mut m, v(1.0, 0.0, 0.0), v(1.0, 0.0, 2.0));
    let e = edge(&m, v(1.0, 0.0, 0.0), v(1.0, 0.0, 2.0));
    assert_eq!(
        refusal(&m, &[e].into(), 0.1, true),
        Some(prism_ops::rounds::FLAT_LINE)
    );
}

/// A fillet or chamfer needs only the two faces beside the edge, not a
/// closed solid (owner, 2026-11-26): an open box's corner rounds, and so
/// does an edge where two loose faces meet.
#[test]
fn an_open_box_and_two_loose_faces_round() {
    let l = 2.0;
    let mut m = cube(l);
    let top = face_at(&m, v(1.0, 1.0, l)).expect("the top");
    m.remove_face(top);
    let e = edge(&m, v(l, 0.0, 0.0), v(l, 0.0, l));
    for (fillet, r) in [(true, 0.3), (false, 0.3)] {
        let mut w = m.clone();
        round_edges(&mut w, &BTreeSet::from([e]), r, fillet)
            .unwrap_or_else(|why| panic!("{fillet}: {why}"));
        assert_eq!(check::problems(&w), Vec::<String>::new());
        // The bottom keeps its corner cut to match, the top rim is open.
        assert!(face_at(&w, v(l - 0.01, 0.01, 0.0)).is_none(), "{fillet}");
        assert_eq!(w.faces().count(), m.faces().count() + 1);
    }
    // Two loose faces meeting at an edge: an L of two rectangles.
    let mut m = Model::new();
    add_polygon(
        &mut m,
        &[
            v(0.0, 0.0, 0.0),
            v(2.0, 0.0, 0.0),
            v(2.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
        ],
    );
    add_polygon(
        &mut m,
        &[
            v(0.0, 0.0, 0.0),
            v(2.0, 0.0, 0.0),
            v(2.0, 0.0, 1.0),
            v(0.0, 0.0, 1.0),
        ],
    );
    let e = edge(&m, v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0));
    round_edges(&mut m, &BTreeSet::from([e]), 0.2, true).expect("rounded");
    assert_eq!(check::problems(&m), Vec::<String>::new());
    assert_eq!(m.faces().count(), 3);
}
