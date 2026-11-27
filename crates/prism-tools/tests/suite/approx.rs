//! The segment fallback (Exact Curves 6a, DECISIONS #32): an uneven scale
//! of circles and curved solids gives fine approximate segments and flat
//! facets, every point on the true ellipse or ellipsoid, the model check
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

#[test]
fn a_disc_scaled_unevenly_becomes_an_approximate_ellipse() {
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    stretch(&mut m, DVec3::new(2.0, 1.0, 1.0));
    clean(&m);
    assert_eq!(exact_left(&m), (0, 0));
    assert!(m.edges().all(|(_, e)| e.approx));
    // One curve: every segment shares a curve id.
    let ids: BTreeSet<_> = m.edges().map(|(_, e)| e.curve_id).collect();
    assert_eq!(ids.len(), 1);
    for (_, p) in m.vertices() {
        assert!(((p.x / 2.0).powi(2) + p.y * p.y - 1.0).abs() < 1e-12, "{p}");
    }
    let area: f64 = m
        .faces()
        .map(|(_, f)| prism_math::newell(&m.loop_points(f, 0)).length() / 2.0)
        .sum();
    assert!((area - 2.0 * PI).abs() < 2e-3 * 2.0 * PI, "{area}");
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
