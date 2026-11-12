//! Push/Pull on a curved face: a new size (Exact Curves 4d for cylinders,
//! 6d for cones, spheres and tori).
//!
//! Every face on the same surface joined to the pulled one through shared
//! points moves square to the surface by the pull: a cylinder or sphere
//! takes a new radius, a torus a new tube radius, and a cone moves square
//! to its side (same angle, its apex along the axis). Flat faces beside it
//! follow when its points can stay in their planes: a plane the points
//! move along (a cylinder's or cone's face through the axis, a sphere's
//! through its centre), or a round cut (a cap square to the axis, or any
//! plane across a sphere), whose circle takes the new size. Anything else
//! would bend, and is refused.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Curve, Surface, arc_through};
use prism_math::{DVec3, Plane, Tol};
use prism_topo::{EdgeId, FaceId, Model, VertexId};

/// Said when a new size would be zero or less.
pub const RADIUS_GONE: &str = "The radius would reach zero.";
/// Said when a torus's tube would reach its axis.
pub const TUBE_AXIS: &str = "The torus's tube would reach its axis.";

/// Whether two surfaces are the same, within tolerance (facing the same
/// way).
fn same(a: &Surface, b: &Surface, tol: &Tol) -> bool {
    let near = |p: DVec3, q: DVec3| tol.is_zero(p.distance(q));
    match (a, b) {
        (Surface::Cylinder(x), Surface::Cylinder(c)) => {
            x.outward == c.outward
                && tol.parallel(x.axis, c.axis)
                && tol.is_zero(c.distance(x.origin) + c.radius)
                && tol.is_zero(x.radius - c.radius)
        }
        (Surface::Cone(x), Surface::Cone(c)) => {
            x.outward == c.outward
                && near(x.apex, c.apex)
                && x.axis.dot(c.axis) > 1.0 - 1e-9
                && (x.half_angle - c.half_angle).abs() < 1e-9
        }
        (Surface::Sphere(x), Surface::Sphere(c)) => {
            x.outward == c.outward && near(x.centre, c.centre) && tol.is_zero(x.radius - c.radius)
        }
        (Surface::Torus(x), Surface::Torus(c)) => {
            x.outward == c.outward
                && near(x.centre, c.centre)
                && tol.parallel(x.axis, c.axis)
                && tol.is_zero(x.major - c.major)
                && tol.is_zero(x.minor - c.minor)
        }
        _ => false,
    }
}

/// The pulled face's surface, and every face on that same surface joined
/// to it through shared points (both halves of a full turn).
pub(crate) fn faces(model: &Model, face: FaceId) -> Option<(Surface, BTreeSet<FaceId>)> {
    let s = model.face(face)?.surface;
    if !s.is_curved() {
        return None;
    }
    let tol = model.tol();
    let mut out: BTreeSet<FaceId> = [face].into();
    let mut todo = vec![face];
    while let Some(f) = todo.pop() {
        let Some(x) = model.face(f) else { continue };
        let vs: BTreeSet<VertexId> = x.loops().flatten().copied().collect();
        for (g, y) in model.faces() {
            if !out.contains(&g)
                && same(&y.surface, &s, &tol)
                && y.loops().flatten().any(|v| vs.contains(v))
            {
                out.insert(g);
                todo.push(g);
            }
        }
    }
    Some((s, out))
}

/// The surface moved by `delta` away from its axis, centre or tube's
/// middle (whichever way it faces). Err when its size would be gone.
fn offset(s: &Surface, delta: f64, tol: &Tol) -> Result<Surface, &'static str> {
    let mut s = *s;
    match &mut s {
        Surface::Cylinder(c) => {
            c.radius += delta;
            if c.radius <= tol.dist {
                return Err(RADIUS_GONE);
            }
        }
        Surface::Sphere(x) => {
            x.radius += delta;
            if x.radius <= tol.dist {
                return Err(RADIUS_GONE);
            }
        }
        Surface::Torus(t) => {
            t.minor += delta;
            if t.minor <= tol.dist {
                return Err(RADIUS_GONE);
            }
            if t.minor >= t.major - tol.dist {
                return Err(TUBE_AXIS);
            }
        }
        Surface::Cone(c) => {
            // Square to its side: the apex slides along the axis.
            c.apex -= c.axis * (delta / c.half_angle.sin());
        }
        Surface::Plane(_) => {}
    }
    Ok(s)
}

