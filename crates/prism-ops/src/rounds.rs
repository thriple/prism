//! Round many edges of a solid at once through the kernel (Fillet Joins
//! step B, brief 32): fillets or chamfers of one size, with clean corners
//! where they meet (mitres, ball corners). The solid goes to the kernel by
//! [`crate::bridge`] and comes back with its untouched faces, edges and
//! corners keeping their ids and paint; new faces are Default. The lines
//! where a round meets a face it is tangent to are soft and smooth.

use std::collections::BTreeSet;

use prism_kernel::solid::Solid;
use prism_math::DVec3;
use prism_topo::{EdgeId, FaceId, Model};

use crate::bridge::{put_back, region, solid_of};

/// Said when there is nothing to round.
pub const NOTHING: &str = "Refused: pick the edges of a solid to round.";
/// Said for a size that is not more than zero.
pub const NO_SIZE: &str = "Refused: the size must be more than zero.";
/// Said when the edges are not all on one closed solid.
pub const TWO_SOLIDS: &str = "Refused: the edges are not all on one closed solid.";

/// Round `edges` (all of one closed solid) by `size`: fillets, or chamfers
/// when `fillet` is false. Returns the solid's faces afterwards. A refusal
/// leaves the model as it was.
pub fn round_edges(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Result<Vec<FaceId>, &'static str> {
    match round_plain(model, edges, size, fillet) {
        Err(why) if why == prism_kernel::solid::TOO_LARGE => {
            round_through_lines(model, edges, size, fillet).ok_or(why)
        }
        done => done,
    }
}

/// [`round_edges`] again where the kernel found the size too large, past
/// lines that are not part of the solid's shape, only drawn on it: a line
/// between two faces of one plane (drawn on a face beside the round's
/// end), and a point where an edge was split and nothing else meets. To
/// the kernel each is a corner a round may not run past (owner's session
/// 22: a round stopped at 0.028 m, then at 0.109 m, where 0.2 m fits).
/// Those near the ends of `edges` are taken off, the edges rounded, and
/// the lines drawn back on. None if it still cannot be done, or the lines
/// would not go back.
fn round_through_lines(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<Vec<FaceId>> {
    let mut work = model.clone();
    let faces = solid_faces(&work, edges).ok()?;
    let seams = drawn_lines(&work, &faces, edges);
    let lines: Vec<(DVec3, DVec3)> = seams
        .iter()
        .filter_map(|e| work.edge(*e))
        .map(|e| (work.vertex(e.a), work.vertex(e.b)))
        .collect();
    if !seams.is_empty() {
        crate::remove(
            &mut work,
            &crate::Selection {
                edges: seams,
                ..Default::default()
            },
        );
    }
    let faces = solid_faces(&work, edges).ok()?;
    let joined = join_straight(&mut work, &faces, edges);
    if lines.is_empty() && joined == 0 {
        return None;
    }
    round_plain(&mut work, edges, size, fillet).ok()?;
    // The lines go back as far as they still lie on the solid: a round
    // that cut into the face one was on leaves its end in the air.
    let had: BTreeSet<EdgeId> = work.edges().map(|(id, _)| id).collect();
    for (a, b) in lines {
        crate::add_segment(&mut work, a, b);
    }
    let used = work.edge_faces();
    let loose: Vec<EdgeId> = work
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !had.contains(id) && used.get(id).is_none_or(|f| f.is_empty()))
        .collect();
    for id in loose {
        let Some(e) = work.edge(id).cloned() else {
            continue;
        };
        work.remove_edge(id);
        for v in [e.a, e.b] {
            if !work.edges().any(|(_, x)| x.a == v || x.b == v) {
                work.remove_vertex(v);
            }
        }
    }
    if !crate::check::problems(&work).is_empty() {
        return None;
    }
    let seed = edges
        .iter()
        .find_map(|e| work.edge_faces().get(e).and_then(|f| f.first().copied()));
    let out = match seed {
        Some(f) => solid_of(&work, f).ok()?.into_iter().collect(),
        None => {
            // The rounded edges are gone: the solid is the faces that were
            // not there before, with the rest it joins.
            let new = work
                .faces()
                .map(|(id, _)| id)
                .find(|id| model.face(*id).is_none())?;
            solid_of(&work, new).ok()?.into_iter().collect()
        }
    };
    *model = work;
    Some(out)
}

