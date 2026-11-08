//! Offset of faces: inward, outward, with holes, and clamped.

#![cfg(not(target_arch = "wasm32"))]

use prism_math::DVec3;
use prism_ops::{check, offset};
use prism_topo::{FaceId, Model};
use proptest::prelude::*;

fn ok(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn face(m: &Model, x: f64, y: f64) -> FaceId {
    prism_ops::face_at(m, DVec3::new(x, y, 0.0)).expect("a face there")
}

fn areas(m: &Model) -> Vec<f64> {
    let mut a: Vec<f64> = m.faces().map(|(id, _)| m.face_area(id)).collect();
    a.sort_by(f64::total_cmp);
    a
}

fn close(a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len(), "{a:?} vs {b:?}");
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() < 1e-9 * (1.0 + y.abs()), "{a:?} vs {b:?}");
    }
}

fn poly(m: &mut Model, pts: &[(f64, f64)]) {
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        prism_ops::add_segment(m, DVec3::new(a.0, a.1, 0.0), DVec3::new(b.0, b.1, 0.0));
    }
}

#[test]
fn a_square_offsets_inward_into_a_ring_and_a_middle() {
    for k in [1e-3, 1.0, 1e3] {
        let mut m = Model::new();
        prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0 * k, 4.0 * k, 0.0);
        let f = face(&m, 2.0 * k, 2.0 * k);
        assert_eq!(offset::offset(&mut m, f, 1.0 * k), Some(1.0 * k));
        ok(&m);
        close(&areas(&m), &[4.0 * k * k, 12.0 * k * k]);
    }
}

#[test]
fn a_square_offsets_outward_into_a_ring() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    let f = face(&m, 2.0, 2.0);
    offset::offset(&mut m, f, -1.0);
    ok(&m);
    // The 6 x 6 outline, less the 4 x 4 face, is a ring of 20.
    close(&areas(&m), &[16.0, 20.0]);
}

#[test]
fn an_l_shape_offsets_with_mitred_corners() {
    let mut m = Model::new();
    poly(
        &mut m,
        &[
            (0.0, 0.0),
            (4.0, 0.0),
            (4.0, 2.0),
            (2.0, 2.0),
            (2.0, 4.0),
            (0.0, 4.0),
        ],
    );
    let f = face(&m, 1.0, 1.0);
    offset::offset(&mut m, f, 0.5);
    ok(&m);
    // The inner L: (0.5..3.5 x 0.5..1.5) plus (0.5..1.5 x 1.5..3.5) = 3 + 2.
    close(&areas(&m), &[5.0, 7.0]);
}

#[test]
fn a_face_with_a_hole_offsets_both_loops() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 10.0, 10.0, 0.0);
    prism_ops::add_rect(&mut m, 4.0, 4.0, 6.0, 6.0, 0.0);
    let f = face(&m, 1.0, 1.0);
    assert_eq!(m.face(f).unwrap().holes.len(), 1);
    offset::offset(&mut m, f, 1.0);
    ok(&m);
    // Outer ring 100 - 64 = 36; the middle 64 - 16 = 48; the ring round the
    // hole 16 - 4 = 12; and the hole's own face, 4.
    close(&areas(&m), &[4.0, 12.0, 36.0, 48.0]);
}

#[test]
fn too_far_stops_short_and_stays_valid() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 2.0, 0.0);
    let f = face(&m, 1.0, 1.0);
    let used = offset::offset(&mut m, f, 5.0).expect("a clamped offset");
    assert!(used < 1.0 && used > 0.99, "{used}");
    ok(&m);
    assert_eq!(m.face_count(), 2);
}

#[test]
fn holes_that_would_meet_the_outline_stop_short() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 10.0, 10.0, 0.0);
    prism_ops::add_rect(&mut m, 2.0, 2.0, 4.0, 4.0, 0.0);
    let f = face(&m, 8.0, 8.0);
    // The hole grows and the outline shrinks: they meet at 1.
    let used = offset::offset(&mut m, f, 3.0).expect("clamped");
    assert!(used < 1.0 && used > 0.99, "{used}");
    ok(&m);
}

