//! Tolerance stage 5 (DECISIONS #42): the hand-written distances that
//! were decisions now ask the document's precision.

use prism_math::{DVec3, Tol};
use prism_ops::add_rect;
use prism_topo::Model;

fn plate(tol: Tol) -> Model {
    let mut m = Model::with_tol(tol);
    add_rect(&mut m, 0.0, 0.0, 1.0, 1.0, 0.0);
    m
}

/// A taper's two ends 5 µm apart along its axis: no span at 0.01 mm (it
/// was any span over 1e-9 m), a span at 0.001 mm.
#[test]
fn a_taper_span_is_judged_by_the_precision() {
    let mut m = plate(Tol::default());
    let r = prism_ops::taper::taper(&mut m, DVec3::ZERO, DVec3::Z, 0.0, 5e-6, 0.5);
    assert_eq!(r, Err(prism_ops::taper::SPAN));
    let mut m = plate(Tol::new(prism_math::tol::FINE));
    let r = prism_ops::taper::taper(&mut m, DVec3::ZERO, DVec3::Z, 0.0, 5e-6, 0.5);
    assert_ne!(r, Err(prism_ops::taper::SPAN));
}

/// The same for a deform's span.
#[test]
fn a_deform_span_is_judged_by_the_precision() {
    let d = prism_ops::deform::Deform {
        kind: prism_ops::deform::DeformKind::Twist,
        origin: DVec3::ZERO,
        axis: DVec3::Z,
        dir: DVec3::X,
        low: 0.0,
        high: 5e-6,
        amount: 0.3,
    };
    let m = plate(Tol::default());
    assert_eq!(prism_ops::deform::refusal(&m, &d), Some(prism_ops::deform::SPAN));
    let m = plate(Tol::new(prism_math::tol::FINE));
    assert_ne!(prism_ops::deform::refusal(&m, &d), Some(prism_ops::deform::SPAN));
}
