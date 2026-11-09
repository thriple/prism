//! Exact circles (Exact Curves part 2a): faces, holes and transforms.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::Curve;
use prism_math::DVec3;
use prism_ops::{check, transform};
use prism_topo::{FaceId, Model, VertexId};

fn circle(m: &mut Model, at: DVec3, r: f64) {
    prism_ops::add_circle(m, at, DVec3::Z, r);
}

fn all(m: &Model) -> BTreeSet<VertexId> {
    m.vertices().map(|(id, _)| id).collect()
}

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

/// Each arc's centre and radius.
fn arcs(m: &Model) -> Vec<(DVec3, f64)> {
    m.edges()
        .filter_map(|(_, e)| match e.curve {
            Curve::Arc { centre, radius, .. } => Some((centre, radius)),
            Curve::Line => None,
        })
        .collect()
}

fn areas(m: &Model) -> Vec<f64> {
    let mut a: Vec<f64> = m.faces().map(|(id, _)| m.face_area(id)).collect();
    a.sort_by(f64::total_cmp);
    a
}

#[test]
fn a_circle_alone_is_a_face_of_its_exact_area() {
    let mut m = Model::new();
    circle(&mut m, DVec3::new(1.0, 2.0, 0.0), 3.0);
    assert_eq!(
        (m.vertex_count(), m.edge_count(), m.face_count()),
        (2, 2, 1)
    );
    let a = areas(&m)[0];
    assert!((a - PI * 9.0).abs() < 1e-9, "{a}");
    // Both halves are one curve.
    let ids: BTreeSet<_> = m.edges().map(|(_, e)| e.curve_id).collect();
    assert_eq!(ids.len(), 1);
    assert!(ids.iter().all(Option::is_some));
    clean(&m);
}

#[test]
fn a_circle_inside_a_face_makes_a_hole_and_a_face() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 10.0, 10.0, 0.0);
    circle(&mut m, DVec3::new(5.0, 5.0, 0.0), 2.0);
    assert_eq!(m.face_count(), 2);
    let a = areas(&m);
    assert!((a[0] - PI * 4.0).abs() < 1e-9);
    assert!((a[1] - (100.0 - PI * 4.0)).abs() < 1e-9);
    assert_eq!(m.faces().map(|(_, f)| f.holes.len()).sum::<usize>(), 1);
    clean(&m);
}

#[test]
fn two_circles_side_by_side_are_two_faces() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    circle(&mut m, DVec3::new(5.0, 0.0, 0.0), 1.0);
    assert_eq!(m.face_count(), 2);
    clean(&m);
}

#[test]
fn moves_turns_and_even_scales_keep_circles_exact() {
    let mut m = Model::new();
    circle(&mut m, DVec3::new(1.0, 0.0, 0.0), 2.0);
    let vs = all(&m);
    transform::translate(&mut m, &vs, DVec3::new(0.0, 3.0, 1.0));
    assert!(
        arcs(&m)
            .iter()
            .all(|(c, r)| c.distance(DVec3::new(1.0, 3.0, 1.0)) < 1e-12 && *r == 2.0)
    );
    transform::rotate(&mut m, &vs, DVec3::ZERO, DVec3::X, PI / 2.0);
    // Now standing up: its face turned with it.
    let n = m.face(FaceId(0)).expect("face").plane().normal;
    assert!(n.z.abs() < 1e-9, "{n}");
    transform::scale(&mut m, &vs, DVec3::ZERO, DVec3::splat(2.0));
    assert!(arcs(&m).iter().all(|(_, r)| (*r - 4.0).abs() < 1e-9));
    assert!((areas(&m)[0] - PI * 16.0).abs() < 1e-8);
    clean(&m);
}

#[test]
fn an_uneven_scale_or_a_bent_arc_is_refused() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    let before = m.clone();
    let vs = all(&m);
    transform::scale(&mut m, &vs, DVec3::ZERO, DVec3::new(2.0, 1.0, 1.0));
    assert_eq!(m, before);
    // Moving one end alone would bend the arcs.
    let one: BTreeSet<VertexId> = vs.iter().take(1).copied().collect();
    transform::translate(&mut m, &one, DVec3::X);
    assert_eq!(m, before);
}

#[test]
fn a_mirror_keeps_the_circle_and_its_face() {
    let mut m = Model::new();
    circle(&mut m, DVec3::new(2.0, 0.0, 0.0), 1.0);
    transform::affine(
        &mut m,
        &prism_math::DAffine3::from_scale(DVec3::new(-1.0, 1.0, 1.0)),
    );
    assert_eq!(m.face_count(), 1);
    assert!(
        arcs(&m)
            .iter()
            .all(|(c, _)| c.distance(DVec3::new(-2.0, 0.0, 0.0)) < 1e-12)
    );
    clean(&m);
}

#[test]
fn erasing_a_circle_takes_its_face() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    let sel = prism_ops::Selection {
        edges: m.edges().map(|(id, _)| id).collect(),
        ..Default::default()
    };
    prism_ops::remove(&mut m, &sel);
    assert_eq!(
        (m.edge_count(), m.face_count(), m.vertex_count()),
        (0, 0, 0)
    );
}

#[test]
fn a_circle_survives_a_line_drawn_elsewhere_in_its_plane() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    prism_ops::add_segment(&mut m, DVec3::new(3.0, 0.0, 0.0), DVec3::new(5.0, 0.0, 0.0));
    assert_eq!(m.face_count(), 1);
    clean(&m);
}

#[test]
fn copies_and_groups_keep_arcs() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    let sel = prism_ops::Selection {
        faces: m.faces().map(|(id, _)| id).collect(),
        ..Default::default()
    };
    let copy = prism_ops::duplicate(&mut m, &sel);
    let vs = copy.all_vertices(&m);
    transform::translate(&mut m, &vs, DVec3::new(4.0, 0.0, 0.0));
    assert_eq!(arcs(&m).len(), 4);
    assert_eq!(m.face_count(), 2);
    // A new curve id for the copy.
    let ids: BTreeSet<_> = m.edges().map(|(_, e)| e.curve_id).collect();
    assert_eq!(ids.len(), 2);
    clean(&m);
    let group = prism_ops::select::take(&mut m, &copy);
    assert_eq!(arcs(&group).len(), 2);
    clean(&group);
    clean(&m);
}

#[test]
fn push_pull_and_offset_wait_for_their_parts() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    let before = m.clone();
    prism_ops::push_pull(&mut m, FaceId(0), 1.0, false);
    assert_eq!(prism_ops::offset::offset(&mut m, FaceId(0), 0.1), None);
    assert_eq!(m, before);
}