#[test]
fn signed_distance_is_positive_inside() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    let f = face(&m, 2.0, 2.0);
    let d = |x, y| offset::signed_distance(&m, f, DVec3::new(x, y, 0.0)).unwrap();
    assert!((d(1.0, 2.0) - 1.0).abs() < 1e-12);
    assert!((d(-0.5, 2.0) + 0.5).abs() < 1e-12);
}

#[test]
fn a_tilted_face_offsets_in_its_own_plane() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.0, 4.0, 0.0);
    let f = face(&m, 2.0, 2.0);
    prism_ops::push_pull(&mut m, f, 3.0, false);
    // A side wall, upright in the x-z plane at y = 0.
    let wall = m
        .faces()
        .find(|(_, fc)| fc.plane().normal.y < -0.9)
        .map(|(id, _)| id)
        .expect("the y = 0 wall");
    offset::offset(&mut m, wall, 0.5);
    ok(&m);
    let a = m.face_area(
        m.faces()
            .filter(|(_, fc)| fc.plane().normal.y < -0.9)
            .map(|(id, _)| id)
            .min_by(|a, b| m.face_area(*a).total_cmp(&m.face_area(*b)))
            .unwrap(),
    );
    assert!((a - 3.0 * 2.0).abs() < 1e-9, "{a}");
}

proptest! {
    #[test]
    fn convex_inward_offsets_stay_inside_and_parallel(
        n in 3usize..9,
        r in 1.0f64..10.0,
        frac in 0.05f64..0.6,
        turn in 0.0f64..1.0,
    ) {
        let mut m = Model::new();
        let pts: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let t = (i as f64 + turn) / n as f64 * std::f64::consts::TAU;
                (r * t.cos(), r * t.sin())
            })
            .collect();
        poly(&mut m, &pts);
        let f = face(&m, 0.0, 0.0);
        // The inscribed radius bounds a valid offset.
        let inr = r * (std::f64::consts::PI / n as f64).cos();
        let d = inr * frac;
        let (loops, used) = offset::offset_loops(&m, f, d).expect("an offset");
        prop_assert!((used - d).abs() < 1e-12);
        let outer = &loops[0];
        let orig: Vec<DVec3> = m.face(f).unwrap().outer.iter().map(|v| m.vertex(*v)).collect();
        for i in 0..outer.len() {
            let e1 = (outer[(i + 1) % outer.len()] - outer[i]).normalize();
            let e0 = (orig[(i + 1) % orig.len()] - orig[i]).normalize();
            prop_assert!(e1.dot(e0) > 1.0 - 1e-9);
            let s = offset::signed_distance(&m, f, outer[i]).unwrap();
            prop_assert!(s > 0.0);
        }
        offset::offset(&mut m, f, d);
        prop_assert!(check::problems(&m).is_empty());
    }
}

/// The distance whose offset loop passes through a point: past a corner it
/// follows the mitre, not the straight line to the corner (owner feedback,
/// 2026-11-08).
#[test]
fn the_distance_through_a_point_follows_the_mitred_loop() {
    let mut m = Model::new();
    poly(&mut m, &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
    let f = face(&m, 2.0, 2.0);
    let at = |x, y| offset::distance_through(&m, f, DVec3::new(x, y, 0.0)).unwrap();
    // Beside an edge: the plain distance.
    assert!((at(5.0, 2.0) + 1.0).abs() < 1e-9);
    assert!((at(1.0, 2.0) - 1.0).abs() < 1e-9);
    // Off a corner: the loop 2 out runs through (6, 5); straight to the
    // corner would be 2.24.
    assert!((at(6.0, 5.0) + 2.0).abs() < 1e-9, "{}", at(6.0, 5.0));
    assert!((at(-1.0, -1.5) + 1.5).abs() < 1e-9);
}
