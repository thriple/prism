//! Solid > Convert > Curve Chain ↔ NURBS Curve (brief 43, part 4b).

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::Curve;
use prism_math::DVec3;
use prism_ops::convert::{self, CURVED, MEETS, MIXED, NOT_A_CHAIN};
use prism_ops::{add_segment, sticky};
use prism_topo::{EdgeId, Model};

fn all_edges(m: &Model) -> BTreeSet<EdgeId> {
    m.edges().map(|(id, _)| id).collect()
}

fn kinds(m: &Model) -> (usize, usize, usize) {
    let mut k = (0, 0, 0);
    for (_, e) in m.edges() {
        match e.curve {
            Curve::Line => k.0 += 1,
            Curve::Arc { .. } => k.1 += 1,
            Curve::Spline(_) => k.2 += 1,
        }
    }
    k
}

fn sound(m: &Model) {
    let p = prism_ops::check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

/// A "D": a straight side and a half circle of radius 1, with its face.
fn d_shape() -> Model {
    let mut m = Model::new();
    add_segment(
        &mut m,
        DVec3::new(0.0, -1.0, 0.0),
        DVec3::new(0.0, 1.0, 0.0),
    );
    sticky::add_arc(
        &mut m,
        DVec3::ZERO,
        DVec3::Z,
        1.0,
        DVec3::new(0.0, -1.0, 0.0),
        PI,
    );
    assert_eq!(m.face_count(), 1);
    m
}

#[test]
fn a_closed_chain_of_a_line_and_an_arc_converts_and_back() {
    let mut m = d_shape();
    let f = m.faces().next().map(|(id, _)| id).unwrap();
    let area = m.face_area(f);
    assert!((area - PI / 2.0).abs() < 1e-9);
    let all = all_edges(&m);
    convert::convert(&mut m, &all).unwrap();
    sound(&m);
    assert_eq!(kinds(&m), (0, 0, 2), "one curve, in two halves");
    assert_eq!(m.vertices().count(), 2);
    assert!((m.face_area(f) - area).abs() < 1e-9, "{}", m.face_area(f));
    // And back: the line and the half circle again, exactly.
    let all = all_edges(&m);
    convert::convert(&mut m, &all).unwrap();
    sound(&m);
    assert_eq!(kinds(&m), (1, 1, 0));
    assert!((m.face_area(f) - area).abs() < 1e-9);
    let r = m
        .edges()
        .find_map(|(_, e)| match e.curve {
            Curve::Arc { radius, .. } => Some(radius),
            _ => None,
        })
        .unwrap();
    assert!((r - 1.0).abs() < 1e-9);
    // The chain comes back as one curve.
    let ids: BTreeSet<_> = m.edges().map(|(_, e)| e.curve_id).collect();
    assert_eq!(ids.len(), 1);
    assert!(ids.iter().next().unwrap().is_some());
}

#[test]
fn an_open_zigzag_converts_and_back_through_the_same_points() {
    let mut m = Model::new();
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(1.0, 1.0, 0.0),
        DVec3::new(2.0, 0.0, 0.5),
        DVec3::new(3.0, 1.0, 0.5),
    ];
    for w in pts.windows(2) {
        add_segment(&mut m, w[0], w[1]);
    }
    let all = all_edges(&m);
    convert::convert(&mut m, &all).unwrap();
    assert_eq!(kinds(&m), (0, 0, 1));
    assert_eq!(m.vertices().count(), 2);
    let all = all_edges(&m);
    convert::convert(&mut m, &all).unwrap();
    assert_eq!(kinds(&m), (3, 0, 0));
    for p in pts {
        assert!(m.vertices().any(|(_, q)| q.distance(p) < 1e-9), "{p}");
    }
}

#[test]
fn a_free_curve_becomes_fine_marked_steps() {
    let mut m = Model::new();
    let c = prism_geom::Nurbs::open(
        &[
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 2.0, 0.0),
            DVec3::new(2.0, -1.0, 0.0),
            DVec3::new(3.0, 0.5, 0.0),
        ],
        3,
    )
    .unwrap();
    sticky::add_spline(&mut m, &c);
    let ends = (c.eval(c.domain().0), c.eval(c.domain().1));
    let all = all_edges(&m);
    convert::convert(&mut m, &all).unwrap();
    let (lines, arcs, splines) = kinds(&m);
    assert_eq!((arcs, splines), (0, 0));
    assert!(lines > 8 && lines < 400, "{lines}");
    assert!(m.edges().all(|(_, e)| e.approx), "marked approximate");
    for p in [ends.0, ends.1] {
        assert!(m.vertices().any(|(_, q)| q.distance(p) < 1e-9));
    }
    // Every point lies on the curve.
    for (_, q) in m.vertices() {
        let t = c.closest(q, c.domain().0, c.domain().1);
        assert!(c.eval(t).distance(q) < 1e-6);
    }
}

#[test]
fn convert_says_why_it_cannot() {
    let p = DVec3::new;
    // A branch.
    let mut m = Model::new();
    add_segment(&mut m, p(0.0, 0.0, 0.0), p(2.0, 0.0, 0.0));
    add_segment(&mut m, p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0));
    assert_eq!(convert::refusal(&m, &all_edges(&m)), Some(NOT_A_CHAIN));
    // Something meets the chain partway.
    let line: BTreeSet<EdgeId> = m
        .edges()
        .filter(|(_, e)| m.vertex(e.a).y.abs() < 1e-9 && m.vertex(e.b).y.abs() < 1e-9)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(line.len(), 2);
    assert_eq!(convert::refusal(&m, &line), Some(MEETS));
    // A curve and lines together.
    let mut m = Model::new();
    let c = prism_geom::Nurbs::open(&[p(0.0, 0.0, 0.0), p(1.0, 1.0, 0.0), p(2.0, 0.0, 0.0)], 2)
        .unwrap();
    sticky::add_spline(&mut m, &c);
    add_segment(&mut m, p(2.0, 0.0, 0.0), p(3.0, 0.0, 0.0));
    assert_eq!(convert::refusal(&m, &all_edges(&m)), Some(MIXED));
    // A cylinder's rim runs along its curved side.
    let mut m = Model::new();
    let f = sticky::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0).faces[0];
    let up = m.face(f).unwrap().plane().unwrap().normal.z.signum();
    prism_ops::pushpull::push_pull(&mut m, f, up, false);
    let rim: BTreeSet<EdgeId> = m
        .edges()
        .filter(|(_, e)| e.curve.is_arc() && m.vertex(e.a).z.abs() < 1e-9)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(rim.len(), 2);
    assert_eq!(convert::refusal(&m, &rim), Some(CURVED));
}
