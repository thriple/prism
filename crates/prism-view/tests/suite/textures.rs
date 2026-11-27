//! Texture coordinates and see-through faces in the scene (Basic Materials
//! 2b): every corner gets its place on a texture, curved faces unrolled
//! without a jump at the seam; see-through faces hide nothing from picking.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use prism_math::{DVec2, DVec3};
use prism_topo::{MaterialId, Model};
use prism_view::Scene;
use prism_view::style::Rgba;

fn cube() -> Model {
    let mut m = Model::new();
    prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
    let f = m.faces().next().map(|(id, _)| id).expect("a face");
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    prism_ops::push_pull(&mut m, f, 2.0 * up, false);
    m
}

fn cylinder() -> Model {
    let mut m = Model::new();
    prism_ops::add_circle(&mut m, DVec3::ZERO, DVec3::Z, 1.0);
    let f = m.faces().next().map(|(id, _)| id).expect("a disc");
    let up = m.face(f).unwrap().plane().unwrap().normal.z;
    prism_ops::push_pull(&mut m, f, 2.0 * up, false);
    m
}

/// A ball of radius 1 at the origin, by Follow Me of a half disc.
fn ball() -> Model {
    let mut m = Model::new();
    prism_ops::add_segment(&mut m, -DVec3::Z, DVec3::Z);
    prism_ops::add_arc(&mut m, DVec3::ZERO, -DVec3::Y, 1.0, -DVec3::Z, PI);
    let prof = prism_ops::face_at(&m, DVec3::new(0.5, 0.0, 0.0)).expect("half disc");
    let before: BTreeSet<_> = m.edges().map(|(id, _)| id).collect();
    prism_ops::add_circle(&mut m, DVec3::new(0.0, 0.0, 10.0), DVec3::Z, 0.5);
    let path: BTreeSet<_> = m
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !before.contains(id))
        .collect();
    assert!(prism_ops::follow::follow(&mut m, &path, prof));
    m
}

/// Every triangle of every curved face: its sides on the texture are as
/// long as in space (within a share: a chord is a little shorter than its
/// arc), so nothing
/// jumps across the seam.
fn unrolled_like_space(s: &Scene, stretch: f64) {
    for f in s.faces.iter().filter(|f| f.curved.is_some()) {
        for t in s.mesh.indices[f.tris.start as usize..f.tris.end as usize].chunks(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (a, b) = (a as usize, b as usize);
                let space = s.mesh.positions[a].distance(s.mesh.positions[b]);
                let tex = s.mesh.uvs[a].distance(s.mesh.uvs[b]);
                assert!(
                    tex <= space * stretch + 1e-9 && tex >= space * 0.98 - 1e-9,
                    "{space} in space, {tex} on the texture"
                );
            }
        }
    }
}

#[test]
fn flat_faces_use_their_placement() {
    let m = cube();
    let s = Scene::build(&m);
    assert_eq!(s.mesh.uvs.len(), s.mesh.positions.len());
    for (i, p) in s.mesh.positions.iter().enumerate() {
        let f = &s.faces[s.mesh.faces[i] as usize];
        let map = m.face(f.id).unwrap().texture_map().unwrap();
        assert!(s.mesh.uvs[i].distance(map.at(*p)) < 1e-12);
    }
}

#[test]
fn a_cylinder_unrolls_in_its_own_frame_without_a_seam() {
    let m = cylinder();
    let s = Scene::build(&m);
    assert_eq!(s.mesh.uvs.len(), s.mesh.positions.len());
    unrolled_like_space(&s, 1.01);
    // The same place as the surface gives, give or take a whole turn.
    for f in s.faces.iter().filter(|f| f.curved.is_some()) {
        let rev = m.face(f.id).unwrap().surface.revolution().unwrap();
        for &i in &s.mesh.indices[f.tris.start as usize..f.tris.end as usize] {
            let (p, uv) = (s.mesh.positions[i as usize], s.mesh.uvs[i as usize]);
            let (theta, h) = rev.param(p);
            let exact = rev.texture_at(theta, h);
            let turns = (uv.x - exact.x) / (2.0 * PI);
            assert!((turns - turns.round()).abs() < 1e-9, "{uv} vs {exact}");
            assert!((uv.y - exact.y).abs() < 1e-9);
        }
    }
}

#[test]
fn a_ball_unrolls_without_a_seam() {
    let s = Scene::build(&ball());
    assert_eq!(s.mesh.uvs.len(), s.mesh.positions.len());
    assert!(s.mesh.uvs.iter().all(|q| q.is_finite()));
    // Round the ball, (θR, sR): stretched toward the poles, but never by
    // a jump across the seam.
    for f in s.faces.iter().filter(|f| f.curved.is_some()) {
        for t in s.mesh.indices[f.tris.start as usize..f.tris.end as usize].chunks(3) {
            let q: Vec<DVec2> = t.iter().map(|i| s.mesh.uvs[*i as usize]).collect();
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                assert!(q[a].distance(q[b]) < 0.5, "a jump: {} to {}", q[a], q[b]);
            }
        }
    }
}

#[test]
fn see_through_faces_hide_nothing_from_picking() {
    let mut m = cube();
    let front = prism_ops::face_at(&m, DVec3::new(1.0, 0.0, 1.0)).expect("front");
    m.edit_face(front, |f| f.paint.front = Some(MaterialId(0)));
    let mut s = Scene::build(&m);
    let eye = DVec3::new(1.0, -5.0, 1.0);
    let behind = DVec3::new(1.0, 2.0, 1.0);
    s.palette.insert(MaterialId(0), Rgba::rgb(40, 190, 60));
    assert!(s.occluded(eye, behind), "a solid face hides");
    s.palette.insert(MaterialId(0), Rgba([40, 190, 60, 128]));
    assert!(!s.occluded(eye, behind), "a see-through one does not");
    // Clicking still picks it: it is the nearest face.
    let (id, _) = s.ray_face(eye, DVec3::Y).expect("a hit");
    assert_eq!(id, front);
    // Where materials are not drawn, it is solid again.
    s.materials_shown = false;
    assert!(s.occluded(eye, behind));
    // A texture with see-through pixels, only while textures are drawn.
    s.materials_shown = true;
    s.palette.insert(MaterialId(0), Rgba::rgb(40, 190, 60));
    s.textures.insert(
        MaterialId(0),
        prism_view::SceneTexture {
            key: "leaves".into(),
            size: [1.0, 1.0],
            width: 1,
            height: 1,
            rgba: std::sync::Arc::new(vec![0, 120, 0, 0]),
            average: Rgba([0, 120, 0, 0]),
            clear: true,
        },
    );
    assert!(!s.occluded(eye, behind));
    s.textures_shown = false;
    assert!(s.occluded(eye, behind));
}
