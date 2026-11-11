//! The rules every model must satisfy (`docs/acceptance/core-model-acceptance.md`).
//! Each returns a list of problems; an empty list means the rule holds.

use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3, Tol};
use prism_topo::{FaceId, Model};

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

/// Loop `k` of a curved face, flat, as (θ, s) on its turned surface.
/// Counter-clockwise when the face points outward.
fn curved_2d(model: &Model, face: &prism_topo::Face, k: usize) -> Vec<DVec2> {
    model.param_loop(face, k).into_iter().map(|x| x.1).collect()
}

fn loops_2d(model: &Model, f: FaceId) -> Vec<Vec<DVec2>> {
    let Some(face) = model.face(f) else {
        return vec![];
    };
    let Some(pl) = face.plane() else {
        // A curved face, laid out flat; each hole moved by whole turns
        // into the outer loop.
        let mut loops: Vec<Vec<DVec2>> = (0..=face.holes.len())
            .map(|k| curved_2d(model, face, k))
            .collect();
        let circle = matches!(
            face.surface.revolution().map(|r| r.profile),
            Some(prism_geom::Profile::Circle { .. })
        );
        for (h, d) in crate::cut::hole_shifts(&loops, circle)
            .into_iter()
            .enumerate()
        {
            for p in &mut loops[h + 1] {
                *p += d;
            }
        }
        return loops;
    };
    let fr = pl.frame();
    model
        .face_loop_points(face)
        .iter()
        .map(|l| l.iter().map(|p| fr.to_2d(*p)).collect())
        .collect()
}

/// No two coplanar faces overlap: a point inside one is not inside another.
///
/// Each flat face's plane, outline and box are found once, and a face is
/// only tested against faces whose box holds its point (sorted along x),
/// so thousands of faces check in moments, not seconds.
pub fn no_overlaps(model: &Model, tol: &Tol) -> Vec<String> {
    struct Entry {
        id: FaceId,
        plane: prism_math::Plane,
        frame: prism_math::plane::Frame,
        loops: Vec<Vec<DVec2>>,
        lo: DVec3,
        hi: DVec3,
    }
    let mut out = Vec::new();
    let pad = DVec3::splat(tol.dist * 2.0);
    let mut flats: Vec<Entry> = model
        .faces()
        .filter_map(|(id, f)| {
            // Curved faces are not checked for overlaps yet.
            let plane = *f.plane()?;
            let pts: Vec<DVec3> = model.face_loop_points(f).into_iter().flatten().collect();
            let lo = pts.iter().fold(DVec3::INFINITY, |a, p| a.min(*p)) - pad;
            let hi = pts.iter().fold(DVec3::NEG_INFINITY, |a, p| a.max(*p)) + pad;
            Some(Entry {
                id,
                plane,
                frame: plane.frame(),
                loops: loops_2d(model, id),
                lo,
                hi,
            })
        })
        .collect();
    flats.sort_by(|a, b| a.lo.x.total_cmp(&b.lo.x).then(a.id.cmp(&b.id)));
    let mut found: Vec<(FaceId, FaceId)> = Vec::new();
    for f in &flats {
        let Some(sample) = interior_point(&f.loops) else {
            out.push(format!("face {:?} has no inside", f.id));
            continue;
        };
        let p3 = f.frame.to_3d(sample);
        // Only faces starting at or before the point along x can hold it.
        let end = flats.partition_point(|g| g.lo.x <= p3.x);
        for g in &flats[..end] {
            if g.id == f.id
                || p3.cmplt(g.lo).any()
                || p3.cmpgt(g.hi).any()
                || !g.plane.same_surface(&f.plane, tol)
            {
                continue;
            }
            let q = g.frame.to_2d(p3);
            if point_in_polygon(q, &g.loops[0]) == Inside::Inside
                && g.loops[1..]
                    .iter()
                    .all(|h| point_in_polygon(q, h) == Inside::Outside)
            {
                found.push((f.id, g.id));
            }
        }
    }
    // In the order the faces are numbered, as before.
    found.sort();
    out.extend(
        found
            .into_iter()
            .map(|(f, g)| format!("faces {f:?} and {g:?} overlap")),
    );
    out
}

