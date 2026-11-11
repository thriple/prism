//! Follow Me along lines (Exact Curves 5c): a profile swept along a path of
//! lines makes a closed solid with the exact volume, mitred at corners; a
//! closed path makes a ring; arcs make cylinders on a straight path; and
//! what cannot be swept exactly is refused with a reason.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_math::DVec3;
use prism_ops::follow::{self, ALONG_PATH, CROSSES_ITSELF, NO_PATH, NOT_A_PATH};
use prism_ops::{add_segment, check, health::face_volume};
use prism_topo::{EdgeId, FaceId, Model};

fn volume(m: &Model) -> f64 {
    m.faces().map(|(id, _)| face_volume(m, id)).sum()
}

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

/// A square of side `s` in the plane x = `x`, centred on (x, cy, cz).
fn square(m: &mut Model, x: f64, cy: f64, cz: f64, s: f64) -> FaceId {
    let h = s / 2.0;
    let c = [(-h, -h), (h, -h), (h, h), (-h, h)].map(|(y, z)| DVec3::new(x, cy + y, cz + z));
    for i in 0..4 {
        add_segment(m, c[i], c[(i + 1) % 4]);
    }
    m.faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.normal.x.abs() > 0.99))
        .map(|(id, _)| id)
        .expect("the square")
}

/// Lines through `pts` (closed if `close`), returned as the path.
fn path(m: &mut Model, pts: &[DVec3], close: bool) -> BTreeSet<EdgeId> {
    let n = pts.len();
    let segs = if close { n } else { n - 1 };
    for i in 0..segs {
        add_segment(m, pts[i], pts[(i + 1) % n]);
    }
    let on = |p: DVec3| pts.iter().any(|q| q.distance(p) < 1e-9);
    m.edges()
        .filter(|(_, e)| on(m.vertex(e.a)) && on(m.vertex(e.b)))
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn a_square_along_an_l_makes_a_mitred_solid_of_the_exact_volume() {
    for k in [1e-3, 1.0, 1e3] {
        let mut m = Model::new();
        // The profile floats above the path's start, square to it.
        let prof = square(&mut m, 0.0, 0.0, 2.0 * k, 1.0 * k);
        let p = path(
            &mut m,
            &[
                DVec3::ZERO,
                DVec3::new(5.0, 0.0, 0.0) * k,
                DVec3::new(5.0, 4.0, 0.0) * k,
            ],
            false,
        );
        assert_eq!(follow::refusal(&m, &p, prof), None);
        assert!(follow::follow(&mut m, &p, prof));
        clean(&m);
        // Two caps and four walls per line.
        assert_eq!(m.face_count(), 2 + 4 * 2);
        let v = 1.0 * 9.0 * k.powi(3);
        assert!((volume(&m) - v).abs() < 1e-9 * v, "{} vs {v}", volume(&m));
    }
}

#[test]
fn a_closed_path_makes_a_ring_with_no_ends() {
    let mut m = Model::new();
    let prof = square(&mut m, 0.0, 0.0, 2.0, 1.0);
    let p = path(
        &mut m,
        &[
            DVec3::ZERO,
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(4.0, 4.0, 0.0),
            DVec3::new(0.0, 4.0, 0.0),
        ],
        true,
    );
    assert!(follow::follow(&mut m, &p, prof));
    clean(&m);
    // Walls only (the profile went); the closed path fills as a face.
    assert_eq!(m.face_count(), 4 * 4 + 1);
    assert!((volume(&m) - 16.0).abs() < 1e-9, "{}", volume(&m));
}

#[test]
fn a_disc_along_a_straight_line_makes_a_cylinder() {
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, DVec3::new(0.0, 0.0, 2.0), DVec3::X, 0.5);
    let disc = m.faces().next().map(|(id, _)| id).expect("a disc");
    let p = path(&mut m, &[DVec3::ZERO, DVec3::new(3.0, 0.0, 0.0)], false);
    assert!(follow::follow(&mut m, &p, disc));
    clean(&m);
    let curved = m.faces().filter(|(_, f)| f.is_curved()).count();
    assert_eq!(curved, 2, "two cylinder halves");
    let v = PI * 0.25 * 3.0;
    assert!((volume(&m) - v).abs() < 1e-9, "{} vs {v}", volume(&m));
    // The seams between the halves are soft and smooth.
    let soft = m.edges().filter(|(_, e)| e.soft && e.smooth).count();
    assert_eq!(soft, 2);
}

