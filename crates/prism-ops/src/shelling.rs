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

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Cone, Curve, Surface};
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
    let plan = plan(model, faces, open, d)?;
    let tol = model.tol();
    let mut s = Model::new();
    let mut base: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    let mut up: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for (v, q) in &plan.moved {
        base.insert(*v, s.add_vertex(model.vertex(*v)));
        up.insert(*v, s.add_vertex(*q));
    }
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
        let hi = edge(&mut s, up[&e.a], up[&e.b], moved.clone(), hi_cid)?;
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
    for id in &plan.kept {
        let f = model.face(*id).ok_or(NOTHING)?;
        let moved = offset_surface(&f.surface, d, &tol).ok_or(TOO_THICK)?;
        let mut low = copy(f, &base, &low_e, f.surface.clone());
        let mut high = copy(f, &up, &high_e, moved);
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
    let mut moved = BTreeMap::new();
    for (v, fs) in &at_vertex {
        moved.insert(*v, move_point(model, fs, model.vertex(*v), d, &tol)?);
    }
    // Each edge moves to the same kind of curve.
    let mut curves = BTreeMap::new();
    for (e, fs) in &at_edge {
        if fs.is_empty() {
            continue;
        }
        let x = model.edge(*e).ok_or(NOTHING)?;
        let (a, b) = (moved[&x.a], moved[&x.b]);
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
        let pts: Vec<DVec3> = f.outer.iter().map(|v| moved[v]).collect();
        if newell(&pts).dot(plane.normal) <= 0.0 {
            return Err(TOO_THICK);
        }
    }
    Ok(Plan {
        body,
        kept,
        moved,
        curves,
        rims,
    })
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
        let n = model.face(*f).ok_or(NOTHING)?.surface.normal_at(p);
        if !ns.iter().any(|m| tol.parallel(*m, n) && m.dot(n) > 0.0) {
            ns.push(n);
        }
    }
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
