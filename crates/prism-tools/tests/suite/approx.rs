//! The fallback for an uneven scale (Exact Curves 6a, DECISIONS #32):
//! circles and arcs become exact ellipses (rational curves); curved solids
//! become flat facets, every point on the true ellipse or ellipsoid, the model check
//! clean, and the volume within the chords' error.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_math::DVec3;
use prism_ops::{check, health::face_volume, transform};
use prism_tools::scenes::{cylinder, sphere, torus};
use prism_topo::{Model, VertexId};

fn volume(m: &Model) -> f64 {
    m.faces().map(|(id, _)| face_volume(m, id)).sum()
}

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn all(m: &Model) -> BTreeSet<VertexId> {
    m.vertices().map(|(id, _)| id).collect()
}

/// Scale the whole model by `k` about the origin.
fn stretch(m: &mut Model, k: DVec3) {
    let vs = all(m);
    transform::scale(m, &vs, DVec3::ZERO, k);
}

fn exact_left(m: &Model) -> (usize, usize) {
    let arcs = m.edges().filter(|(_, e)| e.curve.is_arc()).count();
    let curved = m.faces().filter(|(_, f)| f.is_curved()).count();
    (arcs, curved)
}

/// An arc or circle scaled unevenly is an exact ellipse, a rational curve
/// (DECISIONS #38; owner's session 30), not segments: every point of it on
/// the true ellipse.
#[test]
fn a_disc_scaled_unevenly_becomes_an_exact_ellipse() {
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    stretch(&mut m, DVec3::new(2.0, 1.0, 1.0));
    clean(&m);
    assert!(m.edges().all(|(_, e)| !e.approx && e.curve.is_spline()));
    for (_, e) in m.edges() {
        let prism_geom::Curve::Spline(sp) = &e.curve else {
            unreachable!()
        };
        for k in 0..=20 {
            let t = sp.t0 + (sp.t1 - sp.t0) * k as f64 / 20.0;
            let p = sp.point(t);
            assert!(((p.x / 2.0).powi(2) + p.y * p.y - 1.0).abs() < 1e-9, "{p}");
        }
    }
    let area: f64 = m.faces().map(|(id, _)| m.face_area(id)).sum();
    assert!((area - 2.0 * PI).abs() < 1e-3 * 2.0 * PI, "{area}");
}

#[test]
fn a_cylinder_scaled_unevenly_becomes_flat_facets() {
    let mut m = cylinder(1.0, 2.0);
    stretch(&mut m, DVec3::new(1.5, 1.0, 1.0));
    clean(&m);
    assert_eq!(exact_left(&m), (0, 0));
    for (_, p) in m.vertices() {
        assert!(((p.x / 1.5).powi(2) + p.y * p.y - 1.0).abs() < 1e-12, "{p}");
    }
    // The lines between facets are soft and smooth.
    assert!(m.edges().any(|(_, e)| e.approx && e.soft && e.smooth));
    let v = PI * 1.5 * 2.0;
    assert!((volume(&m) - v).abs() < 2e-3 * v, "{} vs {v}", volume(&m));
}

#[test]
fn spheres_and_tori_scaled_unevenly_become_flat_facets() {
    let k = DVec3::new(2.0, 1.0, 0.5);
    let mut s = sphere(DVec3::ZERO, 1.0);
    stretch(&mut s, k);
    clean(&s);
    assert_eq!(exact_left(&s), (0, 0));
    for (_, p) in s.vertices() {
        assert!(((p / k).length() - 1.0).abs() < 1e-12, "{p}");
    }
    let v = 4.0 / 3.0 * PI * k.x * k.y * k.z;
    assert!((volume(&s) - v).abs() < 4e-3 * v, "{} vs {v}", volume(&s));

    let mut t = torus(3.0, 1.0);
    stretch(&mut t, k);
    clean(&t);
    assert_eq!(exact_left(&t), (0, 0));
    let v = 2.0 * PI * PI * 3.0 * k.x * k.y * k.z;
    assert!((volume(&t) - v).abs() < 4e-3 * v, "{} vs {v}", volume(&t));
}

#[test]
fn an_even_scale_keeps_everything_exact() {
    let mut m = cylinder(1.0, 2.0);
    stretch(&mut m, DVec3::splat(2.0));
    clean(&m);
    assert!(m.edges().all(|(_, e)| !e.approx));
    assert!(exact_left(&m).1 > 0);
}
