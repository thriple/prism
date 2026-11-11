//! A pull and a new radius stop at curved faces (Exact Curves 6c-2): the
//! moving face goes until it first meets another face, whether it meets it
//! at an edge, along its own outline, or where the other face faces it head
//! on.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::follow;
use prism_ops::pushpull::{Pulled, push_pull};
use prism_ops::reach::limit;
use prism_ops::{add_arc, add_circle, add_segment, face_at};
use prism_topo::{FaceId, Model};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-7, "{a} vs {b}");
}

/// A flat face through four corners.
fn quad(m: &mut Model, c: [DVec3; 4]) {
    for i in 0..4 {
        add_segment(m, c[i], c[(i + 1) % 4]);
    }
}

/// A sphere of radius `r` about `c`, by Follow Me of a half disc round a
/// circle drawn far away.
fn sphere(m: &mut Model, c: DVec3, r: f64) {
    add_segment(m, c - DVec3::Z * r, c + DVec3::Z * r);
    add_arc(m, c, -DVec3::Y, r, c - DVec3::Z * r, PI);
    let prof = face_at(m, c + DVec3::X * r * 0.5).expect("a half disc");
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    add_circle(m, c + DVec3::Z * 10.0, DVec3::Z, 0.5);
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(follow::follow(m, &path, prof));
}

/// A cylinder of radius `r` about the z axis from z = 0 to `h`.
fn cylinder(m: &mut Model, r: f64, h: f64) {
    add_circle(m, DVec3::ZERO, DVec3::Z, r);
    let disc = face_at(m, DVec3::ZERO).expect("a disc");
    let up = m.face(disc).and_then(|f| f.plane()).map(|p| p.normal.z);
    let d = h * up.unwrap_or(1.0).signum();
    assert!(!matches!(push_pull(m, disc, d, false), Pulled::Nothing));
}

/// The distance limit gives for a pull of `d` along the world direction
/// `way` (the face's normal may point either way).
fn stop(m: &Model, f: FaceId, way: DVec3, d: f64) -> f64 {
    let n = m.face(f).and_then(|x| x.plane()).expect("flat").normal;
    let s = n.dot(way).signum();
    limit(m, f, s * d) * s
}

fn cylinder_side(m: &Model, x: f64, y: f64) -> FaceId {
    m.faces()
        .filter(|(_, f)| matches!(f.surface, Surface::Cylinder(_)))
        .map(|(id, _)| id)
        .find(|id| side_holds(m, *id, x, y))
        .expect("a cylinder side")
}

/// Whether the cylinder face's points include ones on the side of (x, y).
fn side_holds(m: &Model, id: FaceId, x: f64, y: f64) -> bool {
    let f = m.face(id).expect("face");
    let pts: Vec<DVec3> = m.face_loop_points(f).into_iter().flatten().collect();
    // The face's half of the circle holds the direction (x, y) when the
    // mean of its points leans that way.
    let mean = pts.iter().fold(DVec3::ZERO, |a, p| a + *p) / pts.len() as f64;
    mean.x * x + mean.y * y > 0.0
}

#[test]
fn a_plate_pushed_down_onto_a_sphere_stops_at_its_top() {
    let mut m = Model::new();
    sphere(&mut m, DVec3::ZERO, 1.0);
    let z = 3.0;
    quad(
        &mut m,
        [
            DVec3::new(-0.5, -0.5, z),
            DVec3::new(0.5, -0.5, z),
            DVec3::new(0.5, 0.5, z),
            DVec3::new(-0.5, 0.5, z),
        ],
    );
    let plate = face_at(&m, DVec3::new(0.0, 0.0, z)).expect("plate");
    close(stop(&m, plate, -DVec3::Z, 10.0), 2.0);
    close(stop(&m, plate, -DVec3::Z, 1.5), 1.5);
    // Upwards, it stops at the path's disc at z = 10.
    close(stop(&m, plate, DVec3::Z, 10.0), 7.0);
}

#[test]
fn a_plate_off_the_top_stops_where_its_edge_meets_the_sphere() {
    let mut m = Model::new();
    sphere(&mut m, DVec3::ZERO, 1.0);
    let z = 3.0;
    quad(
        &mut m,
        [
            DVec3::new(0.3, -0.5, z),
            DVec3::new(0.8, -0.5, z),
            DVec3::new(0.8, 0.5, z),
            DVec3::new(0.3, 0.5, z),
        ],
    );
    let plate = face_at(&m, DVec3::new(0.5, 0.0, z)).expect("plate");
    // The sphere's highest point under the plate is on its edge x = 0.3.
    close(stop(&m, plate, -DVec3::Z, 10.0), z - (1.0f64 - 0.09).sqrt());
}

