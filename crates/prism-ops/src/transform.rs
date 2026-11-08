//! Moving, rotating and scaling vertices.
//!
//! A planar face owns its plane. After a transform each touched face gets a
//! refitted plane; if its vertices no longer lie on one plane, it is split
//! along a diagonal into planar pieces (folded), never triangulated as a
//! whole. Then vertices that ended up in the same place are welded.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::Surface;
use prism_math::{DQuat, DVec3, Plane, Tol};
use prism_topo::{Face, FaceId, Model, VertexId};

pub fn translate(model: &mut Model, vs: &BTreeSet<VertexId>, d: DVec3) {
    apply(model, vs, |p| p + d);
}

/// Rotate about the line through `centre` along `axis`, by `angle` radians.
pub fn rotate(model: &mut Model, vs: &BTreeSet<VertexId>, centre: DVec3, axis: DVec3, angle: f64) {
    let q = DQuat::from_axis_angle(axis.normalize(), angle);
    apply(model, vs, |p| centre + q * (p - centre));
}

/// Scale about `centre` by `k` along each axis.
pub fn scale(model: &mut Model, vs: &BTreeSet<VertexId>, centre: DVec3, k: DVec3) {
    apply(model, vs, |p| centre + (p - centre) * k);
}

/// Move vertices by `d` without the coplanar rule afterwards, for edits
/// that make several moves and then run it once ([`settle_at`]).
pub fn translate_only(model: &mut Model, vs: &BTreeSet<VertexId>, d: DVec3) {
    apply_only(model, vs, |p| p + d);
}

/// [`rotate`] without the coplanar rule afterwards.
pub fn rotate_only(
    model: &mut Model,
    vs: &BTreeSet<VertexId>,
    centre: DVec3,
    axis: DVec3,
    angle: f64,
) {
    let q = DQuat::from_axis_angle(axis.normalize(), angle);
    apply_only(model, vs, |p| centre + q * (p - centre));
}

fn apply(model: &mut Model, vs: &BTreeSet<VertexId>, f: impl Fn(DVec3) -> DVec3) {
    let targets: Vec<(VertexId, DVec3)> = vs.iter().map(|v| (*v, f(model.vertex(*v)))).collect();
    apply_only(model, vs, f);
    // Welding may have replaced some moved vertices: find those by place.
    let tol = model.tol();
    let gone: Vec<DVec3> = targets
        .iter()
        .filter(|(v, _)| !model.has_vertex(*v))
        .map(|(_, p)| *p)
        .collect();
    let mut touched: BTreeSet<VertexId> = vs
        .iter()
        .copied()
        .filter(|v| model.has_vertex(*v))
        .collect();
    if !gone.is_empty() {
        touched.extend(
            model
                .vertices()
                .filter(|(_, p)| gone.iter().any(|q| tol.same_point(*p, *q)))
                .map(|(id, _)| id),
        );
    }
    crate::settle::settle_rigid(model, &[touched]);
}

/// Move a few vertices by `d`, welding only them onto what they land on
/// (not the whole model), without the coplanar rule. For Push/Pull.
pub(crate) fn translate_few(model: &mut Model, vs: &BTreeSet<VertexId>, d: DVec3) {
    let faces: Vec<FaceId> = model
        .faces()
        .filter(|(_, face)| face.loops().flatten().any(|v| vs.contains(v)))
        .map(|(id, _)| id)
        .collect();
    for v in vs {
        let p = model.vertex(*v);
        model.set_vertex(*v, p + d);
    }
    let tol = model.tol();
    weld_new(model, vs, &tol);
    for id in faces {
        settle_face(model, id, &tol);
    }
}

fn apply_only(model: &mut Model, vs: &BTreeSet<VertexId>, f: impl Fn(DVec3) -> DVec3) {
    for v in vs {
        let p = model.vertex(*v);
        model.set_vertex(*v, f(p));
    }
    let tol = model.tol();
    weld(model, &tol);
    let touched: Vec<FaceId> = model
        .faces()
        .filter(|(_, face)| face.loops().flatten().any(|v| vs.contains(v)))
        .map(|(id, _)| id)
        .collect();
    for id in touched {
        settle_face(model, id, &tol);
    }
}

/// Refit a face's plane after its vertices moved, or fold it into planar
/// pieces if they no longer lie on one plane.
fn settle_face(model: &mut Model, id: FaceId, tol: &Tol) {
    let Some(face) = model.face(id).cloned() else {
        return;
    };
    let outer = model.points(&face.outer);
    let Some(plane) = Plane::fit(&outer, tol) else {
        model.remove_face(id);
        return;
    };
    let all_on = face
        .loops()
        .flatten()
        .all(|v| plane.contains(model.vertex(*v), tol));
    if all_on {
        if let Some(f) = model.face_mut(id) {
            f.surface = Surface::Plane(plane);
        }
        return;
    }
    if !face.holes.is_empty() {
        // A face with holes cannot be folded along one diagonal; put its
        // vertices back on the best-fit plane instead.
        for v in face.loops().flatten() {
            let p = model.vertex(*v);
            model.set_vertex(*v, plane.project(p));
        }
        if let Some(f) = model.face_mut(id) {
            f.surface = Surface::Plane(plane);
        }
        return;
    }
    model.remove_face(id);
    for piece in fold(model, &face.outer, tol) {
        if let Some(pl) = Plane::fit(&model.points(&piece), tol) {
            for w in 0..piece.len() {
                model.add_edge(piece[w], piece[(w + 1) % piece.len()]);
            }
            model.add_face(Face {
                outer: piece,
                holes: vec![],
                surface: Surface::Plane(pl),
            });
        }
    }
}

