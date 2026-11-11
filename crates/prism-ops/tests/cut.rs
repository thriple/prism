//! Planes meet curved faces exactly (Exact Curves 6b): Intersect Faces
//! finds where a curved face crosses another face, keeps lines and
//! circles exact, uses fine segments for the rest (DECISIONS #32), and
//! splits the curved face in (θ, s).

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::{Curve, Surface};
use prism_math::DVec3;
use prism_ops::crossing::{Shape, crossings_with};
use prism_ops::follow;
use prism_ops::intersect::intersect_faces;
use prism_ops::pushpull::{Pulled, push_pull};
use prism_ops::{add_arc, add_circle, add_rect, add_segment, check, health::face_volume};
use prism_topo::{FaceId, Model};

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

/// The faces on a curved surface of the kind `f` picks.
fn curved(m: &Model, f: impl Fn(&Surface) -> bool) -> Vec<FaceId> {
    m.faces()
        .filter(|(_, x)| x.is_curved() && f(&x.surface))
        .map(|(id, _)| id)
        .collect()
}

fn sides(m: &Model) -> Vec<FaceId> {
    curved(m, |s| matches!(s, Surface::Cylinder(_)))
}

/// The curved faces' share of the volume.
fn curved_volume(m: &Model) -> f64 {
    m.faces()
        .filter(|(_, f)| f.is_curved())
        .map(|(id, _)| face_volume(m, id))
        .sum()
}

/// A cylinder of radius `r` from z = 0 to `h`, by Push/Pull of a circle.
fn cylinder(r: f64, h: f64) -> Model {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, r);
    let disc = m.faces().next().map(|(id, _)| id).expect("a disc");
    let pulled = push_pull(&mut m, disc, h, false);
    assert!(!matches!(pulled, Pulled::Nothing));
    // Push/Pull may face the disc either way: make it upwards.
    if m.vertices().all(|(_, p)| p.z <= 1e-9) {
        panic!("pulled down");
    }
    assert_eq!(sides(&m).len(), 2);
    m
}

/// A flat face through four corners.
fn quad(m: &mut Model, c: [DVec3; 4]) {
    for i in 0..4 {
        add_segment(m, c[i], c[(i + 1) % 4]);
    }
}

fn approx_count(m: &Model) -> usize {
    m.edges().filter(|(_, e)| e.approx).count()
}

#[test]
fn a_plane_square_to_the_axis_cuts_a_cylinder_in_exact_arcs() {
    let mut m = cylinder(1.0, 2.0);
    let before = curved_volume(&m);
    add_rect(&mut m, -2.0, -2.0, 2.0, 2.0, 1.0);
    let n = intersect_faces(&mut m, None);
    assert_eq!(n, 2, "one arc per side");
    clean(&m);
    assert_eq!(sides(&m).len(), 4, "each side cut in two");
    assert_eq!(approx_count(&m), 0);
    // The circle at z = 1 is exact, and cuts the square into a disc and a
    // square with a hole.
    let arcs = m
        .edges()
        .filter(|(_, e)| {
            matches!(e.curve, Curve::Arc { centre, radius, .. }
                if (centre.z - 1.0).abs() < 1e-12 && (radius - 1.0).abs() < 1e-12)
        })
        .count();
    assert_eq!(arcs, 2);
    let flat_at_1 = m
        .faces()
        .filter(|(_, f)| f.plane().is_some_and(|p| (p.origin.z - 1.0).abs() < 1e-9))
        .count();
    assert_eq!(flat_at_1, 2);
    assert!((curved_volume(&m) - before).abs() < 1e-9);
    // Nothing more to cut.
    assert_eq!(intersect_faces(&mut m, None), 0);
}

