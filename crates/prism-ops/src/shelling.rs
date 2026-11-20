//! Shell / Thicken (brief 42, part 3a): faces become a solid of a set
//! thickness.
//!
//! The faces joined to the ones given by edges are taken whole: an open
//! sheet becomes a slab, and a closed body becomes hollow, less the faces
//! left open (a box becomes a tray). Each face moves along its own normal
//! by the thickness (out for more than zero, in for less), and a corner
//! goes where the moved faces meet, so the walls are the same thickness
//! everywhere. Where an edge has a face on one side only (a sheet's rim, or
//! the edge of an open face), a wall joins it to its moved copy: flat for a
//! line (two flat triangles if it cannot be), and for an arc a cylinder, a
//! cone or a flat ring.
//!
//! Curved faces (part 3c): cylinders, cones, spheres and tori move to the
//! same kind of surface (a cylinder of a radius the thickness more or
//! less), and their arcs to arcs; a point on an edge moves the shortest
//! way onto every moved face it is on. A freeform face on its own becomes
//! the slab Push/Pull makes of it (`thicken`). Corners of up to three
//! surfaces' directions are exact.
//!
//! Joined freeform faces (brief 49 part 1): a body of two or more
//! freeform faces, joined only where they meet smoothly (the same normal
//! along the shared edge, so each side's offset already meets the
//! other's there), offsets each face on its own and shares the fitted
//! offset of a join between its two faces.
//!
//! Creases (part 2): a body may also mix flat (and other analytic) faces
//! with freeform ones. Where two kept faces join at an angle, their
//! offsets no longer meet by themselves: the true mitre is found by
//! marching along the shared edge and, at each point, solving for where
//! both offsets meet exactly (Newton, in the plane of their two normals,
//! as `corner` does for an exact flat corner). An inside corner (the
//! mitre trims back within each face's own edge) is cut this way. An
//! outside corner (part 2b: the mitre would reach past a freeform face's
//! own edge, which does not extend) gets no single mitre; each face
//! keeps its own straight offset edge instead, and a small face bridges
//! the gap between the two. A fold too sharp to mitre at all (close to
//! doubling back on itself) is refused by name.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Cone, Curve, Nurbs, Surface};
use prism_math::tol::ANGLE;
use prism_math::{DVec3, Plane, Tol, newell};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

pub const NOTHING: &str = "Refused: click a face to shell or thicken.";
pub const ZERO: &str = "Refused: a thickness of zero makes nothing.";
pub const ALL_OPEN: &str = "Refused: every face is left open, so there is nothing to thicken.";
pub const FREEFORM: &str = "Refused: Shell / Thicken takes a freeform face on its own, but not one joined to other faces, nor faces bounded by freeform curves, yet.";
pub const MIXED: &str =
    "Refused: these faces face different ways. Solid > Heal turns them the same way.";
pub const BRANCHES: &str = "Refused: more than two faces meet at an edge here.";
pub const CORNER: &str = "Refused: more than three faces meet at a corner on the rim here, or round a curved face, and their moved copies do not meet at one point. Not yet.";
pub const TOO_THICK: &str = "Refused: that thickness turns a face inside out. Try a smaller one.";
pub const SHARP: &str = "Refused: these faces fold back too sharply here to cut a mitre. A rounded corner there is not done yet.";

/// The faces joined to `seeds` by edges: their sheets or bodies.
pub fn body(model: &Model, seeds: &BTreeSet<FaceId>) -> BTreeSet<FaceId> {
    let ef = model.edge_faces();
    let mut out = BTreeSet::new();
    let mut todo: Vec<FaceId> = seeds
        .iter()
        .copied()
        .filter(|f| model.face(*f).is_some())
        .collect();
    while let Some(f) = todo.pop() {
        if !out.insert(f) {
            continue;
        }
        let Some(face) = model.face(f) else {
            continue;
        };
        for e in face.edge_ids() {
            todo.extend(
                ef.get(&e)
                    .into_iter()
                    .flatten()
                    .filter(|g| !out.contains(*g)),
            );
        }
    }
    out
}

/// What the shell is made of.
struct Plan {
    body: BTreeSet<FaceId>,
    kept: Vec<FaceId>,
    /// Where each point of a kept face moves to.
    moved: BTreeMap<VertexId, DVec3>,
    /// Corners of four or more faces whose moved copies miss one point
    /// (part 3d): where each edge there ends, round the corner in order. A
    /// small flat face fills the gap between them.
    splits: BTreeMap<VertexId, Vec<(EdgeId, DVec3)>>,
    /// Each kept face's edges moved with it.
    curves: BTreeMap<EdgeId, Curve>,
    /// Edges with a kept face on one side only: the edge, the way that face
    /// walks it, and the face.
    rims: Vec<(EdgeId, VertexId, VertexId, FaceId)>,
}

/// Why `shell` would refuse, if it would.
pub fn refusal(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    thickness: f64,
) -> Option<&'static str> {
    slab(model, faces, open, thickness).err()
}

/// Shell or thicken the bodies of `faces` by `thickness`, leaving `open`
/// open, in one step.
pub fn shell(
    model: &mut Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    thickness: f64,
) -> Result<(), &'static str> {
    let (s, body) = slab(model, faces, open, thickness)?;
    let tol = model.tol();
    crate::thicken::remove_faces(model, &body);
    crate::boolean::join(model, &s, &tol);
    Ok(())
}

