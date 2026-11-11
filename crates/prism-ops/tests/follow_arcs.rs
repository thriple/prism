//! Follow Me along arcs (Exact Curves 5d): a profile turned about each
//! arc's axis makes flat rings, cylinders, cones, spheres and tori, all
//! exact; a lathe (a profile round a circle about its axis) makes a closed
//! solid of revolution; lines and arcs joined tangentially make a bend;
//! and what cannot be swept exactly is refused with a reason.

use std::collections::BTreeSet;
use std::f64::consts::{FRAC_PI_2, PI};

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::follow::{self, CROSSES_AXIS};
use prism_ops::{add_arc, add_circle, add_segment, check, health::face_volume};
use prism_topo::{EdgeId, FaceId, Model};

fn volume(m: &Model) -> f64 {
    m.faces().map(|(id, _)| face_volume(m, id)).sum()
}

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

/// A flat face through `pts` (lines), found by its normal.
fn poly(m: &mut Model, pts: &[DVec3], normal: DVec3) -> FaceId {
    for i in 0..pts.len() {
        add_segment(m, pts[i], pts[(i + 1) % pts.len()]);
    }
    m.faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.normal.dot(normal).abs() > 0.99))
        .map(|(id, _)| id)
        .expect("the face")
}

/// The edges new since `before`.
fn new_edges(m: &Model, before: &BTreeSet<EdgeId>) -> BTreeSet<EdgeId> {
    m.edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect()
}

fn edges(m: &Model) -> BTreeSet<EdgeId> {
    m.edges().map(|(id, _)| id).collect()
}

/// A circle about the z axis at height `z` as the path, and the face it
/// fills (left out of volumes).
fn circle_path(m: &mut Model, z: f64, r: f64) -> (BTreeSet<EdgeId>, FaceId) {
    let before = edges(m);
    let faces: BTreeSet<FaceId> = m.faces().map(|(id, _)| id).collect();
    add_circle(m, DVec3::new(0.0, 0.0, z), DVec3::Z, r);
    let disc = m
        .faces()
        .map(|(id, _)| id)
        .find(|id| !faces.contains(id))
        .expect("the path's disc");
    (new_edges(m, &before), disc)
}

fn count(m: &Model, f: impl Fn(&Surface) -> bool) -> usize {
    m.faces().filter(|(_, x)| f(&x.surface)).count()
}

#[test]
fn a_rectangle_turned_round_a_circle_makes_a_tube() {
    for k in [1e-3, 1.0, 1e3] {
        let mut m = Model::new();
        let prof = poly(
            &mut m,
            &[
                DVec3::new(1.0, 0.0, 0.0) * k,
                DVec3::new(2.0, 0.0, 0.0) * k,
                DVec3::new(2.0, 0.0, 1.0) * k,
                DVec3::new(1.0, 0.0, 1.0) * k,
            ],
            DVec3::Y,
        );
        let (p, disc) = circle_path(&mut m, 0.0, 0.5 * k);
        assert_eq!(follow::refusal(&m, &p, prof), None);
        assert!(follow::follow(&mut m, &p, prof));
        clean(&m);
        // Per half of the circle: two flat rings and two cylinders.
        assert_eq!(count(&m, |s| matches!(s, Surface::Cylinder(_))), 4);
        assert_eq!(m.face_count(), 8 + 1);
        close(volume(&m) - face_volume(&m, disc), 3.0 * PI * k.powi(3));
        // The seams between the halves are soft and smooth.
        let soft = m.edges().filter(|(_, e)| e.soft && e.smooth).count();
        assert_eq!(soft, 8);
    }
}

