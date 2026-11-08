//! Transforms, erase, duplicate and connected, beyond the golden cases.

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::select::Selection;
use prism_ops::{check, transform};
use prism_topo::{Model, VertexId};

fn boxed(k: f64) -> Model {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0 * k, 4.0 * k, 0.0);
    let f = prism_ops::face_at(&m, DVec3::new(2.0, 2.0, 0.0) * k).expect("face");
    prism_ops::push_pull(&mut m, f, 2.0 * k, false);
    m
}

fn all(m: &Model) -> BTreeSet<VertexId> {
    m.vertices().map(|(id, _)| id).collect()
}

#[test]
fn rotating_a_box_keeps_every_face_planar_and_whole() {
    for k in [1e-4, 1.0, 1e3] {
        let mut m = boxed(k);
        let faces_before: Vec<_> = m.faces().map(|(id, _)| id).collect();
        let vs = all(&m);
        transform::rotate(
            &mut m,
            &vs,
            DVec3::new(1.0, 2.0, 0.5) * k,
            DVec3::new(1.0, 1.0, 3.0),
            0.7,
        );
        transform::rotate(&mut m, &vs, DVec3::ZERO, DVec3::new(-2.0, 0.3, 1.0), 2.1);
        let faces_after: Vec<_> = m.faces().map(|(id, _)| id).collect();
        assert_eq!(
            faces_before, faces_after,
            "no face was split or triangulated"
        );
        assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
        assert!((m.face_area(faces_after[0]) - 16.0 * k * k).abs() < 1e-6 * k * k * 16.0);
    }
}

#[test]
fn scaling_keeps_faces_planar() {
    let mut m = boxed(1.0);
    let vs = all(&m);
    transform::scale(
        &mut m,
        &vs,
        DVec3::new(2.0, 2.0, 1.0),
        DVec3::new(1.5, 0.5, 3.0),
    );
    assert_eq!(m.face_count(), 6);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
}

#[test]
fn lifting_one_corner_folds_the_face_instead_of_triangulating_everything() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    let corner = prism_ops::vertex_at(&m, DVec3::new(4.0, 4.0, 0.0)).expect("corner");
    transform::translate(&mut m, &[corner].into(), DVec3::new(0.0, 0.0, 1.0));
    // One fold along a diagonal: two planar faces, one new edge.
    assert_eq!(m.face_count(), 2);
    assert_eq!(m.edge_count(), 5);
    assert!(check::planar(&m, &m.tol()).is_empty());
}

#[test]
fn moving_a_point_onto_another_welds_them() {
    let mut m = Model::new();
    prism_ops::add_segment(&mut m, DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0));
    prism_ops::add_segment(&mut m, DVec3::new(6.0, 0.0, 0.0), DVec3::new(6.0, 3.0, 0.0));
    let end = prism_ops::vertex_at(&m, DVec3::new(6.0, 0.0, 0.0)).expect("end");
    transform::translate(&mut m, &[end].into(), DVec3::new(-2.0, 0.0, 0.0));
    assert_eq!(
        m.vertex_count(),
        3,
        "the two ends at (4, 0, 0) are one vertex now"
    );
    assert!(check::no_duplicates(&m, &m.tol()).is_empty());
}

#[test]
fn erasing_the_line_across_a_face_removes_both_halves() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    prism_ops::add_segment(&mut m, DVec3::new(0.0, 2.0, 0.0), DVec3::new(4.0, 2.0, 0.0));
    assert_eq!(m.face_count(), 2);
    let a = prism_ops::vertex_at(&m, DVec3::new(0.0, 2.0, 0.0)).expect("a");
    let b = prism_ops::vertex_at(&m, DVec3::new(4.0, 2.0, 0.0)).expect("b");
    let e = m.edge_between(a, b).expect("middle edge");
    prism_ops::remove(&mut m, &Selection::edge(e));
    assert_eq!(
        m.face_count(),
        0,
        "as the prototype: faces using the edge go"
    );
    assert_eq!(m.edge_count(), 6);
}

#[test]
fn connected_and_duplicate() {
    let mut m = boxed(1.0);
    prism_ops::add_segment(
        &mut m,
        DVec3::new(10.0, 0.0, 0.0),
        DVec3::new(12.0, 0.0, 0.0),
    );
    let corner = prism_ops::vertex_at(&m, DVec3::ZERO).expect("corner");
    let sel = prism_ops::connected(&m, corner);
    assert_eq!(
        (sel.vertices.len(), sel.edges.len(), sel.faces.len()),
        (8, 12, 6)
    );
    let copy = prism_ops::duplicate(&mut m, &sel);
    assert_eq!((copy.edges.len(), copy.faces.len()), (12, 6));
    // Move the copy away; the original is untouched and the copy is a box.
    let vs = copy.all_vertices(&m);
    transform::translate(&mut m, &vs, DVec3::new(0.0, 10.0, 0.0));
    assert_eq!(m.face_count(), 12);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
}