/// The new solid on its own, and the faces it replaces.
pub fn slab(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
) -> Result<(Model, BTreeSet<FaceId>), &'static str> {
    if let Some(r) = freeform_alone(model, faces, open, d) {
        return r;
    }
    if let Some(r) = freeform_joined(model, faces, open, d) {
        return r;
    }
    let plan = plan(model, faces, open, d)?;
    let tol = model.tol();
    let mut s = Model::new();
    let mut base: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    let mut up: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for (v, q) in &plan.moved {
        base.insert(*v, s.add_vertex(model.vertex(*v)));
        up.insert(*v, s.add_vertex(*q));
    }
    // At a split corner, each edge ends at its own point (or one it
    // shares with the next).
    let mut ends: BTreeMap<(VertexId, EdgeId), VertexId> = BTreeMap::new();
    for (v, fan) in &plan.splits {
        base.insert(*v, s.add_vertex(model.vertex(*v)));
        let mut made: Vec<(DVec3, VertexId)> = Vec::new();
        for (e, q) in fan {
            let id = match made.iter().find(|(p, _)| tol.same_point(*p, *q)) {
                Some((_, id)) => *id,
                None => {
                    let id = s.add_vertex(*q);
                    made.push((*q, id));
                    id
                }
            };
            ends.insert((*v, *e), id);
        }
    }
    let end = |v: VertexId, e: EdgeId| up.get(&v).copied().unwrap_or_else(|| ends[&(v, e)]);
    // Each edge and its moved copy.
    let mut low_e: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut high_e: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut cids = BTreeMap::new();
    for (id, moved) in &plan.curves {
        let e = model.edge(*id).ok_or(NOTHING)?;
        let cid = e
            .curve_id
            .map(|c| *cids.entry(c).or_insert_with(|| s.new_curve_id()));
        let lo = edge(&mut s, base[&e.a], base[&e.b], e.curve.clone(), cid)?;
        let hi_cid = cid.map(|_| s.new_curve_id());
        let hi = edge(&mut s, end(e.a, *id), end(e.b, *id), moved.clone(), hi_cid)?;
        s.set_soft(lo, e.soft, e.smooth);
        s.set_soft(hi, e.soft, e.smooth);
        low_e.insert(*id, lo);
        high_e.insert(*id, hi);
    }
    let copy = |f: &Face,
                vmap: &BTreeMap<VertexId, VertexId>,
                emap: &BTreeMap<EdgeId, EdgeId>,
                surface: Surface| {
        let map = |l: &Vec<VertexId>| l.iter().map(|v| vmap[v]).collect::<Vec<_>>();
        let edges = (0..=f.holes.len())
            .map(|k| f.loop_edges(k).map(|x| emap[&x.edge]).collect())
            .collect();
        let mut c = Face::with_edges(
            map(&f.outer),
            f.holes.iter().map(map).collect(),
            surface,
            edges,
        );
        c.paint = f.paint;
        c
    };
    // The moved copy of a face: at a split corner it goes from where the
    // edge in ends to where the edge out ends, by a short edge.
    let mut shorts: BTreeMap<(VertexId, FaceId), Option<EdgeId>> = BTreeMap::new();
    let mut high_copy = |s: &mut Model, id: FaceId, f: &Face, surface: Surface| {
        let mut loops: Vec<Vec<VertexId>> = Vec::new();
        let mut edges: Vec<Vec<EdgeId>> = Vec::new();
        for k in 0..=f.holes.len() {
            let sides: Vec<_> = f.loop_edges(k).collect();
            let (mut l, mut es) = (Vec::new(), Vec::new());
            for (i, side) in sides.iter().enumerate() {
                let v = side.from;
                if let Some(u) = up.get(&v) {
                    l.push(*u);
                } else {
                    let prev = sides[(i + sides.len() - 1) % sides.len()].edge;
                    let (a, b) = (ends[&(v, prev)], ends[&(v, side.edge)]);
                    l.push(a);
                    let short = (a != b).then(|| s.add_edge(a, b)).flatten();
                    shorts.insert((v, id), short);
                    if let Some(x) = short {
                        es.push(x);
                        l.push(b);
                    }
                }
                es.push(high_e[&side.edge]);
            }
            loops.push(l);
            edges.push(es);
        }
        let outer = loops.remove(0);
        let mut c = Face::with_edges(outer, loops, surface, edges);
        c.paint = f.paint;
        c
    };
    for id in &plan.kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let moved = offset_surface(&f.surface, d, &tol).ok_or(TOO_THICK)?;
        let mut low = copy(f, &base, &low_e, f.surface.clone());
        let mut high = high_copy(&mut s, *id, f, moved);
        // The solid lies between the face and its copy: whichever is
        // behind the other turns round to face out of it.
        if d > 0.0 {
            low.reverse();
        } else {
            high.reverse();
        }
        s.add_face(low);
        s.add_face(high);
    }
    // The small face at each split corner, walked against the faces round
    // it (as a moved copy is, so it turns round with them).
    for (v, fan) in &plan.splits {
        let mut l: Vec<VertexId> = Vec::new();
        let mut es: Vec<EdgeId> = Vec::new();
        for j in (0..fan.len()).rev() {
            let (e_in, _) = fan[j];
            let (e_out, _) = fan[(j + 1) % fan.len()];
            let Some(f) = plan.kept.iter().copied().find(|f| {
                model.face(*f).is_some_and(|x| {
                    let sides: Vec<_> = x.sides().collect();
                    sides.iter().any(|a| a.to == *v && a.edge == e_in)
                        && sides.iter().any(|b| b.from == *v && b.edge == e_out)
                })
            }) else {
                return Err(CORNER);
            };
            if let Some(Some(x)) = shorts.get(&(*v, f)) {
                l.push(ends[&(*v, e_out)]);
                es.push(*x);
            }
        }
        if l.len() >= 3 {
            let pts: Vec<DVec3> = l.iter().map(|x| s.vertex(*x)).collect();
            let n = newell(&pts).normalize_or_zero();
            let mut cap =
                Face::with_edges(l, vec![], Surface::Plane(Plane::new(pts[0], n)), vec![es]);
            if d < 0.0 {
                cap.reverse();
            }
            s.add_face(cap);
        }
    }
    // A line up from each point of a rim.
    let mut rise: BTreeMap<VertexId, EdgeId> = BTreeMap::new();
    for (_, a, b, _) in &plan.rims {
        for v in [a, b] {
            if !rise.contains_key(v) {
                rise.insert(*v, edge(&mut s, base[v], up[v], Curve::Line, None)?);
            }
        }
    }
    for (id, from, to, face) in &plan.rims {
        let e = model.edge(*id).ok_or(NOTHING)?;
        let f = model.face(*face).ok_or(NOTHING)?;
        // The wall faces out of the solid: to the right of the edge, walked
        // the face's way, about the face's normal.
        let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
        let mid = e.curve.point(pa, pb, 0.5);
        let ahead = (e.curve.point(pa, pb, 0.51) - e.curve.point(pa, pb, 0.49))
            * if *from == e.a { 1.0 } else { -1.0 };
        let out = ahead.cross(f.surface.normal_at(mid));
        // Walked against the copy of the face beside it.
        let (p, q) = if d > 0.0 { (*from, *to) } else { (*to, *from) };
        let lo = low_e[id];
        let hi = high_e[id];
        match &e.curve {
            Curve::Line => {
                let pts = [base[&p], base[&q], up[&q], up[&p]];
                if flat_plane(&s, &pts, &tol).is_some() {
                    let plane = Plane::new(s.vertex(pts[0]), loop_normal(&s, &pts).normalize());
                    s.add_face(Face::with_edges(
                        pts.to_vec(),
                        vec![],
                        Surface::Plane(plane),
                        vec![vec![lo, rise[&q], hi, rise[&p]]],
                    ));
                } else {
                    let diag = edge(&mut s, pts[0], pts[2], Curve::Line, None)?;
                    for (l, es) in [
                        ([pts[0], pts[1], pts[2]], [lo, rise[&q], diag]),
                        ([pts[0], pts[2], pts[3]], [diag, hi, rise[&p]]),
                    ] {
                        let plane = Plane::new(s.vertex(l[0]), loop_normal(&s, &l).normalize());
                        s.add_face(Face::with_edges(
                            l.to_vec(),
                            vec![],
                            Surface::Plane(plane),
                            vec![es.to_vec()],
                        ));
                    }
                }
            }
            Curve::Arc {
                centre,
                axis,
                radius,
            } => {
                let Curve::Arc {
                    centre: c2,
                    radius: r2,
                    ..
                } = plan.curves[id]
                else {
                    return Err(FREEFORM);
                };
                let wall = arc_wall(*centre, *axis, *radius, c2, r2, mid, out, &tol)?;
                s.add_face(Face::with_edges(
                    vec![base[&p], base[&q], up[&q], up[&p]],
                    vec![],
                    wall,
                    vec![vec![lo, rise[&q], hi, rise[&p]]],
                ));
            }
            Curve::Spline(_) => return Err(FREEFORM),
        }
    }
    // A line up between walls that meet smoothly is soft.
    let ef = s.edge_faces();
    for e in rise.values() {
        let Some([f, g]) = ef.get(e).map(|x| x.as_slice()) else {
            continue;
        };
        let at = {
            let x = s.edge(*e).ok_or(TOO_THICK)?;
            (s.vertex(x.a) + s.vertex(x.b)) * 0.5
        };
        let (nf, ng) = (
            s.face(*f).map(|x| x.surface.normal_at(at)),
            s.face(*g).map(|x| x.surface.normal_at(at)),
        );
        if let (Some(a), Some(b)) = (nf, ng)
            && tol.parallel(a, b)
            && a.dot(b) > 0.0
        {
            s.set_soft(*e, true, true);
        }
    }
    if !crate::boolean::is_solid(&s) {
        return Err(TOO_THICK);
    }
    Ok((s, plan.body))
}