#[test]
fn a_plane_through_the_axis_cuts_a_cylinder_in_lines() {
    let mut m = cylinder(1.0, 2.0);
    let before = curved_volume(&m);
    let d = DVec3::new(1.0, 1.0, 0.0).normalize() * 2.0;
    quad(
        &mut m,
        [
            -d - DVec3::Z,
            d - DVec3::Z,
            d + DVec3::Z * 3.0,
            -d + DVec3::Z * 3.0,
        ],
    );
    let n = intersect_faces(&mut m, None);
    assert!(n >= 2, "{n}");
    clean(&m);
    assert_eq!(sides(&m).len(), 4);
    assert_eq!(approx_count(&m), 0);
    let lines = m
        .edges()
        .filter(|(_, e)| {
            let (a, b) = (m.vertex(e.a), m.vertex(e.b));
            !e.curve.is_arc()
                && (a.x - a.y).abs() < 1e-9
                && (b.x - b.y).abs() < 1e-9
                && (a.truncate().length() - 1.0).abs() < 1e-9
                && (b.truncate().length() - 1.0).abs() < 1e-9
                && (a.z - b.z).abs() > 1.9
        })
        .count();
    assert_eq!(lines, 2, "two lines along the side");
    assert!((curved_volume(&m) - before).abs() < 1e-9);
}

#[test]
fn a_slanted_plane_cuts_a_cylinder_in_fine_segments() {
    let mut m = cylinder(1.0, 2.0);
    let before = curved_volume(&m);
    // The plane z = 1 + 0.3 x.
    let z = |x: f64| 1.0 + 0.3 * x;
    quad(
        &mut m,
        [
            DVec3::new(-2.0, -2.0, z(-2.0)),
            DVec3::new(2.0, -2.0, z(2.0)),
            DVec3::new(2.0, 2.0, z(2.0)),
            DVec3::new(-2.0, 2.0, z(-2.0)),
        ],
    );
    assert_eq!(intersect_faces(&mut m, None), 2);
    clean(&m);
    assert!(prism_ops::health::check(&m).crossing.is_empty());
    assert_eq!(sides(&m).len(), 4);
    let fine: Vec<_> = m.edges().filter(|(_, e)| e.approx).collect();
    assert!(fine.len() >= 20, "{}", fine.len());
    // Every point exact: on the cylinder and the plane.
    for (_, e) in &fine {
        for v in [e.a, e.b] {
            let p = m.vertex(v);
            assert!((p.truncate().length() - 1.0).abs() < 1e-9, "{p}");
            assert!((p.z - z(p.x)).abs() < 1e-9, "{p}");
        }
    }
    // Each half of the ellipse is one curve.
    let ids: BTreeSet<_> = fine.iter().map(|(_, e)| e.curve_id).collect();
    assert_eq!(ids.len(), 2);
    // The chords cut a little inside the true ellipse.
    let after = curved_volume(&m);
    assert!(
        (after - before).abs() < 1e-2 * before.abs(),
        "{after} {before}"
    );
    assert_eq!(intersect_faces(&mut m, None), 0);
}

/// A sphere of radius 1 about the origin, by Follow Me of a half disc.
fn sphere() -> Model {
    let mut m = Model::new();
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
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    add_circle(&mut m, DVec3::new(0.0, 0.0, 3.0), DVec3::Z, 0.5);
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(follow::follow(&mut m, &path, prof));
    m
}

#[test]
fn any_plane_cuts_a_sphere_in_an_exact_circle() {
    let mut m = sphere();
    let before = curved_volume(&m);
    // A slanted plane through (0, 0, 0.3), normal (1, 0, 2).
    let n = DVec3::new(1.0, 0.0, 2.0).normalize();
    let o = DVec3::new(0.0, 0.0, 0.3);
    let u = DVec3::Y * 1.6;
    let v = n.cross(DVec3::Y).normalize() * 1.6;
    quad(&mut m, [o - u - v, o + u - v, o + u + v, o - u + v]);
    let n_cut = intersect_faces(&mut m, None);
    assert!(n_cut >= 2, "{n_cut}");
    clean(&m);
    assert_eq!(approx_count(&m), 0, "a circle, not segments");
    // Each half of the sphere cut in two.
    let parts = curved(&m, |s| matches!(s, Surface::Sphere(_)));
    assert_eq!(parts.len(), 4);
    // The arcs lie on the sphere and the plane.
    let r = (1.0 - (o.dot(n)).powi(2)).sqrt();
    let arcs = m
        .edges()
        .filter(|(_, e)| matches!(e.curve, Curve::Arc { radius, .. } if (radius - r).abs() < 1e-9))
        .count();
    assert!(arcs >= 2, "{arcs}");
    assert!(
        (curved_volume(&m) - before).abs() < 1e-3,
        "{}",
        curved_volume(&m)
    );
}