/// The faces of the one closed solid all of `edges` are on.
fn solid_faces(model: &Model, edges: &BTreeSet<EdgeId>) -> Result<BTreeSet<FaceId>, &'static str> {
    let first = *edges.first().ok_or(NOTHING)?;
    let edge_faces = model.edge_faces();
    let seed = edge_faces
        .get(&first)
        .and_then(|v| v.first())
        .copied()
        .ok_or(TWO_SOLIDS)?;
    let faces = solid_of(model, seed).map_err(|_| TWO_SOLIDS)?;
    for e in edges {
        let on = edge_faces
            .get(e)
            .is_some_and(|fs| fs.iter().filter(|f| faces.contains(f)).count() == 2);
        if !on {
            return Err(TWO_SOLIDS);
        }
    }
    Ok(faces.into_iter().collect())
}

/// The faces of `faces` with a point at an end of `edges`.
fn end_faces(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    edges: &BTreeSet<EdgeId>,
) -> BTreeSet<FaceId> {
    let ends: BTreeSet<_> = edges
        .iter()
        .filter_map(|e| model.edge(*e))
        .flat_map(|e| [e.a, e.b])
        .collect();
    faces
        .iter()
        .copied()
        .filter(|f| {
            model
                .face(*f)
                .is_some_and(|x| x.loops().flatten().any(|v| ends.contains(v)))
        })
        .collect()
}

/// Straight lines between two flat faces of one plane, facing the same
/// way, one of them at an end of `edges`: drawn on the solid, not part of
/// its shape.
fn drawn_lines(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    edges: &BTreeSet<EdgeId>,
) -> BTreeSet<EdgeId> {
    let near = end_faces(model, faces, edges);
    let tol = model.tol();
    model
        .edge_faces()
        .into_iter()
        .filter(|(e, fs)| {
            let [f, g] = fs.as_slice() else { return false };
            if edges.contains(e) || !(near.contains(f) || near.contains(g)) {
                return false;
            }
            let straight = model.edge(*e).is_some_and(|x| !x.curve.is_curved());
            match (
                model.face(*f).and_then(|x| x.plane()),
                model.face(*g).and_then(|x| x.plane()),
            ) {
                (Some(p), Some(q)) => {
                    straight
                        && faces.contains(f)
                        && faces.contains(g)
                        && p.normal.dot(q.normal) > 0.0
                        && p.same_surface(q, &tol, model.face_extent(model.face(*g).unwrap()))
                }
                _ => false,
            }
        })
        .map(|(e, _)| e)
        .collect()
}

/// [`round_edges`] as the kernel takes it, with nothing done first.
fn round_plain(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Result<Vec<FaceId>, &'static str> {
    let first = *edges.first().ok_or(NOTHING)?;
    if !(size.is_finite() && size > model.tol().dist) {
        return Err(NO_SIZE);
    }
    let edge_faces = model.edge_faces();
    let seed = edge_faces
        .get(&first)
        .and_then(|v| v.first())
        .copied()
        .ok_or(TWO_SOLIDS)?;
    let faces = solid_of(model, seed).map_err(|_| TWO_SOLIDS)?;
    for e in edges {
        let on = edge_faces
            .get(e)
            .is_some_and(|fs| fs.iter().filter(|f| faces.contains(f)).count() == 2);
        if !on {
            return Err(TWO_SOLIDS);
        }
    }
    let r = region(model, &faces)?;
    let mut s = Solid::build(&r, model.tol().dist)?;
    let keys: Vec<u64> = edges.iter().map(|e| u64::from(e.0)).collect();
    s.round(&keys, size, fillet)?;
    let back = s.region()?;
    let mut work = model.clone();
    let before: BTreeSet<EdgeId> = work.edges().map(|(id, _)| id).collect();
    let out = put_back(&mut work, &faces, &back)?;
    soften_tangent(&mut work, &before);
    if !crate::check::problems(&work).is_empty() {
        return Err(prism_kernel::solid::CANNOT_STORE);
    }
    *model = work;
    Ok(out)
}