#[test]
fn a_half_disc_turned_about_its_diameter_makes_a_sphere() {
    let mut m = Model::new();
    // The diameter on the z axis, the half circle through +x.
    add_segment(
        &mut m,
        DVec3::new(0.0, 0.0, -1.0),
        DVec3::new(0.0, 0.0, 1.0),
    );
    add_arc(
        &mut m,
        DVec3::ZERO,
        -DVec3::Y,
        1.0,
        DVec3::new(0.0, 0.0, -1.0),
        PI,
    );
    let prof = m.faces().next().map(|(id, _)| id).expect("a half disc");
    let (p, disc) = circle_path(&mut m, 3.0, 0.5);
    assert_eq!(follow::refusal(&m, &p, prof), None);
    assert!(follow::follow(&mut m, &p, prof));
    clean(&m);
    // Two halves of a sphere; the diameter went with the profile.
    assert_eq!(count(&m, |s| matches!(s, Surface::Sphere(_))), 2);
    assert_eq!(m.face_count(), 2 + 1);
    close(volume(&m) - face_volume(&m, disc), 4.0 / 3.0 * PI);
}

#[test]
fn a_triangle_turned_about_a_side_makes_a_cone() {
    let mut m = Model::new();
    let prof = poly(
        &mut m,
        &[
            DVec3::ZERO,
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 2.0),
        ],
        DVec3::Y,
    );
    let (p, disc) = circle_path(&mut m, 3.0, 0.5);
    assert!(follow::follow(&mut m, &p, prof));
    clean(&m);
    assert_eq!(count(&m, |s| matches!(s, Surface::Cone(_))), 2);
    // Two cone halves and two halves of the base.
    assert_eq!(m.face_count(), 4 + 1);
    close(volume(&m) - face_volume(&m, disc), PI * 2.0 / 3.0);
}

#[test]
fn a_disc_turned_round_a_circle_makes_a_torus() {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::new(2.0, 0.0, 0.0), DVec3::Y, 0.5);
    let prof = m.faces().next().map(|(id, _)| id).expect("a disc");
    let (p, disc) = circle_path(&mut m, 3.0, 1.0);
    assert!(follow::follow(&mut m, &p, prof));
    clean(&m);
    assert_eq!(count(&m, |s| matches!(s, Surface::Torus(_))), 4);
    assert_eq!(m.face_count(), 4 + 1);
    // 2π² R r².
    close(
        volume(&m) - face_volume(&m, disc),
        2.0 * PI * PI * 2.0 * 0.25,
    );
    // Soft where the disc's halves meet, and where the path's halves do.
    let soft = m.edges().filter(|(_, e)| e.soft && e.smooth).count();
    assert_eq!(soft, 4 + 4);
}

/// A square of side 1 standing in the plane y = `y`, from x = 1.5 to 2.5
/// and z = 0 to 1.
fn standing_square(m: &mut Model, y: f64) -> FaceId {
    poly(
        m,
        &[
            DVec3::new(1.5, y, 0.0),
            DVec3::new(2.5, y, 0.0),
            DVec3::new(2.5, y, 1.0),
            DVec3::new(1.5, y, 1.0),
        ],
        DVec3::Y,
    )
}

#[test]
fn a_square_along_a_quarter_arc_makes_an_elbow() {
    let mut m = Model::new();
    let prof = standing_square(&mut m, 0.0);
    let before = edges(&m);
    add_arc(
        &mut m,
        DVec3::new(0.0, 0.0, 3.0),
        DVec3::Z,
        2.0,
        DVec3::new(2.0, 0.0, 3.0),
        FRAC_PI_2,
    );
    let p = new_edges(&m, &before);
    assert_eq!(follow::refusal(&m, &p, prof), None);
    assert!(follow::follow(&mut m, &p, prof));
    clean(&m);
    // Two caps, two flat quarter rings, two cylinder walls.
    assert_eq!(m.face_count(), 6);
    assert_eq!(count(&m, |s| matches!(s, Surface::Cylinder(_))), 2);
    // Pappus: the area times the distance its centre travels.
    close(volume(&m), 2.0 * FRAC_PI_2);
}