#[test]
fn a_box_through_a_cylinder_cuts_where_they_meet() {
    let mut m = cylinder(1.0, 4.0);
    add_rect(&mut m, -2.0, -0.5, 2.0, 0.5, 1.0);
    let face = m
        .faces()
        .find(|(_, f)| f.plane().is_some_and(|p| (p.origin.z - 1.0).abs() < 1e-9))
        .map(|(id, _)| id)
        .expect("the box's base");
    assert!(!matches!(
        push_pull(&mut m, face, 2.0, false),
        Pulled::Nothing
    ));
    let before = curved_volume(&m);
    // Model Health sees the faces crossing, and not once they are cut.
    let crossing = prism_ops::health::check(&m).crossing;
    assert!(crossing.faces.len() >= 6, "{:?}", crossing.faces);
    intersect_faces(&mut m, None);
    clean(&m);
    assert!(prism_ops::health::check(&m).crossing.is_empty());
    assert_eq!(approx_count(&m), 0, "lines and arcs only");
    // Four lines down the sides (y = ±0.5) and four arcs (z = 1, 3).
    let lines = m
        .edges()
        .filter(|(_, e)| {
            let (a, b) = (m.vertex(e.a), m.vertex(e.b));
            !e.curve.is_arc()
                && (a.y.abs() - 0.5).abs() < 1e-9
                && (a.truncate().length() - 1.0).abs() < 1e-9
                && (b.truncate().length() - 1.0).abs() < 1e-9
                && (a.z - b.z).abs() > 1.9
        })
        .count();
    assert_eq!(lines, 4);
    assert!((curved_volume(&m) - before).abs() < 1e-9);
    assert_eq!(intersect_faces(&mut m, None), 0);
}

#[test]
fn two_cylinders_cross_in_fine_segments_on_both() {
    let mut m = cylinder(1.0, 4.0);
    // A thinner cylinder along x, through the first at z = 2.
    let mut other = Model::new();
    add_circle(&mut other, DVec3::new(-3.0, 0.0, 2.0), DVec3::X, 0.5);
    let disc = other.faces().next().map(|(id, _)| id).expect("a disc");
    push_pull(&mut other, disc, 6.0, false);
    m.append(&other);
    let (a, b) = {
        let s = sides(&m);
        (s[0], s[3])
    };
    let _ = crossings_with(&m, a, b);
    intersect_faces(&mut m, None);
    clean(&m);
    assert!(approx_count(&m) > 0);
    // Every fine point on both cylinders.
    for (_, e) in m.edges().filter(|(_, e)| e.approx) {
        for v in [e.a, e.b] {
            let p = m.vertex(v);
            assert!((p.truncate().length() - 1.0).abs() < 1e-9, "{p}");
            let q = p - DVec3::new(0.0, 0.0, 2.0);
            assert!((q.y.hypot(q.z) - 0.5).abs() < 1e-9, "{p}");
        }
    }
    assert!(sides(&m).len() > 4);
}

#[test]
fn crossings_of_shapes_are_exact_or_fine() {
    let mut m = cylinder(1.0, 2.0);
    add_rect(&mut m, -2.0, -2.0, 2.0, 2.0, 1.0);
    let flat = m
        .faces()
        .find(|(_, f)| f.plane().is_some_and(|p| (p.origin.z - 1.0).abs() < 1e-9))
        .map(|(id, _)| id)
        .expect("the square");
    for s in sides(&m) {
        let p = crossings_with(&m, s, flat);
        assert_eq!(p.len(), 1);
        assert!(matches!(p[0].shape, Shape::Arc { .. }), "{:?}", p[0].shape);
    }
}

