//! Profile lines and reversed faces (owner, 2026-11-25): a face reversed
//! against its neighbour is drawn purple but is not a fold, so the edges
//! between it and faces in its own plane are not profiles, and its corner
//! with a face square to it profiles just as if it were the right way out.

use prism_math::DVec3;
use prism_topo::Model;
use prism_view::Scene;

fn edge_normals(s: &Scene, a: DVec3, b: DVec3) -> (DVec3, DVec3) {
    let e = s
        .edges
        .iter()
        .find(|e| {
            (e.a.distance(a) < 1e-9 && e.b.distance(b) < 1e-9)
                || (e.a.distance(b) < 1e-9 && e.b.distance(a) < 1e-9)
        })
        .expect("the edge");
    (e.normals[0].unwrap(), e.normals[1].unwrap())
}

#[test]
fn an_edge_between_a_face_and_a_reversed_one_in_its_plane_is_no_profile() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 1.0, 0.0);
    prism_ops::add_segment(&mut m, DVec3::new(1.0, 0.0, 0.0), DVec3::new(1.0, 1.0, 0.0));
    assert_eq!(m.face_count(), 2);
    let right = prism_ops::sticky::face_at(&m, DVec3::new(1.5, 0.5, 0.0)).unwrap();
    prism_ops::health::reverse(&mut m, &[right].into());
    let s = Scene::build(&m);
    let (n1, n2) = edge_normals(&s, DVec3::new(1.0, 0.0, 0.0), DVec3::new(1.0, 1.0, 0.0));
    assert!(n1.dot(n2) > 0.999, "seen as one flat surface: {n1} {n2}");
}

#[test]
fn a_reversed_side_of_a_box_still_profiles_at_its_corners() {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 1.0, 1.0, 0.0);
    let f = m.faces().next().map(|(id, _)| id).unwrap();
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    prism_ops::push_pull(&mut m, f, up, false);
    let side = prism_ops::sticky::face_at(&m, DVec3::new(0.5, 0.0, 0.5)).unwrap();
    prism_ops::health::reverse(&mut m, &[side].into());
    let s = Scene::build(&m);
    // The top front edge: top faces up, the (reversed) front is read as
    // facing out again, toward -y.
    let (n1, n2) = edge_normals(&s, DVec3::new(0.0, 0.0, 1.0), DVec3::new(1.0, 0.0, 1.0));
    let (up_n, front) = if n1.z.abs() > 0.5 { (n1, n2) } else { (n2, n1) };
    assert!(up_n.z > 0.999 && front.y < -0.999, "{n1} {n2}");
}
