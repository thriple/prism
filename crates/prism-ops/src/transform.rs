//! Moving, rotating and scaling vertices.
//!
//! A planar face owns its plane. After a transform each touched face gets a
//! refitted plane; if its vertices no longer lie on one plane, it is split
//! along a diagonal into planar pieces (folded), never triangulated as a
//! whole. Then vertices that ended up in the same place are welded.
//!
//! Arcs move with their ends and stay exact (Exact Curves part 2): a move,
//! turn or even scale of both ends moves the arc's centre, turns its axis
//! and scales its radius. A transform that would bend an arc (one end
//! moving, or an uneven scale) is refused for now: it changes nothing.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::Surface;
use prism_math::{DAffine3, DQuat, DVec3, Plane, Tol};
use prism_topo::{Face, FaceId, Model, VertexId};

pub fn translate(model: &mut Model, vs: &BTreeSet<VertexId>, d: DVec3) {
    apply(model, vs, &DAffine3::from_translation(d));
}

/// The turn about the line through `centre` along `axis`, by `angle`.
pub fn turn(centre: DVec3, axis: DVec3, angle: f64) -> DAffine3 {
    let q = DQuat::from_axis_angle(axis.normalize(), angle);
    DAffine3::from_translation(centre)
        * DAffine3::from_quat(q)
        * DAffine3::from_translation(-centre)
}

/// Rotate about the line through `centre` along `axis`, by `angle` radians.
pub fn rotate(model: &mut Model, vs: &BTreeSet<VertexId>, centre: DVec3, axis: DVec3, angle: f64) {
    apply(model, vs, &turn(centre, axis, angle));
}

/// Scale about `centre` by `k` along each axis.
pub fn scale(model: &mut Model, vs: &BTreeSet<VertexId>, centre: DVec3, k: DVec3) {
    apply(model, vs, &scaling(centre, k));
}

/// The scale about `centre` by `k` along each axis.
pub fn scaling(centre: DVec3, k: DVec3) -> DAffine3 {
    DAffine3::from_translation(centre)
        * DAffine3::from_scale(k)
        * DAffine3::from_translation(-centre)
}

/// Said when a move would bend an arc.
pub const ARC_WOULD_BEND: &str =
    "Refused: an arc would bend. Move its whole curve, or scale it evenly.";
/// Said when a move would fold a face with arcs.
pub const ARC_FACE_WOULD_FOLD: &str =
    "Refused: a face with arcs would fold. Keep it flat, or move all of it.";

/// Said when a move would fold a face with holes.
pub const HOLE_FACE_WOULD_FOLD: &str =
    "Refused: a face with a hole would fold. Keep it flat, or move all of it.";

/// Said when a move would bend a curved face.
pub const CURVED_WOULD_BEND: &str = "Refused: a curved face would bend. Move all of it.";

/// Whether moving `vs` by `m` keeps every arc an arc and its faces flat:
/// each arc with an end in `vs` has both there, `m` scales evenly if any
/// does, and a face with arcs (or with holes and an arc's end) that moves
/// only in part stays in its plane (such a face cannot be folded into flat
/// pieces yet).
pub fn arcs_follow(model: &Model, vs: &BTreeSet<VertexId>, m: &DAffine3) -> bool {
    arcs_refusal(model, vs, m).is_none()
}