/// A freeform face on its own: the slab Push/Pull makes of it.
fn freeform_alone(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
) -> Option<Result<(Model, BTreeSet<FaceId>), &'static str>> {
    let body = body(model, faces);
    let [f] = body.iter().copied().collect::<Vec<_>>()[..] else {
        return None;
    };
    model.face(f)?.surface.patch()?;
    if open.contains(&f) {
        return Some(Err(ALL_OPEN));
    }
    if model.tol().is_zero(d) {
        return Some(Err(ZERO));
    }
    Some(crate::thicken::slab(model, f, d).map(|s| (s.model, body)))
}

/// A body of two or more faces, at least one of them freeform, joined
/// smoothly or at a crease (brief 49 parts 1 and 2): each face is offset
/// on its own (a freeform face as `thicken::offset_surface` fits it, a
/// flat or other analytic face exactly); where two kept faces share an
/// edge smoothly, their offsets already meet there; where they meet at an
/// angle, the mitre is found (`crease_curve`). A rim (an edge with one
/// kept face, at a free boundary or an open face) gets a ruled wall, cut
/// square across. A body with no freeform face at all is left to `plan`,
/// which already does flat and curved analytic faces exactly.
fn freeform_joined(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
) -> Option<Result<(Model, BTreeSet<FaceId>), &'static str>> {
    let body = body(model, faces);
    if body.len() < 2 {
        return None;
    }
    if body.iter().any(|f| model.face(*f).is_none()) {
        return None;
    }
    if !body
        .iter()
        .any(|f| model.face(*f).is_some_and(|x| x.surface.patch().is_some()))
    {
        return None;
    }
    Some(freeform_slab(model, &body, open, d))
}

/// Whether the two faces at an interior edge meet smoothly: their normals
/// agree along the edge, sampled at several points (not just the
/// middle), so each face's offset point there is the same.
fn smooth_join(model: &Model, e: EdgeId, fa: FaceId, fb: FaceId, tol: &Tol) -> bool {
    let (Some(edge), Some(a), Some(b)) = (model.edge(e), model.face(fa), model.face(fb)) else {
        return false;
    };
    let (pa, pb) = (model.vertex(edge.a), model.vertex(edge.b));
    (1..8).all(|i| {
        let p = edge.curve.point(pa, pb, i as f64 / 8.0);
        let (na, nb) = (a.surface.normal_at(p), b.surface.normal_at(p));
        tol.parallel(na, nb) && na.dot(nb) > 0.0
    })
}

