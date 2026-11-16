//! Shell / Thicken (brief 42, part 3a): faces become a solid of a set
//! thickness.
//!
//! The faces joined to the ones given by edges are taken whole: an open
//! sheet becomes a slab, and a closed body becomes hollow, less the faces
//! left open (a box becomes a tray). Each face moves along its own normal
//! by the thickness (out for more than zero, in for less), and a corner
//! goes where the moved faces meet, so the walls are the same thickness
//! everywhere. Where an edge has a face on one side only (a sheet's rim, or
//! the edge of an open face), a flat wall joins it to its moved copy; one
//! that cannot be flat is two flat triangles. This part takes flat faces
//! with straight edges, and corners of up to three planes.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::Surface;
use prism_math::tol::ANGLE;
use prism_math::{DVec3, Plane, Tol, newell};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId, loop_pairs};

pub const NOTHING: &str = "Refused: click a face to shell or thicken.";
pub const ZERO: &str = "Refused: a thickness of zero makes nothing.";
pub const ALL_OPEN: &str = "Refused: every face is left open, so there is nothing to thicken.";
pub const FLAT_ONLY: &str =
    "Refused: Shell / Thicken takes flat faces with straight edges for now.";
pub const MIXED: &str =
    "Refused: these faces face different ways. Solid > Heal turns them the same way.";
pub const BRANCHES: &str = "Refused: more than two faces meet at an edge here.";
pub const CORNER: &str = "Refused: more than three faces meet at a corner here, and their moved copies do not meet at one point. Not yet.";
pub const TOO_THICK: &str = "Refused: that thickness turns a face inside out. Try a smaller one.";

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
    /// Edges with a kept face on one side only, walked the way that face
    /// walks them, with its normal.
    rims: Vec<(VertexId, VertexId, DVec3)>,
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
    let p = plan(model, faces, open, d)?;
    let mut s = Model::new();
    let mut base: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    let mut up: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for (v, q) in &p.moved {
        base.insert(*v, s.add_vertex(model.vertex(*v)));
        up.insert(*v, s.add_vertex(*q));
    }
    let add = |s: &mut Model, f: &Face, map: &BTreeMap<VertexId, VertexId>, plane: Plane| {
        let map_loop = |l: &[VertexId]| -> Vec<VertexId> { l.iter().map(|v| map[v]).collect() };
        for l in f.loops() {
            for (a, b) in loop_pairs(l) {
                s.add_edge(map[&a], map[&b]);
            }
        }
        let mut c = Face::new(
            map_loop(&f.outer),
            f.holes.iter().map(|h| map_loop(h)).collect(),
            Surface::Plane(plane),
        );
        c.paint = f.paint;
        c
    };
    for id in &p.kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let plane = *f.plane().ok_or(FLAT_ONLY)?;
        let mut low = add(&mut s, f, &base, plane);
        let at = p.moved[&f.outer[0]];
        let mut high = add(&mut s, f, &up, Plane::new(at, plane.normal));
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
    let tol = model.tol();
    for (a, b, n) in &p.rims {
        // The wall faces out of the solid: to the right of a to b, about
        // the face's normal.
        let out = (model.vertex(*b) - model.vertex(*a)).cross(*n);
        let pts = [base[a], base[b], up[b], up[a]];
        for e in [(pts[1], pts[2]), (pts[3], pts[0])] {
            s.add_edge(e.0, e.1);
        }
        let tris: Vec<Vec<VertexId>> = if flat_plane(&s, &pts, &tol).is_some() {
            vec![pts.to_vec()]
        } else {
            s.add_edge(pts[0], pts[2]);
            vec![vec![pts[0], pts[1], pts[2]], vec![pts[0], pts[2], pts[3]]]
        };
        for mut l in tris {
            let mut n = loop_normal(&s, &l).normalize_or_zero();
            if n.dot(out) < 0.0 {
                l.reverse();
                n = -n;
            }
            let plane = Plane::new(s.vertex(l[0]), n);
            s.add_face(Face::new(l, vec![], Surface::Plane(plane)));
        }
    }
    if !crate::boolean::is_solid(&s) {
        return Err(TOO_THICK);
    }
    Ok((s, p.body))
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
    // Every face of the body that stays: flat, with straight edges.
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let straight = f
            .edge_ids()
            .all(|e| model.edge(e).is_some_and(|x| !x.curve.is_curved()));
        if f.plane().is_none() || !straight {
            return Err(FLAT_ONLY);
        }
    }
    // How the body's faces use each edge.
    let mut uses: BTreeMap<EdgeId, Vec<(bool, VertexId, VertexId, DVec3)>> = BTreeMap::new();
    for id in &body {
        let f = model.face(*id).ok_or(NOTHING)?;
        let n = f.plane().map_or(DVec3::ZERO, |p| p.normal);
        let keep = !open.contains(id);
        for side in f.sides() {
            uses.entry(side.edge)
                .or_default()
                .push((keep, side.from, side.to, n));
        }
    }
    let mut rims = Vec::new();
    for u in uses.values() {
        if u.len() > 2 {
            return Err(BRANCHES);
        }
        let kept: Vec<_> = u.iter().filter(|x| x.0).collect();
        match kept.as_slice() {
            [one] => rims.push((one.1, one.2, one.3)),
            [x, y] if x.1 == y.1 => return Err(MIXED),
            _ => {}
        }
    }
    // Where each point goes: the nearest point on every moved plane of
    // its faces.
    let mut normals: BTreeMap<VertexId, Vec<DVec3>> = BTreeMap::new();
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let n = f.plane().ok_or(FLAT_ONLY)?.normal;
        for v in f.loops().flatten() {
            let ns = normals.entry(*v).or_default();
            if !ns.iter().any(|m| tol.parallel(*m, n) && m.dot(n) > 0.0) {
                ns.push(n);
            }
        }
    }
    let mut moved = BTreeMap::new();
    for (v, ns) in &normals {
        let x = corner(ns, d, &tol).ok_or(CORNER)?;
        moved.insert(*v, model.vertex(*v) + x);
    }
    // No face may turn inside out: each side keeps its way and length, and
    // the face its way round.
    for id in &kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let n = f.plane().ok_or(FLAT_ONLY)?.normal;
        for l in f.loops() {
            for (a, b) in loop_pairs(l) {
                let (old, new) = (model.vertex(b) - model.vertex(a), moved[&b] - moved[&a]);
                if new.length() < tol.dist || new.dot(old) <= 0.0 {
                    return Err(TOO_THICK);
                }
            }
        }
        let pts: Vec<DVec3> = f.outer.iter().map(|v| moved[v]).collect();
        if newell(&pts).dot(n) <= 0.0 {
            return Err(TOO_THICK);
        }
    }
    Ok(Plan {
        body,
        kept,
        moved,
        rims,
    })
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
