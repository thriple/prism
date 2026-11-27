//! Texture placement (Basic Materials part 2, brief 28): the default
//! placement on flat faces, exact coordinates on curved ones, and a
//! texture travelling with a face that is moved, turned or scaled.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::{Cone, Cylinder, Surface};
use prism_math::{DVec2, DVec3, Plane};
use prism_ops::{add_rect, push_pull, transform};
use prism_topo::{FaceId, MaterialId, Model, UvMap, VertexId};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn close(a: DVec2, b: DVec2) -> bool {
    a.distance(b) < 1e-9
}

/// A 2 m cube on the ground, every face painted.
fn painted_cube() -> Model {
    let mut m = Model::new();
    add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    let f = m.faces().next().map(|(id, _)| id).expect("a face");
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    push_pull(&mut m, f, 2.0 * up, false);
    let ids: Vec<FaceId> = m.faces().map(|(id, _)| id).collect();
    for id in ids {
        m.edit_face(id, |f| f.paint.front = Some(MaterialId(0)));
    }
    m
}

fn face_facing(m: &Model, n: DVec3) -> FaceId {
    m.faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.normal.distance(n) < 1e-9))
        .map(|(id, _)| id)
        .expect("a face")
}

#[test]
fn the_default_placement_is_anchored_to_the_model() {
    // A floor: u along red, v along green, from the origin.
    let floor = UvMap::default_for(&Plane::new(v(5.0, 5.0, 3.0), DVec3::Z));
    assert!(close(floor.at(v(1.5, 2.5, 3.0)), DVec2::new(1.5, 2.5)));
    // Two coplanar faces share it, so the texture lines up across them.
    let other = UvMap::default_for(&Plane::new(v(-7.0, 1.0, 3.0), DVec3::Z));
    assert_eq!(floor, other);
    // A ceiling: still along red, and not mirrored seen from below.
    let ceiling = UvMap::default_for(&Plane::new(v(0.0, 0.0, 3.0), -DVec3::Z));
    assert_eq!(ceiling.u, DVec3::X);
    assert!(ceiling.v.distance(-DVec3::Y) < 1e-12);
    // Walls: u level, reading left to right from outside; v straight up.
    let front = UvMap::default_for(&Plane::new(DVec3::ZERO, -DVec3::Y));
    let right = UvMap::default_for(&Plane::new(v(2.0, 0.0, 0.0), DVec3::X));
    assert!(front.u.distance(DVec3::X) < 1e-12 && front.v.distance(DVec3::Z) < 1e-12);
    assert!(right.u.distance(DVec3::Y) < 1e-12 && right.v.distance(DVec3::Z) < 1e-12);
    // Round a box's corner the rows stay level: the same height is the
    // same v on both walls.
    for z in [0.0, 0.7, 1.9] {
        assert_eq!(front.at(v(2.0, 0.0, z)).y, right.at(v(2.0, 0.0, z)).y);
    }
    // A sloping roof: u level, v up the slope.
    let n = v(0.0, -1.0, 1.0).normalize();
    let roof = UvMap::default_for(&Plane::new(v(0.0, 0.0, 2.0), n));
    assert!(roof.u.z.abs() < 1e-12 && roof.v.z > 0.0);
    assert!((roof.u.length() - 1.0).abs() < 1e-12 && (roof.v.length() - 1.0).abs() < 1e-12);
}

#[test]
fn a_moved_turned_or_scaled_face_takes_its_texture_with_it() {
    for step in 0..3 {
        let mut m = painted_cube();
        let front = face_facing(&m, -DVec3::Y);
        let corner = v(2.0, 0.0, 2.0);
        let before = m.face(front).unwrap().texture_map().unwrap().at(corner);
        let all: BTreeSet<VertexId> = m.vertices().map(|(id, _)| id).collect();
        let mat = match step {
            0 => prism_math::DAffine3::from_translation(v(0.3, 0.0, 1.1)),
            1 => transform::turn(v(1.0, 1.0, 0.0), DVec3::Z, 0.7),
            _ => transform::scaling(v(1.0, 1.0, 0.0), v(2.0, 2.0, 2.0)),
        };
        match step {
            0 => transform::translate(&mut m, &all, v(0.3, 0.0, 1.1)),
            1 => transform::rotate(&mut m, &all, v(1.0, 1.0, 0.0), DVec3::Z, 0.7),
            _ => transform::scale(&mut m, &all, v(1.0, 1.0, 0.0), v(2.0, 2.0, 2.0)),
        }
        let face = m.face(front).expect("the same face");
        assert!(face.paint.uv.is_some(), "the placement is kept");
        let after = face.texture_map().unwrap().at(mat.transform_point3(corner));
        assert!(close(before, after), "step {step}: {before} then {after}");
    }
}

#[test]
fn an_unpainted_face_keeps_the_default() {
    let mut m = painted_cube();
    let front = face_facing(&m, -DVec3::Y);
    m.edit_face(front, |f| f.paint = Default::default());
    let all: BTreeSet<VertexId> = m.vertices().map(|(id, _)| id).collect();
    transform::translate(&mut m, &all, v(1.0, 0.0, 0.0));
    assert_eq!(m.face(front).unwrap().paint.uv, None);
}

#[test]
fn a_cylinder_unrolls_without_stretch() {
    let r = 0.75;
    let c = Cylinder {
        origin: DVec3::ZERO,
        axis: DVec3::Z,
        radius: r,
        outward: true,
    };
    let rev = Surface::Cylinder(c).revolution().unwrap();
    // Once round is the circumference: one texture tile of that width.
    let a = rev.texture_at(0.0, 0.0);
    let b = rev.texture_at(2.0 * PI, 0.0);
    assert!(((b - a).x - 2.0 * PI * r).abs() < 1e-12);
    // Up the side is height, unchanged.
    assert!(((rev.texture_at(0.0, 1.3) - a).y - 1.3).abs() < 1e-12);
}

#[test]
fn a_cone_unrolls_exactly_into_a_sector() {
    let cone = Cone {
        apex: DVec3::ZERO,
        axis: DVec3::Z,
        half_angle: 0.4,
        outward: true,
    };
    let rev = Surface::Cone(cone).revolution().unwrap();
    // Distances along the surface are kept: between two points at the same
    // height, the straight line in the sector is the chord of the arc of
    // radius slant through angle θ·sin(half angle).
    let (h, t) = (2.0, 1.1);
    let slant = h / 0.4f64.cos();
    let a = rev.texture_at(0.0, h);
    let b = rev.texture_at(t, h);
    let chord = 2.0 * slant * (t * 0.4f64.sin() / 2.0).sin();
    assert!((a.distance(b) - chord).abs() < 1e-9, "{}", a.distance(b));
    // A line from the apex unrolls to its slant length.
    assert!((rev.texture_at(0.3, 0.0).distance(rev.texture_at(0.3, h)) - slant).abs() < 1e-9);
}