/// Why moving `vs` by `m` is refused ([`arcs_follow`]), if it is: what the
/// tools say.
pub fn arcs_refusal(model: &Model, vs: &BTreeSet<VertexId>, m: &DAffine3) -> Option<&'static str> {
    let arcs_ok = model.edges().all(|(_, e)| {
        let (a, b) = (vs.contains(&e.a), vs.contains(&e.b));
        !e.curve.is_arc() || (!a && !b) || (a && b && e.curve.transformed(m).is_some())
    });
    if !arcs_ok {
        return Some(ARC_WOULD_BEND);
    }
    // Curved faces move whole, or not at all (Exact Curves 4a).
    let curved_bends = model.faces().any(|(_, f)| {
        f.is_curved() && {
            let n = f.loops().flatten().filter(|v| vs.contains(v)).count();
            n > 0 && n < f.loops().flatten().count()
        }
    });
    if curved_bends {
        return Some(CURVED_WOULD_BEND);
    }
    let tol = model.tol();
    // Arc ends: a face with holes that bends is flattened by moving its
    // points (`settle_face`), which would take these off their circles.
    let arc_ends: BTreeSet<VertexId> = model
        .edges()
        .filter(|(_, e)| e.curve.is_arc())
        .flat_map(|(_, e)| [e.a, e.b])
        .collect();
    let flat = model.faces().all(|(id, f)| {
        let moved: Vec<VertexId> = f
            .loops()
            .flatten()
            .copied()
            .filter(|v| vs.contains(v))
            .collect();
        let touches_arcs = model.face_has_arcs(id)
            || (!f.holes.is_empty() && f.loops().flatten().any(|v| arc_ends.contains(v)));
        !touches_arcs
            || moved.is_empty()
            || moved.len() == f.loops().flatten().count()
            || f.plane().is_some_and(|pl| {
                moved
                    .iter()
                    .all(|v| pl.contains(m.transform_point3(model.vertex(*v)), &tol))
            })
    });
    if !flat {
        return Some(ARC_FACE_WOULD_FOLD);
    }
    // A face with holes that would bend cannot be folded into flat pieces
    // yet; flattening it would move points it shares with its neighbours
    // (the ring inside a hole), bending them in turn.
    let holes_flat = model.faces().all(|(_, f)| {
        let n = f.loops().flatten().filter(|v| vs.contains(v)).count();
        f.holes.is_empty() || n == 0 || n == f.loops().flatten().count() || {
            let after: Vec<DVec3> = f
                .loops()
                .flatten()
                .map(|v| {
                    let p = model.vertex(*v);
                    if vs.contains(v) {
                        m.transform_point3(p)
                    } else {
                        p
                    }
                })
                .collect();
            Plane::fit(&after, &tol).is_some_and(|pl| after.iter().all(|q| pl.contains(*q, &tol)))
        }
    });
    (!holes_flat).then_some(HOLE_FACE_WOULD_FOLD)
}

/// Send the arcs with both ends in `vs` through `m`.
fn move_arcs(model: &mut Model, vs: &BTreeSet<VertexId>, m: &DAffine3) {
    let arcs: Vec<(prism_topo::EdgeId, prism_geom::Curve)> = model
        .edges()
        .filter(|(_, e)| e.curve.is_arc() && vs.contains(&e.a) && vs.contains(&e.b))
        .filter_map(|(id, e)| Some((id, e.curve.transformed(m)?)))
        .collect();
    for (id, c) in arcs {
        model.set_curve(id, c);
    }
}

/// Move vertices by `d` without the coplanar rule afterwards, for edits
/// that make several moves and then run it once ([`settle_at`]).
pub fn translate_only(model: &mut Model, vs: &BTreeSet<VertexId>, d: DVec3) {
    apply_only(model, vs, &DAffine3::from_translation(d));
}

/// [`rotate`] without the coplanar rule afterwards.
pub fn rotate_only(
    model: &mut Model,
    vs: &BTreeSet<VertexId>,
    centre: DVec3,
    axis: DVec3,
    angle: f64,
) {
    apply_only(model, vs, &turn(centre, axis, angle));
}

fn apply(model: &mut Model, vs: &BTreeSet<VertexId>, m: &DAffine3) {
    if !arcs_follow(model, vs, m) {
        return;
    }
    let targets: Vec<(VertexId, DVec3)> = vs
        .iter()
        .map(|v| (*v, m.transform_point3(model.vertex(*v))))
        .collect();
    apply_only(model, vs, m);
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
    move_arcs(model, vs, &DAffine3::from_translation(d));
    let tol = model.tol();
    weld_new(model, vs, &tol);
    for id in faces {
        settle_face(model, id, &tol);
    }
}

