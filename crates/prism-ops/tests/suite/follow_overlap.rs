//! Follow Me refuses a sweep that would run into itself (Exact Curves
//! 6c-2): further along the path, or round past a sphere's poles.

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::follow::{self, CROSSES_AXIS, OVERLAPS};
use prism_ops::{add_arc, add_circle, add_segment, check, face_at};
use prism_topo::{EdgeId, FaceId, Model};

/// A square of side `s` in the plane x = 0, centred on the origin.
fn square(m: &mut Model, s: f64) -> FaceId {
    let h = s / 2.0;
    let c = [(-h, -h), (h, -h), (h, h), (-h, h)].map(|(y, z)| DVec3::new(0.0, y, z));
    for i in 0..4 {
        add_segment(m, c[i], c[(i + 1) % 4]);
    }
    face_at(m, DVec3::ZERO).expect("the square")
}

/// Lines through `pts`, returned as the path.
fn path(m: &mut Model, pts: &[DVec3]) -> BTreeSet<EdgeId> {
    for w in pts.windows(2) {
        add_segment(m, w[0], w[1]);
    }
    let on = |p: DVec3| pts.iter().any(|q| q.distance(p) < 1e-9);
    m.edges()
        .filter(|(_, e)| on(m.vertex(e.a)) && on(m.vertex(e.b)))
        .map(|(id, _)| id)
        .collect()
}

/// A path that comes back over its first line, 0.5 above it.
fn returning(m: &mut Model) -> BTreeSet<EdgeId> {
    path(
        m,
        &[
            DVec3::ZERO,
            DVec3::new(10.0, 0.0, 0.0),
            DVec3::new(10.0, 6.0, 0.25),
            DVec3::new(5.0, 6.0, 0.5),
            DVec3::new(5.0, -6.0, 0.5),
        ],
    )
}

#[test]
fn a_profile_too_wide_for_a_returning_path_is_refused() {
    let mut m = Model::new();
    let prof = square(&mut m, 2.0);
    let p = returning(&mut m);
    assert_eq!(follow::refusal(&m, &p, prof), Some(OVERLAPS));
    let faces = m.face_count();
    assert!(!follow::follow(&mut m, &p, prof));
    assert_eq!(m.face_count(), faces, "nothing made");
}

#[test]
fn a_narrow_profile_passes_over_its_own_sweep() {
    let mut m = Model::new();
    let prof = square(&mut m, 0.4);
    let p = returning(&mut m);
    assert_eq!(follow::refusal(&m, &p, prof), None);
    assert!(follow::follow(&mut m, &p, prof));
    let probs = check::problems(&m);
    assert!(probs.is_empty(), "{probs:?}");
}

#[test]
fn a_sphere_profile_past_the_poles_is_refused() {
    let mut m = Model::new();
    // A circle's larger piece, cut off by the chord x = 0.6: its arc runs
    // round through x = -1, across the axis, though its ends do not.
    let (a, b) = (DVec3::new(0.6, 0.0, 0.8), DVec3::new(0.6, 0.0, -0.8));
    add_segment(&mut m, a, b);
    let minor = 2.0 * 0.8f64.asin();
    add_arc(
        &mut m,
        DVec3::ZERO,
        -DVec3::Y,
        1.0,
        a,
        std::f64::consts::TAU - minor,
    );
    let prof = face_at(&m, DVec3::new(-0.5, 0.0, 0.0)).expect("the profile");
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    add_circle(&mut m, DVec3::new(0.0, 0.0, 5.0), DVec3::Z, 0.5);
    let p: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert_eq!(follow::refusal(&m, &p, prof), Some(CROSSES_AXIS));
}