#[test]
fn line_crossing_a_line_splits_both() {
    let mut m = Model::new();
    prism_ops::add_segment(&mut m, DVec3::new(0.0, 0.0, 0.0), DVec3::new(4.0, 4.0, 0.0));
    prism_ops::add_segment(&mut m, DVec3::new(0.0, 4.0, 0.0), DVec3::new(4.0, 0.0, 0.0));
    assert_eq!((m.vertex_count(), m.edge_count()), (5, 4));
    // A segment through an existing vertex splits there too.
    prism_ops::add_segment(&mut m, DVec3::new(2.0, 0.0, 0.0), DVec3::new(2.0, 4.0, 0.0));
    assert_eq!((m.vertex_count(), m.edge_count()), (7, 6));
}

#[test]
fn a_big_polygon_makes_a_face() {
    // The prototype's search gave up beyond 14 vertices.
    let mut m = Model::new();
    let n = 40;
    let pt = |i: usize| {
        let a = std::f64::consts::TAU * i as f64 / n as f64;
        DVec3::new(10.0 * a.cos(), 10.0 * a.sin(), 0.0)
    };
    for i in 0..n {
        prism_ops::add_segment(&mut m, pt(i), pt((i + 1) % n));
    }
    assert_eq!(m.face_count(), 1);
    assert_eq!(m.faces().next().expect("face").1.outer.len(), n);
}

#[test]
fn drawing_around_islands_makes_them_holes() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 1.0, 1.0, 2.0, 2.0, 0.0);
    prism_ops::add_rect(&mut m, 3.0, 1.0, 4.0, 2.0, 0.0);
    prism_ops::add_rect(&mut m, 0.0, 0.0, 5.0, 3.0, 0.0);
    let host = prism_ops::face_at(&m, DVec3::new(0.5, 0.5, 0.0)).expect("host");
    assert_eq!(m.face(host).expect("face").holes.len(), 2);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
}

#[test]
fn a_line_drawn_up_a_cube_side_splits_that_side() {
    let mut m = boxed(1.0);
    prism_ops::add_segment(&mut m, DVec3::new(2.0, 0.0, 0.0), DVec3::new(2.0, 0.0, 2.0));
    assert_eq!(m.face_count(), 7);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
}

/// DECISIONS #15 (changed after the owner tried it): erasing the edge
/// between two faces on one plane leaves one face, as in SketchUp.
#[test]
fn erasing_the_edge_between_two_flat_faces_merges_them() {
    for k in [1e-3, 1.0, 1e3] {
        let mut m = Model::new();
        // The owner's L: a square and a smaller rectangle beside it.
        prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0 * k, 4.0 * k, 0.0);
        prism_ops::add_rect(&mut m, 4.0 * k, 0.0, 7.0 * k, 2.0 * k, 0.0);
        assert_eq!(m.face_count(), 2);
        let a = prism_ops::vertex_at(&m, DVec3::new(4.0, 0.0, 0.0) * k).unwrap();
        let b = prism_ops::vertex_at(&m, DVec3::new(4.0, 2.0, 0.0) * k).unwrap();
        let shared = m.edge_between(a, b).expect("the shared edge");
        prism_ops::remove(&mut m, &Selection::edge(shared));
        assert_eq!(m.face_count(), 1, "one face is left");
        let (id, _) = m.faces().next().unwrap();
        assert!((m.face_area(id) - 22.0 * k * k).abs() < 1e-9 * k * k);
        assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
        // The merged face can be offset, as the owner wanted.
        assert!(prism_ops::offset::offset(&mut m, id, 0.5 * k).is_some());
        assert!(check::problems(&m).is_empty());
    }
}

/// Faces that meet at an angle are both erased with their edge.
#[test]
fn erasing_an_edge_between_faces_at_an_angle_takes_both() {
    let mut m = boxed(1.0);
    let before = m.face_count();
    let a = prism_ops::vertex_at(&m, DVec3::new(0.0, 0.0, 0.0)).unwrap();
    let b = prism_ops::vertex_at(&m, DVec3::new(4.0, 0.0, 0.0)).unwrap();
    let e = m.edge_between(a, b).expect("a bottom edge");
    prism_ops::remove(&mut m, &Selection::edge(e));
    assert_eq!(m.face_count(), before - 2);
}

