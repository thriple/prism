//! Sweep and Patch (Freeform NURBS part 5).

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;

use prism_geom::Nurbs;
use prism_math::DVec3;
use prism_ops::{add_arc, add_circle, add_segment, add_spline, check, sweep};
use prism_topo::{EdgeId, FaceId, Model};

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn all(m: &Model) -> BTreeSet<EdgeId> {
    m.edges().map(|(id, _)| id).collect()
}

fn open(pts: &[[f64; 3]]) -> Nurbs {
    let pts: Vec<DVec3> = pts.iter().map(|p| DVec3::from_array(*p)).collect();
    Nurbs::open(&pts, 3).unwrap()
}

fn surface(m: &Model, f: FaceId) -> prism_geom::Patch {
    m.face(f).unwrap().surface.patch().unwrap().clone()
}

#[test]
fn an_open_profile_turns_with_a_curved_path() {
    let mut m = Model::new();
    // A bump drawn across the path's start, square to it.
    let profile = open(&[
        [-2.0, 0.0, 0.0],
        [-1.0, 0.0, 1.5],
        [1.0, 0.0, 1.5],
        [2.0, 0.0, 0.0],
    ]);
    // A path from the bump's middle, curving round a quarter turn.
    let path = open(&[
        [0.0, 0.0, 0.0],
        [0.0, 3.0, 0.0],
        [1.0, 5.0, 0.0],
        [4.0, 6.0, 0.0],
    ]);
    add_spline(&mut m, &profile);
    add_spline(&mut m, &path);
    let edges = all(&m);
    let faces = sweep::sweep(&mut m, &edges).unwrap();
    clean(&m);
    let p = surface(&m, faces[0]);
    let [(u0, u1), (v0, v1)] = p.surface.domain();
    // It starts as the profile itself.
    for i in 0..=8 {
        let u = u0 + (u1 - u0) * i as f64 / 8.0;
        let q = p.surface.eval(u, v0);
        let on = profile.eval(profile.closest(q, 0.0, 1.0));
        assert!(q.distance(on) < 1e-9, "{q}");
    }
    // At the far end it has turned with the path: square to it there.
    let (a, b) = (p.surface.eval(u0, v1), p.surface.eval(u1, v1));
    let end_dir = path.eval_d(1.0).1.normalize();
    let chord = (b - a).normalize();
    assert!(chord.dot(end_dir).abs() < 0.02, "{}", chord.dot(end_dir));
    assert!((a.distance(b) - 4.0).abs() < 0.02, "the same width");
    // The bump's top stays straight above the path all the way.
    for j in 0..=20 {
        let v = v0 + (v1 - v0) * j as f64 / 20.0;
        let q = p.surface.eval((u0 + u1) / 2.0, v);
        let on = path.eval(path.closest(q, 0.0, 1.0));
        assert!((q.distance(on) - 1.125).abs() < 0.01, "{}", q.distance(on));
        assert!((q.z - 1.125).abs() < 0.01, "{q}");
    }
    assert_eq!(
        m.edge_count(),
        5,
        "the two curves, the profile's copy at the end, and the two sides"
    );
}

#[test]
fn along_a_straight_path_an_open_profile_slides_exactly() {
    let mut m = Model::new();
    let profile = open(&[
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 2.0],
        [3.0, 0.0, -1.0],
        [4.0, 0.0, 0.0],
    ]);
    add_spline(&mut m, &profile);
    add_segment(&mut m, DVec3::new(4.0, 0.0, 0.0), DVec3::new(4.0, 5.0, 1.0));
    let edges = all(&m);
    let faces = sweep::sweep(&mut m, &edges).unwrap();
    clean(&m);
    assert_eq!(faces.len(), 1);
    assert_eq!(
        m.edge_count(),
        4,
        "the two, and their copies at the far ends"
    );
    let p = surface(&m, faces[0]);
    let [(u0, u1), (v0, v1)] = p.surface.domain();
    for i in 0..=6 {
        for j in 0..=6 {
            let u = u0 + (u1 - u0) * i as f64 / 6.0;
            let v = v0 + (v1 - v0) * j as f64 / 6.0;
            let q = p.surface.eval(u, v);
            // On the profile moved some way along the path.
            let back = q - DVec3::new(0.0, 5.0, 1.0) * ((q.y) / 5.0);
            let on = profile.eval(profile.closest(back, 0.0, 1.0));
            assert!(back.distance(on) < 1e-9, "{q}");
        }
    }
}