#[test]
fn a_plane_through_a_spheres_axis_cuts_it_through_the_poles() {
    let mut m = sphere();
    let before = curved_volume(&m);
    // Upright, through the z axis at 45° (the halves' seams are at 0°
    // and 180°).
    let d = DVec3::new(1.0, 1.0, 0.0).normalize() * 1.5;
    quad(
        &mut m,
        [
            -d - DVec3::Z * 1.5,
            d - DVec3::Z * 1.5,
            d + DVec3::Z * 1.5,
            -d + DVec3::Z * 1.5,
        ],
    );
    assert!(intersect_faces(&mut m, None) >= 2);
    clean(&m);
    assert_eq!(approx_count(&m), 0, "great circles, not segments");
    assert_eq!(curved(&m, |s| matches!(s, Surface::Sphere(_))).len(), 4);
    assert!(
        (curved_volume(&m) - before).abs() < 1e-3,
        "{}",
        curved_volume(&m)
    );
}

#[test]
fn a_plane_square_to_a_torus_axis_cuts_two_circles() {
    let mut m = Model::new();
    // A disc of radius 0.5 at x = 2, turned round a circle about z.
    add_circle(&mut m, DVec3::new(2.0, 0.0, 0.0), DVec3::Y, 0.5);
    let prof = m.faces().next().map(|(id, _)| id).expect("a disc");
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    add_circle(&mut m, DVec3::new(0.0, 0.0, 3.0), DVec3::Z, 1.0);
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(follow::follow(&mut m, &path, prof));
    let tori = |m: &Model| curved(m, |s| matches!(s, Surface::Torus(_))).len();
    let n = tori(&m);
    let vol = curved_volume(&m);
    add_rect(&mut m, -3.0, -3.0, 3.0, 3.0, 0.2);
    assert!(intersect_faces(&mut m, None) >= 4);
    clean(&m);
    assert_eq!(approx_count(&m), 0, "circles round the axis");
    assert!(tori(&m) > n, "{} {n}", tori(&m));
    assert!((curved_volume(&m) - vol).abs() < 1e-3);
}

#[test]
fn a_cut_curved_face_scaled_unevenly_becomes_facets() {
    use prism_ops::transform::{arcs_refusal, scale, scaling};
    // Cut slanted: its pieces are not rectangles in (θ, s), so faceting
    // cuts the cells the outline crosses into triangles (step 6b-2).
    let mut m = cylinder(1.0, 2.0);
    quad(
        &mut m,
        [
            DVec3::new(-2.0, -2.0, 0.4),
            DVec3::new(2.0, -2.0, 1.6),
            DVec3::new(2.0, 2.0, 1.6),
            DVec3::new(-2.0, 2.0, 0.4),
        ],
    );
    intersect_faces(&mut m, None);
    let all: BTreeSet<_> = m.vertices().map(|(id, _)| id).collect();
    let k = DVec3::new(2.0, 1.0, 1.0);
    let uneven = scaling(DVec3::ZERO, k);
    assert_eq!(arcs_refusal(&m, &all, &uneven), None);
    scale(&mut m, &all, DVec3::ZERO, k);
    clean(&m);
    assert!(sides(&m).is_empty(), "every curved face is facets now");
    // Every point of the old cylinder is on the stretched one.
    for (_, p) in m.vertices() {
        if p.z > -1e-9 && p.z < 2.0 + 1e-9 && (p.x.abs() < 2.0 - 1e-6 || p.y.abs() < 2.0 - 1e-6) {
            let r = (p.x / 2.0).hypot(p.y);
            assert!(r < 1.0 + 1e-9, "{p}");
        }
    }
}
