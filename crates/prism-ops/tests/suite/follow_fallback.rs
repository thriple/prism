//! Follow Me's fallback (Exact Curves 6a, DECISIONS #32): what cannot be
//! swept exactly is swept with fine approximate segments instead of being
//! refused: a disc round a mitred corner, a profile beside an arc's axis,
//! a line meeting an arc at a corner.

use std::collections::BTreeSet;
use std::f64::consts::{FRAC_PI_2, PI};

use prism_math::DVec3;
use prism_ops::follow;
use prism_ops::{add_arc, add_circle, add_segment, check, health::face_volume};
use prism_topo::{EdgeId, FaceId, Model};

fn volume(m: &Model) -> f64 {
    m.faces().map(|(id, _)| face_volume(m, id)).sum()
}

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn edges(m: &Model) -> BTreeSet<EdgeId> {
    m.edges().map(|(id, _)| id).collect()
}

fn new_edges(m: &Model, before: &BTreeSet<EdgeId>) -> BTreeSet<EdgeId> {
    edges(m).difference(before).copied().collect()
}

fn approx(m: &Model) -> usize {
    m.edges().filter(|(_, e)| e.approx).count()
}

fn poly(m: &mut Model, pts: &[DVec3], normal: DVec3) -> FaceId {
    for i in 0..pts.len() {
        add_segment(m, pts[i], pts[(i + 1) % pts.len()]);
    }
    m.faces()
        .find(|(_, f)| f.plane().is_some_and(|p| p.normal.dot(normal).abs() > 0.99))
        .map(|(id, _)| id)
        .expect("the face")
}

#[test]
fn a_disc_round_a_corner_is_swept_with_fine_segments() {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::new(0.0, 0.0, 2.0), DVec3::X, 0.5);
    let disc = m.faces().next().map(|(id, _)| id).unwrap();
    let before = edges(&m);
    add_segment(&mut m, DVec3::ZERO, DVec3::new(3.0, 0.0, 0.0));
    add_segment(&mut m, DVec3::new(3.0, 0.0, 0.0), DVec3::new(3.0, 3.0, 0.0));
    let p = new_edges(&m, &before);
    assert_eq!(follow::refusal(&m, &p, disc), None);
    assert!(follow::follow(&mut m, &p, disc));
    clean(&m);
    assert!(approx(&m) > 0);
    // Soft and smooth between the facets.
    assert!(m.edges().any(|(_, e)| e.approx && e.soft && e.smooth));
    // A mitred bend: the section's area times the path's length, less the
    // chords' error.
    let v = PI * 0.25 * 6.0;
    assert!((volume(&m) - v).abs() < 3e-3 * v, "{} vs {v}", volume(&m));
}

#[test]
fn a_profile_beside_an_arcs_axis_follows_its_chords() {
    let mut m = Model::new();
    // In the plane y = 1, beside the z axis, not through it.
    let prof = poly(
        &mut m,
        &[
            DVec3::new(1.5, 1.0, 0.0),
            DVec3::new(2.5, 1.0, 0.0),
            DVec3::new(2.5, 1.0, 1.0),
            DVec3::new(1.5, 1.0, 1.0),
        ],
        DVec3::Y,
    );
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
    assert!(approx(&m) > 0);
    assert!(volume(&m) > 0.0);
}

#[test]
fn a_line_meeting_an_arc_at_a_corner_is_mitred_onto_its_chords() {
    // The line comes in 8 degrees off the arc's tangent; a narrow profile
    // stands at its start.
    let mut m = Model::new();
    let a = 8f64.to_radians();
    let start = DVec3::new(2.0 + 3.0 * a.sin(), -3.0 * a.cos(), 3.0);
    let prof = poly(
        &mut m,
        &[
            DVec3::new(start.x - 0.1, start.y, 0.0),
            DVec3::new(start.x + 0.1, start.y, 0.0),
            DVec3::new(start.x + 0.1, start.y, 0.2),
            DVec3::new(start.x - 0.1, start.y, 0.2),
        ],
        DVec3::Y,
    );
    let before = edges(&m);
    add_segment(&mut m, start, DVec3::new(2.0, 0.0, 3.0));
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
    assert!(approx(&m) > 0);
    assert!(volume(&m) > 0.0);
}
