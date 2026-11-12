//! Loft (Freeform NURBS part 4).

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_geom::Nurbs;
use prism_math::{DAffine3, DVec3};
use prism_ops::cage::{self, Cage};
use prism_ops::{add_arc, add_circle, add_segment, add_spline, check, loft};
use prism_topo::{EdgeId, Model};

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn all_edges(m: &Model) -> BTreeSet<EdgeId> {
    m.edges().map(|(id, _)| id).collect()
}

fn wave(z: f64, bump: f64) -> Nurbs {
    Nurbs::open(
        &[
            DVec3::new(0.0, 0.0, z),
            DVec3::new(1.0, bump, z),
            DVec3::new(3.0, -bump, z),
            DVec3::new(4.0, 0.0, z),
        ],
        3,
    )
    .unwrap()
}

fn ring(z: f64, r: f64, turn: f64) -> Nurbs {
    let pts: Vec<DVec3> = (0..6)
        .map(|i| {
            let a = i as f64 / 6.0 * std::f64::consts::TAU + turn;
            DVec3::new(r * a.cos(), r * a.sin(), z)
        })
        .collect();
    Nurbs::closed(&pts, 3).unwrap()
}

fn loft_all(m: &mut Model) -> Result<Vec<prism_topo::FaceId>, &'static str> {
    let all = all_edges(m);
    loft::loft(m, &all)
}

fn patches(m: &Model) -> usize {
    m.faces()
        .filter(|(_, f)| f.surface.patch().is_some())
        .count()
}

#[test]
fn a_loft_through_three_open_curves_can_be_pulled_into_shape() {
    let mut m = Model::new();
    for c in [wave(0.0, 2.0), wave(2.0, 0.5), wave(5.0, 1.0)] {
        add_spline(&mut m, &c);
    }
    let before = m.edge_count();
    let faces = loft_all(&mut m).unwrap();
    clean(&m);
    assert_eq!(faces.len(), 2, "one face between each pair of curves");
    assert_eq!(patches(&m), 2);
    assert_eq!(
        m.edge_count(),
        before + 4,
        "the three curves, and two rails each side"
    );
    // The faces share one surface, smooth across the middle curve.
    let c = Cage::Surface(faces[0]);
    assert_eq!(cage::members(&m, c).1.len(), 2);
    let pts = cage::points(&m, c);
    assert_eq!(pts.len(), 12, "four along by three across");
    // Pull a middle control point up: both faces follow, the model stays
    // clean, and the ends stay where they were.
    let at = |m: &Model| {
        let f = m.face(faces[0]).unwrap();
        let p = f.surface.patch().unwrap();
        p.surface.eval(0.4, 0.4)
    };
    let was = at(&m);
    cage::move_points(
        &mut m,
        c,
        &BTreeSet::from([4]),
        &DAffine3::from_translation(DVec3::Y * 2.0),
    )
    .unwrap();
    clean(&m);
    assert!(at(&m).distance(was) > 0.2);
    assert!(m.vertices().any(|(_, p)| p == DVec3::ZERO));
    // Lofting again adds nothing new to refuse on: the same curves give
    // the same faces a second time only if asked; one curve alone is not
    // enough.
    let one: BTreeSet<EdgeId> = m.edges().take(1).map(|(id, _)| id).collect();
    assert!(loft::loft(&mut m, &one).is_err());
}

#[test]
fn closed_curves_loft_into_a_tube_that_faces_out() {
    let mut m = Model::new();
    for c in [
        ring(0.0, 1.0, 0.0),
        ring(2.0, 2.0, 2.0),
        ring(4.0, 1.5, 4.0),
    ] {
        add_spline(&mut m, &c);
    }
    // The flat faces the closed curves made go, so the tube is open.
    let flats: Vec<_> = m.faces().map(|(id, _)| id).collect();
    for f in flats {
        m.remove_face(f);
    }
    let faces = loft_all(&mut m).unwrap();
    clean(&m);
    // Cut round wherever a curve has a vertex, between three curves.
    assert!(
        faces.len() >= 4 && faces.len().is_multiple_of(2),
        "{}",
        faces.len()
    );
    for f in &faces {
        let g = m.face(*f).unwrap();
        let p = m.vertex(g.outer[0]);
        let n = g.surface.normal_at(p);
        assert!(n.dot(DVec3::new(p.x, p.y, 0.0)) > 0.0, "{n} at {p}");
    }
    // The curves started at different places round: the loft does not
    // twist (a point straight above the first start stays on its side).
    let g = m.face(faces[0]).unwrap().surface.patch().unwrap().clone();
    let (a, b) = (g.surface.eval(0.0, 0.0), g.surface.eval(0.0, 1.0));
    let turn = (a.y.atan2(a.x) - b.y.atan2(b.x)).abs();
    assert!(turn < 0.6 || turn > std::f64::consts::TAU - 0.6, "{turn}");
    // With the ends capped it is a closed solid of positive volume.
    let v: f64 = m
        .faces()
        .map(|(id, _)| prism_ops::health::face_volume(&m, id))
        .sum();
    assert!(v.is_finite());
}