/// The slab of a joined body with at least one freeform face: each kept
/// face offset on its own, sharing an offset edge with its neighbour
/// (smoothly, or mitred at a crease), walled at its rims.
fn freeform_slab(
    model: &Model,
    body: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
) -> Result<(Model, BTreeSet<FaceId>), &'static str> {
    use prism_geom::{Patch, Spline};
    let tol = model.tol();
    if !d.is_finite() || tol.is_zero(d) {
        return Err(ZERO);
    }
    let kept: Vec<FaceId> = body.iter().copied().filter(|f| !open.contains(f)).collect();
    if kept.is_empty() {
        return Err(ALL_OPEN);
    }
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        if f.surface.patch().is_some()
            && crate::crossing::Laid::new(model, *id).is_none_or(|l| l.wraps())
        {
            return Err(crate::thicken::ROUND);
        }
    }
    // How the body's faces use each edge, as the analytic plan() does.
    let mut uses: BTreeMap<EdgeId, Vec<(bool, VertexId, VertexId, FaceId)>> = BTreeMap::new();
    for id in body {
        let f = model.face(*id).ok_or(NOTHING)?;
        let keep = !open.contains(id);
        for side in f.sides() {
            uses.entry(side.edge)
                .or_default()
                .push((keep, side.from, side.to, *id));
        }
    }
    let mut rims: Vec<(EdgeId, VertexId, VertexId, FaceId)> = Vec::new();
    let mut driver: BTreeMap<EdgeId, FaceId> = BTreeMap::new();
    let mut creases: Vec<(EdgeId, FaceId, FaceId)> = Vec::new();
    for (e, u) in &uses {
        if u.len() > 2 {
            return Err(BRANCHES);
        }
        let kept_uses: Vec<_> = u.iter().filter(|x| x.0).collect();
        match kept_uses.as_slice() {
            [one] => {
                rims.push((*e, one.1, one.2, one.3));
                driver.insert(*e, one.3);
            }
            [x, y] => {
                if x.1 == y.1 {
                    return Err(MIXED);
                }
                if smooth_join(model, *e, x.3, y.3, &tol) {
                    driver.insert(*e, x.3);
                } else {
                    creases.push((*e, x.3, y.3));
                }
            }
            _ => {}
        }
    }
    let mut s = Model::new();
    let eps = tol.dist * 0.1;
    // Each kept face's own offset surface: a freeform one fitted (its
    // own edge still bounds it), a flat or other analytic one exact and
    // already unbounded.
    let mut offsets: BTreeMap<FaceId, Surface> = BTreeMap::new();
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let surface = if let Some(patch) = f.surface.patch() {
            let laid = crate::crossing::Laid::new(model, *id).ok_or(TOO_THICK)?;
            let top =
                crate::thicken::offset_surface(patch, &laid.polys[0], d, eps).ok_or(TOO_THICK)?;
            Surface::Nurbs(Patch {
                flipped: patch.flipped,
                ..Patch::whole(top)
            })
        } else {
            offset_surface(&f.surface, d, &tol).ok_or(TOO_THICK)?
        };
        offsets.insert(*id, surface);
    }
    // Where each point a kept face reaches moves to: the exact corner of
    // every kept face there (`plan`'s own move_point, which already works
    // on any surface, flat or freeform).
    let mut at_vertex: BTreeMap<VertexId, Vec<FaceId>> = BTreeMap::new();
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        for v in f.loops().flatten() {
            let fs = at_vertex.entry(*v).or_default();
            if !fs.contains(id) {
                fs.push(*id);
            }
        }
    }
    let mut base: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    let mut up: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    let mut moved: BTreeMap<VertexId, DVec3> = BTreeMap::new();
    for (v, fs) in &at_vertex {
        let p = model.vertex(*v);
        base.insert(*v, s.add_vertex(p));
        let q = move_point(model, fs, p, d, &tol)?;
        moved.insert(*v, q);
        up.insert(*v, s.add_vertex(q));
    }
    // Too sharp a fold has no mitre (close to doubling back on itself):
    // checked for every crease up front, at the same points `smooth_join`
    // samples, so the reason is specific. The rest are classified inside
    // or outside (part 2b): an outside corner's mitre would reach past
    // one face's own edge, which a NURBS surface cannot extend to meet.
    let mut outside: BTreeMap<EdgeId, bool> = BTreeMap::new();
    for (e, fa, fb) in &creases {
        let edge_data = model.edge(*e).ok_or(NOTHING)?;
        let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
        let along = |t: f64| edge_data.curve.point(pa, pb, t);
        let (face_a, face_b) = (
            model.face(*fa).ok_or(NOTHING)?,
            model.face(*fb).ok_or(NOTHING)?,
        );
        let sharp = (0..=8).any(|i| {
            let p = along(i as f64 / 8.0);
            let ns = unique_normals(
                &[face_a.surface.normal_at(p), face_b.surface.normal_at(p)],
                &tol,
            );
            corner(&ns, d, &tol).is_none()
        });
        if sharp {
            return Err(SHARP);
        }
        let (off_a, off_b) = (&offsets[fa], &offsets[fb]);
        let raw_meet = |p: DVec3| {
            let ns = unique_normals(
                &[face_a.surface.normal_at(p), face_b.surface.normal_at(p)],
                &tol,
            );
            let guess = corner(&ns, d, &tol).map_or(p, |x| p + x);
            meet(off_a, off_b, guess, &tol)
        };
        let extends = (0..=8).any(|i| {
            let x = raw_meet(along(i as f64 / 8.0));
            !truly_on(off_a, x, &tol) || !truly_on(off_b, x, &tol)
        });
        outside.insert(*e, extends);
    }
    // At an outside corner's end, where only the two creased faces meet,
    // they no longer reach one shared point: each gets its own there,
    // the plain offset of that end along its own normal (there is
    // nothing else at a corner like that to correct it towards). Where a
    // third face also meets there, bridging it is not done yet (as more
    // than three faces at a corner on the rim already is): refused by
    // the same name, rather than built wrong.
    let mut split: BTreeMap<(VertexId, FaceId), VertexId> = BTreeMap::new();
    let mut split_pt: BTreeMap<(VertexId, FaceId), DVec3> = BTreeMap::new();
    for (e, fa, fb) in &creases {
        if !outside[e] {
            continue;
        }
        let edge_data = model.edge(*e).ok_or(NOTHING)?;
        for v in [edge_data.a, edge_data.b] {
            if at_vertex[&v].len() != 2 {
                return Err(CORNER);
            }
            let p = model.vertex(v);
            for f in [*fa, *fb] {
                let q = p + model.face(f).ok_or(NOTHING)?.surface.normal_at(p) * d;
                split_pt.insert((v, f), q);
                split.insert((v, f), s.add_vertex(q));
            }
        }
    }
    let vertex_for = |v: VertexId, f: FaceId| split.get(&(v, f)).copied().unwrap_or(up[&v]);
    let point_for = |v: VertexId, f: FaceId| split_pt.get(&(v, f)).copied().unwrap_or(moved[&v]);
    // Each original edge's offset is fitted once, shared by every face
    // that uses it: a rim or a smooth join by whichever kept face drives
    // it, a crease by solving where its two faces' offsets truly meet
    // (or, at an outside corner, by each face on its own: part 2b).
    let mut bottom_of: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut top_of: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut fitted: BTreeMap<EdgeId, (Nurbs, Nurbs)> = BTreeMap::new();
    // At an outside corner (part 2b), the two faces either side of a
    // crease keep their own top edge rather than share a mitre: looked up
    // by the edge and the face using it, falling back to `top_of` (the
    // usual, shared top edge) everywhere else.
    let mut crease_top: BTreeMap<(EdgeId, FaceId), EdgeId> = BTreeMap::new();
    // An outside corner's own bridge, finished once the rims below have
    // given it the risers it closes against.
    let mut bridges: Vec<Bridge> = Vec::new();
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        for side in f.sides() {
            if bottom_of.contains_key(&side.edge) || driver.get(&side.edge) != Some(id) {
                continue;
            }
            let e = model.edge(side.edge).ok_or(NOTHING)?;
            let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
            let along = |t: f64| e.curve.point(pa, pb, t);
            let naive = |p: DVec3| p + f.surface.normal_at(p) * d;
            // Corrected to meet the vertex's true corner at each end (it
            // may be mitred with a face this edge does not touch, or be
            // this face's own corner of an outside crease); blended
            // smoothly across the edge, zero where there is no mitre.
            let (corr_a, corr_b) = (
                point_for(e.a, *id) - naive(pa),
                point_for(e.b, *id) - naive(pb),
            );
            let span = (pb - pa).length_squared().max(1e-300);
            let off = |p: DVec3| {
                let t = ((p - pa).dot(pb - pa) / span).clamp(0.0, 1.0);
                naive(p) + corr_a * (1.0 - t) + corr_b * t
            };
            let breaks = match f.surface.patch() {
                Some(patch) => crate::thicken::kinks(&e.curve, &along, patch),
                None => vec![0.0, 1.0],
            };
            let (low, high) =
                crate::thicken::edge_pair(&along, &off, &breaks, eps).ok_or(TOO_THICK)?;
            let b = edge(&mut s, base[&e.a], base[&e.b], e.curve.clone(), None)?;
            let t = edge(
                &mut s,
                vertex_for(e.a, *id),
                vertex_for(e.b, *id),
                Curve::Spline(Spline::whole(high.clone())),
                None,
            )?;
            bottom_of.insert(side.edge, b);
            top_of.insert(side.edge, t);
            fitted.insert(side.edge, (low, high));
        }
    }
    for (e, fa, fb) in &creases {
        let edge_data = model.edge(*e).ok_or(NOTHING)?;
        let (pa, pb) = (model.vertex(edge_data.a), model.vertex(edge_data.b));
        let along = |t: f64| edge_data.curve.point(pa, pb, t);
        let (face_a, face_b) = (
            model.face(*fa).ok_or(NOTHING)?,
            model.face(*fb).ok_or(NOTHING)?,
        );
        let (off_a, off_b) = (&offsets[fa], &offsets[fb]);
        let span = (pb - pa).length_squared().max(1e-300);
        let mut breaks = match face_a.surface.patch() {
            Some(patch) => crate::thicken::kinks(&edge_data.curve, &along, patch),
            None => vec![0.0, 1.0],
        };
        if let Some(patch) = face_b.surface.patch() {
            breaks.extend(crate::thicken::kinks(&edge_data.curve, &along, patch));
            breaks.sort_by(f64::total_cmp);
            breaks.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
        }
        if outside[e] {
            // An outside corner (part 2b): each face keeps its own
            // straight offset edge instead of a mitre that would reach
            // past one of them, bridged by a small face between the two
            // once the rims below give it the risers it closes against.
            let corrected = |surface: &Surface, face_id: FaceId, p: DVec3| -> DVec3 {
                let naive = |q: DVec3| q + surface.normal_at(q) * d;
                let t = ((p - pa).dot(pb - pa) / span).clamp(0.0, 1.0);
                let (ea, eb) = (
                    point_for(edge_data.a, face_id),
                    point_for(edge_data.b, face_id),
                );
                naive(p) + (ea - naive(pa)) * (1.0 - t) + (eb - naive(pb)) * t
            };
            let fn_a = |t: f64| corrected(&face_a.surface, *fa, along(t));
            let fn_b = |t: f64| corrected(&face_b.surface, *fb, along(t));
            let (high_a, high_b) =
                crate::thicken::twin_fit(&fn_a, &fn_b, &breaks, eps).ok_or(TOO_THICK)?;
            let b = edge(
                &mut s,
                base[&edge_data.a],
                base[&edge_data.b],
                edge_data.curve.clone(),
                None,
            )?;
            let ta = edge(
                &mut s,
                vertex_for(edge_data.a, *fa),
                vertex_for(edge_data.b, *fa),
                Curve::Spline(Spline::whole(high_a.clone())),
                None,
            )?;
            let tb = edge(
                &mut s,
                vertex_for(edge_data.a, *fb),
                vertex_for(edge_data.b, *fb),
                Curve::Spline(Spline::whole(high_b.clone())),
                None,
            )?;
            bottom_of.insert(*e, b);
            crease_top.insert((*e, *fa), ta);
            crease_top.insert((*e, *fb), tb);
            // The bridge faces the way both faces do (their un-offset
            // normals, which still point away from the material on
            // either side of the gap).
            let out_ref = {
                let mid = along(0.5);
                (face_a.surface.normal_at(mid) + face_b.surface.normal_at(mid)).normalize_or_zero()
            };
            bridges.push(Bridge {
                fa: *fa,
                fb: *fb,
                ea: edge_data.a,
                eb: edge_data.b,
                ta,
                tb,
                high_a,
                high_b,
                out_ref,
            });
            continue;
        }
        // Corrected to meet the vertex's true corner at each end (it may
        // involve a third face this crease does not touch); blended
        // smoothly across the edge, zero where there is no such face.
        let raw_meet = |p: DVec3| {
            let ns = unique_normals(
                &[face_a.surface.normal_at(p), face_b.surface.normal_at(p)],
                &tol,
            );
            let guess = corner(&ns, d, &tol).map_or(p, |x| p + x);
            meet(off_a, off_b, guess, &tol)
        };
        let (corr_a, corr_b) = (
            moved[&edge_data.a] - raw_meet(pa),
            moved[&edge_data.b] - raw_meet(pb),
        );
        let meet_at = |t: f64| -> DVec3 {
            let p = along(t);
            let bt = ((p - pa).dot(pb - pa) / span).clamp(0.0, 1.0);
            raw_meet(p) + corr_a * (1.0 - bt) + corr_b * bt
        };
        let curve = crease_curve(&meet_at, off_a, off_b, &breaks, eps).ok_or(TOO_THICK)?;
        let b = edge(
            &mut s,
            base[&edge_data.a],
            base[&edge_data.b],
            edge_data.curve.clone(),
            None,
        )?;
        let t = edge(
            &mut s,
            up[&edge_data.a],
            up[&edge_data.b],
            Curve::Spline(Spline::whole(curve)),
            None,
        )?;
        bottom_of.insert(*e, b);
        top_of.insert(*e, t);
    }
    let copy = |vertex_for: &dyn Fn(VertexId) -> VertexId,
                edge_for: &dyn Fn(EdgeId) -> Option<EdgeId>,
                f: &Face,
                surface: Surface|
     -> Result<Face, &'static str> {
        let mut vs: Vec<Vec<VertexId>> = f
            .loops()
            .map(|l| l.iter().map(|v| vertex_for(*v)).collect())
            .collect();
        let mut es: Vec<Vec<EdgeId>> = Vec::with_capacity(vs.len());
        for (k, vrow) in vs.iter().enumerate() {
            let mut row = Vec::with_capacity(vrow.len());
            for x in f.loop_edges(k) {
                row.push(edge_for(x.edge).ok_or(NOTHING)?);
            }
            es.push(row);
        }
        let outer = vs.remove(0);
        let mut c = Face::with_edges(outer, vs, surface, es);
        c.paint = f.paint;
        Ok(c)
    };
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let mut low = copy(
            &|v| base[&v],
            &|e| bottom_of.get(&e).copied(),
            f,
            f.surface.clone(),
        )?;
        let mut high = copy(
            &|v| vertex_for(v, *id),
            &|e| {
                crease_top
                    .get(&(e, *id))
                    .or_else(|| top_of.get(&e))
                    .copied()
            },
            f,
            offsets[id].clone(),
        )?;
        if d > 0.0 {
            low.reverse();
        } else {
            high.reverse();
        }
        s.add_face(low);
        s.add_face(high);
    }
    // A line up from each point of a rim, and its wall: a ruled surface
    // between the edge and its offset, cut square across. At a rim
    // whose end an outside corner split (part 2b), the line goes up to
    // that face's own corner instead of the shared one, so two rims that
    // share a split vertex but not a face get their own, separate lines.
    let mut rise: BTreeMap<(VertexId, Option<FaceId>), EdgeId> = BTreeMap::new();
    for (_, a, b, face) in &rims {
        for v in [a, b] {
            let key = (*v, split.contains_key(&(*v, *face)).then_some(*face));
            if let std::collections::btree_map::Entry::Vacant(e) = rise.entry(key) {
                e.insert(edge(
                    &mut s,
                    base[v],
                    vertex_for(*v, *face),
                    Curve::Line,
                    None,
                )?);
            }
        }
    }
    for (e_id, from, to, face) in &rims {
        let e = model.edge(*e_id).ok_or(NOTHING)?;
        let (low, high) = fitted.get(e_id).ok_or(TOO_THICK)?;
        let (b, t) = (bottom_of[e_id], top_of[e_id]);
        let flip = (*from == e.a) != (d > 0.0);
        let ruled = Patch {
            flipped: flip,
            ..Patch::whole(crate::thicken::ruled(low, high).ok_or(TOO_THICK)?)
        };
        let (va, vb) = (base[&e.a], base[&e.b]);
        let (ua, ub) = (vertex_for(e.a, *face), vertex_for(e.b, *face));
        let key = |v: VertexId| (v, split.contains_key(&(v, *face)).then_some(*face));
        let (ra, rb) = (rise[&key(e.a)], rise[&key(e.b)]);
        let (loop_v, loop_e) = if flip {
            (vec![va, ua, ub, vb], vec![ra, t, rb, b])
        } else {
            (vec![va, vb, ub, ua], vec![b, rb, t, ra])
        };
        let _ = to;
        s.add_face(Face::with_edges(
            loop_v,
            vec![],
            crate::thicken::flat(&ruled, eps).unwrap_or(Surface::Nurbs(ruled)),
            vec![loop_e],
        ));
    }
    // Each outside corner's own bridge (part 2b): a ruled face between
    // the two faces' top edges, closed at either end that split by a cap
    // between their own corners and a small triangle against the risers
    // either side of it (left as the one shared point, with no cap, at
    // an end nothing split: a third face there kept its single corner).
    for br in bridges {
        let raw = Patch::whole(crate::thicken::ruled(&br.high_a, &br.high_b).ok_or(TOO_THICK)?);
        let flipped = raw.normal(0.5, 0.5).dot(br.out_ref) < 0.0;
        let bridge = Patch { flipped, ..raw };
        // Each end's own cap triangle faces past that end, along the
        // crease: the same direction its two rim walls' own end caps
        // already face, which the risers it shares with them must match.
        let tangent = model.vertex(br.eb) - model.vertex(br.ea);
        let mut loop_v = vec![vertex_for(br.ea, br.fa), vertex_for(br.eb, br.fa)];
        let mut loop_e = vec![br.ta];
        if let (Some(&pa_v), Some(&pb_v)) = (split.get(&(br.eb, br.fa)), split.get(&(br.eb, br.fb)))
        {
            loop_e.push(corner_triangle(
                &mut s, br.eb, br.fa, br.fb, pa_v, pb_v, tangent, &base, &rise,
            )?);
            loop_v.push(pb_v);
        }
        loop_e.push(br.tb);
        let ea_fb = vertex_for(br.ea, br.fb);
        if ea_fb != loop_v[0] {
            loop_e.push(corner_triangle(
                &mut s, br.ea, br.fa, br.fb, loop_v[0], ea_fb, -tangent, &base, &rise,
            )?);
            loop_v.push(ea_fb);
        }
        s.add_face(Face::with_edges(
            loop_v,
            vec![],
            crate::thicken::flat(&bridge, eps).unwrap_or(Surface::Nurbs(bridge)),
            vec![loop_e],
        ));
    }
    if !crate::boolean::is_solid(&s) {
        return Err(TOO_THICK);
    }
    Ok((s, body.clone()))
}