#[test]
fn a_wall_pushed_sideways_stops_at_a_cylinders_side() {
    let mut m = Model::new();
    cylinder(&mut m, 1.0, 2.0);
    let x = 3.0;
    quad(
        &mut m,
        [
            DVec3::new(x, -0.5, 0.5),
            DVec3::new(x, 0.5, 0.5),
            DVec3::new(x, 0.5, 1.5),
            DVec3::new(x, -0.5, 1.5),
        ],
    );
    let wall = face_at(&m, DVec3::new(x, 0.0, 1.0)).expect("wall");
    close(stop(&m, wall, -DVec3::X, 10.0), 2.0);
    // A wall above the cylinder passes it.
    let mut m = Model::new();
    cylinder(&mut m, 1.0, 2.0);
    quad(
        &mut m,
        [
            DVec3::new(x, -0.5, 2.5),
            DVec3::new(x, 0.5, 2.5),
            DVec3::new(x, 0.5, 3.5),
            DVec3::new(x, -0.5, 3.5),
        ],
    );
    let wall = face_at(&m, DVec3::new(x, 0.0, 3.0)).expect("wall");
    close(stop(&m, wall, -DVec3::X, 10.0), 10.0);
}

#[test]
fn a_cylinders_cap_pushed_in_still_stops_at_the_far_cap() {
    let mut m = Model::new();
    cylinder(&mut m, 1.0, 2.0);
    let top = face_at(&m, DVec3::new(0.2, 0.1, 2.0)).expect("top");
    close(stop(&m, top, -DVec3::Z, 5.0), 2.0);
    close(stop(&m, top, DVec3::Z, 5.0), 5.0);
}

/// A cylinder side pulled by `d` along its normal; the radius change the
/// limit allows.
fn radius_stop(m: &Model, side: FaceId, grow: f64) -> f64 {
    let Surface::Cylinder(c) = m.face(side).expect("side").surface else {
        panic!("not a cylinder");
    };
    let d = if c.outward { grow } else { -grow };
    let l = limit(m, side, d);
    if c.outward { l } else { -l }
}

#[test]
fn a_new_radius_stops_at_a_wall_and_at_a_post_inside() {
    let mut m = Model::new();
    cylinder(&mut m, 1.0, 2.0);
    quad(
        &mut m,
        [
            DVec3::new(3.0, -2.0, -1.0),
            DVec3::new(3.0, 2.0, -1.0),
            DVec3::new(3.0, 2.0, 3.0),
            DVec3::new(3.0, -2.0, 3.0),
        ],
    );
    let side = cylinder_side(&m, 1.0, 0.0);
    close(radius_stop(&m, side, 5.0), 2.0);
    close(radius_stop(&m, side, 1.0), 1.0);
    // A thin post inside: shrinking stops at its nearest corner.
    let mut m = Model::new();
    cylinder(&mut m, 1.0, 2.0);
    quad(
        &mut m,
        [
            DVec3::new(0.5, -0.1, 0.5),
            DVec3::new(0.5, 0.1, 0.5),
            DVec3::new(0.5, 0.1, 1.5),
            DVec3::new(0.5, -0.1, 1.5),
        ],
    );
    let side = cylinder_side(&m, 1.0, 0.0);
    close(radius_stop(&m, side, -0.9), -(1.0 - 0.26f64.sqrt()));
}

#[test]
fn a_new_radius_stops_at_a_sphere_beside_it() {
    let mut m = Model::new();
    cylinder(&mut m, 1.0, 2.0);
    sphere(&mut m, DVec3::new(3.0, 0.0, 1.0), 0.5);
    let side = cylinder_side(&m, 1.0, 0.0);
    close(radius_stop(&m, side, 5.0), 1.5);
}

#[test]
fn a_pulled_plate_stops_and_lands_on_the_sphere() {
    let mut m = Model::new();
    sphere(&mut m, DVec3::ZERO, 1.0);
    quad(
        &mut m,
        [
            DVec3::new(-0.5, -0.5, 3.0),
            DVec3::new(0.5, -0.5, 3.0),
            DVec3::new(0.5, 0.5, 3.0),
            DVec3::new(-0.5, 0.5, 3.0),
        ],
    );
    let plate = face_at(&m, DVec3::new(0.0, 0.0, 3.0)).expect("plate");
    let n = m.face(plate).and_then(|f| f.plane()).expect("flat").normal;
    let d = -10.0 * n.z.signum();
    assert!(!matches!(
        push_pull(&mut m, plate, d, false),
        Pulled::Nothing
    ));
    // The plate's pulled copy rests on the sphere's top at z = 1.
    let low = m
        .vertices()
        .map(|(_, p)| p)
        .filter(|p| (p.x.abs() - 0.5).abs() < 1e-9 && (p.y.abs() - 0.5).abs() < 1e-9)
        .map(|p| p.z)
        .fold(f64::MAX, f64::min);
    close(low, 1.0);
}
