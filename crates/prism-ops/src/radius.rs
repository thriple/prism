//! Push/Pull on a cylinder face: a new radius (Exact Curves 4d).
//!
//! Every face on the same cylinder joined to the pulled one through shared
//! points takes the new radius, about the same axis. Their points move
//! straight out from (or in towards) the axis, and the arcs round the axis
//! touching them take the new radius, so the caps follow.

use std::collections::BTreeSet;

use prism_geom::{Curve, Cylinder, Surface};
use prism_topo::{FaceId, Model, VertexId};

/// Said when a new radius would be zero or less.
pub const RADIUS_GONE: &str = "The radius would reach zero.";

/// The pulled face's cylinder, and every face on that same cylinder
/// joined to it through shared points (both halves of a full turn).
fn faces(model: &Model, face: FaceId) -> Option<(Cylinder, BTreeSet<FaceId>)> {
    let Surface::Cylinder(c) = model.face(face)?.surface else {
        return None;
    };
    let tol = model.tol();
    let same = |x: &Cylinder| {
        x.outward == c.outward
            && tol.parallel(x.axis, c.axis)
            && tol.is_zero(c.distance(x.origin) + c.radius)
            && tol.is_zero(x.radius - c.radius)
    };
    let mut out: BTreeSet<FaceId> = [face].into();
    let mut todo = vec![face];
    while let Some(f) = todo.pop() {
        let Some(x) = model.face(f) else { continue };
        let vs: BTreeSet<VertexId> = x.loops().flatten().copied().collect();
        for (g, y) in model.faces() {
            if let Surface::Cylinder(yc) = y.surface
                && same(&yc)
                && !out.contains(&g)
                && y.loops().flatten().any(|v| vs.contains(v))
            {
                out.insert(g);
                todo.push(g);
            }
        }
    }
    Some((c, out))
}

/// The new radius for a pull of `distance` along the face's normal.
fn new_radius(c: &Cylinder, distance: f64) -> f64 {
    if c.outward {
        c.radius + distance
    } else {
        c.radius - distance
    }
}

fn points(model: &Model, faces: &BTreeSet<FaceId>) -> BTreeSet<VertexId> {
    faces
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.loops().flatten().copied().collect::<Vec<_>>())
        .collect()
}

/// Whether `curve` is an arc round the cylinder's axis.
fn round_axis(model: &Model, c: &Cylinder, curve: &Curve) -> bool {
    let tol = model.tol();
    match *curve {
        Curve::Arc { centre, axis, .. } => {
            tol.parallel(axis, c.axis) && tol.is_zero((centre - c.axis_point(centre)).length())
        }
        Curve::Line => false,
    }
}

/// Why pulling curved `face` by `distance` is refused, if it is: the
/// radius would reach zero, or a face or arc touching the cylinder would
/// bend (only caps square to the axis and arcs round it follow).
pub fn refusal(model: &Model, face: FaceId, distance: f64) -> Option<&'static str> {
    let (c, fs) = faces(model, face)?;
    let tol = model.tol();
    if new_radius(&c, distance) <= tol.dist {
        return Some(RADIUS_GONE);
    }
    let vs = points(model, &fs);
    let bends_face = model.faces().any(|(g, x)| {
        !fs.contains(&g)
            && x.loops().flatten().any(|v| vs.contains(v))
            && !x.plane().is_some_and(|p| tol.parallel(p.normal, c.axis))
    });
    let bends_arc = model.edges().any(|(_, e)| {
        e.curve.is_arc()
            && (vs.contains(&e.a) || vs.contains(&e.b))
            && !round_axis(model, &c, &e.curve)
    });
    (bends_face || bends_arc).then_some(crate::pushpull::CURVED_PULL)
}

/// Give curved `face` (and the faces on its cylinder joined to it) the
/// radius a pull of `distance` makes. Does nothing when refused.
pub fn apply(model: &mut Model, face: FaceId, distance: f64) -> bool {
    if model.tol().is_zero(distance) || refusal(model, face, distance).is_some() {
        return false;
    }
    let Some((c, fs)) = faces(model, face) else {
        return false;
    };
    let r = new_radius(&c, distance);
    let k = r / c.radius;
    let vs = points(model, &fs);
    for v in &vs {
        let p = model.vertex(*v);
        let a = c.axis_point(p);
        model.set_vertex(*v, a + (p - a) * k);
    }
    let arcs: Vec<_> = model
        .edges()
        .filter(|(_, e)| (vs.contains(&e.a) || vs.contains(&e.b)) && e.curve.is_arc())
        .map(|(id, e)| (id, e.curve))
        .collect();
    for (e, curve) in arcs {
        if let Curve::Arc { centre, axis, .. } = curve {
            model.set_curve(
                e,
                Curve::Arc {
                    centre,
                    axis,
                    radius: r,
                },
            );
        }
    }
    for f in &fs {
        model.edit_face(*f, |x| {
            if let Surface::Cylinder(y) = &mut x.surface {
                y.radius = r;
            }
        });
    }
    true
}
