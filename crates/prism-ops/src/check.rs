//! The rules every model must satisfy (`docs/acceptance/core-model-acceptance.md`).
//! Each returns a list of problems; an empty list means the rule holds.

use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, Tol};
use prism_topo::{FaceId, Model, VertexId};

use crate::regions::{interior_point, same_cycle};

/// Every rule at once.
pub fn problems(model: &Model) -> Vec<String> {
    let tol = model.tol();
    let mut out = model.bookkeeping_problems();
    out.extend(no_overlaps(model, &tol));
    out.extend(no_duplicates(model, &tol));
    out.extend(holes_inside(model, &tol));
    out.extend(planar(model, &tol));
    out
}

fn loops_2d(model: &Model, f: FaceId) -> Vec<Vec<DVec2>> {
    let Some(face) = model.face(f) else {
        return vec![];
    };
    let fr = face.plane().frame();
    face.loops()
        .map(|l| l.iter().map(|v| fr.to_2d(model.vertex(*v))).collect())
        .collect()
}

/// No two coplanar faces overlap: a point inside one is not inside another.
pub fn no_overlaps(model: &Model, tol: &Tol) -> Vec<String> {
    let mut out = Vec::new();
    let faces: Vec<FaceId> = model.faces().map(|(id, _)| id).collect();
    for &f in &faces {
        let Some(ff) = model.face(f) else { continue };
        let fr = ff.plane().frame();
        let Some(sample) = interior_point(&loops_2d(model, f)) else {
            out.push(format!("face {f:?} has no inside"));
            continue;
        };
        let p3 = fr.to_3d(sample);
        for &g in &faces {
            let Some(gg) = model.face(g) else { continue };
            if g == f || !gg.plane().same_surface(ff.plane(), tol) {
                continue;
            }
            let gf = gg.plane().frame();
            let q = gf.to_2d(p3);
            let poly = |l: &[VertexId]| -> Vec<DVec2> {
                l.iter().map(|v| gf.to_2d(model.vertex(*v))).collect()
            };
            if point_in_polygon(q, &poly(&gg.outer)) == Inside::Inside
                && gg
                    .holes
                    .iter()
                    .all(|h| point_in_polygon(q, &poly(h)) == Inside::Outside)
            {
                out.push(format!("faces {f:?} and {g:?} overlap"));
            }
        }
    }
    out
}

/// No two vertices closer than the tolerance, and no two faces with the same loop.
pub fn no_duplicates(model: &Model, tol: &Tol) -> Vec<String> {
    let mut out = Vec::new();
    let vs: Vec<_> = model.vertices().collect();
    for (i, (a, pa)) in vs.iter().enumerate() {
        for (b, pb) in &vs[i + 1..] {
            if tol.same_point(*pa, *pb) {
                out.push(format!("vertices {a:?} and {b:?} are in the same place"));
            }
        }
    }
    let fs: Vec<_> = model.faces().collect();
    for (i, (a, fa)) in fs.iter().enumerate() {
        for (b, fb) in &fs[i + 1..] {
            if same_cycle(&fa.outer, &fb.outer) {
                out.push(format!("faces {a:?} and {b:?} have the same loop"));
            }
        }
    }
    out
}

/// Every hole is strictly inside its outer loop, and holes do not touch
/// each other or the outer loop.
pub fn holes_inside(model: &Model, _tol: &Tol) -> Vec<String> {
    let mut out = Vec::new();
    for (id, f) in model.faces() {
        let loops = loops_2d(model, id);
        for (hi, h) in f.holes.iter().enumerate() {
            if h.iter().any(|v| f.outer.contains(v)) {
                out.push(format!("face {id:?}: hole {hi} touches the outer loop"));
            }
            for (k, other) in f.holes.iter().enumerate() {
                if k != hi && h.iter().any(|v| other.contains(v)) {
                    out.push(format!("face {id:?}: holes {hi} and {k} touch"));
                }
            }
            if loops[hi + 1]
                .iter()
                .any(|p| point_in_polygon(*p, &loops[0]) != Inside::Inside)
            {
                out.push(format!(
                    "face {id:?}: hole {hi} is not inside the outer loop"
                ));
            }
        }
    }
    out
}

/// Every face's vertices are on its plane, and its outer loop winds the way
/// its normal points.
pub fn planar(model: &Model, tol: &Tol) -> Vec<String> {
    let mut out = Vec::new();
    for (id, f) in model.faces() {
        let pl = f.plane();
        if f.loops()
            .flatten()
            .any(|v| !pl.contains(model.vertex(*v), tol))
        {
            out.push(format!("face {id:?} is not planar"));
        }
        let n = prism_math::newell(&model.points(&f.outer));
        if n.dot(pl.normal) <= 0.0 {
            out.push(format!("face {id:?} winds against its normal"));
        }
    }
    out
}
