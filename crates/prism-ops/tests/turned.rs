//! Cones, spheres and tori in the core (Exact Curves 5b), made with the
//! lathe: exact, checked, healthy, and kept exact by moves, turns, mirrors
//! and even scales.

use std::f64::consts::PI;

use prism_geom::Surface;
use prism_math::{DVec2, DVec3};
use prism_ops::lathe::{Side, lathe};
use prism_ops::{check, health, transform};
use prism_topo::{FaceId, Model, VertexId};

fn p(r: f64, h: f64) -> DVec2 {
    DVec2::new(r, h)
}

/// A ball of radius `r` about `c`, its poles along `axis`.
fn ball(c: DVec3, axis: DVec3, r: f64) -> Model {
    let mut m = Model::new();
    let round = Side::Arc {
        centre: DVec2::ZERO,
        ccw: true,
    };
    lathe(
        &mut m,
        c,
        axis,
        &[(p(0.0, -r), round), (p(r, 0.0), round), (p(0.0, r), Side::Line)],
    )
    .expect("a ball");
    m
}

/// A cone of base radius `r` and height `h` standing on the ground.
fn cone(r: f64, h: f64) -> Model {
    let mut m = Model::new();
    lathe(
        &mut m,
        DVec3::ZERO,
        DVec3::Z,
        &[
            (p(0.0, 0.0), Side::Line),
            (p(r, 0.0), Side::Line),
            (p(0.0, h), Side::Line),
        ],
    )
    .expect("a cone");
    m
}

/// A ring: a tube of radius `b` round a circle of radius `a`.
fn ring(a: f64, b: f64) -> Model {
    let mut m = Model::new();
    let round = Side::Arc {
        centre: p(a, 0.0),
        ccw: true,
    };
    lathe(
        &mut m,
        DVec3::ZERO,
        DVec3::Z,
        &[(p(a, -b), round), (p(a + b, 0.0), round), (p(a, b), round), (p(a - b, 0.0), round)],
    )
    .expect("a ring");
    m
}

fn volume(m: &Model) -> f64 {
    m.faces().map(|(id, _)| health::face_volume(m, id)).sum()
}