/// An outside corner's own bridge (part 2b), kept until the rims below
/// have given it the risers its ends close against.
struct Bridge {
    fa: FaceId,
    fb: FaceId,
    ea: VertexId,
    eb: VertexId,
    ta: EdgeId,
    tb: EdgeId,
    high_a: Nurbs,
    high_b: Nurbs,
    out_ref: DVec3,
}

/// The small flat face closing the gap, at one end of an outside
/// corner's bridge, between the base vertex and the two faces' own
/// corners there (`pa_v`, `pb_v`): against the rim risers either side of
/// it, facing `out_ref` (past that end, along the crease). Returns the
/// new edge between the two corners, which the bridge itself also uses.
#[allow(clippy::too_many_arguments)]
fn corner_triangle(
    s: &mut Model,
    v: VertexId,
    fa: FaceId,
    fb: FaceId,
    pa_v: VertexId,
    pb_v: VertexId,
    out_ref: DVec3,
    base: &BTreeMap<VertexId, VertexId>,
    rise: &BTreeMap<(VertexId, Option<FaceId>), EdgeId>,
) -> Result<EdgeId, &'static str> {
    let cap = edge(s, pa_v, pb_v, Curve::Line, None)?;
    let ra = *rise.get(&(v, Some(fa))).ok_or(TOO_THICK)?;
    let rb = *rise.get(&(v, Some(fb))).ok_or(TOO_THICK)?;
    let pts = [s.vertex(base[&v]), s.vertex(pa_v), s.vertex(pb_v)];
    let n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalize_or_zero();
    let (tri_v, tri_e, n) = if n.dot(out_ref) >= 0.0 {
        ([base[&v], pa_v, pb_v], [ra, cap, rb], n)
    } else {
        ([base[&v], pb_v, pa_v], [rb, cap, ra], -n)
    };
    s.add_face(Face::with_edges(
        tri_v.to_vec(),
        vec![],
        Surface::Plane(Plane::new(pts[0], n)),
        vec![tri_e.to_vec()],
    ));
    Ok(cap)
}