fn apply_only(model: &mut Model, vs: &BTreeSet<VertexId>, m: &DAffine3) {
    if !arcs_follow(model, vs, m) {
        return;
    }
    keep_straight(model, vs);
    for v in vs {
        let p = model.vertex(*v);
        model.set_vertex(*v, m.transform_point3(p));
    }
    move_arcs(model, vs, m);
    let tol = model.tol();
    weld(model, &tol);
    let touched: Vec<FaceId> = model
        .faces()
        .filter(|(_, face)| face.loops().flatten().any(|v| vs.contains(v)))
        .map(|(id, _)| id)
        .collect();
    for id in touched {
        if move_curved(model, id, m) {
            continue;
        }
        settle_face(model, id, &tol);
    }
}

/// A curved face moved whole: its surface follows `m` exactly. False for a
/// flat face.
fn move_curved(model: &mut Model, id: FaceId, m: &DAffine3) -> bool {
    let Some(Surface::Cylinder(c)) = model.face(id).map(|f| f.surface) else {
        return false;
    };
    let moved = c.transformed(m).unwrap_or_else(|| {
        // Uneven: the model check reports it off its surface until the
        // segment fallback (DECISIONS #32).
        let k = m.matrix3.determinant().abs().cbrt();
        prism_geom::Cylinder {
            origin: m.transform_point3(c.origin),
            axis: m.transform_vector3(c.axis).normalize(),
            radius: c.radius * k,
            outward: c.outward != (m.matrix3.determinant() < 0.0),
        }
    });
    model.edit_face(id, |f| f.surface = Surface::Cylinder(moved));
    true
}

/// Refit a face's plane after its vertices moved, or fold it into planar
/// pieces if they no longer lie on one plane.
fn settle_face(model: &mut Model, id: FaceId, tol: &Tol) {
    let Some(face) = model.face(id).cloned() else {
        return;
    };
    if face.is_curved() {
        // Curved faces move whole (`move_curved`) or stretch along their
        // axis (Push/Pull of a cylinder's cap): the surface stays.
        return;
    }
    let outer = model.loop_points(&face, 0);
    let Some(plane) = Plane::fit(&outer, tol) else {
        model.remove_face(id);
        return;
    };
    if face
        .edge_ids()
        .any(|e| model.edge(e).is_some_and(|x| x.curve.is_arc()))
    {
        // Faces with arcs only move whole or within their plane
        // (`arcs_follow`), so they stay flat: refit the plane.
        model.edit_face(id, |f| f.surface = Surface::Plane(plane));
        return;
    }
    let all_on = face
        .loops()
        .flatten()
        .all(|v| plane.contains(model.vertex(*v), tol));
    if all_on {
        model.edit_face(id, |f| f.surface = Surface::Plane(plane));
        return;
    }
    if !face.holes.is_empty() {
        // A face with holes cannot be folded along one diagonal; put its
        // vertices back on the best-fit plane instead.
        for v in face.loops().flatten() {
            let p = model.vertex(*v);
            model.set_vertex(*v, plane.project(p));
        }
        model.edit_face(id, |f| f.surface = Surface::Plane(plane));
        return;
    }
    model.remove_face(id);
    for piece in fold(model, &face.outer, tol) {
        if let Some(pl) = Plane::fit(&model.points(&piece), tol) {
            for w in 0..piece.len() {
                model.add_edge(piece[w], piece[(w + 1) % piece.len()]);
            }
            model.add_face(Face::new(piece, vec![], Surface::Plane(pl)));
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
            let e = model.edge(id).copied();
            model.remove_edge(id);
            let made = match e {
                Some(e) if e.curve.is_arc() => model.add_curve_edge(ma, mb, e.curve, e.curve_id),
                _ => model.add_edge(ma, mb),
            };
            if let (Some(id), Some(e)) = (made, e) {
                model.set_soft(id, e.soft, e.smooth);
            }
        }
    }
    let ids: Vec<FaceId> = model.faces().map(|(id, _)| id).collect();
    // Faces that became the same: facing the same way, one stays; back to
    // back (a box's top pushed down onto its bottom), both go.
    let mut seen: BTreeMap<Vec<VertexId>, (FaceId, Vec<VertexId>)> = BTreeMap::new();
    let mut gone: Vec<FaceId> = Vec::new();
    for id in ids {
        model.edit_face(id, |f| {
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
            // Same outline and same holes: the same face.
            let mut key = f.outer.clone();
            key.sort();
            let mut holes: Vec<Vec<VertexId>> = f
                .holes
                .iter()
                .map(|h| {
                    let mut h = h.clone();
                    h.sort();
                    h
                })
                .collect();
            holes.sort();
            for h in holes {
                key.push(VertexId(u32::MAX));
                key.extend(h);
            }
            if f.outer.len() < 3 {
                gone.push(id);
            } else if let Some((other, loop_)) = seen.get(&key) {
                if !same_direction(loop_, &f.outer) {
                    gone.push(*other);
                }
                gone.push(id);
            } else {
                seen.insert(key, (id, f.outer.clone()));
            }
        });
    }
    for id in gone {
        model.remove_face(id);
    }
    for v in into.keys() {
        model.remove_vertex(*v);
    }
}