#[test]
fn lines_and_a_tangent_arc_make_a_smooth_bend() {
    let mut m = Model::new();
    let prof = standing_square(&mut m, -3.0);
    let before = edges(&m);
    add_segment(
        &mut m,
        DVec3::new(2.0, -3.0, 3.0),
        DVec3::new(2.0, 0.0, 3.0),
    );
    add_arc(
        &mut m,
        DVec3::new(0.0, 0.0, 3.0),
        DVec3::Z,
        2.0,
        DVec3::new(2.0, 0.0, 3.0),
        FRAC_PI_2,
    );
    add_segment(
        &mut m,
        DVec3::new(0.0, 2.0, 3.0),
        DVec3::new(-3.0, 2.0, 3.0),
    );
    let p = new_edges(&m, &before);
    assert_eq!(follow::refusal(&m, &p, prof), None);
    assert!(follow::follow(&mut m, &p, prof));
    clean(&m);
    assert_eq!(m.face_count(), 2 + 4 * 3);
    close(volume(&m), 3.0 + PI + 3.0);
    // The square's sides are soft where the arc meets the lines.
    let soft = m.edges().filter(|(_, e)| e.soft && e.smooth).count();
    assert_eq!(soft, 8);
}

#[test]
fn a_profile_away_from_an_arc_start_is_turned_to_it() {
    // The profile stands in the plane through the axis at y = 0; the arc
    // starts a quarter turn round, on the y axis.
    let mut m = Model::new();
    let prof = standing_square(&mut m, 0.0);
    let before = edges(&m);
    add_arc(
        &mut m,
        DVec3::new(0.0, 0.0, 3.0),
        DVec3::Z,
        2.0,
        DVec3::new(0.0, 2.0, 3.0),
        FRAC_PI_2,
    );
    let p = new_edges(&m, &before);
    assert!(follow::follow(&mut m, &p, prof));
    clean(&m);
    close(volume(&m), PI);
    // The profile moved to the arc's start.
    let at_start = m
        .vertices()
        .any(|(_, v)| v.distance(DVec3::new(0.0, 1.5, 0.0)) < 1e-9);
    assert!(at_start);
}

#[test]
fn what_cannot_be_turned_is_refused_with_a_reason() {
    // A profile across the axis.
    let mut a = Model::new();
    let prof = poly(
        &mut a,
        &[
            DVec3::new(-0.5, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 1.0),
            DVec3::new(-0.5, 0.0, 1.0),
        ],
        DVec3::Y,
    );
    let (p, _) = circle_path(&mut a, 3.0, 0.5);
    assert_eq!(follow::refusal(&a, &p, prof), Some(CROSSES_AXIS));
    let before = a.clone();
    assert!(!follow::follow(&mut a, &p, prof));
    assert_eq!(a, before, "a refused sweep changes nothing");
    // A profile off the axis, or a corner where a line meets an arc, is
    // followed with the arcs as fine segments since Exact Curves 6a
    // (`follow_fallback.rs`).
}

#[test]
fn a_path_drawn_from_the_middle_of_the_profile_works() {
    // As people do in SketchUp: the path starts inside the profile face.
    let mut m = Model::new();
    let prof = poly(
        &mut m,
        &[
            DVec3::new(0.0, -0.5, -0.5),
            DVec3::new(0.0, 0.5, -0.5),
            DVec3::new(0.0, 0.5, 0.5),
            DVec3::new(0.0, -0.5, 0.5),
        ],
        DVec3::X,
    );
    let before = edges(&m);
    add_segment(&mut m, DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0));
    add_segment(&mut m, DVec3::new(4.0, 0.0, 0.0), DVec3::new(4.0, 3.0, 0.0));
    let p = new_edges(&m, &before);
    assert_eq!(follow::refusal(&m, &p, prof), None);
    assert!(follow::follow(&mut m, &p, prof));
    clean(&m);
    close(volume(&m), 7.0);
}
