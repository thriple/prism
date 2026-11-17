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

// ---------- Part 4c: Faces ↔ Smooth Patch ----------

use prism_geom::Surface;
use prism_math::Plane;
use prism_ops::convert::{BORDERS, NOT_A_GRID};
use prism_topo::{Face, FaceId, VertexId};

/// A wavy sheet of `nu` by `nv` flat four-sided faces: it bends along x
/// only, so each face is flat.
fn wavy(nu: usize, nv: usize) -> (Model, Vec<DVec3>) {
    let mut m = Model::new();
    let at = |i: usize, j: usize| DVec3::new(i as f64, j as f64, 0.4 * (i as f64 * 1.3).sin());
    let mut g: Vec<Vec<VertexId>> = Vec::new();
    let mut pts = Vec::new();
    for i in 0..=nu {
        g.push((0..=nv).map(|j| m.add_vertex(at(i, j))).collect());
        pts.extend((0..=nv).map(|j| at(i, j)));
    }
    for i in 0..=nu {
        for j in 0..=nv {
            if i < nu {
                m.add_edge(g[i][j], g[i + 1][j]);
            }
            if j < nv {
                m.add_edge(g[i][j], g[i][j + 1]);
            }
        }
    }
    for i in 0..nu {
        for j in 0..nv {
            let l = vec![g[i][j], g[i + 1][j], g[i + 1][j + 1], g[i][j + 1]];
            let q: Vec<DVec3> = l.iter().map(|v| m.vertex(*v)).collect();
            let n = prism_math::newell(&q).normalize();
            m.add_face(Face::new(l, vec![], Surface::Plane(Plane::new(q[0], n))));
        }
    }
    (m, pts)
}

fn all_faces(m: &Model) -> BTreeSet<FaceId> {
    m.faces().map(|(id, _)| id).collect()
}

#[test]
fn a_grid_of_faces_becomes_one_patch_through_its_points_and_back() {
    let (mut m, pts) = wavy(3, 2);
    sound(&m);
    let look = m.faces().next().unwrap().1.plane().unwrap().normal;
    let faces = all_faces(&m);
    convert::convert_faces(&mut m, &faces).unwrap();
    sound(&m);
    assert_eq!(m.face_count(), 1);
    assert_eq!(m.vertices().count(), 4, "its corners");
    let (_, f) = m.faces().next().unwrap();
    let patch = f.surface.patch().expect("a freeform face").clone();
    for p in &pts {
        assert!(
            patch.distance(*p) < 1e-9,
            "through {p}: {}",
            patch.distance(*p)
        );
    }
    // It faces the way the faces did.
    assert!(f.surface.normal_at(pts[0]).dot(look) > 0.0);
    // And back: flat faces along it, soft between them.
    let faces = all_faces(&m);
    convert::convert_faces(&mut m, &faces).unwrap();
    sound(&m);
    assert!(m.face_count() >= 6, "{}", m.face_count());
    assert!(m.faces().all(|(_, f)| f.plane().is_some()));
    let ef = m.edge_faces();
    for (id, e) in m.edges() {
        let inner = ef.get(&id).is_some_and(|v| v.len() == 2);
        assert_eq!(e.soft, inner, "soft between faces, plain round the rim");
    }
}

#[test]
fn faces_that_are_not_a_grid_on_their_own_are_refused() {
    // Five of a three-by-two grid: not rows and columns.
    let (m, _) = wavy(3, 2);
    let mut five = all_faces(&m);
    let last = *five.iter().next_back().unwrap();
    five.remove(&last);
    assert_eq!(convert::faces_refusal(&m, &five), Some(NOT_A_GRID));
    // A triangle.
    let mut t = Model::new();
    let p = DVec3::new;
    add_segment(&mut t, p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0));
    add_segment(&mut t, p(1.0, 0.0, 0.0), p(0.0, 1.0, 0.0));
    add_segment(&mut t, p(0.0, 1.0, 0.0), p(0.0, 0.0, 0.0));
    assert_eq!(convert::faces_refusal(&t, &all_faces(&t)), Some(NOT_A_GRID));
    // A box's top: its sides border the box's other faces.
    let mut b = Model::new();
    for (a, c) in [
        (p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0)),
        (p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0)),
        (p(1.0, 1.0, 0.0), p(0.0, 1.0, 0.0)),
        (p(0.0, 1.0, 0.0), p(0.0, 0.0, 0.0)),
    ] {
        add_segment(&mut b, a, c);
    }
    let base = b.faces().next().map(|(id, _)| id).unwrap();
    let up = b.face(base).unwrap().plane().unwrap().normal.z.signum();
    prism_ops::pushpull::push_pull(&mut b, base, up, false);
    let top: BTreeSet<FaceId> = b
        .faces()
        .filter(|(_, f)| {
            f.plane()
                .is_some_and(|pl| pl.normal.z > 0.99 && pl.origin.z > 0.5)
        })
        .map(|(id, _)| id)
        .collect();
    assert_eq!(top.len(), 1);
    assert_eq!(convert::faces_refusal(&b, &top), Some(BORDERS));
}
