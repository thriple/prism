//! Trim, Extend and Project Curves (Shapes & Placement part 5).
//!
//! * **Trim** takes away one piece of a loose line or arc (the piece the
//!   click is on, between where it is crossed or joined). It never touches
//!   a face: an edge that bounds one is refused (that is the Eraser's job).
//! * **Extend** carries a straight edge on from a free end to the first
//!   thing in the way: another edge, or a face.
//! * **Project** drops lines (and arcs turned the same way as the face)
//!   straight onto a flat face, where they cut it like any drawn edge.

use std::collections::BTreeSet;

use prism_geom::Curve;
use prism_math::DVec3;
use prism_topo::{EdgeId, FaceId, Model, VertexId};

use crate::sticky::{add_arc, add_circle, add_segment};

pub const NOT_LOOSE: &str = "That edge bounds a face: use the Eraser, or Trim a loose edge";
pub const NOT_STRAIGHT: &str = "Extend works on straight edges for now";
pub const END_JOINED: &str = "That end is joined to something: Extend works from a free end";
pub const NOTHING_IN_THE_WAY: &str = "Nothing in the way: carried on, the edge meets nothing";
pub const NOT_FLAT: &str = "Project onto a flat face";
pub const NOTHING_TO_PROJECT: &str =
    "Select lines (or arcs turned the same way as the face) to project first";

/// Whether an edge bounds no face.
pub fn is_loose(model: &Model, edge: EdgeId) -> bool {
    model
        .edge_faces()
        .get(&edge)
        .is_none_or(std::vec::Vec::is_empty)
}

/// Why Trim refuses an edge, if it does.
pub fn trim_refusal(model: &Model, edge: EdgeId) -> Option<&'static str> {
    if model.edge(edge).is_none() {
        return Some("There is no such edge");
    }
    (!is_loose(model, edge)).then_some(NOT_LOOSE)
}

/// Where the straight `edge` carried on from its free `end` first meets
/// something, or why it does not.
pub fn extension(
    model: &Model,
    edge: EdgeId,
    end: VertexId,
) -> Result<(DVec3, DVec3), &'static str> {
    let e = model.edge(edge).ok_or("There is no such edge")?;
    if e.curve.is_curved() {
        return Err(NOT_STRAIGHT);
    }
    if !e.has(end) {
        return Err("That is not an end of the edge");
    }
    // Free: no other edge touches it.
    if model.edges().any(|(id, x)| id != edge && x.has(end)) {
        return Err(END_JOINED);
    }
    let other = if e.a == end { e.b } else { e.a };
    let p = model.vertex(end);
    let d = (p - model.vertex(other)).normalize_or_zero();
    if d == DVec3::ZERO {
        return Err(NOTHING_IN_THE_WAY);
    }
    let tol = model.tol().dist;
    let mut best: Option<f64> = None;
    let mut consider = |t: f64| {
        if t > tol * 4.0 && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    };
    for (id, x) in model.edges() {
        if id == edge || x.has(end) {
            continue;
        }
        match &x.curve {
            Curve::Line => {
                let (a, b) = (model.vertex(x.a), model.vertex(x.b));
                if let Some(t) = ray_segment(p, d, a, b, tol) {
                    consider(t);
                }
            }
            Curve::Arc {
                centre,
                axis,
                radius,
            } => {
                let den = d.dot(*axis);
                if den.abs() < prism_math::tol::UNIT_GUARD {
                    continue;
                }
                let t = (*centre - p).dot(*axis) / den;
                let hit = p + d * t;
                if ((hit - *centre).length() - radius).abs() <= tol * 2.0
                    && on_arc(
                        *centre,
                        *axis,
                        model.vertex(x.a),
                        model.vertex(x.b),
                        hit,
                        tol,
                    )
                {
                    consider(t);
                }
            }
            Curve::Spline(_) => {}
        }
    }
    for (id, f) in model.faces() {
        let Some(pl) = f.plane() else { continue };
        let den = d.dot(pl.normal);
        if den.abs() < prism_math::tol::UNIT_GUARD {
            continue;
        }
        let t = (pl.origin - p).dot(pl.normal) / den;
        if t > tol * 4.0 && crate::face_at(model, p + d * t) == Some(id) {
            consider(t);
        }
    }
    best.map(|t| (p, p + d * t)).ok_or(NOTHING_IN_THE_WAY)
}