/// Whether two loops with the same vertices run the same way round.
fn same_direction(a: &[VertexId], b: &[VertexId]) -> bool {
    let n = a.len();
    if n != b.len() || n == 0 {
        return false;
    }
    let Some(start) = b.iter().position(|v| *v == a[0]) else {
        return false;
    };
    (0..n).all(|i| a[i] == b[(start + i) % n])
}

/// Map a whole model by an affine transform (placing a group's geometry in
/// its parent, Explode). Faces get their planes refitted; a mirroring
/// transform turns every loop round, so faces still face out.
pub fn affine(model: &mut Model, m: &prism_math::DAffine3) {
    let verts: Vec<(VertexId, DVec3)> = model.vertices().collect();
    for (v, p) in verts {
        model.set_vertex(v, m.transform_point3(p));
    }
    let guides: Vec<(prism_topo::GuideId, prism_topo::Guide)> =
        model.guides().map(|(id, g)| (id, *g)).collect();
    for (id, g) in guides {
        let moved = match g {
            prism_topo::Guide::Line { origin, dir } => prism_topo::Guide::Line {
                origin: m.transform_point3(origin),
                dir: m.transform_vector3(dir).normalize(),
            },
            prism_topo::Guide::Point { at } => prism_topo::Guide::Point {
                at: m.transform_point3(at),
            },
        };
        model.remove_guide(id);
        model.add_guide(moved);
    }
    // Arcs follow. Under an uneven scale an arc would become an ellipse:
    // it keeps its moved centre, its turned axis and its radius scaled by
    // the average stretch, and the model check reports its ends off it
    // until the segment fallback (DECISIONS #32, after Exact Curves).
    let all: BTreeSet<VertexId> = model.vertices().map(|(id, _)| id).collect();
    move_arcs(model, &all, m);
    let uneven: Vec<(prism_topo::EdgeId, prism_geom::Curve)> = model
        .edges()
        .filter_map(|(id, e)| match e.curve {
            prism_geom::Curve::Arc {
                centre,
                axis,
                radius,
            } if e.curve.transformed(m).is_none() => {
                let k = m.matrix3.determinant().abs().cbrt();
                let flip = m.matrix3.determinant().signum();
                let n = (m.matrix3.inverse().transpose() * axis).normalize() * flip;
                Some((
                    id,
                    prism_geom::Curve::Arc {
                        centre: m.transform_point3(centre),
                        axis: n,
                        radius: radius * k,
                    },
                ))
            }
            _ => None,
        })
        .collect();
    for (id, c) in uneven {
        model.set_curve(id, c);
    }
    let mirror = m.matrix3.determinant() < 0.0;
    let tol = model.tol();
    let ids: Vec<FaceId> = model.faces().map(|(id, _)| id).collect();
    for id in ids {
        if mirror {
            model.edit_face(id, |f| f.reverse());
        }
        if move_curved(model, id, m) {
            continue;
        }
        let Some(face) = model.face(id) else {
            continue;
        };
        let Some(plane) = Plane::fit(&model.loop_points(face, 0), &tol) else {
            model.remove_face(id);
            continue;
        };
        model.edit_face(id, |f| f.surface = Surface::Plane(plane));
    }
}

