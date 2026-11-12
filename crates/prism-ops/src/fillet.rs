//! Corner fillets (brief 24, step 3d): a corner between two straight edges
//! is rounded by an arc tangent to both, and the edges are trimmed to it.

use prism_geom::Curve;
use prism_math::DVec3;
use prism_topo::{Model, VertexId};

use crate::select::{Selection, remove};
use crate::{add_arc, insert_point, vertex_at};

/// Said when the corner is not two straight edges of a face.
pub const NOT_A_CORNER: &str = "Only a corner of a face between two straight edges can be rounded.";
/// Said when the edges are too short for the radius.
pub const TOO_SHORT: &str = "The edges at this corner are too short for that radius.";

/// A fillet: where the arc touches each edge, and its circle and sweep.
struct Plan {
    a: DVec3,
    b: DVec3,
    centre: DVec3,
    normal: DVec3,
    sweep: f64,
}

/// The fillet at corner `v`, or why it cannot be made.
fn plan(model: &Model, v: VertexId, radius: f64) -> Result<Plan, &'static str> {
    let edges: Vec<_> = model.edges().filter(|(_, e)| e.has(v)).collect();
    let [(_, e1), (_, e2)] = edges.as_slice() else {
        return Err(NOT_A_CORNER);
    };
    if e1.curve.is_curved() || e2.curve.is_curved() {
        return Err(NOT_A_CORNER);
    }
    let tol = model.tol();
    let p = model.vertex(v);
    let far = |e: &prism_topo::Edge| model.vertex(if e.a == v { e.b } else { e.a });
    let (d1, d2) = (far(e1) - p, far(e2) - p);
    let (u1, u2) = (d1.normalize_or_zero(), d2.normalize_or_zero());
    let n = u1.cross(u2);
    if n.length() <= 1e-9 || radius <= tol.dist {
        // In a line, or no radius.
        return Err(NOT_A_CORNER);
    }
    let theta = u1.dot(u2).clamp(-1.0, 1.0).acos();
    // How far from the corner the arc touches each edge.
    let t = radius / (theta / 2.0).tan();
    if t >= d1.length() - tol.dist || t >= d2.length() - tol.dist {
        return Err(TOO_SHORT);
    }
    let (a, b) = (p + u1 * t, p + u2 * t);
    let centre = p + (u1 + u2).normalize() * (radius / (theta / 2.0).sin());
    // The short way round, from a to b.
    let normal = {
        let n = n.normalize();
        let arc = Curve::Arc {
            centre,
            axis: n,
            radius,
        };
        if arc.sweep(a, b) <= std::f64::consts::PI {
            n
        } else {
            -n
        }
    };
    let sweep = std::f64::consts::PI - theta;
    Ok(Plan {
        a,
        b,
        centre,
        normal,
        sweep,
    })
}

/// Why the corner at `corner` cannot be rounded to `radius`, if it cannot.
pub fn refusal(model: &Model, corner: DVec3, radius: f64) -> Option<&'static str> {
    let Some(v) = vertex_at(model, corner) else {
        return Some(NOT_A_CORNER);
    };
    plan(model, v, radius).err()
}

/// A tenth of the shorter edge at the corner (the radius when none was
/// typed). None if it is not a corner.
pub fn default_radius(model: &Model, corner: DVec3) -> Option<f64> {
    let v = vertex_at(model, corner)?;
    let p = model.vertex(v);
    model
        .edges()
        .filter(|(_, e)| e.has(v))
        .map(|(_, e)| model.vertex(if e.a == v { e.b } else { e.a }).distance(p) / 10.0)
        .min_by(f64::total_cmp)
}

/// Round the corner at `corner` with an arc of `radius` tangent to both
/// edges, trimming them to it. Returns false (and changes nothing) if it
/// cannot ([`refusal`] says why).
pub fn fillet(model: &mut Model, corner: DVec3, radius: f64) -> bool {
    let Some(v) = vertex_at(model, corner) else {
        return false;
    };
    let Ok(plan) = plan(model, v, radius) else {
        return false;
    };
    let tol = model.tol();
    let (a, b) = (
        insert_point(model, plan.a, &tol),
        insert_point(model, plan.b, &tol),
    );
    add_arc(model, plan.centre, plan.normal, radius, plan.a, plan.sweep);
    // The two short pieces at the corner, and the corner piece with them.
    let short: Vec<_> = model
        .edges()
        .filter(|(_, e)| e.has(v) && (e.has(a) || e.has(b)))
        .map(|(id, _)| id)
        .collect();
    let mut sel = Selection::default();
    sel.edges.extend(short);
    remove(model, &sel);
    true
}
