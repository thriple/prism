//! A scene placed by a transform matches the scene built from the
//! transformed model (components drawn without rebuilding each instance).

use prism_math::{DAffine3, DQuat, DVec3};
use prism_topo::Model;
use prism_view::Scene;

/// An L-shaped block, so a mirror shows.
fn block() -> Model {
    let mut m = Model::new();
    let pts = [
        (0.0, 0.0),
        (3.0, 0.0),
        (3.0, 1.0),
        (1.0, 1.0),
        (1.0, 2.0),
        (0.0, 2.0),
    ];
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        prism_ops::add_segment(&mut m, DVec3::new(a.0, a.1, 0.0), DVec3::new(b.0, b.1, 0.0));
    }
    let f = prism_ops::face_at(&m, DVec3::new(0.5, 0.5, 0.0)).expect("base");
    let up = m.face(f).unwrap().plane().unwrap().normal.z > 0.0;
    prism_ops::push_pull(&mut m, f, if up { 1.5 } else { -1.5 }, false);
    m
}

#[test]
fn placing_matches_building_from_the_moved_model() {
    let m = block();
    let local = Scene::build(&m);
    let turn = DAffine3::from_translation(DVec3::new(5.0, -2.0, 1.0))
        * DAffine3::from_quat(DQuat::from_axis_angle(
            DVec3::new(0.2, 0.5, 1.0).normalize(),
            0.8,
        ));
    for t in [
        turn,
        turn * DAffine3::from_scale(DVec3::new(-1.0, 1.0, 1.0)),
        turn * DAffine3::from_scale(DVec3::splat(2.5)),
    ] {
        let placed = local.placed(&t).expect("an even transform");
        let mut moved = m.clone();
        prism_ops::transform::affine(&mut moved, &t);
        let built = Scene::build(&moved);
        let (a, b) = (placed.bounds.unwrap(), built.bounds.unwrap());
        assert!((a.0 - b.0).length() < 1e-9 && (a.1 - b.1).length() < 1e-9);
        // The same face is hit along many rays, at the same distance.
        let centre = (b.0 + b.1) * 0.5;
        for i in 0..40 {
            let k = f64::from(i);
            let o = centre + DVec3::new((k * 1.7).sin(), (k * 0.9).cos(), (k * 0.3).sin()) * 20.0;
            let d =
                (centre + DVec3::new((k * 0.4).cos(), (k * 1.3).sin(), 0.2) * 0.8 - o).normalize();
            let (p, q) = (placed.ray_face(o, d), built.ray_face(o, d));
            assert_eq!(p.map(|x| x.0), q.map(|x| x.0), "ray {i}");
            if let (Some(p), Some(q)) = (p, q) {
                assert!((p.1 - q.1).abs() < 1e-9);
            }
        }
        // Every triangle still faces the way its face does.
        for f in &placed.faces {
            for t in placed.mesh.indices[f.tris.start as usize..f.tris.end as usize].chunks(3) {
                let [a, b, c] = [0, 1, 2].map(|j| placed.mesh.positions[t[j] as usize]);
                assert!(
                    (b - a).cross(c - a).dot(f.normal) > 0.0,
                    "a triangle faces in"
                );
            }
        }
    }
    let stretch = DAffine3::from_scale(DVec3::new(1.0, 2.0, 1.0));
    assert!(
        local.placed(&stretch).is_none(),
        "a stretch is rebuilt instead"
    );
}
