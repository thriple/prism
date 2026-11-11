//! The model rules still catch what they are for (they were made fast on
//! big models, 2026-11-12): overlapping coplanar faces, a face repeated,
//! two points in one place; and a model of many faces checks quickly.

use prism_geom::Surface;
use prism_math::{DVec3, Plane};
use prism_ops::check;
use prism_topo::{Face, Model, VertexId};

fn square(m: &mut Model, x: f64, y: f64, s: f64) -> Vec<VertexId> {
    let vs: Vec<VertexId> = [(0.0, 0.0), (s, 0.0), (s, s), (0.0, s)]
        .iter()
        .map(|(a, b)| m.add_vertex(DVec3::new(x + a, y + b, 0.0)))
        .collect();
    for i in 0..4 {
        m.add_edge(vs[i], vs[(i + 1) % 4]);
    }
    vs
}

fn flat(vs: Vec<VertexId>) -> Face {
    Face::new(
        vs,
        vec![],
        Surface::Plane(Plane::new(DVec3::ZERO, DVec3::Z)),
    )
}

#[test]
fn overlapping_faces_a_repeated_face_and_doubled_points_are_found() {
    let mut m = Model::new();
    let a = square(&mut m, 0.0, 0.0, 2.0);
    let b = square(&mut m, 0.5, 0.5, 2.0);
    m.add_face(flat(a.clone()));
    m.add_face(flat(b));
    let tol = m.tol();
    let o = check::no_overlaps(&m, &tol);
    assert_eq!(o.len(), 2, "each overlaps the other: {o:?}");
    // The same loop twice.
    m.add_face(flat(a));
    let d = check::no_duplicates(&m, &tol);
    assert!(d.iter().any(|x| x.contains("same loop")), "{d:?}");
    // Two points in one place.
    m.add_vertex(DVec3::new(5.0, 5.0, 0.0));
    m.add_vertex(DVec3::new(5.0, 5.0, 0.0));
    let d = check::no_duplicates(&m, &tol);
    assert!(d.iter().any(|x| x.contains("same place")), "{d:?}");
}

#[test]
fn a_model_of_many_faces_checks_quickly() {
    let mut m = Model::new();
    for i in 0..40 {
        for j in 0..40 {
            let vs = square(&mut m, f64::from(i) * 2.0, f64::from(j) * 2.0, 1.0);
            m.add_face(flat(vs));
        }
    }
    assert!(check::problems(&m).is_empty());
}
