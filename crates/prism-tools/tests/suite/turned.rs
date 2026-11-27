//! Cones, spheres and tori in the core (Exact Curves 5b): built by hand,
//! they keep the model check clean, hold their exact volume, move whole and
//! exactly.

use std::f64::consts::PI;

use prism_math::{DAffine3, DQuat, DVec3};
use prism_ops::check;
use prism_ops::health::face_volume;
use prism_tools::scenes::{cone, sphere, torus};
use prism_topo::Model;

fn volume(m: &Model) -> f64 {
    m.faces().map(|(id, _)| face_volume(m, id)).sum()
}

fn clean(m: &Model) {
    let p = check::problems(m);
    assert!(p.is_empty(), "{p:?}");
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

#[test]
fn turned_solids_are_clean_and_hold_their_exact_volume() {
    for k in [1e-3, 1.0, 1e3] {
        let s = sphere(DVec3::new(1.0, 2.0, 3.0) * k, 2.0 * k);
        clean(&s);
        let v = 4.0 / 3.0 * PI * (2.0 * k).powi(3);
        assert!(close(volume(&s), v), "sphere {} vs {v}", volume(&s));

        let c = cone(1.5 * k, 4.0 * k);
        clean(&c);
        let v = PI * (1.5 * k).powi(2) * 4.0 * k / 3.0;
        assert!(close(volume(&c), v), "cone {} vs {v}", volume(&c));

        let t = torus(3.0 * k, 1.0 * k);
        clean(&t);
        let v = 2.0 * PI * PI * 3.0 * k * (1.0 * k).powi(2);
        assert!(close(volume(&t), v), "torus {} vs {v}", volume(&t));
    }
}

#[test]
fn moves_turns_mirrors_and_even_scales_keep_them_exact() {
    let m = DAffine3::from_scale_rotation_translation(
        DVec3::splat(2.5),
        DQuat::from_axis_angle(DVec3::new(1.0, 2.0, 0.5).normalize(), 0.9),
        DVec3::new(4.0, -1.0, 7.0),
    );
    let mirror = DAffine3::from_scale(DVec3::new(-1.0, 1.0, 1.0));
    for model in [sphere(DVec3::ZERO, 1.0), cone(1.0, 2.0), torus(2.0, 0.5)] {
        let before = volume(&model);
        for (t, k) in [(m, 2.5f64.powi(3)), (mirror, 1.0)] {
            let mut x = model.clone();
            prism_ops::transform::affine(&mut x, &t);
            clean(&x);
            assert!(
                close(volume(&x), before * k),
                "{} vs {}",
                volume(&x),
                before * k
            );
        }
    }
}

/// Every drawn triangle lies with its corners on the surface, faces out,
/// and has the surface's normals; together they cover the surface's area.
fn shaded(m: &Model, on: impl Fn(DVec3) -> f64, out: impl Fn(DVec3) -> DVec3, area: f64) {
    let s = prism_view::Scene::build(m);
    let (mut total, mut edge_on) = (0.0, 0.0);
    for f in s.faces.iter().filter(|f| f.curved.is_some()) {
        for t in s.mesh.indices[f.tris.start as usize..f.tris.end as usize].chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|i| s.mesh.positions[t[i] as usize]);
            let n = (b - a).cross(c - a);
            total += n.length() / 2.0;
            let facing = n.normalize_or_zero().dot(out((a + b + c) / 3.0));
            if facing.abs() < 0.05 {
                // A sliver near a pole, edge-on to the surface.
                edge_on += n.length() / 2.0;
            } else {
                assert!(facing > 0.0, "front faces out: {a} {b} {c}");
            }
            for i in t {
                let (p, nn) = (s.mesh.positions[*i as usize], s.mesh.normals[*i as usize]);
                assert!(on(p).abs() < 1e-9, "on the surface: {}", on(p));
                // At a cone's apex each way down has its own normal.
                let want = out(p);
                assert!(
                    want.is_nan() || nn.distance(want) < 1e-6,
                    "the surface's normal at {p}"
                );
            }
        }
    }
    assert!((total - area).abs() < 0.01 * area, "area {total} vs {area}");
    assert!(edge_on < 1e-4 * area, "slivers: {edge_on}");
}

#[test]
fn turned_faces_are_shaded_from_a_grid_on_their_surface() {
    let c0 = DVec3::new(1.0, 2.0, 3.0);
    shaded(
        &sphere(c0, 2.0),
        |p| p.distance(c0) - 2.0,
        |p| (p - c0).normalize(),
        4.0 * PI * 4.0,
    );
    let (r, h): (f64, f64) = (1.5, 4.0);
    let slant = r.hypot(h);
    shaded(
        &cone(r, h),
        |p| {
            if p.z.abs() < 1e-12 && p.truncate().length() < r - 1e-9 {
                0.0 // the base disc
            } else {
                p.truncate().length() - r * (h - p.z) / h
            }
        },
        |p| match p.truncate().extend(0.0).try_normalize() {
            Some(e) => (e * h + DVec3::Z * r).normalize(),
            None => DVec3::NAN,
        },
        PI * r * slant,
    );
    let (big, small) = (3.0, 1.0);
    let core = |p: DVec3| p.truncate().normalize().extend(0.0) * big;
    shaded(
        &torus(big, small),
        |p| p.distance(core(p)) - small,
        |p| (p - core(p)).normalize(),
        4.0 * PI * PI * big * small,
    );
}

