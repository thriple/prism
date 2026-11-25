//! The top of a cylinder scaled, slid or turned: the side follows as the
//! surface between its two ends, exactly (owner, 2026-11-25: "resizing the
//! face in any direction, or even sliding the face around and the walls
//! have to adjust"). Volumes are worked out by hand.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_geom::{Curve, Surface};
use prism_math::DVec3;
use prism_ops::pushpull::{Pulled, push_pull};
use prism_ops::transform;
use prism_ops::{add_circle, face_at};
use prism_topo::{Model, VertexId};

/// A cylinder of radius 1 about the z axis from z = 0 to 2.
fn cylinder() -> Model {
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let disc = face_at(&m, DVec3::ZERO).expect("a disc");
    let up = m
        .face(disc)
        .and_then(|f| f.plane())
        .map(|p| p.normal.z)
        .unwrap();
    assert!(!matches!(
        push_pull(&mut m, disc, 2.0 * up.signum(), false),
        Pulled::Nothing
    ));
    m
}

fn top(m: &Model) -> BTreeSet<VertexId> {
    m.vertices()
        .filter(|(_, p)| (p.z - 2.0).abs() < 1e-9)
        .map(|(id, _)| id)
        .collect()
}

fn volume(m: &Model) -> f64 {
    m.faces()
        .map(|(id, _)| prism_ops::health::face_volume(m, id))
        .sum::<f64>()
        .abs()
}

fn sound(m: &Model) {
    assert_eq!(prism_ops::check::problems(m), Vec::<String>::new());
    assert!(prism_ops::boolean::is_solid(m), "closed");
}

fn sides(m: &Model) -> Vec<Surface> {
    m.faces()
        .filter(|(_, f)| f.is_curved())
        .map(|(_, f)| f.surface.clone())
        .collect()
}

fn near(a: f64, b: f64, what: &str) {
    assert!((a - b).abs() < 1e-6, "{what}: {a} is not {b}");
}

#[test]
fn scaling_the_top_evenly_makes_a_cone() {
    let mut m = cylinder();
    let vs = top(&m);
    let c = DVec3::new(0.0, 0.0, 2.0);
    assert_eq!(
        transform::arcs_refusal(&m, &vs, &transform::scaling(c, DVec3::splat(0.5))),
        None
    );
    transform::scale(&mut m, &vs, c, DVec3::splat(0.5));
    sound(&m);
    assert!(
        sides(&m).iter().all(|s| matches!(s, Surface::Cone(_))),
        "{:?}",
        sides(&m)
    );
    // A frustum: π h (R² + R r + r²) / 3.
    near(volume(&m), PI * 2.0 * (1.0 + 0.5 + 0.25) / 3.0, "frustum");
}

#[test]
fn sliding_the_top_up_keeps_a_cylinder() {
    let mut m = cylinder();
    let vs = top(&m);
    transform::translate(&mut m, &vs, DVec3::Z);
    sound(&m);
    assert!(sides(&m).iter().all(|s| matches!(s, Surface::Cylinder(_))));
    near(volume(&m), 3.0 * PI, "taller");
}

#[test]
fn sliding_the_top_sideways_leans_the_side() {
    let mut m = cylinder();
    let vs = top(&m);
    transform::translate(&mut m, &vs, DVec3::new(0.5, 0.25, 0.0));
    sound(&m);
    assert!(sides(&m).iter().all(|s| matches!(s, Surface::Nurbs(_))));
    // Every slice is still the unit circle (Cavalieri).
    near(volume(&m), 2.0 * PI, "leaning");
}

#[test]
fn scaling_the_top_one_way_makes_an_ellipse_exactly() {
    let mut m = cylinder();
    let vs = top(&m);
    let c = DVec3::new(0.0, 0.0, 2.0);
    let k = DVec3::new(2.0, 1.0, 1.0);
    assert_eq!(
        transform::arcs_refusal(&m, &vs, &transform::scaling(c, k)),
        None
    );
    transform::scale(&mut m, &vs, c, k);
    sound(&m);
    // The top's edges are exact curves, not segments.
    let top_edges = m.edges().filter(|(_, e)| {
        (m.vertex(e.a).z - 2.0).abs() < 1e-9 && (m.vertex(e.b).z - 2.0).abs() < 1e-9
    });
    let kinds: Vec<_> = top_edges.map(|(_, e)| e.curve.is_spline()).collect();
    assert_eq!(kinds, vec![true, true], "two exact halves of the ellipse");
    // A slice at height z is an ellipse of half-axes 1 + z/2 and 1:
    // the integral of π (1 + z/2) over 0..2 is 3π.
    near(volume(&m), 3.0 * PI, "to an ellipse");
    // A point of the ellipse lies on its edge exactly.
    let e = m
        .edges()
        .find(|(_, e)| e.curve.is_spline())
        .map(|(_, e)| e.clone())
        .unwrap();
    let (pa, pb) = (m.vertex(e.a), m.vertex(e.b));
    let q = e.curve.point(pa, pb, 0.3);
    near((q.x / 2.0).powi(2) + q.y.powi(2), 1.0, "on the ellipse");
    let _ = Curve::Line;
}

#[test]
fn turning_the_top_twists_the_side() {
    let mut m = cylinder();
    let vs = top(&m);
    transform::rotate(&mut m, &vs, DVec3::new(0.0, 0.0, 2.0), DVec3::Z, PI / 6.0);
    sound(&m);
    assert!(sides(&m).iter().all(|s| matches!(s, Surface::Nurbs(_))));
}

#[test]
fn twice_in_a_row() {
    // An edit of a band an edit made: slid, then scaled.
    let mut m = cylinder();
    let vs = top(&m);
    transform::translate(&mut m, &vs, DVec3::new(0.5, 0.0, 0.0));
    let vs = top(&m);
    transform::scale(
        &mut m,
        &vs,
        DVec3::new(0.5, 0.0, 2.0),
        DVec3::new(1.0, 0.5, 1.0),
    );
    sound(&m);
}