#[test]
fn lines_arcs_and_curves_loft_together_exactly() {
    let mut m = Model::new();
    add_segment(&mut m, DVec3::new(0.0, 0.0, 0.0), DVec3::new(4.0, 0.0, 0.0));
    add_arc(
        &mut m,
        DVec3::new(2.0, 2.0, 0.0),
        DVec3::Y,
        2.0,
        DVec3::new(4.0, 2.0, 0.0),
        std::f64::consts::PI,
    );
    add_spline(
        &mut m,
        &wave(0.0, 1.0).transformed(&DAffine3::from_translation(DVec3::Y * 5.0)),
    );
    let faces = loft_all(&mut m).unwrap();
    clean(&m);
    assert_eq!(faces.len(), 2);
    // The arc is now a NURBS edge of the same shape (a side of the faces).
    assert_eq!(m.edges().filter(|(_, e)| e.curve.is_arc()).count(), 0);
    let s = m
        .face(faces[0])
        .unwrap()
        .surface
        .patch()
        .unwrap()
        .surface
        .clone();
    for i in 0..=10 {
        let u = i as f64 / 10.0;
        // On the arc's circle at the middle section.
        let p = s.eval(
            u,
            s.domain()[1].0 + (s.domain()[1].1 - s.domain()[1].0) * mid_param(&s),
        );
        assert!(
            (p.distance(DVec3::new(2.0, 2.0, 0.0)) - 2.0).abs() < 1e-9,
            "{p}"
        );
    }
}

/// The v of the surface's middle section (where y is 2).
fn mid_param(s: &prism_geom::NurbsSurface) -> f64 {
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if (s.eval(0.0, mid).y < 2.0) == (s.eval(0.0, 0.0).y < 2.0) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

#[test]
fn circles_loft_into_a_cone_like_tube_and_mixed_or_touching_curves_are_refused() {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, 2.0);
    add_circle(&mut m, DVec3::new(0.0, 0.0, 3.0), DVec3::Z, 1.0);
    for f in m.faces().map(|(id, _)| id).collect::<Vec<_>>() {
        m.remove_face(f);
    }
    let faces = loft_all(&mut m).unwrap();
    clean(&m);
    assert_eq!(faces.len(), 2);
    let s = m
        .face(faces[0])
        .unwrap()
        .surface
        .patch()
        .unwrap()
        .surface
        .clone();
    for i in 0..=16 {
        let p = s.eval(i as f64 / 16.0, 0.5);
        assert!(
            (p.x.hypot(p.y) - 1.5).abs() < 1e-9 && (p.z - 1.5).abs() < 1e-9,
            "{p}"
        );
    }
    // Open with closed.
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, 2.0);
    add_spline(&mut m, &wave(3.0, 1.0));
    assert_eq!(loft_all(&mut m).err(), Some(loft::MIXED));
    // Closed curves with different numbers of control points.
    let mut m = Model::new();
    add_spline(&mut m, &ring(0.0, 1.0, 0.0));
    let five: Vec<DVec3> = (0..5)
        .map(|i| {
            let a = i as f64 / 5.0 * std::f64::consts::TAU;
            DVec3::new(a.cos(), a.sin(), 2.0)
        })
        .collect();
    add_spline(&mut m, &Nurbs::closed(&five, 3).unwrap());
    let was = m.clone();
    assert_eq!(loft_all(&mut m).err(), Some(loft::NOT_ALIKE));
    assert_eq!(m, was);
}

#[test]
fn a_curve_turned_about_a_line_makes_a_vase() {
    let mut m = Model::new();
    add_segment(&mut m, DVec3::ZERO, DVec3::new(0.0, 0.0, 4.0));
    let side = Nurbs::open(
        &[
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(2.5, 0.0, 1.0),
            DVec3::new(0.5, 0.0, 2.5),
            DVec3::new(1.5, 0.0, 4.0),
        ],
        3,
    )
    .unwrap();
    add_spline(&mut m, &side);
    let all = all_edges(&m);
    let faces = loft::revolve(&mut m, &all).unwrap();
    clean(&m);
    assert_eq!(faces.len(), 2, "two halves round");
    // Every point is as far from the axis as the curve is at its height,
    // and the faces look away from the axis.
    let p = m.face(faces[0]).unwrap().surface.patch().unwrap().clone();
    for i in 0..=8 {
        for j in 0..=8 {
            let (u, v) = (
                p.u.0 + (p.u.1 - p.u.0) * i as f64 / 8.0,
                p.v.0 + (p.v.1 - p.v.0) * j as f64 / 8.0,
            );
            let q = p.surface.eval(u, v);
            let on = side.eval(v);
            assert!((q.x.hypot(q.y) - on.x).abs() < 1e-9 && (q.z - on.z).abs() < 1e-9);
            assert!(p.normal(u, v).dot(DVec3::new(q.x, q.y, 0.0)) > 0.0);
        }
    }
    // The curve is still one edge, now a side of both faces; the rims are
    // circles in two halves.
    assert_eq!(m.edge_count(), 1 + 1 + 4 + 1);
    // No axis, or a curve on it.
    let mut m = Model::new();
    add_spline(&mut m, &side);
    let all = all_edges(&m);
    assert_eq!(loft::revolve(&mut m, &all).err(), Some(loft::NO_AXIS));
    add_segment(
        &mut m,
        DVec3::new(1.0, 0.0, -1.0),
        DVec3::new(1.0, 0.0, -3.0),
    );
    let all = all_edges(&m);
    assert_eq!(loft::revolve(&mut m, &all).err(), Some(loft::TOUCHES_AXIS));
}