/// Carry a straight edge on from its free end to the first thing it
/// meets; the new piece cuts what it meets as any drawn line does.
pub fn extend(model: &mut Model, edge: EdgeId, end: VertexId) -> Result<(), &'static str> {
    let (from, to) = extension(model, edge, end)?;
    add_segment(model, from, to);
    Ok(())
}

/// The distance along the ray `p + t d` (unit `d`) at which it meets the
/// segment `a`-`b`, if it does (within `tol`).
fn ray_segment(p: DVec3, d: DVec3, a: DVec3, b: DVec3, tol: f64) -> Option<f64> {
    let v = b - a;
    let w = p - a;
    let (bb, c) = (d.dot(v), v.dot(v));
    let (dd, e) = (d.dot(w), v.dot(w));
    let den = c - bb * bb;
    if den.abs() < 1e-12 * c.max(1e-30) {
        return None;
    }
    let t = (bb * e - c * dd) / den;
    let s = (e - bb * dd) / den;
    let (pp, qq) = (p + d * t, a + v * s);
    let len = c.sqrt();
    (pp.distance(qq) <= tol * 2.0 && s * len >= -tol && s * len <= len + tol).then_some(t)
}

/// Whether `hit` (on the circle) lies on the arc from `a` to `b`, counter-
/// clockwise about `axis`.
fn on_arc(centre: DVec3, axis: DVec3, a: DVec3, b: DVec3, hit: DVec3, tol: f64) -> bool {
    let angle = |q: DVec3| {
        let (u, v) = (a - centre, q - centre);
        let t = u.cross(v).dot(axis).atan2(u.dot(v));
        if t < 0.0 {
            t + std::f64::consts::TAU
        } else {
            t
        }
    };
    let r = (a - centre).length().max(1e-30);
    let sweep = if a.distance(b) <= tol {
        std::f64::consts::TAU
    } else {
        angle(b)
    };
    angle(hit) <= sweep + tol / r
}

/// Project the `edges` straight onto the flat `face`: lines, and arcs or
/// circles in a plane parallel to it. Returns how many were projected and
/// how many left out (curves of other kinds, edges square to the face,
/// edges of the face itself).
pub fn project(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    face: FaceId,
) -> Result<(usize, usize), &'static str> {
    let f = model.face(face).ok_or(NOT_FLAT)?;
    let pl = *f.plane().ok_or(NOT_FLAT)?;
    let n = pl.normal.normalize_or_zero();
    let own: BTreeSet<EdgeId> = f
        .loops()
        .enumerate()
        .flat_map(|(k, _)| f.loop_edge_ids(k).to_vec())
        .collect();
    let tol = model.tol().dist;
    let down = |p: DVec3| p - n * (p - pl.origin).dot(n);
    let mut jobs: Vec<Job> = Vec::new();
    let mut left = 0;
    for id in edges {
        let Some(e) = model.edge(*id) else { continue };
        if own.contains(id) {
            left += 1;
            continue;
        }
        let (a, b) = (model.vertex(e.a), model.vertex(e.b));
        match &e.curve {
            Curve::Line => {
                let (pa, pb) = (down(a), down(b));
                if pa.distance(pb) <= tol {
                    left += 1;
                } else {
                    jobs.push(Job::Line(pa, pb));
                }
            }
            Curve::Arc {
                centre,
                axis,
                radius,
            } if axis.dot(n).abs() > 1.0 - prism_math::tol::ANGLE => {
                let c = down(*centre);
                if e.a == e.b || a.distance(b) <= tol {
                    jobs.push(Job::Circle(c, *axis, *radius));
                } else {
                    let (u, v) = (a - *centre, b - *centre);
                    let mut sweep = u.cross(v).dot(*axis).atan2(u.dot(v));
                    if sweep < 0.0 {
                        sweep += std::f64::consts::TAU;
                    }
                    jobs.push(Job::Arc(c, *axis, *radius, down(a), sweep));
                }
            }
            _ => left += 1,
        }
    }
    if jobs.is_empty() {
        return Err(NOTHING_TO_PROJECT);
    }
    let done = jobs.len();
    for j in jobs {
        match j {
            Job::Line(a, b) => {
                add_segment(model, a, b);
            }
            Job::Circle(c, axis, r) => {
                add_circle(model, c, axis, r);
            }
            Job::Arc(c, axis, r, start, sweep) => {
                add_arc(model, c, axis, r, start, sweep);
            }
        }
    }
    Ok((done, left))
}

enum Job {
    Line(DVec3, DVec3),
    Circle(DVec3, DVec3, f64),
    Arc(DVec3, DVec3, f64, DVec3, f64),
}