#[test]
fn a_circle_swept_along_a_curve_is_a_pipe_of_its_radius() {
    let mut m = Model::new();
    let path = open(&[
        [0.0, 0.0, 0.0],
        [0.0, 3.0, 0.0],
        [3.0, 5.0, 2.0],
        [6.0, 5.0, 2.0],
    ]);
    add_spline(&mut m, &path);
    add_circle(&mut m, DVec3::ZERO, DVec3::Y, 0.4);
    for f in m.faces().map(|(id, _)| id).collect::<Vec<_>>() {
        m.remove_face(f);
    }
    let edges = all(&m);
    let faces = sweep::sweep(&mut m, &edges).unwrap();
    clean(&m);
    assert_eq!(faces.len(), 2, "two halves round");
    // Every point of the pipe is its radius from the path, nearly (it is
    // fitted between the copies), and it faces away from the path.
    for f in &faces {
        let p = surface(&m, *f);
        for i in 0..=8 {
            for j in 0..=20 {
                let (u, v) = (p.u.0 + (p.u.1 - p.u.0) * i as f64 / 8.0, j as f64 / 20.0);
                let q = p.surface.eval(u, v);
                let on = path.eval(path.closest(q, 0.0, 1.0));
                assert!((q.distance(on) - 0.4).abs() < 0.01, "{}", q.distance(on));
                assert!(p.normal(u, v).dot(q - on) > 0.0);
            }
        }
    }
    // Two closed curves, or one curve, are refused.
    let mut m = Model::new();
    add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    add_circle(&mut m, DVec3::new(5.0, 0.0, 0.0), DVec3::Z, 1.0);
    let edges = all(&m);
    assert_eq!(sweep::sweep(&mut m, &edges).err(), Some(sweep::BOTH_CLOSED));
    let one: BTreeSet<EdgeId> = m.edges().take(1).map(|(id, _)| id).collect();
    assert_eq!(sweep::sweep(&mut m, &one).err(), Some(sweep::TWO_FOR_SWEEP));
}

#[test]
fn a_ring_of_four_curves_is_patched_exactly_along_all_four() {
    let mut m = Model::new();
    let sides = [
        open(&[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 1.5],
            [3.0, 0.0, 1.5],
            [4.0, 0.0, 0.0],
        ]),
        open(&[[4.0, 0.0, 0.0], [4.5, 1.0, 1.0], [4.0, 3.0, 0.0]]),
        open(&[[4.0, 3.0, 0.0], [2.0, 3.0, -1.0], [0.0, 3.0, 0.0]]),
    ];
    for c in &sides {
        add_spline(&mut m, c);
    }
    // The fourth side is a plain line, with a vertex part way along it.
    add_segment(&mut m, DVec3::new(0.0, 3.0, 0.0), DVec3::new(0.0, 1.0, 0.0));
    add_segment(&mut m, DVec3::new(0.0, 1.0, 0.0), DVec3::ZERO);
    let before = m.edge_count();
    let edges = all(&m);
    let faces = sweep::patch(&mut m, &edges).unwrap();
    clean(&m);
    assert_eq!(faces.len(), 2, "cut across at the line's middle vertex");
    assert_eq!(
        m.edge_count(),
        before + 2,
        "one cut across, and the right side split"
    );
    // Every curve lies on the surface.
    for c in &sides {
        for i in 0..=10 {
            let q = c.eval(i as f64 / 10.0);
            let near = faces
                .iter()
                .map(|f| surface(&m, *f).distance(q).abs())
                .fold(f64::INFINITY, f64::min);
            assert!(near < 1e-9, "{near}");
        }
    }
    // Three curves are refused; an arc as the fourth side is patched,
    // exactly along the arc.
    let mut m = Model::new();
    for c in &sides {
        add_spline(&mut m, c);
    }
    let edges = all(&m);
    assert_eq!(
        sweep::patch(&mut m, &edges).err(),
        Some(sweep::FOUR_FOR_PATCH)
    );
    add_arc(
        &mut m,
        DVec3::new(0.0, 1.5, 0.0),
        DVec3::X,
        1.5,
        DVec3::new(0.0, 3.0, 0.0),
        std::f64::consts::PI,
    );
    let edges = all(&m);
    let faces = sweep::patch(&mut m, &edges).expect("an arc is a side");
    clean(&m);
    for i in 0..=10 {
        let a = std::f64::consts::PI * f64::from(i) / 10.0;
        // The half circle from (0, 3, 0) about X through (0, 1.5, ±1.5).
        let q = DVec3::new(0.0, 1.5 + 1.5 * a.cos(), 1.5 * a.sin());
        let q2 = DVec3::new(0.0, 1.5 + 1.5 * a.cos(), -1.5 * a.sin());
        let near = faces
            .iter()
            .map(|f| {
                let s = surface(&m, *f);
                s.distance(q).abs().min(s.distance(q2).abs())
            })
            .fold(f64::INFINITY, f64::min);
        assert!(near < 1e-9, "{near}");
    }
}