/// No two vertices closer than the tolerance, and no two faces with the same loop.
pub fn no_duplicates(model: &Model, tol: &Tol) -> Vec<String> {
    let mut out = Vec::new();
    // Sorted along x: only points within the tolerance along x can be the
    // same point.
    let mut vs: Vec<_> = model.vertices().collect();
    vs.sort_by(|a, b| a.1.x.total_cmp(&b.1.x).then(a.0.cmp(&b.0)));
    let mut pairs = Vec::new();
    for (i, (a, pa)) in vs.iter().enumerate() {
        for (b, pb) in &vs[i + 1..] {
            if pb.x - pa.x >= tol.dist {
                break;
            }
            if tol.same_point(*pa, *pb) {
                pairs.push(((*a).min(*b), (*a).max(*b)));
            }
        }
    }
    pairs.sort();
    for (a, b) in pairs {
        out.push(format!("vertices {a:?} and {b:?} are in the same place"));
    }
    // Faces with the same outer loop have the same edges: only faces that
    // share a set of edges are compared.
    let mut by_edges: std::collections::BTreeMap<Vec<prism_topo::EdgeId>, Vec<FaceId>> =
        Default::default();
    for (id, f) in model.faces() {
        let mut key = f.loop_edge_ids(0).to_vec();
        key.sort();
        by_edges.entry(key).or_default().push(id);
    }
    let mut same = Vec::new();
    for ids in by_edges.values().filter(|v| v.len() > 1) {
        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                let (Some(fa), Some(fb)) = (model.face(*a), model.face(*b)) else {
                    continue;
                };
                if same_cycle(fa.loop_edge_ids(0), fb.loop_edge_ids(0))
                    && !(closed(fa) && closed(fb) && !same_way(fa, fb))
                {
                    same.push((*a, *b));
                }
            }
        }
    }
    same.sort();
    for (a, b) in same {
        out.push(format!("faces {a:?} and {b:?} have the same loop"));
    }
    out
}

/// On a closed surface (a sphere or a torus), a loop walked one way and the
/// same loop walked back bound the two sides of it: two faces, not one
/// repeated (the two halves of a sphere between two half circles).
fn closed(f: &prism_topo::Face) -> bool {
    matches!(
        f.surface,
        prism_geom::Surface::Sphere(_) | prism_geom::Surface::Torus(_)
    )
}

/// Whether two faces walk the same loop the same way round: the same
/// vertices with the same edges after each.
fn same_way(a: &prism_topo::Face, b: &prism_topo::Face) -> bool {
    let pairs = |f: &prism_topo::Face| -> Vec<_> {
        f.outer
            .iter()
            .copied()
            .zip(f.loop_edge_ids(0).iter().copied())
            .collect()
    };
    let (pa, pb) = (pairs(a), pairs(b));
    let n = pa.len();
    n == pb.len() && (0..n).any(|k| (0..n).all(|i| pa[i] == pb[(k + i) % n]))
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

/// Every face's vertices are on its surface, and its outer loop winds the
/// way its normal points.
pub fn planar(model: &Model, tol: &Tol) -> Vec<String> {
    let mut out = Vec::new();
    for (id, f) in model.faces() {
        let Some(pl) = f.plane() else {
            if f.loops()
                .flatten()
                .any(|v| f.surface.distance(model.vertex(*v)).abs() > tol.dist)
            {
                out.push(format!("face {id:?} is off its surface"));
            }
            // Unrolled, the outer loop winds about the outward normal.
            let p = curved_2d(model, f, 0);
            let area: f64 = (0..p.len())
                .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
                .sum();
            let outward = f.surface.outward();
            if (area > 0.0) != outward {
                out.push(format!("face {id:?} winds against its normal"));
            }
            continue;
        };
        if f.loops()
            .flatten()
            .any(|v| !pl.contains(model.vertex(*v), tol))
        {
            out.push(format!("face {id:?} is not planar"));
        }
        let n = prism_math::newell(&model.loop_points(f, 0));
        if n.dot(pl.normal) <= 0.0 {
            out.push(format!("face {id:?} winds against its normal"));
        }
    }
    out
}
