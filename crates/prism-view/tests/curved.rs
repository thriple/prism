//! Curved faces in the scene (Exact Curves 4b): shaded round and picked
//! on the exact surface.

use std::collections::BTreeSet;

use prism_math::{DAffine3, DQuat, DVec3};
use prism_topo::Model;
use prism_view::Scene;

/// A disc of radius 1 on the ground, pulled up to a cylinder 2 tall.
fn cylinder() -> Model {
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let f = m.faces().next().map(|(id, _)| id).expect("a disc");
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    prism_ops::push_pull(&mut m, f, 2.0 * up, false);
    m
}

#[test]
fn a_cylinder_is_shaded_round_with_triangles_on_its_surface() {
    let m = cylinder();
    let s = Scene::build(&m);
    let curved: Vec<_> = s.faces.iter().filter(|f| f.curved.is_some()).collect();
    assert_eq!(curved.len(), 2);
    let mut area = 0.0;
    for f in &curved {
        for t in s.mesh.indices[f.tris.start as usize..f.tris.end as usize].chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|i| s.mesh.positions[t[i] as usize]);
            let n = (b - a).cross(c - a);
            assert!(
                n.dot((a + b + c).truncate().extend(0.0)) > 0.0,
                "front faces out"
            );
            area += n.length() / 2.0;
            for i in t {
                let (p, n) = (s.mesh.positions[*i as usize], s.mesh.normals[*i as usize]);
                assert!(
                    (p.truncate().length() - 1.0).abs() < 1e-12,
                    "on the surface"
                );
                // The surface's own normal at every point: it shades round.
                assert!(n.distance(p.truncate().extend(0.0)) < 1e-12, "{n}");
            }
        }
    }
    let exact = std::f64::consts::TAU * 2.0;
    assert!((area - exact).abs() < exact * 1e-3, "{area}");
}

#[test]
fn rays_pick_a_cylinder_where_they_meet_it() {
    let m = cylinder();
    let s = Scene::build(&m);
    // From the side, at mid height: the near side, one unit in.
    let (id, t) = s
        .ray_face(DVec3::new(-5.0, 0.2, 1.0), DVec3::X)
        .expect("a hit");
    assert!(s.face(id).unwrap().curved.is_some());
    let x = (1.0f64 - 0.04).sqrt();
    assert!((t - (5.0 - x)).abs() < 1e-12, "{t}");
    // Above the top: misses the side, hits the cap.
    let (cap, t) = s
        .ray_face(DVec3::new(0.3, 0.0, 5.0), -DVec3::Z)
        .expect("the cap");
    assert!(s.face(cap).unwrap().curved.is_none());
    assert!((t - 3.0).abs() < 1e-12);
    // Past its height: nothing.
    assert!(s.ray_face(DVec3::new(-5.0, 0.0, 2.5), DVec3::X).is_none());
    // Every way round: each half picks its own side.
    let mut halves = BTreeSet::new();
    for k in 0..16 {
        let a = f64::from(k) * std::f64::consts::TAU / 16.0 + 0.1;
        let dir = -DVec3::new(a.cos(), a.sin(), 0.0);
        let (id, t) = s.ray_face(-dir * 5.0 + DVec3::Z, dir).expect("a hit");
        assert!((t - 4.0).abs() < 1e-12, "at {a}: {t}");
        halves.insert(id);
    }
    assert_eq!(halves.len(), 2);
}

#[test]
fn a_placed_cylinder_picks_like_one_built_there() {
    let m = cylinder();
    let place = DAffine3::from_scale_rotation_translation(
        DVec3::splat(2.0),
        DQuat::from_rotation_x(0.4),
        DVec3::new(3.0, 1.0, 0.5),
    );
    let mirror = DAffine3::from_scale(DVec3::new(-1.0, 1.0, 1.0));
    for t in [place, place * mirror] {
        let placed = Scene::build(&m).placed(&t).expect("an even scale");
        let mut moved = m.clone();
        prism_ops::transform::affine(&mut moved, &t);
        let built = Scene::build(&moved);
        let mut hits = 0;
        for k in 0..12 {
            let a = f64::from(k) * 0.5;
            let o = t.transform_point3(DVec3::new(5.0 * a.cos(), 5.0 * a.sin(), 1.3));
            let d = t.transform_point3(DVec3::new(0.0, 0.0, 0.9)) - o;
            let (p, b) = (placed.ray_face(o, d), built.ray_face(o, d));
            assert_eq!(p.is_some(), b.is_some(), "ray {k}");
            if let (Some(p), Some(b)) = (p, b) {
                assert!((p.1 - b.1).abs() < 1e-9, "ray {k}: {p:?} {b:?}");
                assert!(built.face(b.0).unwrap().curved.is_some(), "the side");
                hits += 1;
            }
        }
        assert_eq!(hits, 12);
    }
}