/// A moving point that only splits a straight edge of a face that stays
/// put leaves that edge straight: the face keeps a copy of the point where
/// it was, and only faces that turn a corner at the point follow it (owner
/// feedback, 2026-11-09: moving the top of a box set in a corner bent the
/// neighbours' corner edges). A point that is straight in every face it
/// is in (a point picked on an edge) still bends the edge, as before, and
/// so does one where the turning faces use both halves of the edge.
fn keep_straight(model: &mut Model, vs: &BTreeSet<VertexId>) {
    let tol = model.tol();
    // Faces running straight through each moving point (with the points on
    // either side), and the points next to it in faces that turn there.
    let mut straight: BTreeMap<VertexId, Vec<(FaceId, VertexId, VertexId)>> = BTreeMap::new();
    let mut corner: BTreeMap<VertexId, BTreeSet<VertexId>> = BTreeMap::new();
    // An arc's end is never a point on a straight edge.
    let on_arc: BTreeSet<VertexId> = model
        .edges()
        .filter(|(_, e)| e.curve.is_arc())
        .flat_map(|(_, e)| [e.a, e.b])
        .collect();
    for (id, face) in model.faces() {
        for l in face.loops() {
            let n = l.len();
            for i in 0..n {
                let v = l[i];
                if !vs.contains(&v) {
                    continue;
                }
                let (a, b) = (l[(i + n - 1) % n], l[(i + 1) % n]);
                let through = !on_arc.contains(&v)
                    && !vs.contains(&a)
                    && !vs.contains(&b)
                    && crate::sticky::on_segment_inside(
                        model.vertex(v),
                        model.vertex(a),
                        model.vertex(b),
                        &tol,
                    )
                    .is_some();
                if through {
                    straight.entry(v).or_default().push((id, a, b));
                } else {
                    corner.entry(v).or_default().extend([a, b]);
                }
            }
        }
    }
    for (v, faces) in straight {
        // Only where the turning faces use at most one half of the straight
        // edge: if they use both, the edge is part of what moves.
        let Some(turning) = corner.get(&v) else {
            continue;
        };
        if faces
            .iter()
            .any(|(_, a, b)| turning.contains(a) && turning.contains(b))
        {
            continue;
        }
        let faces: Vec<FaceId> = faces.iter().map(|f| f.0).collect();
        let w = model.add_vertex(model.vertex(v));
        let mut pairs = Vec::new();
        for id in &faces {
            model.edit_face(*id, |face| {
                for l in std::iter::once(&mut face.outer).chain(face.holes.iter_mut()) {
                    let n = l.len();
                    for i in 0..n {
                        if l[i] == v {
                            l[i] = w;
                            pairs.push((l[(i + n - 1) % n], l[(i + 1) % n]));
                        }
                    }
                }
            });
        }
        for (a, b) in pairs {
            for x in [a, b] {
                model.add_edge(x, w);
                // The old edge goes unless a face that follows still runs
                // along it.
                if let Some(e) = model.edge_between(x, v)
                    && !runs_along(model, x, v)
                {
                    model.remove_edge(e);
                }
            }
        }
        // The new edges are there now.
        for id in faces {
            model.relink_face(id);
        }
    }
}

/// Whether some face's loop has `a` and `b` next to each other.
fn runs_along(model: &Model, a: VertexId, b: VertexId) -> bool {
    model.faces().any(|(_, f)| {
        f.loops().any(|l| {
            let n = l.len();
            (0..n).any(|i| {
                let (p, q) = (l[i], l[(i + 1) % n]);
                (p, q) == (a, b) || (p, q) == (b, a)
            })
        })
    })
}