/// The point of `s` nearest `x`, and its normal there: for a freeform
/// face past its own edge (an outside corner's mitre can reach past
/// it), its surface does not extend, so the edge's own point and normal
/// stand for it there, as a straight continuation would (a flat or
/// other analytic surface already has no edge to reach past).
fn probe(s: &Surface, x: DVec3) -> (DVec3, DVec3) {
    match s {
        Surface::Nurbs(patch) => {
            let q = patch.project(x);
            (q, patch.normal_at(q))
        }
        _ => (s.project(x), s.normal_at(x)),
    }
}

/// Whether `x` is genuinely on `s` (within tolerance of its nearest
/// point): a flat or other analytic surface always is (unbounded); a
/// freeform one is not once `x` is past its own edge.
fn truly_on(s: &Surface, x: DVec3, tol: &Tol) -> bool {
    match s {
        Surface::Nurbs(patch) => patch.project(x).distance(x) <= tol.dist,
        _ => true,
    }
}

/// How far `x` is from `s`, signed by its normal (`probe`'s tangent
/// stands in for the true surface past a freeform face's own edge).
fn offset_distance(s: &Surface, x: DVec3) -> f64 {
    let (q, n) = probe(s, x);
    (x - q).dot(n)
}

/// The point on both `a` and `b` closest to `guess`: Newton, moving only
/// in the plane of their two normals (as `corner` does exactly for flat
/// surfaces; a curved one is only locally flat, so this is iterated).
/// Falls back to the last point tried if it does not converge; the
/// caller checks the result against the true surfaces itself.
fn meet(a: &Surface, b: &Surface, guess: DVec3, tol: &Tol) -> DVec3 {
    let mut x = guess;
    for _ in 0..20 {
        let (qa, na) = probe(a, x);
        let (qb, nb) = probe(b, x);
        let (fa, fb) = ((x - qa).dot(na), (x - qb).dot(nb));
        if tol.is_zero(fa) && tol.is_zero(fb) {
            return x;
        }
        let Some(step) = solve2(na, nb, fa, fb, tol) else {
            return x;
        };
        x -= step;
    }
    x
}

/// The vector in the plane of unit `a` and `b` with `a·v = p` and
/// `b·v = q`; None where they are too nearly parallel to separate (as
/// `corner` finds a fold too sharp to move both ways at once).
fn solve2(a: DVec3, b: DVec3, p: f64, q: f64, tol: &Tol) -> Option<DVec3> {
    if tol.parallel(a, b) {
        return None;
    }
    let c = a.dot(b);
    let denom = 1.0 - c * c;
    let s = (p - q * c) / denom;
    let t = (q - p * c) / denom;
    Some(a * s + b * t)
}

/// The curve where two creased faces' offsets truly meet: `meet_at(t)`
/// for the nodes given, fitted to a NURBS curve of degree 5, split until
/// the fit itself lies on both true offset surfaces within `eps` (as
/// `thicken::edge_pair` checks a face's own offset).
fn crease_curve(
    meet_at: &dyn Fn(f64) -> DVec3,
    off_a: &Surface,
    off_b: &Surface,
    nodes: &[f64],
    eps: f64,
) -> Option<prism_geom::Nurbs> {
    let mut ts = nodes.to_vec();
    for _ in 0..crate::thicken::ROUNDS {
        let knots = prism_geom::knots_over(crate::thicken::DEGREE, &ts, nodes)?;
        let curve = prism_geom::Nurbs::fitted(meet_at, crate::thicken::DEGREE, knots)?;
        let error = |t: f64| {
            let p = curve.eval(t);
            offset_distance(off_a, p)
                .abs()
                .max(offset_distance(off_b, p).abs())
        };
        let split: Vec<bool> = ts
            .windows(2)
            .map(|w| crate::thicken::off_between(w, &error, eps))
            .collect();
        if !split.contains(&true) {
            return Some(curve);
        }
        ts = crate::thicken::halved(&ts, &split);
    }
    None
}

fn edge(
    s: &mut Model,
    a: VertexId,
    b: VertexId,
    curve: Curve,
    cid: Option<prism_topo::CurveId>,
) -> Result<EdgeId, &'static str> {
    match curve {
        Curve::Line => s.add_edge(a, b),
        c => s.add_curve_edge(a, b, c, cid),
    }
    .ok_or(TOO_THICK)
}