/// The pull of `distance` along the face's normal, away from the axis,
/// centre or tube's middle.
fn delta(s: &Surface, distance: f64) -> f64 {
    if s.outward() { distance } else { -distance }
}

/// Where a point of the surface goes, square to it.
fn square_move(old: &Surface, new: &Surface, p: DVec3, delta: f64, tol: &Tol) -> DVec3 {
    if let (Surface::Cone(c), Surface::Cone(n)) = (old, new)
        && tol.is_zero(p.distance(c.apex))
    {
        return n.apex;
    }
    let out = if old.outward() {
        old.normal_at(p)
    } else {
        -old.normal_at(p)
    };
    p + out * delta
}

/// Where the round cut of `plane` across the surface has its centre, if
/// the plane cuts it in a circle (square to the axis; any plane across a
/// sphere).
fn round_centre(s: &Surface, plane: &Plane, tol: &Tol) -> Option<DVec3> {
    if let Surface::Sphere(x) = s {
        return Some(x.centre - plane.normal * plane.distance(x.centre));
    }
    let rev = s.revolution()?;
    if !tol.parallel(plane.normal, rev.axis) {
        return None;
    }
    Some(rev.origin - plane.normal * plane.distance(rev.origin))
}

/// A point `p` of the surface that must stay in `planes`: where it goes.
fn follow_planes(
    old: &Surface,
    new: &Surface,
    p: DVec3,
    delta: f64,
    planes: &[Plane],
    tol: &Tol,
) -> Result<DVec3, &'static str> {
    let bend = crate::pushpull::CURVED_PULL;
    let q = square_move(old, new, p, delta, tol);
    if planes.iter().all(|pl| tol.is_zero(pl.distance(q))) {
        return Ok(q);
    }
    // One plane (or several the same): its round cut takes the new size.
    let first = planes.first().ok_or(bend)?;
    if !planes
        .iter()
        .all(|pl| tol.parallel(pl.normal, first.normal) && tol.is_zero(pl.distance(p)))
    {
        return Err(bend);
    }
    let c = round_centre(old, first, tol).ok_or(bend)?;
    let e = (p - c).normalize_or_zero();
    if e == DVec3::ZERO {
        return Err(bend);
    }
    // Along the cut from its centre to the new surface (Newton steps).
    let f = |r: f64| new.distance(c + e * r);
    let mut r = (p - c).length();
    let h = tol.dist.max(r * 1e-7);
    for _ in 0..50 {
        let fr = f(r);
        if fr.abs() <= tol.dist * 1e-3 {
            break;
        }
        let d = (f(r + h) - f(r - h)) / (2.0 * h);
        if d.abs() < 1e-12 {
            return Err(RADIUS_GONE);
        }
        r -= fr / d;
    }
    // No circle left where the plane cuts the new surface: it is gone.
    if r > tol.dist && f(r).abs() <= tol.dist {
        Ok(c + e * r)
    } else {
        Err(RADIUS_GONE)
    }
}

/// What a new size changes.
struct Plan {
    surface: Surface,
    faces: BTreeSet<FaceId>,
    points: BTreeMap<VertexId, DVec3>,
    curves: Vec<(EdgeId, Curve)>,
}