#[test]
fn what_cannot_be_swept_is_refused_with_a_reason() {
    let mut m = Model::new();
    let prof = square(&mut m, 0.0, 0.0, 2.0, 1.0);
    assert_eq!(follow::refusal(&m, &BTreeSet::new(), prof), Some(NO_PATH));
    // A branching path.
    let mut b = m.clone();
    let mut p = path(&mut b, &[DVec3::ZERO, DVec3::new(3.0, 0.0, 0.0)], false);
    p.extend(path(
        &mut b,
        &[DVec3::new(1.0, 0.0, 0.0), DVec3::new(1.0, 2.0, 0.0)],
        false,
    ));
    assert_eq!(follow::refusal(&b, &p, prof), Some(NOT_A_PATH));
    // A path along the profile's own plane.
    let mut a = m.clone();
    let p = path(&mut a, &[DVec3::ZERO, DVec3::new(0.0, 3.0, 0.0)], false);
    assert_eq!(follow::refusal(&a, &p, prof), Some(ALONG_PATH));
    // A sharp corner for a wide profile.
    let mut c = Model::new();
    let wide = square(&mut c, -1.5, 3.0, 0.0, 4.0);
    let p = path(
        &mut c,
        &[
            DVec3::new(-1.0, 3.0, 0.0),
            DVec3::new(1.0, 3.0, 0.0),
            DVec3::new(-1.0, 3.5, 0.0),
        ],
        false,
    );
    assert_eq!(follow::refusal(&c, &p, wide), Some(CROSSES_ITSELF));
    // A disc round a corner.
    let mut d = Model::new();
    prism_ops::add_circle(&mut d, DVec3::new(0.0, 0.0, 2.0), DVec3::X, 0.5);
    let disc = d.faces().next().map(|(id, _)| id).unwrap();
    let p = path(
        &mut d,
        &[
            DVec3::ZERO,
            DVec3::new(3.0, 0.0, 0.0),
            DVec3::new(3.0, 3.0, 0.0),
        ],
        false,
    );
    // Not refused since Exact Curves 6a: the fallback (`follow_fallback.rs`).
    assert_eq!(follow::refusal(&d, &p, disc), None);
    // A closed path that is not flat still closes: carried along each
    // line onto the corners' halving planes, the profile comes back to
    // where it started (RING_TWISTS is only a safety net).
    let mut t = Model::new();
    let prof = square(&mut t, -0.5, 0.0, 0.0, 0.5);
    let p = path(
        &mut t,
        &[
            DVec3::ZERO,
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(4.0, 4.0, 0.0),
            DVec3::new(0.0, 4.0, 3.0),
        ],
        true,
    );
    assert_eq!(follow::refusal(&t, &p, prof), None);
    assert!(follow::follow(&mut t, &p, prof));
    clean(&t);
    let before = d.clone();
    assert!(!follow::follow(&mut d, &p, disc));
    assert_eq!(d, before, "a refused sweep changes nothing");
}

#[test]
fn a_tilted_free_profile_is_squared_up_first() {
    let mut m = Model::new();
    // A square in a plane leaning back from x = 0.
    let c = [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)]
        .map(|(y, z): (f64, f64)| DVec3::new(0.3 * z, y, 2.0 + z));
    for i in 0..4 {
        add_segment(&mut m, c[i], c[(i + 1) % 4]);
    }
    let prof = m.faces().next().map(|(id, _)| id).unwrap();
    let p = path(&mut m, &[DVec3::ZERO, DVec3::new(5.0, 0.0, 0.0)], false);
    assert!(follow::follow(&mut m, &p, prof));
    clean(&m);
    // Projected square to x: the section is 1 x 1, swept 5 from x = 0.
    assert!((volume(&m) - 5.0).abs() < 1e-9, "{}", volume(&m));
}

/// Checking a sweep and then making it (as the tool does) reuses the
/// checked sweep: the model is exactly the one a sweep alone makes, and a
/// different model is not given the old result.
#[test]
fn checking_then_sweeping_makes_the_same_model_as_sweeping() {
    let make = |w: f64| {
        let mut m = Model::new();
        let prof = square(&mut m, 0.0, 0.0, 2.0, w);
        let p = path(
            &mut m,
            &[
                DVec3::ZERO,
                DVec3::new(5.0, 0.0, 0.0),
                DVec3::new(5.0, 4.0, 0.0),
            ],
            false,
        );
        (m, p, prof)
    };
    let (mut alone, p, prof) = make(1.0);
    assert!(follow::follow(&mut alone, &p, prof));
    let (mut checked, p, prof) = make(1.0);
    assert_eq!(follow::refusal(&checked, &p, prof), None);
    assert!(follow::follow(&mut checked, &p, prof));
    assert_eq!(checked.geometry_json(), alone.geometry_json());
    // A wider profile right after: its own sweep, not the last one.
    let (mut wide, p, prof) = make(1.5);
    assert_eq!(follow::refusal(&wide, &p, prof), None);
    assert!(follow::follow(&mut wide, &p, prof));
    let v = 1.5 * 1.5 * 9.0;
    assert!((volume(&wide) - v).abs() < 1e-9 * v, "{}", volume(&wide));
}