fn area(m: &Model) -> f64 {
    m.faces().map(|(id, _)| m.face_area(id)).sum()
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

/// Clean: no check problems, one closed shell facing out, nothing off its
/// surface.
fn clean(m: &Model) {
    assert_eq!(check::problems(m), Vec::<String>::new());
    let h = health::check(m);
    assert_eq!((h.watertight, h.leaky), (1, 0), "{h:?}");
    assert!(h.reversed.is_empty(), "{h:?}");
    assert!(h.off_circle.is_empty(), "{h:?}");
}

#[test]
fn a_ball_is_exact_and_clean() {
    let r = 1.3;
    let m = ball(DVec3::new(2.0, -1.0, 3.0), DVec3::new(0.0, 0.6, 0.8), r);
    clean(&m);
    assert_eq!(m.faces().filter(|(_, f)| matches!(f.surface, Surface::Sphere(_))).count(), 4);
    close(volume(&m), 4.0 / 3.0 * PI * r * r * r);
    close(area(&m), 4.0 * PI * r * r);
}

#[test]
fn a_cone_is_exact_and_clean() {
    let (r, h) = (1.5, 2.0);
    let m = cone(r, h);
    clean(&m);
    close(volume(&m), PI * r * r * h / 3.0);
    close(area(&m), PI * r * r + PI * r * (r * r + h * h).sqrt());
    assert!(m.faces().any(|(_, f)| matches!(f.surface, Surface::Cone(_))));
}

#[test]
fn a_ring_is_exact_and_clean() {
    let (a, b) = (2.0, 0.5);
    let m = ring(a, b);
    clean(&m);
    close(volume(&m), 2.0 * PI * PI * a * b * b);
    close(area(&m), 4.0 * PI * PI * a * b);
    // Its seams and rings are tangent all round: all soft.
    assert!(m.edges().all(|(_, e)| e.soft && e.smooth));
}

/// A turned vase: a cylinder foot, a cone, a hollow (a concave torus
/// piece) and a flat top.
#[test]
fn a_turned_profile_with_every_kind_is_exact_and_clean() {
    let mut m = Model::new();
    let faces = lathe(
        &mut m,
        DVec3::ZERO,
        DVec3::Z,
        &[
            (p(0.0, 0.0), Side::Line),
            (p(1.0, 0.0), Side::Line),
            (p(1.0, 0.5), Side::Line),
            (p(0.6, 1.5), Side::Arc {
                centre: p(1.0, 2.0),
                ccw: false,
            }),
            (p(1.0, 2.5), Side::Line),
            (p(0.0, 2.5), Side::Line),
        ],
    );
    // The arc's ends must be on one circle: (1, 2.5) is 0.5 from its
    // centre, (0.6, 1.5) about 0.64.
    assert!(faces.is_none(), "an arc whose ends are not on one circle");
    let mut m = Model::new();
    let d = (0.4f64 * 0.4 + 0.5 * 0.5).sqrt();
    lathe(
        &mut m,
        DVec3::ZERO,
        DVec3::Z,
        &[
            (p(0.0, 0.0), Side::Line),
            (p(1.0, 0.0), Side::Line),
            (p(1.0, 0.5), Side::Line),
            (p(0.6, 1.5), Side::Arc {
                centre: p(1.0, 2.0),
                ccw: false,
            }),
            (p(1.0 - d, 2.0), Side::Line),
            (p(1.0 - d, 2.5), Side::Line),
            (p(0.0, 2.5), Side::Line),
        ],
    )
    .expect("a vase");
    clean(&m);
    let kinds = |k: fn(&Surface) -> bool| m.faces().filter(|(_, f)| k(&f.surface)).count();
    assert_eq!(kinds(|s| matches!(s, Surface::Cone(_))), 2);
    assert_eq!(kinds(|s| matches!(s, Surface::Torus(c) if !c.outward)), 2);
    // Volume by pieces: cylinder, frustum, the hollow part turned (Pappus),
    // and the top cylinder.
    let frustum = PI * 1.0 / 3.0 * (1.0 + 0.6 + 0.36);
    let a0 = 0.5f64.atan2(-0.4); // the arc's start, about its centre
    let a1 = PI;
    // The region between the arc and the axis, from h = 1.5 to 2: by
    // integrating π ρ² dh along the arc.
    let n = 20000;
    let mut hollow = 0.0;
    for i in 0..n {
        let t0 = a0 + (a1 - a0) * i as f64 / n as f64;
        let t1 = a0 + (a1 - a0) * (i + 1) as f64 / n as f64;
        let tm = 0.5 * (t0 + t1);
        let rho = 1.0 + d * tm.cos();
        let dh = d * (t1.sin() - t0.sin());
        hollow -= PI * rho * rho * dh;
    }
    let want = PI * 0.5 + frustum + hollow + PI * (1.0 - d) * (1.0 - d) * 0.5;
    assert!((volume(&m) - want).abs() < 1e-6, "{} vs {want}", volume(&m));
}

#[test]
fn moves_turns_mirrors_and_even_scales_keep_them_exact() {
    let r = 1.0;
    let base = ball(DVec3::ZERO, DVec3::Z, r);
    let all: std::collections::BTreeSet<VertexId> = base.vertices().map(|(id, _)| id).collect();
    type Op = Box<dyn Fn(&mut Model, &std::collections::BTreeSet<VertexId>)>;
    let ops: Vec<(Op, f64)> = vec![
        (Box::new(|x, vs| transform::translate(x, vs, DVec3::new(3.0, 1.0, -2.0))), 1.0),
        (Box::new(|x, vs| transform::rotate(x, vs, DVec3::X, DVec3::new(1.0, 2.0, 0.5).normalize(), 0.7)), 1.0),
        // A mirror of the whole model (as a group is placed) turns every
        // loop round, so faces still face out.
        (Box::new(|x, _| transform::affine(x, &transform::scaling(DVec3::Y, DVec3::new(-1.0, 1.0, 1.0)))), 1.0),
        (Box::new(|x, vs| transform::scale(x, vs, DVec3::ZERO, DVec3::splat(2.5))), 2.5),
    ];
    for (op, k) in ops {
        let mut x = base.clone();
        op(&mut x, &all);
        clean(&x);
        close(volume(&x), 4.0 / 3.0 * PI * (r * k).powi(3));
    }
    // An uneven scale is refused by Scale; placed in a stretched group it
    // cannot keep a ball a ball, and Model Health says so.
    let mut x = base.clone();
    let stretch = DVec3::new(2.0, 1.0, 1.0);
    transform::scale(&mut x, &all, DVec3::ZERO, stretch);
    assert_eq!(x, base, "refused");
    transform::affine(&mut x, &transform::scaling(DVec3::ZERO, stretch));
    let h = health::check(&x);
    assert!(!h.off_circle.faces.is_empty() || !h.off_circle.edges.is_empty(), "{h:?}");
}

#[test]
fn push_pull_says_it_works_on_flat_faces_and_cylinders() {
    let m = cone(1.0, 1.0);
    let side = m
        .faces()
        .find(|(_, f)| matches!(f.surface, Surface::Cone(_)))
        .map(|(id, _)| id)
        .unwrap();
    assert_eq!(
        prism_ops::pushpull::refusal(&m, side, 0.2),
        Some(prism_ops::pushpull::NOT_PULLABLE)
    );
    // The base beside the cone would bend it.
    let base: FaceId = m.faces().find(|(_, f)| f.plane().is_some()).map(|(id, _)| id).unwrap();
    assert!(prism_ops::pushpull::refusal(&m, base, 0.2).is_some());
}

#[test]
fn a_ball_is_written_and_read_back_the_same() {
    let m = ball(DVec3::new(1.0, 2.0, 3.0), DVec3::X, 0.75);
    let back = Model::from_json(&m.to_json()).expect("reads");
    assert_eq!(back, m);
}