/// Join, on the faces at the ends of `edges` (of the solid `faces`), each
/// pair of straight edges that meet in line at a point nothing else meets
/// and lie between the same two faces: the point is only where a line was
/// once split, but to the kernel it is a corner, and a round may not run
/// past a corner (owner's session 22: a round stopped at 0.028 m, the
/// height of such a point below the edge, where 0.2 m fits). Edges being
/// rounded are left. Returns how many points went.
fn join_straight(model: &mut Model, faces: &BTreeSet<FaceId>, edges: &BTreeSet<EdgeId>) -> usize {
    let ends: BTreeSet<_> = edges
        .iter()
        .filter_map(|e| model.edge(*e))
        .flat_map(|e| [e.a, e.b])
        .collect();
    // The points on the faces at those ends.
    let near = end_faces(model, faces, edges);
    let points: BTreeSet<_> = near
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.loops().flatten().copied().collect::<Vec<_>>())
        .collect();
    let tol = model.tol();
    let mut done = 0;
    for v in points {
        if ends.contains(&v) {
            continue;
        }
        let at: Vec<EdgeId> = model
            .edges()
            .filter(|(_, e)| e.a == v || e.b == v)
            .map(|(id, _)| id)
            .collect();
        let [e1, e2] = at[..] else { continue };
        if edges.contains(&e1) || edges.contains(&e2) {
            continue;
        }
        let ef = model.edge_faces();
        let pair = |e: EdgeId| {
            let mut fs = ef.get(&e).cloned().unwrap_or_default();
            fs.sort();
            fs
        };
        let fs = pair(e1);
        if fs.len() != 2 || fs != pair(e2) || !fs.iter().all(|f| faces.contains(f)) {
            continue;
        }
        let (Some(x1), Some(x2)) = (model.edge(e1).cloned(), model.edge(e2).cloned()) else {
            continue;
        };
        if x1.curve.is_curved() || x2.curve.is_curved() {
            continue;
        }
        let a = if x1.a == v { x1.b } else { x1.a };
        let b = if x2.a == v { x2.b } else { x2.a };
        let (pa, pb, pv) = (model.vertex(a), model.vertex(b), model.vertex(v));
        // In line: the point is on the line from a to b, between them.
        let along = pb - pa;
        let t = (pv - pa).dot(along) / along.length_squared().max(f64::MIN_POSITIVE);
        if !(0.0..=1.0).contains(&t) || !tol.same_point(pa + along * t, pv) {
            continue;
        }
        let Some(joined) = model.add_edge(a, b) else {
            continue;
        };
        model.set_soft(joined, x1.soft, x1.smooth);
        for f in &fs {
            model.edit_face(*f, |face| {
                face.outer.retain(|x| *x != v);
                for h in &mut face.holes {
                    h.retain(|x| *x != v);
                }
            });
        }
        model.remove_edge(e1);
        model.remove_edge(e2);
        model.remove_vertex(v);
        done += 1;
    }
    done
}

/// Why rounding `edges` would be refused, if it would.
pub fn refusal(
    model: &Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<&'static str> {
    let mut m = model.clone();
    round_edges(&mut m, edges, size, fillet).err()
}

/// The new edges (not in `before`) between two faces that are tangent
/// there are soft and smooth: the line between a round and its faces.
fn soften_tangent(model: &mut Model, before: &BTreeSet<EdgeId>) {
    let edge_faces = model.edge_faces();
    let mut soft = Vec::new();
    for (id, e) in model.edges() {
        if before.contains(&id) {
            continue;
        }
        let Some([f, g]) = edge_faces.get(&id).map(Vec::as_slice) else {
            continue;
        };
        let mid = middle(model, e);
        let (Some(a), Some(b)) = (model.face(*f), model.face(*g)) else {
            continue;
        };
        let (na, nb) = (a.surface.normal_at(mid), b.surface.normal_at(mid));
        if na.dot(nb) > 1.0 - 1e-6 {
            soft.push(id);
        }
    }
    for id in soft {
        model.set_soft(id, true, true);
    }
}

