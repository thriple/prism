//! A freeform face cut from its patch is saved and read back the same
//! (Solids & CAD part 3, step 3a).

use std::sync::Arc;

use prism_geom::{Curve, NurbsSurface, Patch, Spline, Surface};
use prism_math::DVec3;
use prism_topo::{Face, FaceId, Model, VertexId};

/// A bicubic bump over the unit square, 0.4 high in the middle.
fn bump() -> NurbsSurface {
    let mut pts = Vec::new();
    for i in 0..4 {
        for j in 0..4 {
            let mid = (i == 1 || i == 2) && (j == 1 || j == 2);
            pts.push(DVec3::new(
                i as f64 / 3.0,
                j as f64 / 3.0,
                if mid { 0.4 } else { 0.05 * i as f64 },
            ));
        }
    }
    let k = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    NurbsSurface::new(
        [3, 3],
        [k.clone(), k],
        [4, 4],
        pts,
        vec![1.0; 16],
        [false, false],
    )
    .unwrap()
}

/// A face on `s` bounded by iso lines through the (u, v) corners `uv` in
/// order (each side runs along u or along v).
fn face_by_corners(
    m: &mut Model,
    s: &Arc<NurbsSurface>,
    patch: Patch,
    uv: &[(f64, f64)],
) -> FaceId {
    let vs: Vec<VertexId> = uv
        .iter()
        .map(|(u, v)| m.add_vertex(s.eval(*u, *v)))
        .collect();
    let mut edges = Vec::new();
    for i in 0..uv.len() {
        let (a, b) = (uv[i], uv[(i + 1) % uv.len()]);
        let (d, at, t0, t1) = if a.1 == b.1 {
            (0, a.1, a.0, b.0)
        } else {
            (1, a.0, a.1, b.1)
        };
        let spline = Spline {
            curve: Arc::new(s.iso(d, at).unwrap()),
            t0,
            t1,
        };
        edges.push(
            m.add_curve_edge(vs[i], vs[(i + 1) % vs.len()], Curve::Spline(spline), None)
                .unwrap(),
        );
    }
    m.add_face(Face::with_edges(
        vs,
        vec![],
        Surface::Nurbs(patch),
        vec![edges],
    ))
}

fn part(s: &Arc<NurbsSurface>, u: (f64, f64), v: (f64, f64)) -> Patch {
    Patch {
        surface: s.clone(),
        u,
        v,
        flipped: false,
    }
}

#[test]
fn a_cut_freeform_face_is_saved_and_read_back() {
    let s = Arc::new(bump());
    let mut m = Model::new();
    let l = face_by_corners(
        &mut m,
        &s,
        part(&s, (0.0, 1.0), (0.0, 1.0)),
        &[
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 0.5),
            (0.5, 0.5),
            (0.5, 1.0),
            (0.0, 1.0),
        ],
    );
    let area = m.face_area(l);
    let c = prism_file::Contents::with_model(m, Default::default());
    let back = prism_file::read(&prism_file::write(&c, "test")).expect("reads");
    let m2 = back.assembly.root().model.clone();
    assert_eq!(m2.faces().count(), 1);
    let (id, f) = m2.faces().next().unwrap();
    assert!(m2.is_trimmed(f));
    assert!((m2.face_area(id) - area).abs() < 1e-12);
}
