//! A face left inside a solid (owner's session 4, 2026-11-10): Push/Pull's
//! new loop keeps the face it started from, so the seam round it has three
//! faces. The seam lies flat in the walls, so it is drawn with the two wall
//! faces, never as an outline from some views only.

#![cfg(not(target_arch = "wasm32"))]

use prism_math::DVec3;
use prism_topo::Model;
use prism_view::scene::Scene;

#[test]
fn a_seam_round_a_face_left_inside_is_drawn_flat() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    let f = prism_ops::face_at(&m, DVec3::new(1.0, 1.0, 0.0)).expect("base");
    let up = m.face(f).unwrap().plane().unwrap().normal.z > 0.0;
    prism_ops::push_pull(&mut m, f, if up { 2.0 } else { -2.0 }, false);
    let side = prism_ops::face_at(&m, DVec3::new(2.0, 1.0, 1.0)).expect("side");
    prism_ops::push_pull(&mut m, side, 1.0, true);
    // As in the session: a wall erased and drawn back (by tracing one of
    // its edges), so it comes after the inside face in the model.
    let wall = prism_ops::face_at(&m, DVec3::new(1.0, 2.0, 1.0)).expect("wall");
    m.remove_face(wall);
    prism_ops::add_segment(&mut m, DVec3::new(0.0, 2.0, 0.0), DVec3::new(0.0, 2.0, 2.0));
    assert!(
        prism_ops::face_at(&m, DVec3::new(1.0, 2.0, 1.0)).is_some(),
        "drawn back"
    );
    let ef = m.edge_faces();
    let scene = Scene::build(&m);
    let mut seams = 0;
    for e in &scene.edges {
        if ef.get(&e.id).map_or(0, Vec::len) < 3 {
            continue;
        }
        seams += 1;
        let [Some(a), Some(b)] = e.normals else {
            panic!("two faces")
        };
        assert!(a.dot(b) > 0.999, "{:?}: {a:?} {b:?}", e.id);
    }
    assert_eq!(seams, 4, "the face's four edges");
}
