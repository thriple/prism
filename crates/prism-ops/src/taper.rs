//! Taper (Shapes & Placement part 7a): shrink or grow a whole group
//! toward or away from an axis, by a factor that runs linearly from 1.0
//! at one end to a chosen factor at the other. A flat face stays flat
//! exactly when its plane is square to the axis (a cap: every point on
//! it is at the same height, so the whole face is one uniform scale) or
//! holds the axis direction (a wall, built only of straight edges: its
//! corners move to new positions that are still provably coplanar). A
//! cylinder on the axis becomes a cone (its radius is now linear in
//! height, which is exactly what a cone is). Anything else -- a slanted
//! face, a sphere, a torus, a cylinder or cone off the axis, a spline --
//! cannot stay exact, so it is refused rather than approximated.

use std::collections::BTreeMap;

use prism_geom::{Cone, Curve, Cylinder, Surface};
use prism_math::{DVec3, Plane};
use prism_topo::{EdgeId, FaceId, Model, VertexId};

pub const AXIS: &str = "Taper needs an axis to shrink or grow toward";
pub const SPAN: &str = "The two ends of a taper must be different places along the axis";
pub const SCALE: &str = "A taper's far end must be a positive size, not zero or less";
pub const CANNOT: &str = "This cannot be tapered exactly: a slanted face, a diagonal edge, or a curved face not on the axis";

/// Two directions at right angles, or close enough.
fn parallel(a: DVec3, b: DVec3) -> bool {
    a.cross(b).length() < 1e-9
}

fn perpendicular(a: DVec3, b: DVec3) -> bool {
    a.dot(b).abs() < 1e-9
}

/// The scale at height `h` along the axis: 1.0 at `low`, `top_scale` at
/// `high`, linear in between (and beyond).
fn scale_at(h: f64, low: f64, high: f64, top_scale: f64) -> f64 {
    1.0 + (h - low) / (high - low) * (top_scale - 1.0)
}

fn moved(p: DVec3, origin: DVec3, axis: DVec3, low: f64, high: f64, top_scale: f64) -> DVec3 {
    let h = (p - origin).dot(axis);
    let r = p - origin - axis * h;
    origin + axis * h + r * scale_at(h, low, high, top_scale)
}

/// Shrink or grow every face and edge of `model` toward `axis` (through
/// `origin`), by `top_scale` at height `high` along it (1.0 at `low`).
/// Nothing is changed unless every face and edge can stay exact.
pub fn taper(
    model: &mut Model,
    origin: DVec3,
    axis: DVec3,
    low: f64,
    high: f64,
    top_scale: f64,
) -> Result<(), &'static str> {
    let axis = axis.normalize_or_zero();
    if axis == DVec3::ZERO {
        return Err(AXIS);
    }
    if (high - low).abs() < 1e-9 {
        return Err(SPAN);
    }
    if top_scale.is_nan() || top_scale <= 1e-6 {
        return Err(SCALE);
    }
    let tol = model.tol();
    let moved = |p: DVec3| moved(p, origin, axis, low, high, top_scale);

    // Every arc must lie in a plane square to the axis (a cap's rim), so
    // scaling it in place keeps it a circle; anything else cannot stay
    // exact, and a spline never can.
    let mut new_curve: BTreeMap<EdgeId, Curve> = BTreeMap::new();
    for (id, e) in model.edges() {
        match e.curve {
            Curve::Line => {}
            Curve::Arc {
                centre,
                axis: a,
                radius,
            } => {
                if !parallel(a, axis) {
                    return Err(CANNOT);
                }
                let h = (centre - origin).dot(axis);
                let ha = (model.vertex(e.a) - origin).dot(axis);
                let hb = (model.vertex(e.b) - origin).dot(axis);
                if (ha - h).abs() > tol.dist || (hb - h).abs() > tol.dist {
                    return Err(CANNOT);
                }
                let radius = radius * scale_at(h, low, high, top_scale);
                if radius <= tol.dist {
                    return Err(CANNOT);
                }
                new_curve.insert(
                    id,
                    Curve::Arc {
                        centre: moved(centre),
                        axis: a,
                        radius,
                    },
                );
            }
            Curve::Spline(_) => return Err(CANNOT),
        }
    }

    let new_vertex: BTreeMap<VertexId, DVec3> =
        model.vertices().map(|(id, p)| (id, moved(p))).collect();

    let mut new_surface: BTreeMap<FaceId, Surface> = BTreeMap::new();
    for (id, f) in model.faces() {
        match &f.surface {
            Surface::Plane(p) => {
                let plane = if parallel(p.normal, axis) {
                    // A cap: every point on it is at the same height, so
                    // the whole face is one uniform scale.
                    Plane::new(moved(p.origin), p.normal)
                } else if perpendicular(p.normal, axis) {
                    // A wall: only exact if every edge is straight (an
                    // arc here would mean a circle crossing the axis,
                    // which the edge pass above already ruled out, but
                    // check again directly for safety).
                    for k in 0..=f.holes.len() {
                        for e in f.loop_edge_ids(k) {
                            if model.edge(*e).is_some_and(|e| e.curve != Curve::Line) {
                                return Err(CANNOT);
                            }
                        }
                    }
                    let pts: Vec<DVec3> = f.loops().flatten().map(|v| new_vertex[v]).collect();
                    let fit = Plane::fit(&pts, &tol).ok_or(CANNOT)?;
                    if pts.iter().any(|p| fit.distance(*p).abs() > tol.dist) {
                        return Err(CANNOT);
                    }
                    fit
                } else {
                    return Err(CANNOT);
                };
                new_surface.insert(id, Surface::Plane(plane));
            }
            Surface::Cylinder(c) => {
                if !parallel(c.axis, axis) || (c.axis_point(origin) - origin).length() > tol.dist {
                    return Err(CANNOT);
                }
                let h0 = (c.origin - origin).dot(axis);
                let slope = c.radius * (top_scale - 1.0) / (high - low);
                let surface = if slope.abs() < 1e-12 {
                    Surface::Cylinder(Cylinder {
                        origin: moved(c.origin),
                        axis: c.axis,
                        radius: c.radius * scale_at(h0, low, high, top_scale),
                        outward: c.outward,
                    })
                } else {
                    let h_apex = h0 - c.radius / slope;
                    let half_angle = slope.abs().atan();
                    if half_angle <= 1e-9 || half_angle >= std::f64::consts::FRAC_PI_2 - 1e-9 {
                        return Err(CANNOT);
                    }
                    Surface::Cone(Cone {
                        apex: origin + axis * h_apex,
                        axis: if slope > 0.0 { axis } else { -axis },
                        half_angle,
                        outward: c.outward,
                    })
                };
                new_surface.insert(id, surface);
            }
            Surface::Cone(_) | Surface::Sphere(_) | Surface::Torus(_) | Surface::Nurbs(_) => {
                return Err(CANNOT);
            }
        }
    }

    for (v, p) in new_vertex {
        model.set_vertex(v, p);
    }
    for (e, c) in new_curve {
        model.set_curve(e, c);
    }
    for (f, s) in new_surface {
        model.edit_face(f, |face| face.surface = s);
    }
    Ok(())
}

/// Why a taper is refused, trying it on a copy.
pub fn refusal(
    model: &Model,
    origin: DVec3,
    axis: DVec3,
    low: f64,
    high: f64,
    top_scale: f64,
) -> Option<&'static str> {
    taper(&mut model.clone(), origin, axis, low, high, top_scale).err()
}