/// Split a loop along diagonals until every piece is planar. Diagonals are
/// tried in a fixed order, so the result is the same every time.
fn fold(model: &Model, l: &[VertexId], tol: &Tol) -> Vec<Vec<VertexId>> {
    let planar = |l: &[VertexId]| -> bool {
        let pts = model.points(l);
        Plane::fit(&pts, tol).is_some_and(|p| pts.iter().all(|q| p.contains(*q, tol)))
    };
    if l.len() <= 3 || planar(l) {
        return vec![l.to_vec()];
    }
    let n = l.len();
    // Prefer a diagonal that leaves both pieces planar, then one that leaves
    // at least one piece planar.
    let halves = |i: usize, j: usize| -> (Vec<VertexId>, Vec<VertexId>) {
        (
            l[i..=j].to_vec(),
            l[j..].iter().chain(l[..=i].iter()).copied().collect(),
        )
    };
    let diagonals: Vec<(usize, usize)> = (0..n)
        .flat_map(|i| (i + 2..n).map(move |j| (i, j)))
        .filter(|&(i, j)| !(i == 0 && j == n - 1))
        .collect();
    let best = diagonals
        .iter()
        .map(|&(i, j)| halves(i, j))
        .find(|(a, b)| planar(a) && planar(b))
        .or_else(|| {
            diagonals
                .iter()
                .map(|&(i, j)| halves(i, j))
                .find(|(a, b)| planar(a) || planar(b))
        });
    match best {
        Some((a, b)) => {
            let mut out = fold(model, &a, tol);
            out.extend(fold(model, &b, tol));
            out
        }
        None => vec![l.to_vec()],
    }
}

/// Merge vertices closer than the tolerance (the lower id stays), then drop
/// edges and faces that became degenerate or repeated.
pub fn weld(model: &mut Model, tol: &Tol) {
    let verts: Vec<(VertexId, DVec3)> = model.vertices().collect();
    let mut into: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for (i, (a, pa)) in verts.iter().enumerate() {
        if into.contains_key(a) {
            continue;
        }
        for (b, pb) in &verts[i + 1..] {
            if !into.contains_key(b) && tol.same_point(*pa, *pb) {
                into.insert(*b, *a);
            }
        }
    }
    apply_weld(model, &into);
}

/// Merge each of `new` onto an older vertex at the same point, if there is
/// one (Push/Pull landing on existing geometry). Only the new vertices are
/// checked, so this stays quick on big models.
pub fn weld_new(model: &mut Model, new: &BTreeSet<VertexId>, tol: &Tol) {
    let mut into: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for v in new {
        let p = model.vertex(*v);
        if let Some((old, _)) = model
            .vertices()
            .find(|(o, q)| !new.contains(o) && tol.same_point(p, *q))
        {
            into.insert(*v, old);
        }
    }
    apply_weld(model, &into);
}

fn apply_weld(model: &mut Model, into: &BTreeMap<VertexId, VertexId>) {
    if into.is_empty() {
        return;
    }
    let map = |v: VertexId| into.get(&v).copied().unwrap_or(v);
    let edges: Vec<(prism_topo::EdgeId, VertexId, VertexId)> =
        model.edges().map(|(id, e)| (id, e.a, e.b)).collect();
    for (id, a, b) in edges {
        let (ma, mb) = (map(a), map(b));
        if (ma, mb) != (a, b) {
            model.remove_edge(id);
            model.add_edge(ma, mb);
        }
    }
    let ids: Vec<FaceId> = model.faces().map(|(id, _)| id).collect();
    let mut seen: Vec<Vec<VertexId>> = Vec::new();
    for id in ids {
        let Some(f) = model.face_mut(id) else {
            continue;
        };
        let fix = |l: &mut Vec<VertexId>| {
            for v in l.iter_mut() {
                *v = map(*v);
            }
            l.dedup();
            while l.len() > 1 && l.first() == l.last() {
                l.pop();
            }
        };
        fix(&mut f.outer);
        for h in &mut f.holes {
            fix(h);
        }
        f.holes.retain(|h| h.len() >= 3);
        let mut key = f.outer.clone();
        key.sort();
        if f.outer.len() < 3 || seen.contains(&key) {
            model.remove_face(id);
        } else {
            seen.push(key);
        }
    }
    for v in into.keys() {
        model.remove_vertex(*v);
    }
}
