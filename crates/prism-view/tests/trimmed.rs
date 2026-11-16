//! A freeform face cut from its patch is drawn and picked (Solids & CAD
//! part 3, step 3a): its triangles cover its area, face its way, and sit
//! on the surface.

use std::sync::Arc;

use prism_geom::{Curve, NurbsSurface, Patch, Spline, Surface};
use prism_math::DVec3;
use prism_topo::{Face, FaceId, Model, VertexId};
use prism_view::scene::Scene;

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
fn an_l_shaped_trim_is_drawn_over_its_area_and_picked() {
    let s = Arc::new(bump());
    let mut m = Model::new();
    let whole = part(&s, (0.0, 1.0), (0.0, 1.0));
    let l = face_by_corners(
        &mut m,
        &s,
        whole,
        &[
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 0.5),
            (0.5, 0.5),
            (0.5, 1.0),
            (0.0, 1.0),
        ],
    );
    let face = m.face(l).unwrap().clone();
    let t = prism_view::tess::trimmed(&m, &face).expect("drawn");
    let mut area = 0.0;
    for tri in &t.tris {
        let [a, b, c] = tri.map(|i| t.points[i]);
        let n = (b - a).cross(c - a);
        area += n.length() / 2.0;
        // Facing up, as the patch does (u cross v is up).
        assert!(n.z > 0.0, "{tri:?} {n:?} {:?}", tri.map(|i| t.params[i]));
    }
    let want = m.face_area(l);
    assert!((area - want).abs() < 2e-3 * want, "{area} vs {want}");
    for p in &t.points {
        assert!(
            s.eval(
                s.closest(*p, [(0.0, 1.0), (0.0, 1.0)]).0,
                s.closest(*p, [(0.0, 1.0), (0.0, 1.0)]).1
            )
            .distance(*p)
                < 1e-9
        );
    }
    // A ray down through the L hits it; one through the cut-off corner
    // does not.
    let scene = Scene::build(&m);
    let down = DVec3::new(0.0, 0.0, -1.0);
    assert_eq!(
        scene
            .ray_face(DVec3::new(0.25, 0.25, 5.0), down)
            .map(|x| x.0),
        Some(l)
    );
    assert_eq!(
        scene.ray_face(DVec3::new(0.8, 0.8, 5.0), down).map(|x| x.0),
        None
    );
}
