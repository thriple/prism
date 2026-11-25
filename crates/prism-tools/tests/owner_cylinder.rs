//! The owner's file `owner-cylinder-1.prism` (2026-11-25): a cylinder in
//! a component with a box pushed through it (new loop) and Intersect
//! Faces with the selection. Its wall, a cylinder face with a hole where
//! the box passes, drew as a fan of triangles across the cylinder's
//! inside: each hole was unrolled from angle 0 rather than from its own
//! place round the cylinder, and with a hole the triangles were not kept
//! short round the curve, so their flat chords cut through it.

use prism_doc::DefId;

fn model() -> prism_topo::Model {
    let c = prism_file::read(include_bytes!("data/owner-cylinder-1.prism")).expect("the file");
    c.assembly
        .def(DefId(1))
        .expect("the component")
        .model
        .clone()
}

#[test]
fn the_cut_cylinder_wall_draws_on_its_surface_with_its_own_area() {
    let m = model();
    let s = prism_view::Scene::build(&m);
    let mut walls = 0;
    for f in &s.faces {
        let face = m.face(f.id).expect("face");
        if !face.is_curved() {
            continue;
        }
        walls += 1;
        let idx = &s.mesh.indices[f.tris.start as usize..f.tris.end as usize];
        let mut area = 0.0;
        for t in idx.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| s.mesh.positions[t[k] as usize]);
            area += (b - a).cross(c - a).length() / 2.0;
            for p in [a, b, c, (a + b + c) / 3.0] {
                let off = face.surface.distance(p).abs();
                assert!(off < 5e-3, "{:?}: a point {off} m off the wall", f.id);
            }
        }
        let want = m.face_area(f.id);
        assert!(
            (area - want).abs() < 0.01 * want,
            "{:?}: drawn {area}, is {want}",
            f.id
        );
    }
    assert_eq!(walls, 4, "two halves with the hole, two pieces in it");
}