/// The surface `d` along a face's normal from `s`; None where it folds
/// through itself (or is freeform).
fn offset_surface(s: &Surface, d: f64, tol: &Tol) -> Option<Surface> {
    let sign = |outward: bool| if outward { d } else { -d };
    let ok = |r: f64| (r > tol.dist).then_some(r);
    Some(match s {
        Surface::Plane(p) => Surface::Plane(Plane::new(p.origin + p.normal * d, p.normal)),
        Surface::Cylinder(c) => Surface::Cylinder(prism_geom::Cylinder {
            radius: ok(c.radius + sign(c.outward))?,
            ..*c
        }),
        Surface::Sphere(x) => Surface::Sphere(prism_geom::Sphere {
            radius: ok(x.radius + sign(x.outward))?,
            ..*x
        }),
        Surface::Torus(t) => {
            let minor = ok(t.minor + sign(t.outward))?;
            if minor >= t.major - tol.dist {
                return None;
            }
            Surface::Torus(prism_geom::Torus { minor, ..*t })
        }
        // A cone's offset is the same cone with its tip moved along the
        // axis: out (away from the axis) moves it back.
        Surface::Cone(c) => Surface::Cone(Cone {
            apex: c.apex - c.axis * (sign(c.outward) / c.half_angle.sin()),
            ..*c
        }),
        Surface::Nurbs(_) => return None,
    })
}

/// The wall between an arc and its moved copy (same axis): a flat ring, a
/// cylinder or a cone, facing `out` at `at` on the arc.
#[allow(clippy::too_many_arguments)]
fn arc_wall(
    c1: DVec3,
    axis: DVec3,
    r1: f64,
    c2: DVec3,
    r2: f64,
    at: DVec3,
    out: DVec3,
    tol: &Tol,
) -> Result<Surface, &'static str> {
    let rise = (c2 - c1).dot(axis);
    if (c2 - c1 - axis * rise).length() > tol.dist {
        return Err(FREEFORM);
    }
    let wall = if tol.is_zero(rise) {
        Surface::Plane(Plane::new(c1, axis))
    } else if tol.is_zero(r1 - r2) {
        Surface::Cylinder(prism_geom::Cylinder {
            origin: c1,
            axis,
            radius: r1,
            outward: true,
        })
    } else {
        let apex = c1 + (c2 - c1) * (r1 / (r1 - r2));
        let into = c1 - apex;
        Surface::Cone(Cone {
            apex,
            axis: into.normalize(),
            half_angle: (r1 / into.length()).atan(),
            outward: true,
        })
    };
    Ok(if wall.normal_at(at).dot(out) < 0.0 {
        wall.flipped()
    } else {
        wall
    })
}

fn plan(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    open: &BTreeSet<FaceId>,
    d: f64,
) -> Result<Plan, &'static str> {
    let tol = model.tol();
    if !d.is_finite() || tol.is_zero(d) {
        return Err(ZERO);
    }
    let body = body(model, faces);
    if body.is_empty() {
        return Err(NOTHING);
    }
    let kept: Vec<FaceId> = body.iter().copied().filter(|f| !open.contains(f)).collect();
    if kept.is_empty() {
        return Err(ALL_OPEN);
    }
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let spline = f
            .edge_ids()
            .any(|e| model.edge(e).is_none_or(|x| x.curve.is_spline()));
        if f.surface.patch().is_some() || spline {
            return Err(FREEFORM);
        }
    }
    // How the body's faces use each edge.
    let mut uses: BTreeMap<EdgeId, Vec<(bool, VertexId, VertexId, FaceId)>> = BTreeMap::new();
    for id in &body {
        let f = model.face(*id).ok_or(NOTHING)?;
        let keep = !open.contains(id);
        for side in f.sides() {
            uses.entry(side.edge)
                .or_default()
                .push((keep, side.from, side.to, *id));
        }
    }
    let mut rims = Vec::new();
    let mut at_edge: BTreeMap<EdgeId, Vec<FaceId>> = BTreeMap::new();
    for (e, u) in &uses {
        if u.len() > 2 {
            return Err(BRANCHES);
        }
        let kept: Vec<_> = u.iter().filter(|x| x.0).collect();
        match kept.as_slice() {
            [one] => rims.push((*e, one.1, one.2, one.3)),
            [x, y] if x.1 == y.1 => return Err(MIXED),
            _ => {}
        }
        at_edge.insert(*e, kept.iter().map(|x| x.3).collect());
    }
    // Where each point goes: the nearest point on every moved face it is
    // on.
    let mut at_vertex: BTreeMap<VertexId, Vec<FaceId>> = BTreeMap::new();
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        for v in f.loops().flatten() {
            let fs = at_vertex.entry(*v).or_default();
            if !fs.contains(id) {
                fs.push(*id);
            }
        }
    }
    let on_rim: BTreeSet<VertexId> = rims.iter().flat_map(|r| [r.1, r.2]).collect();
    let mut moved = BTreeMap::new();
    let mut splits = BTreeMap::new();
    for (v, fs) in &at_vertex {
        match move_point(model, fs, model.vertex(*v), d, &tol) {
            Ok(p) => {
                moved.insert(*v, p);
            }
            Err(CORNER) if !on_rim.contains(v) => {
                splits.insert(*v, split_corner(model, *v, fs, d)?);
            }
            Err(e) => return Err(e),
        }
    }
    // Where an edge's moved copy ends at `v`.
    let end = |v: VertexId, e: EdgeId| -> DVec3 {
        moved.get(&v).copied().unwrap_or_else(|| {
            splits[&v]
                .iter()
                .find(|(x, _)| *x == e)
                .map_or(DVec3::ZERO, |(_, q)| *q)
        })
    };
    // Each edge moves to the same kind of curve.
    let mut curves = BTreeMap::new();
    for (e, fs) in &at_edge {
        if fs.is_empty() {
            continue;
        }
        let x = model.edge(*e).ok_or(NOTHING)?;
        let (a, b) = (end(x.a, *e), end(x.b, *e));
        let (pa, pb) = (model.vertex(x.a), model.vertex(x.b));
        let at = |t: f64| move_point(model, fs, x.curve.point(pa, pb, t), d, &tol);
        let c = match &x.curve {
            Curve::Line => {
                let m = at(0.5)?;
                if !tol.is_zero(distance_to_line(m, a, b)) || (b - a).dot(pb - pa) <= 0.0 {
                    return Err(TOO_THICK);
                }
                Curve::Line
            }
            Curve::Arc { axis, .. } => {
                let centre = circumcentre(a, at(0.5)?, b).ok_or(TOO_THICK)?;
                let radius = a.distance(centre);
                for t in [0.25, 0.75] {
                    let q = at(t)?;
                    if !tol.is_zero(q.distance(centre) - radius)
                        || !tol.is_zero((q - centre).dot(*axis))
                    {
                        return Err(CORNER);
                    }
                }
                if radius <= tol.dist {
                    return Err(TOO_THICK);
                }
                Curve::Arc {
                    centre,
                    axis: *axis,
                    radius,
                }
            }
            Curve::Spline(_) => return Err(FREEFORM),
        };
        curves.insert(*e, c);
    }
    // No flat face of straight sides may turn inside out.
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let Some(plane) = f.plane() else {
            continue;
        };
        if f.edge_ids()
            .any(|e| !matches!(curves.get(&e), Some(Curve::Line)))
        {
            continue;
        }
        let sides: Vec<_> = f.loop_edges(0).collect();
        let mut pts: Vec<DVec3> = Vec::new();
        for (i, side) in sides.iter().enumerate() {
            let prev = sides[(i + sides.len() - 1) % sides.len()].edge;
            pts.push(end(side.from, prev));
            pts.push(end(side.from, side.edge));
        }
        if newell(&pts).dot(plane.normal) <= 0.0 {
            return Err(TOO_THICK);
        }
    }
    Ok(Plan {
        body,
        kept,
        moved,
        splits,
        curves,
        rims,
    })
}