fn plan(model: &Model, face: FaceId, distance: f64) -> Result<Plan, &'static str> {
    let bend = crate::pushpull::CURVED_PULL;
    let (s, fs) = faces(model, face).ok_or(bend)?;
    let tol = model.tol();
    let d = delta(&s, distance);
    let new = offset(&s, d, &tol)?;
    let vs: BTreeSet<VertexId> = fs
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.loops().flatten().copied().collect::<Vec<_>>())
        .collect();
    // The flat faces beside it each point must stay in; a curved one would
    // bend.
    let mut planes: BTreeMap<VertexId, Vec<Plane>> = BTreeMap::new();
    for (g, x) in model.faces() {
        if fs.contains(&g) {
            continue;
        }
        for v in x.loops().flatten().filter(|v| vs.contains(v)) {
            match x.plane() {
                Some(p) => planes.entry(*v).or_default().push(*p),
                None => return Err(bend),
            }
        }
    }
    let rev = s.revolution();
    let mut points = BTreeMap::new();
    for v in &vs {
        let p = model.vertex(*v);
        let pl = planes.get(v).map(Vec::as_slice).unwrap_or(&[]);
        let q = follow_planes(&s, &new, p, d, pl, &tol)?;
        // No point may pass the axis (a cone pulled in past its apex).
        if let Some(r) = &rev {
            let side = |x: DVec3| {
                let w = x - r.origin;
                w - r.axis * w.dot(r.axis)
            };
            let (a, b) = (side(p), side(q));
            if a.length() > tol.dist && a.dot(b) <= tol.dist * a.length() {
                return Err(RADIUS_GONE);
            }
        }
        points.insert(*v, q);
    }
    // Arcs: each through its moved ends and middle. An arc touching the
    // moved points that is not on the surface would bend.
    let on_surface: BTreeSet<EdgeId> = fs
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.edge_ids().collect::<Vec<_>>())
        .collect();
    let mut curves = Vec::new();
    for (id, e) in model.edges() {
        if !(vs.contains(&e.a) || vs.contains(&e.b)) || !e.curve.is_curved() {
            continue;
        }
        if !on_surface.contains(&id) {
            return Err(bend);
        }
        let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
        let m = e.curve.point(pa, pb, 0.5);
        // The middle stays in the flat faces along this edge.
        let mid_planes: Vec<Plane> = model
            .faces()
            .filter(|(g, x)| !fs.contains(g) && x.edge_ids().any(|x| x == id))
            .filter_map(|(_, x)| x.plane().copied())
            .collect();
        let m2 = follow_planes(&s, &new, m, d, &mid_planes, &tol)?;
        let at = |v: VertexId| points.get(&v).copied().unwrap_or(model.vertex(v));
        let spec = arc_through(at(e.a), m2, at(e.b), tol.dist).ok_or(bend)?;
        curves.push((
            id,
            Curve::Arc {
                centre: spec.centre,
                axis: spec.normal,
                radius: spec.radius,
            },
        ));
    }
    Ok(Plan {
        surface: new,
        faces: fs,
        points,
        curves,
    })
}

/// Why pulling curved `face` by `distance` is refused, if it is: its size
/// would reach zero (or a torus's tube its axis), or a face or arc beside
/// it would bend.
pub fn refusal(model: &Model, face: FaceId, distance: f64) -> Option<&'static str> {
    if model.tol().is_zero(distance) {
        return None;
    }
    plan(model, face, distance).err()
}

/// Give curved `face` (and the faces on its surface joined to it) the size
/// a pull of `distance` makes. Does nothing when refused.
pub fn apply(model: &mut Model, face: FaceId, distance: f64) -> bool {
    if model.tol().is_zero(distance) {
        return false;
    }
    let Ok(plan) = plan(model, face, distance) else {
        return false;
    };
    for (v, p) in &plan.points {
        model.set_vertex(*v, *p);
    }
    for (e, c) in plan.curves {
        model.set_curve(e, c);
    }
    for f in &plan.faces {
        model.edit_face(*f, |x| x.surface = plan.surface);
    }
    true
}
