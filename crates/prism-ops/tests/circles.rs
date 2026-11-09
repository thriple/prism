//! Exact circles (Exact Curves part 2): faces, holes, crossings and
//! transforms.

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

fn seg(m: &mut Model, a: (f64, f64), b: (f64, f64)) {
    prism_ops::add_segment(m, DVec3::new(a.0, a.1, 0.0), DVec3::new(b.0, b.1, 0.0));
}

fn sum(a: &[f64]) -> f64 {
    a.iter().sum()
}

/// The area of the slice of a unit circle cut off by a chord `h` from the
/// centre.
fn slice(h: f64) -> f64 {
    h.acos() - h * (1.0 - h * h).sqrt()
}

#[test]
fn a_line_across_a_circle_splits_its_face() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    seg(&mut m, (-2.0, 0.5), (2.0, 0.5));
    let a = areas(&m);
    assert_eq!(a.len(), 2);
    assert!((a[0] - slice(0.5)).abs() < 1e-9, "{a:?}");
    assert!((sum(&a) - PI).abs() < 1e-9);
    // Four arcs of one curve: the two halves, each cut once.
    assert_eq!(arcs(&m).len(), 4);
    let ids: BTreeSet<_> = m
        .edges()
        .filter(|(_, e)| e.curve.is_arc())
        .map(|(_, e)| e.curve_id)
        .collect();
    assert_eq!(ids.len(), 1);
    clean(&m);
}

#[test]
fn a_line_through_both_seams_still_splits_the_circle() {
    // The seams are at the centre ± the plane's u (y here): a straight edge
    // joins the same two vertices as both arcs.
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    seg(&mut m, (0.0, -1.0), (0.0, 1.0));
    let a = areas(&m);
    assert_eq!(a.len(), 2);
    assert!(a.iter().all(|x| (x - PI / 2.0).abs() < 1e-9), "{a:?}");
    clean(&m);
}

#[test]
fn a_line_ending_on_a_circle_splits_the_arc() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    seg(&mut m, (3.0, 0.0), (1.0, 0.0));
    assert_eq!(
        (m.vertex_count(), arcs(&m).len(), m.face_count()),
        (4, 3, 1)
    );
    // A chord from there splits the face.
    seg(&mut m, (1.0, 0.0), (-1.0, 0.0));
    let a = areas(&m);
    assert_eq!(a.len(), 2);
    assert!(a.iter().all(|x| (x - PI / 2.0).abs() < 1e-9), "{a:?}");
    clean(&m);
}

#[test]
fn a_circle_across_lines_splits_them_and_their_faces() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 10.0, 10.0, 0.0);
    // Centred on the left side: half inside the square, half out.
    circle(&mut m, DVec3::new(0.0, 5.0, 0.0), 2.0);
    let a = areas(&m);
    assert_eq!(a.len(), 3, "{a:?}");
    assert!((a[0] - 2.0 * PI).abs() < 1e-9 && (a[1] - 2.0 * PI).abs() < 1e-9);
    assert!((a[2] - (100.0 - 2.0 * PI)).abs() < 1e-9);
    clean(&m);
}

#[test]
fn two_overlapping_circles_make_three_faces() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    circle(&mut m, DVec3::new(1.0, 0.0, 0.0), 1.0);
    let a = areas(&m);
    assert_eq!(a.len(), 3, "{a:?}");
    let lens = 2.0 * PI / 3.0 - 3f64.sqrt() / 2.0;
    assert!(a.iter().any(|x| (x - lens).abs() < 1e-9), "{a:?}");
    assert!((sum(&a) - (2.0 * PI - lens)).abs() < 1e-9);
    clean(&m);
}

#[test]
fn touching_circles_and_lines_share_one_vertex() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    // Touching from outside at (1, 0), off the seams.
    circle(&mut m, DVec3::new(2.0, 0.0, 0.0), 1.0);
    assert_eq!((m.vertex_count(), m.face_count()), (5, 2));
    // A line touching the first circle at (-1, 0).
    seg(&mut m, (-1.0, -2.0), (-1.0, 2.0));
    assert_eq!((m.vertex_count(), m.face_count()), (8, 2));
    let a = areas(&m);
    assert!(a.iter().all(|x| (x - PI).abs() < 1e-9), "{a:?}");
    clean(&m);
}

#[test]
fn a_circle_drawn_again_over_a_cut_one_changes_nothing() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    seg(&mut m, (-2.0, 0.5), (2.0, 0.5));
    let before = m.clone();
    circle(&mut m, DVec3::ZERO, 1.0);
    assert_eq!(m, before);
}

#[test]
fn circles_touching_inside_at_a_shared_seam_do_not_overlap() {
    // Found by the property tests: the two circles meet at (6, 7), a seam
    // of both, leaving it the same way.
    let mut m = Model::new();
    seg(&mut m, (0.0, 0.0), (7.0, 7.0));
    circle(&mut m, DVec3::new(6.0, 5.0, 0.0), 2.0);
    circle(&mut m, DVec3::new(6.0, 4.0, 0.0), 3.0);
    clean(&m);
    assert!((sum(&areas(&m)) - 9.0 * PI).abs() < 1e-9);
}

#[test]
fn a_copy_moved_onto_its_circle_cuts_it_exactly() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    let sel = prism_ops::Selection {
        faces: m.faces().map(|(id, _)| id).collect(),
        ..Default::default()
    };
    let copy = prism_ops::duplicate(&mut m, &sel);
    let vs = copy.all_vertices(&m);
    transform::translate(&mut m, &vs, DVec3::new(1.0, 0.0, 0.0));
    let a = areas(&m);
    assert_eq!(a.len(), 3, "{a:?}");
    let lens = 2.0 * PI / 3.0 - 3f64.sqrt() / 2.0;
    assert!((sum(&a) - (2.0 * PI - lens)).abs() < 1e-9, "{a:?}");
    clean(&m);
}

#[test]
fn lifting_a_corner_of_a_face_with_arcs_is_refused() {
    // Found by the undo property test: the face cannot fold into flat
    // pieces yet, so the move would leave its arcs out of its plane.
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    circle(&mut m, DVec3::new(4.0, 2.0, 0.0), 1.0);
    let before = m.clone();
    let corner = prism_ops::vertex_at(&m, DVec3::ZERO).expect("corner");
    transform::translate(&mut m, &[corner].into(), DVec3::Z);
    assert_eq!(m, before);
    // Sliding it within the plane is fine.
    transform::translate(&mut m, &[corner].into(), DVec3::new(-1.0, 0.0, 0.0));
    assert_ne!(m, before);
    clean(&m);
}

#[test]
fn model_health_finds_arcs_off_their_circle_and_repairs_them() {
    let mut m = Model::new();
    circle(&mut m, DVec3::ZERO, 1.0);
    assert!(prism_ops::health::check(&m).off_circle.is_empty());
    // Knock a seam off the circle (as an old file or a bug might).
    let seam = m
        .vertices()
        .find(|(_, p)| p.distance(DVec3::Y) < 1e-9)
        .map(|(id, _)| id)
        .expect("a seam at (0, 1)");
    m.set_vertex(seam, DVec3::new(0.0, 1.01, 0.0));
    let r = prism_ops::health::check(&m);
    assert_eq!(r.off_circle.edges.len(), 2, "both halves end there");
    assert_eq!(prism_ops::health::repair_arcs(&mut m), 2);
    assert!(prism_ops::health::check(&m).off_circle.is_empty());
    assert!(m.vertex(seam).distance(DVec3::Y) < 1e-12);
    clean(&m);
    assert!((areas(&m)[0] - PI).abs() < 1e-9);
}