/// A point halfway along an edge.
fn middle(model: &Model, e: &prism_topo::Edge) -> DVec3 {
    let (a, b) = (model.vertex(e.a), model.vertex(e.b));
    match &e.curve {
        prism_geom::Curve::Line => (a + b) * 0.5,
        prism_geom::Curve::Arc { centre, axis, .. } => {
            let (xa, xb) = (a - *centre, b - *centre);
            let mut sweep = axis.dot(xa.cross(xb)).atan2(xa.dot(xb));
            if sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
            *centre + prism_math::DQuat::from_axis_angle(*axis, sweep / 2.0) * xa
        }
        prism_geom::Curve::Spline(sp) => sp.point(sp.halfway()),
    }
}

/// The straight run `edge` is part of: the segments end to end with it,
/// along the same line, each between faces on the same two planes as its
/// own (a line drawn in pieces, rounded as a whole). Just `edge` when it
/// is curved or not between two flat faces.
pub fn straight_run(model: &Model, edge: EdgeId) -> BTreeSet<EdgeId> {
    let mut out = BTreeSet::from([edge]);
    let edge_faces = model.edge_faces();
    let planes = |e: EdgeId| -> Option<[prism_math::Plane; 2]> {
        let fs = edge_faces.get(&e)?;
        let [f, g] = fs.as_slice() else { return None };
        Some([*model.face(*f)?.plane()?, *model.face(*g)?.plane()?])
    };
    let tol = model.tol();
    let same_plane = |p: &prism_math::Plane, q: &prism_math::Plane| {
        p.normal.dot(q.normal) > 1.0 - prism_math::tol::ANGLE
            && tol.is_zero((q.origin - p.origin).dot(p.normal))
    };
    let same_pair = |a: &[prism_math::Plane; 2], b: &[prism_math::Plane; 2]| {
        (same_plane(&a[0], &b[0]) && same_plane(&a[1], &b[1]))
            || (same_plane(&a[0], &b[1]) && same_plane(&a[1], &b[0]))
    };
    let Some(e0) = model.edge(edge) else {
        return out;
    };
    let Some(pair) = planes(edge) else {
        return out;
    };
    if !matches!(e0.curve, prism_geom::Curve::Line) {
        return out;
    }
    let (a0, b0) = (model.vertex(e0.a), model.vertex(e0.b));
    let dir = (b0 - a0).normalize_or_zero();
    let on_line = |p: DVec3| {
        let w = p - a0;
        tol.is_zero((w - dir * w.dot(dir)).length())
    };
    for start in [e0.a, e0.b] {
        let (mut v, mut from) = (start, edge);
        loop {
            let next: Vec<EdgeId> = model
                .edges()
                .filter(|(id, x)| {
                    *id != from
                        && !out.contains(id)
                        && x.has(v)
                        && matches!(x.curve, prism_geom::Curve::Line)
                        && on_line(model.vertex(x.a))
                        && on_line(model.vertex(x.b))
                        && planes(*id).is_some_and(|p| same_pair(&p, &pair))
                })
                .map(|(id, _)| id)
                .collect();
            let [n] = next.as_slice() else { break };
            let x = model.edge(*n).expect("an edge");
            out.insert(*n);
            v = if x.a == v { x.b } else { x.a };
            from = *n;
        }
    }
    out
}

/// A size that fits every edge of `edges`: the smallest of the straight
/// edges' own fits (curved edges have none of their own and take it too).
pub fn fit(model: &Model, edges: &BTreeSet<EdgeId>) -> Option<f64> {
    edges
        .iter()
        .filter_map(|e| crate::round::default_size(model, *e))
        .min_by(f64::total_cmp)
}