#[test]
fn picking_puts_the_point_on_the_exact_surface() {
    let c0 = DVec3::new(1.0, 2.0, 3.0);
    let s = prism_view::Scene::build(&sphere(c0, 2.0));
    let o = c0 + DVec3::new(10.0, 3.0, 4.0);
    let d = (c0 + DVec3::new(0.3, 0.2, -0.1) - o).normalize();
    let (_, t) = s.ray_face(o, d).expect("hits the sphere");
    assert!(((o + d * t).distance(c0) - 2.0).abs() < 1e-12);
    let s = prism_view::Scene::build(&torus(3.0, 1.0));
    let (_, t) = s
        .ray_face(DVec3::new(3.0, 0.4, 10.0), -DVec3::Z)
        .expect("hits the torus");
    let p = DVec3::new(3.0, 0.4, 10.0 - t);
    let core = p.truncate().normalize().extend(0.0) * 3.0;
    assert!((p.distance(core) - 1.0).abs() < 1e-12, "on the tube: {p}");
    assert!(p.z > 0.0, "the top, the nearer side");
    // Through the hole: nothing.
    assert!(
        s.ray_face(DVec3::new(0.0, 0.0, 10.0), -DVec3::Z).is_none(),
        "the hole is empty"
    );
}

#[test]
fn model_health_finds_a_point_off_its_surface_and_puts_it_back() {
    for mut m in [sphere(DVec3::ZERO, 1.0), cone(1.0, 2.0), torus(2.0, 0.5)] {
        let v = m.vertices().map(|(id, _)| id).next().expect("a point");
        let p = m.vertex(v);
        m.set_vertex(v, p + DVec3::new(0.01, -0.02, 0.015));
        let r = prism_ops::health::check(&m);
        assert!(!r.off_circle.faces.is_empty(), "found off its surface");
        prism_ops::health::repair_arcs(&mut m);
        clean(&m);
        for (_, f) in m.faces() {
            for v in f.loops().flatten() {
                assert!(f.surface.distance(m.vertex(*v)).abs() < 1e-9);
            }
        }
    }
}

/// The general path (faces that are not rectangles in (θ, s)) gives the
/// same surface: every point on it, facing out, the same area.
#[test]
fn the_general_grid_matches_the_fast_one() {
    for m in [sphere(DVec3::ZERO, 1.0), cone(1.0, 2.0), torus(2.0, 0.5)] {
        for (_, f) in m.faces().filter(|(_, f)| f.surface.is_curved()) {
            let area = |t: &prism_view::tess::Turned| -> f64 {
                t.tris
                    .iter()
                    .map(|x| {
                        let [a, b, c] = x.map(|i| t.points[i]);
                        (b - a).cross(c - a).length() / 2.0
                    })
                    .sum()
            };
            let fast = prism_view::tess::turned_with(&m, f, true).unwrap();
            let slow = prism_view::tess::turned_with(&m, f, false).unwrap();
            assert!((area(&fast) - area(&slow)).abs() < 1e-9 * area(&fast));
            for (p, n) in slow.points.iter().zip(&slow.normals) {
                assert!(f.surface.distance(*p).abs() < 1e-9);
                // At a cone's apex the normal depends on the way in.
                let rev = f.surface.revolution().unwrap();
                if rev.axis_distance(*p) > 1e-9 {
                    assert!(n.dot(f.surface.normal_at(*p)) > 0.99, "{p}");
                }
            }
        }
    }
}

/// The path circle about the z axis at height 3, and its edges.
fn lathe(m: &mut Model, r: f64) -> std::collections::BTreeSet<prism_topo::EdgeId> {
    let before: std::collections::BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    prism_ops::add_circle(m, DVec3::new(0.0, 0.0, 3.0), DVec3::Z, r);
    m.edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect()
}

#[test]
fn follow_me_spheres_and_tori_shade_like_built_ones() {
    // A half disc turned about its diameter (Exact Curves 5d).
    let mut m = Model::new();
    prism_ops::add_segment(
        &mut m,
        DVec3::new(0.0, 0.0, -1.0),
        DVec3::new(0.0, 0.0, 1.0),
    );
    prism_ops::add_arc(
        &mut m,
        DVec3::ZERO,
        -DVec3::Y,
        1.0,
        DVec3::new(0.0, 0.0, -1.0),
        PI,
    );
    let prof = m.faces().next().map(|(id, _)| id).unwrap();
    let p = lathe(&mut m, 0.5);
    assert!(prism_ops::follow::follow(&mut m, &p, prof));
    shaded(&m, |p| p.length() - 1.0, |p| p.normalize(), 4.0 * PI);
    // A disc turned round a circle.
    let mut t = Model::new();
    prism_ops::add_circle(&mut t, DVec3::new(2.0, 0.0, 0.0), DVec3::Y, 0.5);
    let prof = t.faces().next().map(|(id, _)| id).unwrap();
    let p = lathe(&mut t, 1.0);
    assert!(prism_ops::follow::follow(&mut t, &p, prof));
    let core = |p: DVec3| p.truncate().normalize().extend(0.0) * 2.0;
    shaded(
        &t,
        |p| p.distance(core(p)) - 0.5,
        |p| (p - core(p)).normalize(),
        4.0 * PI * PI * 2.0 * 0.5,
    );
}
