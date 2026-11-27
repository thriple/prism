//! Push/Pull on a cone, sphere or torus face gives it a new size (Exact
//! Curves 6d): a sphere's radius, a torus's tube radius, a cone moved
//! square to its side. Flat faces beside it follow; volumes stay exact.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::pushpull::{Pulled, push_pull, refusal};
use prism_ops::radius::{RADIUS_GONE, TUBE_AXIS};
use prism_ops::{add_arc, add_circle, add_segment, check, face_at, follow, health::face_volume};
use prism_topo::{FaceId, Model};

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

/// The solid's volume: every face but the path's disc up at z = 10.
fn volume(m: &Model) -> f64 {
    m.faces()
        .filter(|(_, f)| m.face_loop_points(f).iter().flatten().all(|p| p.z < 5.0))
        .map(|(id, _)| face_volume(m, id))
        .sum()
}

/// Turn the profile face found at `inside` once round the z axis.
fn lathe(m: &mut Model, inside: DVec3) {
    let prof = face_at(m, inside).expect("the profile");
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    add_circle(m, DVec3::new(0.0, 0.0, 10.0), DVec3::Z, 0.5);
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(follow::follow(m, &path, prof));
    clean(m);
}

/// A face on a surface `pick` accepts.
fn curved(m: &Model, pick: impl Fn(&Surface) -> bool) -> FaceId {
    m.faces()
        .find(|(_, f)| f.is_curved() && pick(&f.surface))
        .map(|(id, _)| id)
        .expect("a curved face")
}

/// Pull `f` by `d` away from its axis or centre.
fn grow(m: &mut Model, f: FaceId, d: f64) -> Pulled {
    let out = m.face(f).expect("face").surface.outward();
    push_pull(m, f, if out { d } else { -d }, false)
}

fn grow_refusal(m: &Model, f: FaceId, d: f64) -> Option<&'static str> {
    let out = m.face(f).expect("face").surface.outward();
    refusal(m, f, if out { d } else { -d })
}

fn sphere_face(s: &Surface) -> bool {
    matches!(s, Surface::Sphere(_))
}

#[test]
fn a_whole_sphere_takes_a_new_radius() {
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
    lathe(&mut m, DVec3::new(0.5, 0.0, 0.0));
    close(volume(&m), 4.0 / 3.0 * PI);
    let f = curved(&m, sphere_face);
    assert!(matches!(grow(&mut m, f, 0.5), Pulled::Stretched));
    clean(&m);
    close(volume(&m), 4.0 / 3.0 * PI * 1.5f64.powi(3));
    for (_, x) in m.faces() {
        if let Surface::Sphere(s) = x.surface {
            close(s.radius, 1.5);
        }
    }
    // Pushed in past its centre: refused.
    let f = curved(&m, sphere_face);
    assert_eq!(grow_refusal(&m, f, -2.0), Some(RADIUS_GONE));
}