/// The owner's case: two rectangles sharing an edge, lined up on one side,
/// both pulled up. Erasing the vertical edge on the lined-up side merges the
/// two side faces into one, as in SketchUp.
#[test]
fn erasing_a_vertical_edge_between_two_lined_up_walls_merges_them() {
    for k in [1e-3, 1.0, 1e3] {
        let mut m = Model::new();
        prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0 * k, 4.0 * k, 0.0);
        prism_ops::add_rect(&mut m, 4.0 * k, 0.0, 7.0 * k, 2.0 * k, 0.0);
        for p in [DVec3::new(2.0, 2.0, 0.0), DVec3::new(5.5, 1.0, 0.0)] {
            let f = prism_ops::face_at(&m, p * k).expect("face");
            prism_ops::push_pull(&mut m, f, 3.0 * k, false);
        }
        let a = prism_ops::vertex_at(&m, DVec3::new(4.0, 0.0, 0.0) * k).unwrap();
        let b = prism_ops::vertex_at(&m, DVec3::new(4.0, 0.0, 3.0) * k).unwrap();
        let e = m.edge_between(a, b).expect("the vertical edge");
        prism_ops::remove(&mut m, &Selection::edge(e));
        let front = prism_ops::face_at(&m, DVec3::new(3.5, 0.0, 1.5) * k).expect("a wall is left");
        assert!(
            (m.face_area(front) - 21.0 * k * k).abs() < 1e-9 * k * k,
            "one merged wall"
        );
        assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    }
}

/// A small box set flat against a tall one is one shape with it: moving
/// its top still reshapes the shared wall cleanly, and the tall box's
/// outer corner stays where it was.
#[test]
fn moving_the_top_of_a_box_set_against_a_wall() {
    let mut m = Model::new();
    let pull = |m: &mut Model, at: DVec3, h: f64| {
        let f = prism_ops::face_at(m, at).expect("base");
        let up = m.face(f).expect("f").plane().normal.z > 0.0;
        prism_ops::push_pull(m, f, if up { h } else { -h }, false);
    };
    prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    pull(&mut m, DVec3::new(1.0, 1.0, 0.0), 3.0);
    prism_ops::add_rect(&mut m, 2.0, 0.0, 3.0, 1.0, 0.0);
    pull(&mut m, DVec3::new(2.5, 0.5, 0.0), 1.0);
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    let top: BTreeSet<VertexId> = m
        .vertices()
        .filter(|(_, p)| (p.z - 1.0).abs() < 1e-9 && p.x > 1.5)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(top.len(), 4);
    transform::translate(&mut m, &top, DVec3::new(0.0, -0.2, 0.0));
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    // The tall box's wall at x = 2 still has its straight corner edge: no
    // face there reaches out to y = -0.2.
    for (_, f) in m.faces() {
        let pts = m.points(&f.outer);
        if f.plane().normal.x.abs() > 0.99 && pts.iter().any(|p| p.z > 2.0) {
            assert!(
                pts.iter().all(|p| p.y > -1e-9),
                "the tall wall bent: {pts:?}"
            );
        }
    }
}

/// A box touching a tall one only along a corner line: moving its top
/// leaves the tall box's corner edge straight, as its corner only split
/// that edge (owner feedback, 2026-11-09: the edge bent and left slivers
/// and a reversed triangle).
#[test]
fn moving_a_box_top_leaves_a_corner_neighbours_edge_straight() {
    let mut m = Model::new();
    let pull = |m: &mut Model, at: DVec3, h: f64| {
        let f = prism_ops::face_at(m, at).expect("base");
        let up = m.face(f).expect("f").plane().normal.z > 0.0;
        prism_ops::push_pull(m, f, if up { h } else { -h }, false);
    };
    prism_ops::add_rect(&mut m, 3.0, 1.0, 4.0, 2.0, 0.0);
    pull(&mut m, DVec3::new(3.5, 1.5, 0.0), 3.0);
    prism_ops::add_rect(&mut m, 2.0, 0.0, 3.0, 1.0, 0.0);
    pull(&mut m, DVec3::new(2.5, 0.5, 0.0), 1.0);
    let top: BTreeSet<VertexId> = m
        .vertices()
        .filter(|(_, p)| (p.z - 1.0).abs() < 1e-9 && p.x < 3.0 + 1e-9 && p.y < 1.0 + 1e-9)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(top.len(), 4);
    transform::translate(&mut m, &top, DVec3::new(-0.2, 0.0, 0.0));
    assert!(check::problems(&m).is_empty(), "{:?}", check::problems(&m));
    // Every face of the tall box stays within its own footprint.
    for (_, f) in m.faces() {
        let pts = m.points(&f.outer);
        if pts.iter().any(|p| p.z > 2.0) {
            assert!(
                pts.iter().all(|p| p.x > 3.0 - 1e-9 && p.y > 1.0 - 1e-9),
                "the tall box bent: {pts:?}"
            );
        }
    }
}
