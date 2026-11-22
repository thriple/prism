//! Trim, Extend and Project Curves (Shapes & Placement part 5).

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::trim;
use prism_ops::{add_circle, add_segment, check, face_at, vertex_at};
use prism_topo::{EdgeId, Model};

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn edge_between(m: &Model, a: DVec3, b: DVec3) -> EdgeId {
    let (va, vb) = (vertex_at(m, a).expect("a"), vertex_at(m, b).expect("b"));
    m.edges()
        .find(|(_, e)| e.has(va) && e.has(vb))
        .map(|(id, _)| id)
        .expect("an edge")
}

#[test]
fn extend_carries_a_free_end_on_to_the_first_edge_and_cuts_it() {
    let mut m = Model::new();
    add_segment(&mut m, DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
    // A wall of lines across the way at x = 3, and a nearer one at x = 2.
    add_segment(
        &mut m,
        DVec3::new(3.0, -1.0, 0.0),
        DVec3::new(3.0, 1.0, 0.0),
    );
    add_segment(
        &mut m,
        DVec3::new(2.0, -1.0, 0.0),
        DVec3::new(2.0, 1.0, 0.0),
    );
    let e = edge_between(&m, DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
    let end = vertex_at(&m, DVec3::new(1.0, 0.0, 0.0)).unwrap();
    let (from, to) = trim::extension(&m, e, end).expect("meets the near one");
    assert!(from.distance(DVec3::new(1.0, 0.0, 0.0)) < 1e-9);
    assert!(to.distance(DVec3::new(2.0, 0.0, 0.0)) < 1e-9, "{to:?}");
    trim::extend(&mut m, e, end).expect("extends");
    // It now reaches x = 2, and cut that line in two there.
    let at = vertex_at(&m, DVec3::new(2.0, 0.0, 0.0)).expect("a vertex where it met");
    assert_eq!(m.edges().filter(|(_, x)| x.has(at)).count(), 3);
    clean(&m);
    // The end it now has is joined, so it cannot be carried on from there.
    let e2 = edge_between(&m, DVec3::new(1.0, 0.0, 0.0), DVec3::new(2.0, 0.0, 0.0));
    assert_eq!(trim::extension(&m, e2, at).err(), Some(trim::END_JOINED));
}

#[test]
fn extend_says_when_nothing_is_in_the_way_or_the_edge_is_curved() {
    let mut m = Model::new();
    add_segment(&mut m, DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
    let e = edge_between(&m, DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
    let end = vertex_at(&m, DVec3::new(1.0, 0.0, 0.0)).unwrap();
    assert_eq!(
        trim::extension(&m, e, end).err(),
        Some(trim::NOTHING_IN_THE_WAY)
    );
    let mut c = Model::new();
    add_circle(&mut c, DVec3::ZERO, DVec3::Z, 1.0);
    let (ce, edge) = c.edges().next().map(|(id, x)| (id, x.clone())).unwrap();
    assert_eq!(
        trim::extension(&c, ce, edge.a).err(),
        Some(trim::NOT_STRAIGHT)
    );
}

#[test]
fn extend_stops_at_a_face_in_the_way() {
    let mut m = Model::new();
    // A square wall in the y-z plane at x = 4, and a line coming at it.
    for (a, b) in [
        ((4.0, -1.0, 0.0), (4.0, 1.0, 0.0)),
        ((4.0, 1.0, 0.0), (4.0, 1.0, 2.0)),
        ((4.0, 1.0, 2.0), (4.0, -1.0, 2.0)),
        ((4.0, -1.0, 2.0), (4.0, -1.0, 0.0)),
    ] {
        add_segment(&mut m, DVec3::new(a.0, a.1, a.2), DVec3::new(b.0, b.1, b.2));
    }
    assert!(
        face_at(&m, DVec3::new(4.0, 0.0, 1.0)).is_some(),
        "the wall is a face"
    );
    add_segment(&mut m, DVec3::new(0.0, 0.0, 1.0), DVec3::new(1.0, 0.0, 1.0));
    let e = edge_between(&m, DVec3::new(0.0, 0.0, 1.0), DVec3::new(1.0, 0.0, 1.0));
    let end = vertex_at(&m, DVec3::new(1.0, 0.0, 1.0)).unwrap();
    let (_, to) = trim::extension(&m, e, end).expect("meets the wall");
    assert!(to.distance(DVec3::new(4.0, 0.0, 1.0)) < 1e-9, "{to:?}");
}

#[test]
fn trim_refuses_an_edge_that_bounds_a_face() {
    let mut m = Model::new();
    for (a, b) in [
        ((0.0, 0.0), (1.0, 0.0)),
        ((1.0, 0.0), (1.0, 1.0)),
        ((1.0, 1.0), (0.0, 1.0)),
        ((0.0, 1.0), (0.0, 0.0)),
    ] {
        add_segment(&mut m, DVec3::new(a.0, a.1, 0.0), DVec3::new(b.0, b.1, 0.0));
    }
    add_segment(&mut m, DVec3::new(1.0, 0.0, 0.0), DVec3::new(3.0, 0.0, 0.0));
    let side = edge_between(&m, DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
    let tail = edge_between(&m, DVec3::new(1.0, 0.0, 0.0), DVec3::new(3.0, 0.0, 0.0));
    assert_eq!(trim::trim_refusal(&m, side), Some(trim::NOT_LOOSE));
    assert_eq!(trim::trim_refusal(&m, tail), None);
}

#[test]
fn project_drops_lines_and_parallel_circles_onto_a_face_and_cuts_it() {
    let mut m = Model::new();
    for (a, b) in [
        ((0.0, 0.0), (1.0, 0.0)),
        ((1.0, 0.0), (1.0, 1.0)),
        ((1.0, 1.0), (0.0, 1.0)),
        ((0.0, 1.0), (0.0, 0.0)),
    ] {
        add_segment(&mut m, DVec3::new(a.0, a.1, 0.0), DVec3::new(b.0, b.1, 0.0));
    }
    let face = face_at(&m, DVec3::new(0.5, 0.5, 0.0)).expect("a face");
    // A line and a circle floating above it, on a second model's edges.
    let mut up = Model::new();
    add_segment(
        &mut up,
        DVec3::new(0.5, -1.0, 3.0),
        DVec3::new(0.5, 2.0, 3.0),
    );
    add_circle(&mut up, DVec3::new(0.2, 0.5, 3.0), DVec3::Z, 0.1);
    let n = m.append(&up);
    assert!(!n.is_empty() || m.edge_count() > 4);
    let floating: BTreeSet<EdgeId> = m
        .edges()
        .filter(|(_, e)| m.vertex(e.a).z > 2.0 && m.vertex(e.b).z > 2.0)
        .map(|(id, _)| id)
        .collect();
    assert!(floating.len() >= 2, "{}", floating.len());
    let faces = m.face_count();
    let (done, left) = trim::project(&mut m, &floating, face).expect("projects");
    // The line, and the circle in its two halves.
    assert_eq!((done, left), (3, 0));
    assert!(
        m.face_count() > faces,
        "the line and the circle cut the face"
    );
    assert!(
        vertex_at(&m, DVec3::new(0.5, 0.0, 0.0)).is_some(),
        "where the line crosses the side"
    );
    clean(&m);
}

#[test]
fn project_says_what_it_cannot_do() {
    let mut m = Model::new();
    for (a, b) in [
        ((0.0, 0.0), (1.0, 0.0)),
        ((1.0, 0.0), (1.0, 1.0)),
        ((1.0, 1.0), (0.0, 1.0)),
        ((0.0, 1.0), (0.0, 0.0)),
    ] {
        add_segment(&mut m, DVec3::new(a.0, a.1, 0.0), DVec3::new(b.0, b.1, 0.0));
    }
    let face = face_at(&m, DVec3::new(0.5, 0.5, 0.0)).expect("a face");
    // A line square to the face has nothing to drop; so has the face's own edge.
    add_segment(&mut m, DVec3::new(2.0, 2.0, 0.0), DVec3::new(2.0, 2.0, 1.0));
    let upright = edge_between(&m, DVec3::new(2.0, 2.0, 0.0), DVec3::new(2.0, 2.0, 1.0));
    let own = edge_between(&m, DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
    let sel: BTreeSet<EdgeId> = [upright, own].into();
    assert_eq!(
        trim::project(&mut m, &sel, face).err(),
        Some(trim::NOTHING_TO_PROJECT)
    );
}
