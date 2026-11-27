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

// ---------- Soften and smooth (Exact Curves 4c) ----------

#[test]
fn soft_edges_are_ghosts_and_drop_out_unless_shown() {
    let m = cylinder();
    let mut s = Scene::build(&m);
    let soft: Vec<_> = s.edges.iter().filter(|e| e.soft).collect();
    assert_eq!(soft.len(), 2, "the seams");
    assert!(soft.iter().all(|e| e.ghost), "drawn as hidden lines");
    let n = s.edges.len();
    s.drop_soft();
    assert_eq!(s.edges.len(), n - 2);
    for (i, e) in s.edges.iter().enumerate() {
        assert_eq!(s.edge(e.id).map(|x| x.id), Some(e.id), "index {i} rebuilt");
    }
}

#[test]
fn shading_runs_across_a_smooth_edge() {
    // Two squares folded along x = 1, smoothed there.
    let mut m = Model::new();
    let pts = |q: [(f64, f64, f64); 4]| q.map(|(x, y, z)| DVec3::new(x, y, z));
    for loop_ in [
        pts([
            (0.0, 0.0, 0.0),
            (1.0, 0.0, 0.0),
            (1.0, 1.0, 0.0),
            (0.0, 1.0, 0.0),
        ]),
        pts([
            (1.0, 0.0, 0.0),
            (2.0, 0.0, 1.0),
            (2.0, 1.0, 1.0),
            (1.0, 1.0, 0.0),
        ]),
    ] {
        for i in 0..4 {
            prism_ops::add_segment(&mut m, loop_[i], loop_[(i + 1) % 4]);
        }
    }
    assert_eq!(m.face_count(), 2);
    let fold = m
        .edges()
        .find(|(_, e)| m.vertex(e.a).x == 1.0 && m.vertex(e.b).x == 1.0)
        .map(|(id, _)| id)
        .expect("the fold");
    let sharp = Scene::build(&m);
    m.set_soft(fold, true, true);
    let smooth = Scene::build(&m);
    let at_fold = |s: &Scene| -> Vec<DVec3> {
        s.mesh
            .positions
            .iter()
            .zip(&s.mesh.normals)
            .filter(|(p, _)| p.x == 1.0)
            .map(|(_, n)| *n)
            .collect()
    };
    let (a, b) = (at_fold(&sharp), at_fold(&smooth));
    assert_eq!(a.len(), 4, "two points of each face");
    assert!(a.iter().any(|n| n.distance(a[0]) > 0.1), "a crease");
    for n in &b {
        assert!(n.distance(b[0]) < 1e-12, "one normal along the fold");
        assert!((n.length() - 1.0).abs() < 1e-12);
    }
    // Away from the fold, the faces keep their own normals.
    let far = smooth
        .mesh
        .positions
        .iter()
        .zip(&smooth.mesh.normals)
        .find(|(p, _)| p.x == 0.0)
        .map(|(_, n)| *n)
        .expect("a far corner");
    assert!(far.cross(DVec3::Z).length() < 1e-12);
}

/// A 2 m cube with `edges` of it rounded by `r` through the kernel (Fillet
/// Joins step B), and the total area of its drawn triangles.
fn rounded(top_only: bool, r: f64) -> f64 {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    let f = m.faces().next().map(|(id, _)| id).unwrap();
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    prism_ops::push_pull(&mut m, f, 2.0 * up, false);
    let edges: BTreeSet<_> = if top_only {
        let top = prism_ops::face_at(&m, DVec3::new(1.0, 1.0, 2.0)).unwrap();
        m.face(top).unwrap().edge_ids().collect()
    } else {
        m.edges().map(|(id, _)| id).collect()
    };
    prism_ops::rounds::round_edges(&mut m, &edges, r, true).unwrap();
    let s = Scene::build(&m);
    s.mesh
        .indices
        .chunks(3)
        .map(|t| {
            let [a, b, c] = [0, 1, 2].map(|i| s.mesh.positions[t[i] as usize]);
            (b - a).cross(c - a).length() / 2.0
        })
        .sum()
}

#[test]
fn rounded_edges_are_drawn_with_the_right_area() {
    use std::f64::consts::PI;
    let (l, r) = (2.0, 0.3);
    let k = l - 2.0 * r;
    // The four top edges, mitred at the corners: each round is a quarter
    // cylinder whose length runs from l at its foot to l - 2r at its top,
    // cut by ellipses: r · ((l - 2r)·π/2 + 2r).
    let mitred = k * k + 4.0 * l * (l - r) + l * l + 4.0 * r * (k * PI / 2.0 + 2.0 * r);
    let drawn = rounded(true, r);
    assert!((drawn - mitred).abs() < mitred * 1e-3, "{drawn} {mitred}");
    // All twelve: flat faces, quarter cylinders and eighth spheres.
    let whole = 6.0 * k * k + 12.0 * (PI * r / 2.0) * k + 4.0 * PI * r * r;
    let drawn = rounded(false, r);
    assert!((drawn - whole).abs() < whole * 1e-3, "{drawn} {whole}");
}
