//! Two mirrored halves of a component, each open where they meet (owner,
//! 2026-11-25: the centre line between them jittered while orbiting low,
//! and looked heavier than the lines round it). Each half's centre edge
//! has one face, so alone it was an outline, and the other half's face
//! was a stranger to it in the line test. Joined, each copy is an edge
//! between both halves' faces.

use prism_math::{DAffine3, DMat3, DVec3};
use prism_topo::Model;
use prism_view::Scene;

/// A slab 4.9 x 14.46 x 0.32, open at x = 0.
fn half() -> Model {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 4.9, 14.46, 0.0);
    let f = prism_ops::sticky::face_at(&m, DVec3::new(1.0, 1.0, 0.0)).unwrap();
    let up = m.face(f).unwrap().plane().unwrap().normal.z.signum();
    prism_ops::pushpull::push_pull(&mut m, f, 0.32 * up, false);
    let centre = m
        .faces()
        .find(|(_, f)| {
            f.plane()
                .is_some_and(|p| p.normal.x.abs() > 0.9 && p.origin.x.abs() < 1e-9)
        })
        .map(|(id, _)| id)
        .unwrap();
    m.remove_face(centre);
    m
}

fn at_centre_top(s: &Scene) -> Vec<&prism_view::scene::SceneEdge> {
    s.edges
        .iter()
        .filter(|e| e.a.x.abs() < 1e-9 && e.b.x.abs() < 1e-9 && (e.a.z - 0.32).abs() < 1e-9)
        .collect()
}

#[test]
fn where_two_mirrored_halves_meet_the_edge_is_between_both_faces() {
    let base = Scene::build(&half());
    let mirror = DAffine3::from_mat3(DMat3::from_diagonal(DVec3::new(-1.0, 1.0, 1.0)));
    let mut s = base.clone();
    s.merge(base.placed(&mirror).expect("a mirror"), 1, false);
    let before: Vec<_> = at_centre_top(&s).iter().map(|e| e.faces).collect();
    assert_eq!(before.len(), 2);
    assert!(before.iter().all(|f| f[1].is_none()), "each has one face");
    s.join_open_edges();
    let centre = at_centre_top(&s);
    for e in &centre {
        let (Some(a), Some(b)) = (e.faces[0], e.faces[1]) else {
            panic!("one face only: {:?}", e.faces);
        };
        assert_ne!(a, b);
        // Both floors face up: no outline between them.
        assert!(e.normals[0].unwrap().dot(e.normals[1].unwrap()) > 0.999);
    }
    assert_ne!(centre[0].faces[0], centre[1].faces[0]);
    // Every open edge of each half is at the centre (its top, bottom and
    // ends): all meet their mirror image.
    let open = s.edges.iter().filter(|e| e.faces[1].is_none()).count();
    assert_eq!(open, 0);
}

#[test]
fn a_lone_open_edge_stays_an_outline() {
    let mut s = Scene::build(&half());
    s.join_open_edges();
    assert!(at_centre_top(&s).iter().all(|e| e.faces[1].is_none()));
}
