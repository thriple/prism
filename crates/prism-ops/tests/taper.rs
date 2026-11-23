//! Taper (Shapes & Placement part 7a).

#![cfg(not(target_arch = "wasm32"))]

use std::f64::consts::PI;

use prism_geom::Surface;
use prism_math::DVec3;
use prism_ops::primitives::Primitive;
use prism_ops::taper;
use prism_ops::{check, validate};

fn frustum_volume(h: f64, a: f64, b: f64) -> f64 {
    h / 3.0 * (a * a + b * b + a * b)
}

fn cone_frustum_volume(h: f64, r1: f64, r2: f64) -> f64 {
    PI * h / 3.0 * (r1 * r1 + r1 * r2 + r2 * r2)
}

#[test]
fn tapering_a_box_makes_a_frustum() {
    let mut m = Primitive::Box {
        w: 2.0,
        d: 2.0,
        h: 2.0,
    }
    .build();
    assert_eq!(
        taper::taper(&mut m, DVec3::ZERO, DVec3::Z, 0.0, 2.0, 0.5),
        Ok(())
    );
    let problems = check::problems(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let c = &validate::validate(&m)[0];
    assert!(c.valid, "{:?}", c.reasons);
    let want = frustum_volume(2.0, 2.0, 1.0);
    assert!(
        (c.volume.expect("closed") - want).abs() < 1e-6,
        "{:?} against {want}",
        c.volume
    );
    // The base stays exactly where it was.
    assert!(prism_ops::vertex_at(&m, DVec3::new(1.0, 1.0, 0.0)).is_some());
    // The top shrinks to half width, centred on the axis.
    assert!(prism_ops::vertex_at(&m, DVec3::new(0.5, 0.5, 2.0)).is_some());
}

#[test]
fn growing_from_the_top_widens_the_base_end_too() {
    // Scaling 1.0 at the bottom and 2.0 at the top is the same shape as
    // scaling 0.5 at the bottom and 1.0 at the top: only the ratio and
    // the span matter, not which end is called "low".
    let mut m = Primitive::Box {
        w: 2.0,
        d: 2.0,
        h: 2.0,
    }
    .build();
    taper::taper(&mut m, DVec3::ZERO, DVec3::Z, 0.0, 2.0, 2.0).expect("tapers");
    let c = &validate::validate(&m)[0];
    assert!(c.valid, "{:?}", c.reasons);
    let want = frustum_volume(2.0, 2.0, 4.0);
    assert!((c.volume.expect("closed") - want).abs() < 1e-6);
}

#[test]
fn tapering_a_cylinder_makes_a_cone_frustum() {
    let mut m = Primitive::Cylinder { r: 1.0, h: 2.0 }.build();
    assert_eq!(
        taper::taper(&mut m, DVec3::ZERO, DVec3::Z, 0.0, 2.0, 0.5),
        Ok(())
    );
    let side = m
        .faces()
        .find(|(_, f)| f.is_curved())
        .expect("a curved side");
    assert!(
        matches!(side.1.surface, Surface::Cone(_)),
        "a tapered cylinder is a cone"
    );
    let problems = check::problems(&m);
    assert!(problems.is_empty(), "{problems:?}");
    let c = &validate::validate(&m)[0];
    assert!(c.valid, "{:?}", c.reasons);
    let want = cone_frustum_volume(2.0, 1.0, 0.5);
    assert!(
        (c.volume.expect("closed") - want).abs() < 1e-6,
        "{:?} against {want}",
        c.volume
    );
}

#[test]
fn refusals() {
    let m = Primitive::Box {
        w: 2.0,
        d: 2.0,
        h: 2.0,
    }
    .build();

    let mut w = m.clone();
    assert_eq!(
        taper::taper(&mut w, DVec3::ZERO, DVec3::ZERO, 0.0, 2.0, 0.5).err(),
        Some(taper::AXIS)
    );
    assert_eq!(
        taper::taper(&mut w, DVec3::ZERO, DVec3::Z, 1.0, 1.0, 0.5).err(),
        Some(taper::SPAN)
    );
    assert_eq!(
        taper::taper(&mut w, DVec3::ZERO, DVec3::Z, 0.0, 2.0, 0.0).err(),
        Some(taper::SCALE)
    );
    assert_eq!(
        taper::taper(&mut w, DVec3::ZERO, DVec3::Z, 0.0, 2.0, -1.0).err(),
        Some(taper::SCALE)
    );
    assert_eq!(w, m, "nothing changed when refused");

    // A box's walls cannot stay exact about a diagonal axis: it is
    // neither square to any wall (a cap) nor held by one (a wall).
    let mut d = m.clone();
    assert_eq!(
        taper::taper(
            &mut d,
            DVec3::ZERO,
            DVec3::new(1.0, 1.0, 0.0),
            0.0,
            2.0,
            0.5
        )
        .err(),
        Some(taper::CANNOT)
    );
    assert_eq!(d, m, "nothing changed when refused");

    // A sphere has no exact taper at all.
    let sphere = Primitive::Sphere { r: 1.0 }.build();
    let mut s = sphere.clone();
    assert_eq!(
        taper::taper(&mut s, DVec3::ZERO, DVec3::Z, -1.0, 1.0, 0.5).err(),
        Some(taper::CANNOT)
    );
    assert_eq!(s, sphere, "nothing changed when refused");
}

#[test]
fn refusal_helper_matches() {
    let m = Primitive::Box {
        w: 2.0,
        d: 2.0,
        h: 2.0,
    }
    .build();
    assert_eq!(
        taper::refusal(&m, DVec3::ZERO, DVec3::Z, 0.0, 2.0, 0.0),
        Some(taper::SCALE)
    );
    assert_eq!(
        taper::refusal(&m, DVec3::ZERO, DVec3::Z, 0.0, 2.0, 0.5),
        None
    );
}