/// A corner of four or more flat faces whose moved copies miss one point:
/// the edges round it in order, each with where its moved copy ends. The
/// ends lie on one plane across the corner (square to the faces' mean
/// direction), so a small flat face fills the gap; each face still moves
/// by the thickness.
fn split_corner(
    model: &Model,
    v: VertexId,
    fs: &[FaceId],
    d: f64,
) -> Result<Vec<(EdgeId, DVec3)>, &'static str> {
    let p = model.vertex(v);
    // Each face's edge in to the corner and out of it.
    let mut out_of: BTreeMap<EdgeId, (EdgeId, DVec3)> = BTreeMap::new();
    let mut sum = DVec3::ZERO;
    for f in fs {
        let face = model.face(*f).ok_or(NOTHING)?;
        let n = face.plane().ok_or(CORNER)?.normal;
        sum += n;
        let sides: Vec<_> = face.sides().collect();
        let e_in = sides.iter().find(|s| s.to == v).ok_or(CORNER)?.edge;
        let e_out = sides.iter().find(|s| s.from == v).ok_or(CORNER)?.edge;
        out_of.insert(e_in, (e_out, n));
    }
    let across = sum.normalize_or_zero();
    let lean: Vec<f64> = fs
        .iter()
        .filter_map(|f| {
            model
                .face(*f)
                .and_then(|x| x.plane())
                .map(|pl| pl.normal.dot(across))
        })
        .collect();
    if lean.iter().any(|c| *c <= ANGLE.sqrt()) {
        return Err(CORNER);
    }
    // As far across as the faces move on average.
    let h = d * lean.len() as f64 / lean.iter().sum::<f64>();
    // Round the corner: from each edge, through the face it leads into.
    let start = *out_of.keys().next().ok_or(CORNER)?;
    let mut fan = Vec::new();
    let mut e = start;
    loop {
        let (next, n_before) = out_of[&e];
        let (_, n_after) = *out_of.get(&next).ok_or(CORNER)?;
        // The edge `next` lies between the face before it and the one after.
        let x = solve3([n_before, n_after, across], [d, d, h]).ok_or(CORNER)?;
        fan.push((next, p + x));
        e = next;
        if e == start {
            break;
        }
        if fan.len() > fs.len() {
            return Err(CORNER);
        }
    }
    if fan.len() != fs.len() {
        return Err(CORNER);
    }
    Ok(fan)
}

/// The point x with a·x, b·x and c·x the given values, if the three
/// directions are independent.
fn solve3([a, b, c]: [DVec3; 3], [p, q, r]: [f64; 3]) -> Option<DVec3> {
    let det = a.dot(b.cross(c));
    (det.abs() > ANGLE).then(|| (b.cross(c) * p + c.cross(a) * q + a.cross(b) * r) / det)
}

/// Where `p`, on the faces `fs`, moves to.
fn move_point(
    model: &Model,
    fs: &[FaceId],
    p: DVec3,
    d: f64,
    tol: &Tol,
) -> Result<DVec3, &'static str> {
    let mut ns: Vec<DVec3> = Vec::new();
    for f in fs {
        ns.push(model.face(*f).ok_or(NOTHING)?.surface.normal_at(p));
    }
    let ns = unique_normals(&ns, tol);
    corner(&ns, d, tol).map(|x| p + x).ok_or(CORNER)
}

fn distance_to_line(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let u = (b - a).normalize_or_zero();
    let v = p - a;
    (v - u * v.dot(u)).length()
}

/// The centre of the circle through three points.
fn circumcentre(a: DVec3, b: DVec3, c: DVec3) -> Option<DVec3> {
    let (ab, ac) = (b - a, c - a);
    let n = ab.cross(ac);
    let nn = n.length_squared();
    (nn > 0.0).then(|| {
        a + (ac.cross(n) * ab.length_squared() + n.cross(ab) * ac.length_squared()) / (2.0 * nn)
    })
}

/// `ns`, with any pointing the same way (within tolerance) as an
/// earlier one dropped: `corner` only wants to see each direction once,
/// and only refuses for directions that truly oppose (it reads a
/// surface merely tangent to another one, not creased, the same way).
fn unique_normals(ns: &[DVec3], tol: &Tol) -> Vec<DVec3> {
    let mut out: Vec<DVec3> = Vec::new();
    for n in ns {
        if !out
            .iter()
            .any(|m: &DVec3| tol.parallel(*m, *n) && m.dot(*n) > 0.0)
        {
            out.push(*n);
        }
    }
    out
}

/// The shortest move that puts a point `d` along each unit normal in
/// `ns`: exact where up to three independent planes meet; None where
/// more do not meet at one point, or the faces fold back on each other.
fn corner(ns: &[DVec3], d: f64, tol: &Tol) -> Option<DVec3> {
    // Faces folded flat back onto each other cannot both move.
    for (i, a) in ns.iter().enumerate() {
        if ns[i + 1..].iter().any(|b| tol.parallel(*a, *b)) {
            return None;
        }
    }
    let mut basis: Vec<DVec3> = Vec::new();
    for n in ns {
        let mut r = *n;
        for b in &orthonormal(&basis) {
            r -= *b * r.dot(*b);
        }
        // Not in the planes so far (as `Tol::parallel` judges an angle).
        if r.length_squared() > 2.0 * ANGLE && basis.len() < 3 {
            basis.push(*n);
        }
    }
    let x = match basis.as_slice() {
        [a] => *a * d,
        [a, b] => (*a + *b) * (d / (1.0 + a.dot(*b))),
        [a, b, c] => {
            let det = a.dot(b.cross(*c));
            (b.cross(*c) + c.cross(*a) + a.cross(*b)) * (d / det)
        }
        _ => return None,
    };
    ns.iter().all(|n| tol.is_zero(n.dot(x) - d)).then_some(x)
}

fn orthonormal(vs: &[DVec3]) -> Vec<DVec3> {
    let mut out: Vec<DVec3> = Vec::new();
    for v in vs {
        let mut r = *v;
        for b in &out {
            r -= *b * r.dot(*b);
        }
        out.push(r.normalize_or_zero());
    }
    out
}

fn loop_normal(s: &Model, l: &[VertexId]) -> DVec3 {
    let pts: Vec<DVec3> = l.iter().map(|v| s.vertex(*v)).collect();
    newell(&pts)
}

/// The plane of a loop of points, if they all lie on one.
fn flat_plane(s: &Model, l: &[VertexId], tol: &Tol) -> Option<Plane> {
    let n = loop_normal(s, l).normalize_or_zero();
    if n == DVec3::ZERO {
        return None;
    }
    let o = s.vertex(l[0]);
    l.iter()
        .all(|v| tol.is_zero((s.vertex(*v) - o).dot(n)))
        .then(|| Plane::new(o, n))
}
