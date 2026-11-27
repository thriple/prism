//! Hole cutters (Shapes & Placement part 6).

#![cfg(not(target_arch = "wasm32"))]

use prism_ops::hole::{HoleKind, HoleSpec};
use prism_ops::{check, validate};
use std::f64::consts::PI;

fn volume(m: &prism_topo::Model, name: &str) -> f64 {
    let problems = check::problems(m);
    assert!(problems.is_empty(), "{name}: {problems:?}");
    let c = &validate::validate(m)[0];
    assert!(c.valid, "{name}: {:?}", c.reasons);
    c.volume.expect("closed")
}

#[test]
fn the_three_cutters_are_valid_solids_of_the_right_volume() {
    let r = 0.5;
    let above = 0.25 * 1.0; // a quarter of the diameter
    // Plain: a cylinder from -3 up to the margin.
    let plain = HoleSpec {
        diameter: 1.0,
        depth: Some(3.0),
        kind: HoleKind::Plain,
    };
    let v = volume(&plain.cutter(10.0), "plain");
    assert!((v - PI * r * r * (3.0 + above)).abs() < 1e-6, "{v}");
    // Counterbore 2 wide and 1 deep.
    let bore = HoleSpec {
        diameter: 1.0,
        depth: Some(3.0),
        kind: HoleKind::Counterbore {
            diameter: 2.0,
            depth: 1.0,
        },
    };
    let v = volume(&bore.cutter(10.0), "counterbore");
    let want = PI * r * r * 2.0 + PI * 1.0 * 1.0 * (1.0 + above);
    assert!((v - want).abs() < 1e-6, "{v} against {want}");
    // Countersink 2 wide at the top, 90 degrees: a cone half a metre deep.
    let sink = HoleSpec {
        diameter: 1.0,
        depth: Some(3.0),
        kind: HoleKind::Countersink {
            diameter: 2.0,
            angle: 90.0,
        },
    };
    let v = volume(&sink.cutter(10.0), "countersink");
    let cone = PI * 0.5 / 3.0 * (0.5 * 0.5 + 1.0 * 1.0 + 0.5 * 1.0);
    let want = PI * r * r * (3.0 - 0.5) + cone + PI * 1.0 * above;
    assert!((v - want).abs() < 1e-3 * want, "{v} against {want}");
}

#[test]
fn bad_holes_are_refused_with_a_reason() {
    let plain = HoleSpec {
        diameter: 1.0,
        depth: Some(1.0),
        kind: HoleKind::Plain,
    };
    assert_eq!(plain.check(5.0), Ok(()));
    assert_eq!(
        HoleSpec {
            diameter: 0.0,
            ..plain
        }
        .check(5.0),
        Err(prism_ops::hole::TOO_SMALL)
    );
    let narrow = HoleSpec {
        kind: HoleKind::Counterbore {
            diameter: 0.5,
            depth: 0.5,
        },
        ..plain
    };
    assert_eq!(narrow.check(5.0), Err(prism_ops::hole::BORE_SMALLER));
    let deep = HoleSpec {
        kind: HoleKind::Counterbore {
            diameter: 2.0,
            depth: 2.0,
        },
        ..plain
    };
    assert_eq!(deep.check(5.0), Err(prism_ops::hole::BORE_DEEPER));
    let flat = HoleSpec {
        kind: HoleKind::Countersink {
            diameter: 2.0,
            angle: 5.0,
        },
        ..plain
    };
    assert_eq!(flat.check(5.0), Err(prism_ops::hole::SINK_ANGLE));
    // Through uses the body's size for its depth.
    let through = HoleSpec {
        depth: None,
        ..plain
    };
    assert_eq!(through.check(5.0), Ok(()));
}
