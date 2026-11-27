//! Bend, Twist and Taper (Shapes & Placement part 7b, brief 51), accurate
//! to tolerance (DECISIONS #40).

#![cfg(not(target_arch = "wasm32"))]

use std::f64::consts::{FRAC_PI_2, PI};

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::deform::{self, Deform, DeformKind};
use prism_ops::primitives::Primitive;
use prism_ops::{check, validate};

fn bar(w: f64, h: f64) -> prism_topo::Model {
    Primitive::Box { w, d: w, h }.build()
}

fn about_z(kind: DeformKind, low: f64, high: f64, amount: f64) -> Deform {
    Deform {
        kind,
        origin: DVec3::ZERO,
        axis: DVec3::Z,
        dir: DVec3::X,
        low,
        high,
        amount,
    }
}

/// Clean, closed, facing out, and its volume.
fn solid_volume(m: &prism_topo::Model) -> f64 {
    let problems = check::problems(m);
    assert!(problems.is_empty(), "{problems:?}");
    let checks = validate::validate(m);
    assert_eq!(checks.len(), 1, "one solid");
    assert!(checks[0].valid, "{:?}", checks[0].reasons);
    checks[0].volume.expect("closed")
}

#[test]
fn a_tapered_box_is_still_six_planes_and_a_frustum() {
    let mut m = bar(2.0, 2.0);
    let d = about_z(DeformKind::Taper, 0.0, 2.0, 0.5);
    assert_eq!(deform::deform(&mut m, &d), Ok(()));
    assert!(
        m.faces()
            .all(|(_, f)| matches!(f.surface, Surface::Plane(_)))
    );
    let want = 2.0 / 3.0 * (4.0 + 1.0 + 2.0);
    let got = solid_volume(&m);
    assert!((got - want).abs() < 1e-6, "{got} against {want}");
}

#[test]
fn a_twisted_box_keeps_flat_caps_and_its_volume() {
    let mut m = bar(1.0, 3.0);
    let d = about_z(DeformKind::Twist, 0.0, 3.0, FRAC_PI_2);
    assert_eq!(deform::deform(&mut m, &d), Ok(()));
    // The top and bottom stay flat; the sides become freeform.
    let flat = m
        .faces()
        .filter(|(_, f)| matches!(f.surface, Surface::Plane(_)))
        .count();
    assert_eq!(flat, 2);
    // A twist moves nothing in or out: the volume stays 3.
    let got = solid_volume(&m);
    assert!((got - 3.0).abs() < 1e-3, "{got}");
    // The top's corners have turned a quarter turn about the axis.
    let top: Vec<DVec3> = m.vertices().map(|(_, p)| p).filter(|p| p.z > 2.9).collect();
    assert!(
        top.iter()
            .any(|p| p.distance(DVec3::new(-0.5, 0.5, 3.0)) < 1e-9),
        "{top:?}"
    );
}

#[test]
fn a_bent_bar_follows_its_arc_and_keeps_its_volume() {
    let mut m = bar(0.2, 4.0);
    let d = about_z(DeformKind::Bend, 0.0, 4.0, FRAC_PI_2);
    assert_eq!(deform::deform(&mut m, &d), Ok(()));
    let r = 4.0 / FRAC_PI_2;
    // The top's middle is a quarter turn round, at (r, 0, r).
    let top: Vec<DVec3> = m
        .vertices()
        .map(|(_, p)| p)
        .filter(|p| p.x > r - 0.2)
        .collect();
    assert_eq!(top.len(), 4, "{top:?}");
    let mid = top.iter().copied().sum::<DVec3>() / 4.0;
    assert!(mid.distance(DVec3::new(r, 0.0, r)) < 1e-9, "{mid}");
    // The top now looks along red, not blue.
    let cap = m
        .faces()
        .filter_map(|(_, f)| f.plane().copied())
        .find(|p| p.normal.x > 0.9)
        .expect("the top, turned");
    assert!(cap.normal.distance(DVec3::X) < 1e-9);
    // Bending about the bar's own middle neither stretches nor squashes it.
    let got = solid_volume(&m);
    assert!((got - 0.16).abs() < 1e-3, "{got}");
}

#[test]
fn a_negative_bend_goes_the_other_way() {
    let mut m = bar(0.2, 4.0);
    let d = about_z(DeformKind::Bend, 0.0, 4.0, -FRAC_PI_2);
    assert_eq!(deform::deform(&mut m, &d), Ok(()));
    assert!(m.vertices().any(|(_, p)| p.x < -2.0));
    assert!(m.vertices().all(|(_, p)| p.x < 0.2));
}

// Where an edge crosses a bend's span end its speed jumps, so the fit joins
// there with multiplicity p (C0), not p-1.
#[test]
fn below_the_span_nothing_moves_and_above_it_carries_on_straight() {
    let mut m = bar(0.2, 4.0);
    // Bend only the middle 2 m, a half turn.
    let d = about_z(DeformKind::Bend, 1.0, 3.0, PI);
    assert_eq!(deform::deform(&mut m, &d), Ok(()));
    // A half turn sends the last metre straight back down, to the ground
    // again but out at x = 2r; the base itself, below the span, stays put.
    let r = 2.0 / PI;
    let ground: Vec<DVec3> = m
        .vertices()
        .map(|(_, p)| p)
        .filter(|p| p.z.abs() < 1e-9)
        .collect();
    let base = ground.iter().filter(|p| p.x < 1.0).count();
    let top = ground
        .iter()
        .filter(|p| (p.x - 2.0 * r).abs() < 0.2 + 1e-9)
        .count();
    assert_eq!((base, top), (4, 4), "the base stays, the top comes down");
    solid_volume(&m);
}

#[test]
fn nothing_changes_for_no_amount() {
    let mut m = bar(1.0, 1.0);
    let before = m.clone();
    for kind in [DeformKind::Bend, DeformKind::Twist] {
        assert_eq!(
            deform::deform(&mut m, &about_z(kind, 0.0, 1.0, 0.0)),
            Ok(())
        );
    }
    assert_eq!(
        deform::deform(&mut m, &about_z(DeformKind::Taper, 0.0, 1.0, 1.0)),
        Ok(())
    );
    assert_eq!(m, before);
}

#[test]
fn refusals_say_why_and_change_nothing() {
    let m = bar(1.0, 1.0);
    let mut cases = vec![
        (about_z(DeformKind::Taper, 0.0, 1.0, 0.0), deform::SCALE),
        (about_z(DeformKind::Twist, 1.0, 1.0, 1.0), deform::SPAN),
        // Bent so tight its own centre is inside the bar.
        (about_z(DeformKind::Bend, 0.0, 1.0, 4.0), deform::FOLD),
    ];
    let mut along = about_z(DeformKind::Bend, 0.0, 1.0, 1.0);
    along.dir = DVec3::Z;
    cases.push((along, deform::DIR));
    let mut no_axis = about_z(DeformKind::Twist, 0.0, 1.0, 1.0);
    no_axis.axis = DVec3::ZERO;
    cases.push((no_axis, deform::AXIS));
    for (d, why) in cases {
        let mut after = m.clone();
        assert_eq!(deform::deform(&mut after, &d), Err(why), "{d:?}");
        assert_eq!(after, m);
        assert_eq!(deform::refusal(&m, &d), Some(why));
    }
    let cylinder = Primitive::Cylinder { r: 0.5, h: 1.0 }.build();
    let mut after = cylinder.clone();
    assert_eq!(
        deform::deform(&mut after, &about_z(DeformKind::Twist, 0.0, 1.0, 1.0)),
        Err(deform::CURVED)
    );
    assert_eq!(after, cylinder);
}
