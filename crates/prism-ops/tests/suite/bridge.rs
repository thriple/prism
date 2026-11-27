//! The bridge to the kernel (Fillet Joins step A, brief 32): Prism's closed
//! solids go to the kernel and come back with no edit, keeping every face,
//! id, paint and the volume. Loose geometry beside them is left alone.

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::bridge::{APPROXIMATE, region, round_trip, solid_of};
use prism_ops::{add_circle, add_polygon, add_rect, check, face_at, health, push_pull};
use prism_topo::{FaceId, Model};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Pull the face at `p` by `d` along +z.
fn pull(m: &mut Model, p: DVec3, d: f64) {
    let f = face_at(m, p).expect("a face there");
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(m, f, d * up, false);
}

/// The kernel's volume, the model left exactly as it was, and still clean.
fn same_after_round_trip(m: &mut Model, seed: DVec3) -> f64 {
    let before = m.clone();
    let seed = face_at(m, seed).expect("a face of the solid");
    let vol = round_trip(m, seed).expect("through the kernel and back");
    assert_eq!(*m, before, "the same faces, edges, ids and paint");
    assert_eq!(check::problems(m), Vec::<String>::new());
    vol
}

/// The volume Prism works out (Model Health's sum).
fn prism_volume(m: &Model, faces: &BTreeSet<FaceId>) -> f64 {
    faces.iter().map(|f| health::face_volume(m, *f)).sum()
}

#[test]
fn a_box_comes_back_the_same() {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 2.0, 3.0, 0.0);
    pull(&mut m, v(1.0, 1.0, 0.0), 4.0);
    // Painted, to show paint comes back too.
    let top = face_at(&m, v(1.0, 1.0, 4.0)).unwrap();
    m.edit_face(top, |f| f.paint.front = Some(prism_topo::MaterialId(3)));
    let vol = same_after_round_trip(&mut m, v(1.0, 1.0, 4.0));
    assert!((vol - 24.0).abs() < 1e-9, "{vol}");
}

#[test]
fn a_box_with_a_hole_through_it_comes_back_the_same() {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    pull(&mut m, v(0.5, 0.5, 0.0), 2.0);
    add_rect(&mut m, 1.0, 1.0, 3.0, 3.0, 2.0);
    pull(&mut m, v(2.0, 2.0, 2.0), -2.0);
    let faces = solid_of(&m, face_at(&m, v(0.5, 0.5, 2.0)).unwrap()).unwrap();
    assert_eq!(faces.len(), 10, "six sides and four walls of the hole");
    let vol = same_after_round_trip(&mut m, v(0.5, 0.5, 2.0));
    assert!((vol - 24.0).abs() < 1e-9, "{vol}");
}

#[test]
fn a_cylinder_comes_back_the_same() {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    pull(&mut m, DVec3::ZERO, 3.0);
    let faces = solid_of(&m, face_at(&m, v(0.0, 0.0, 3.0)).unwrap()).unwrap();
    let expect = prism_volume(&m, &faces);
    let vol = same_after_round_trip(&mut m, v(0.0, 0.0, 3.0));
    assert!((vol - 3.0 * std::f64::consts::PI).abs() < 1e-6, "{vol}");
    assert!((vol - expect).abs() < 1e-6, "{vol} {expect}");
}

#[test]
fn a_block_with_a_round_hole_comes_back_and_loose_geometry_is_untouched() {
    // A block, a circle on its top pushed down through it, and beside it a
    // loose rectangle and a loose line.
    let mut m = Model::new();
    add_rect(&mut m, -2.0, -2.0, 2.0, 2.0, 0.0);
    pull(&mut m, v(1.5, 1.5, 0.0), 1.0);
    add_circle(&mut m, v(0.0, 0.0, 1.0), DVec3::Z, 1.0);
    pull(&mut m, v(0.0, 0.0, 1.0), -1.0);
    add_rect(&mut m, 5.0, 0.0, 6.0, 1.0, 0.0);
    prism_ops::add_segment(&mut m, v(8.0, 0.0, 0.0), v(9.0, 1.0, 0.0));
    let loose = face_at(&m, v(5.5, 0.5, 0.0)).expect("the loose rectangle");
    assert!(solid_of(&m, loose).is_err(), "a loose face is not a solid");
    let faces = solid_of(&m, face_at(&m, v(1.5, 1.5, 1.0)).unwrap()).unwrap();
    assert!(!faces.contains(&loose));
    let vol = same_after_round_trip(&mut m, v(1.5, 1.5, 1.0));
    assert!((vol - (16.0 - std::f64::consts::PI)).abs() < 1e-6, "{vol}");
}

#[test]
fn an_l_shaped_prism_comes_back_the_same() {
    let mut m = Model::new();
    add_polygon(
        &mut m,
        &[
            v(0.0, 0.0, 0.0),
            v(3.0, 0.0, 0.0),
            v(3.0, 1.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(1.0, 2.0, 0.0),
            v(0.0, 2.0, 0.0),
        ],
    );
    pull(&mut m, v(0.5, 0.5, 0.0), 1.5);
    let vol = same_after_round_trip(&mut m, v(0.5, 0.5, 1.5));
    assert!((vol - 4.0 * 1.5).abs() < 1e-9, "{vol}");
}

#[test]
fn an_open_box_and_an_approximate_edge_are_refused() {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    pull(&mut m, v(1.0, 1.0, 0.0), 2.0);
    let top = face_at(&m, v(1.0, 1.0, 2.0)).unwrap();
    let mut open = m.clone();
    open.remove_face(top);
    let side = face_at(&open, v(1.0, 0.0, 1.0)).unwrap();
    assert_eq!(
        solid_of(&open, side).err(),
        Some(prism_kernel::solid::NOT_CLOSED)
    );
    let faces = solid_of(&m, top).unwrap();
    let e = m.face(top).unwrap().edge_ids().next().unwrap();
    m.set_approx(e, true);
    assert_eq!(region(&m, &faces).err(), Some(APPROXIMATE));
}

#[test]
fn two_boxes_touching_along_an_edge_are_two_solids() {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 1.0, 1.0, 0.0);
    add_rect(&mut m, 1.0, 1.0, 2.0, 2.0, 0.0);
    pull(&mut m, v(0.5, 0.5, 0.0), 1.0);
    pull(&mut m, v(1.5, 1.5, 0.0), 1.0);
    let shared = m
        .edge_faces()
        .into_values()
        .filter(|fs| fs.len() == 4)
        .count();
    assert_eq!(shared, 1, "one edge with four faces");
    let a = solid_of(&m, face_at(&m, v(0.5, 0.5, 1.0)).unwrap()).unwrap();
    let b = solid_of(&m, face_at(&m, v(1.5, 1.5, 1.0)).unwrap()).unwrap();
    assert_eq!((a.len(), b.len()), (6, 6));
    assert!(a.is_disjoint(&b));
    let vol = same_after_round_trip(&mut m, v(0.5, 0.5, 1.0));
    assert!((vol - 1.0).abs() < 1e-9, "{vol}");
}