#[test]
fn a_hemisphere_and_a_cap_keep_their_flat_bases() {
    // A half ball: its base through the centre widens with it.
    let mut m = Model::new();
    add_segment(&mut m, DVec3::ZERO, DVec3::new(0.0, 0.0, 1.0));
    add_segment(&mut m, DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
    add_arc(
        &mut m,
        DVec3::ZERO,
        -DVec3::Y,
        1.0,
        DVec3::new(1.0, 0.0, 0.0),
        PI / 2.0,
    );
    lathe(&mut m, DVec3::new(0.3, 0.0, 0.3));
    close(volume(&m), 2.0 / 3.0 * PI);
    let f = curved(&m, sphere_face);
    assert!(matches!(grow(&mut m, f, 0.5), Pulled::Stretched));
    clean(&m);
    close(volume(&m), 2.0 / 3.0 * PI * 1.5f64.powi(3));
    // A cap cut off at z = 0.5: its base stays at z = 0.5.
    let mut m = Model::new();
    let rim = DVec3::new(0.75f64.sqrt(), 0.0, 0.5);
    add_segment(&mut m, DVec3::new(0.0, 0.0, 0.5), DVec3::new(0.0, 0.0, 1.0));
    add_segment(&mut m, DVec3::new(0.0, 0.0, 0.5), rim);
    add_arc(&mut m, DVec3::ZERO, -DVec3::Y, 1.0, rim, PI / 3.0);
    lathe(&mut m, DVec3::new(0.2, 0.0, 0.6));
    let cap = |r: f64| {
        let h = r - 0.5;
        PI * h * h * (3.0 * r - h) / 3.0
    };
    close(volume(&m), cap(1.0));
    let f = curved(&m, sphere_face);
    assert!(matches!(grow(&mut m, f, 0.2), Pulled::Stretched));
    clean(&m);
    close(volume(&m), cap(1.2));
    let low = m
        .vertices()
        .map(|(_, p)| p.z)
        .filter(|z| *z < 5.0)
        .fold(f64::MAX, f64::min);
    close(low, 0.5);
}

#[test]
fn a_torus_takes_a_new_tube_radius() {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::new(2.0, 0.0, 0.0), DVec3::Y, 0.5);
    lathe(&mut m, DVec3::new(2.0, 0.0, 0.0));
    let ring = |r: f64| 2.0 * PI * PI * 2.0 * r * r;
    close(volume(&m), ring(0.5));
    let torus = |s: &Surface| matches!(s, Surface::Torus(_));
    let f = curved(&m, torus);
    assert!(matches!(grow(&mut m, f, 0.25), Pulled::Stretched));
    clean(&m);
    close(volume(&m), ring(0.75));
    let f = curved(&m, torus);
    assert_eq!(grow_refusal(&m, f, -0.75), Some(RADIUS_GONE));
    assert_eq!(grow_refusal(&m, f, 1.5), Some(TUBE_AXIS));
}

#[test]
fn a_cone_moves_square_to_its_side_and_keeps_its_base() {
    // A cone of base radius 1 and height 2 standing on z = 0.
    let mut m = Model::new();
    let (o, r, a) = (DVec3::ZERO, DVec3::X, DVec3::new(0.0, 0.0, 2.0));
    add_segment(&mut m, o, r);
    add_segment(&mut m, r, a);
    add_segment(&mut m, a, o);
    lathe(&mut m, DVec3::new(0.3, 0.0, 0.3));
    let cone = |h: f64| PI * (h / 2.0).powi(2) * h / 3.0;
    close(volume(&m), cone(2.0));
    let is_cone = |s: &Surface| matches!(s, Surface::Cone(_));
    let f = curved(&m, is_cone);
    let d = 0.1;
    assert!(matches!(grow(&mut m, f, d), Pulled::Stretched));
    clean(&m);
    // Same angle; the apex rises by d / sin(angle), the base stays.
    let sin = 1.0 / 5f64.sqrt();
    let h = 2.0 + d / sin;
    close(volume(&m), cone(h));
    let top = m
        .vertices()
        .map(|(_, p)| p.z)
        .filter(|z| *z < 5.0)
        .fold(f64::MIN, f64::max);
    close(top, h);
    // Pushed in so far the cone would vanish: refused.
    let f = curved(&m, is_cone);
    assert_eq!(grow_refusal(&m, f, -2.0), Some(RADIUS_GONE));
}

#[test]
fn a_growing_sphere_stops_at_a_wall() {
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
    lathe(&mut m, DVec3::new(0.5, 0.0, 0.0));
    let c =
        [(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (-2.0, 2.0)].map(|(y, z)| DVec3::new(3.0, y, z));
    for i in 0..4 {
        add_segment(&mut m, c[i], c[(i + 1) % 4]);
    }
    let f = curved(&m, sphere_face);
    let out = m.face(f).expect("face").surface.outward();
    let d = if out { 5.0 } else { -5.0 };
    close(prism_ops::reach::limit(&m, f, d), d * 0.4);
    assert!(matches!(grow(&mut m, f, 5.0), Pulled::Stretched));
    for (_, x) in m.faces() {
        if let Surface::Sphere(s) = x.surface {
            close(s.radius, 3.0);
        }
    }
}
