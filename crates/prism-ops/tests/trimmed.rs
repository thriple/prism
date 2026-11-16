//! Freeform faces cut from their patch (Solids & CAD part 3, step 3a):
//! area and volume by Green's theorem in (u, v) agree with the whole
//! patch's, and with the patches the cut leaves.

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

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

#[test]
fn a_whole_patch_has_its_area_and_the_same_volume_both_ways() {
    let s = Arc::new(bump());
    let mut m = Model::new();
    let whole = part(&s, (0.0, 1.0), (0.0, 1.0));
    let f = face_by_corners(
        &mut m,
        &s,
        whole.clone(),
        &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    );
    let face = m.face(f).unwrap().clone();
    assert!(!m.is_trimmed(&face));
    let area = m.face_area(f);
    assert!(area > 1.0 && area < 1.5, "{area}");
    // Green's theorem over the loop agrees with the grid integration.
    let uv: Vec<_> = m.patch_loop(&face, 0).into_iter().map(|x| x.1).collect();
    close(
        whole.loop_integrals(&uv).1,
        prism_ops::health::face_volume(&m, f),
    );
}

#[test]
fn an_l_shaped_trim_is_the_whole_less_a_corner_patch() {
    let s = Arc::new(bump());
    let mut m = Model::new();
    let whole = part(&s, (0.0, 1.0), (0.0, 1.0));
    let full = face_by_corners(
        &mut m,
        &s,
        whole.clone(),
        &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    );
    let corner = face_by_corners(
        &mut m,
        &s,
        part(&s, (0.5, 1.0), (0.5, 1.0)),
        &[(0.5, 0.5), (1.0, 0.5), (1.0, 1.0), (0.5, 1.0)],
    );
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
    assert!(m.is_trimmed(m.face(l).unwrap()));
    close(m.face_area(l), m.face_area(full) - m.face_area(corner));
    let vol = |f| prism_ops::health::face_volume(&m, f);
    close(vol(l), vol(full) - vol(corner));
}

#[test]
fn a_cage_edit_of_a_cut_surface_is_refused() {
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
    let up = prism_math::DAffine3::from_translation(DVec3::Z);
    let r = prism_ops::cage::moved(&m, prism_ops::cage::Cage::Surface(l), &[5].into(), &up);
    assert_eq!(r.err(), Some(prism_ops::cage::TRIMMED));
    // The model check is clean for the cut face.
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
}

/// An iso line on `s` from (u, v) `a` to `b` (one of them constant), as
/// an edge between existing vertices.
fn iso_edge(m: &mut Model, s: &Arc<NurbsSurface>, a: (f64, f64), b: (f64, f64)) {
    let find = |m: &Model, q: (f64, f64)| {
        let p = s.eval(q.0, q.1);
        m.vertices()
            .find(|(_, x)| x.distance(p) < 1e-9)
            .map(|(id, _)| id)
    };
    let va = find(m, a).unwrap_or_else(|| m.add_vertex(s.eval(a.0, a.1)));
    let vb = find(m, b).unwrap_or_else(|| m.add_vertex(s.eval(b.0, b.1)));
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
    m.add_curve_edge(va, vb, Curve::Spline(spline), None)
        .unwrap();
}

#[test]
fn a_line_across_a_freeform_face_cuts_it_in_two() {
    let s = Arc::new(bump());
    let mut m = Model::new();
    let whole = part(&s, (0.0, 1.0), (0.0, 1.0));
    let f = face_by_corners(
        &mut m,
        &s,
        whole,
        &[
            (0.0, 0.0),
            (0.5, 0.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (0.5, 1.0),
            (0.0, 1.0),
        ],
    );
    let (area, volume) = (m.face_area(f), prism_ops::health::face_volume(&m, f));
    iso_edge(&mut m, &s, (0.5, 0.0), (0.5, 1.0));
    let made = prism_ops::cut::split_face(&mut m, f);
    assert_eq!(made.len(), 2, "{made:?}");
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
    close(made.iter().map(|g| m.face_area(*g)).sum(), area);
    close(
        made.iter()
            .map(|g| prism_ops::health::face_volume(&m, *g))
            .sum(),
        volume,
    );
    // Each half is the patch of its half.
    let mut n = Model::new();
    let left = face_by_corners(
        &mut n,
        &s,
        part(&s, (0.0, 0.5), (0.0, 1.0)),
        &[(0.0, 0.0), (0.5, 0.0), (0.5, 1.0), (0.0, 1.0)],
    );
    let small = made
        .iter()
        .map(|g| m.face_area(*g))
        .fold(f64::INFINITY, f64::min);
    close(
        small.min(area - small),
        n.face_area(left).min(area - n.face_area(left)),
    );
}

#[test]
fn a_closed_loop_inside_cuts_a_hole_and_an_island() {
    let s = Arc::new(bump());
    let mut m = Model::new();
    let f = face_by_corners(
        &mut m,
        &s,
        part(&s, (0.0, 1.0), (0.0, 1.0)),
        &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    );
    let area = m.face_area(f);
    let sq = [(0.3, 0.3), (0.6, 0.3), (0.6, 0.6), (0.3, 0.6)];
    for i in 0..4 {
        iso_edge(&mut m, &s, sq[i], sq[(i + 1) % 4]);
    }
    let made = prism_ops::cut::split_face(&mut m, f);
    assert_eq!(made.len(), 2, "{made:?}");
    assert_eq!(prism_ops::check::problems(&m), Vec::<String>::new());
    let holed = made.iter().find(|g| !m.face(**g).unwrap().holes.is_empty());
    assert!(holed.is_some(), "the outer piece has a hole");
    close(made.iter().map(|g| m.face_area(*g)).sum(), area);
    // The island is the patch over the square.
    let mut n = Model::new();
    let island = face_by_corners(&mut n, &s, part(&s, (0.3, 0.6), (0.3, 0.6)), &sq);
    let got = made
        .iter()
        .find(|g| m.face(**g).unwrap().holes.is_empty())
        .map(|g| m.face_area(*g))
        .unwrap();
    close(got, n.face_area(island));
}
