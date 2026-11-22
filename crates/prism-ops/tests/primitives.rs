//! The primitives (Shapes & Placement part 4): each is a valid, closed
//! solid facing out, of the size it was made at.

#![cfg(not(target_arch = "wasm32"))]

use prism_ops::primitives::Primitive;
use prism_ops::{check, validate};

use std::f64::consts::PI;

fn volume_of(p: &Primitive) -> f64 {
    match *p {
        Primitive::Box { w, d, h } => w * d * h,
        Primitive::Cylinder { r, h } => PI * r * r * h,
        Primitive::Cone { r, h } => PI * r * r * h / 3.0,
        Primitive::Sphere { r } => 4.0 / 3.0 * PI * r.powi(3),
        Primitive::Torus { major, minor } => 2.0 * PI * PI * major * minor * minor,
        Primitive::Capsule { r, h } => PI * r * r * h + 4.0 / 3.0 * PI * r.powi(3),
    }
}

#[test]
fn every_primitive_is_a_valid_solid_of_its_size_standing_on_the_ground() {
    for p in Primitive::kinds() {
        let m = p.build();
        let problems = check::problems(&m);
        assert!(problems.is_empty(), "{}: {problems:?}", p.name());
        let checks = validate::validate(&m);
        assert_eq!(checks.len(), 1, "{}: one solid", p.name());
        let c = &checks[0];
        assert!(c.valid, "{}: {:?}", p.name(), c.reasons);
        let v = c.volume.expect("closed");
        assert!(
            (v - volume_of(&p)).abs() < 0.02 * volume_of(&p),
            "{}: volume {v} against {}",
            p.name(),
            volume_of(&p)
        );
        let (lo, hi) = m.bbox().expect("a box");
        if let Primitive::Torus { minor, .. } = p {
            // Its points are all at the tube's middle height.
            assert!((lo.z - minor).abs() < 1e-9 && (hi.z - minor).abs() < 1e-9);
        } else {
            assert!(
                lo.z.abs() < 1e-9,
                "{} stands on the ground: {lo:?}",
                p.name()
            );
            assert!(
                (hi.z - p.height()).abs() < 1e-9,
                "{} is {} tall, not {}",
                p.name(),
                p.height(),
                hi.z
            );
        }
        assert!(
            ((lo.x + hi.x) / 2.0).abs() < 1e-9 && ((lo.y + hi.y) / 2.0).abs() < 1e-9,
            "{} is centred: {lo:?} {hi:?}",
            p.name()
        );
    }
}

#[test]
fn sizes_are_changed_one_at_a_time_and_bad_ones_refused() {
    let b = Primitive::Box {
        w: 1.0,
        d: 2.0,
        h: 3.0,
    };
    assert_eq!(
        b.with("d", 5.0),
        Ok(Primitive::Box {
            w: 1.0,
            d: 5.0,
            h: 3.0
        })
    );
    assert_eq!(b.with("d", 0.0), Err(prism_ops::primitives::TOO_SMALL));
    assert_eq!(b.with("r", 1.0), Err("No such size"));
    let t = Primitive::Torus {
        major: 1.0,
        minor: 0.2,
    };
    assert_eq!(t.with("minor", 1.0), Err(prism_ops::primitives::TORUS_TUBE));
    assert!(t.with("major", 3.0).is_ok());
    // Different sizes make different, still valid, solids.
    for p in [
        Primitive::Capsule { r: 0.1, h: 3.0 },
        Primitive::Cone { r: 2.0, h: 0.2 },
        Primitive::Torus {
            major: 2.0,
            minor: 1.9,
        },
    ] {
        let c = &validate::validate(&p.build())[0];
        assert!(c.valid, "{}: {:?}", p.name(), c.reasons);
    }
}
