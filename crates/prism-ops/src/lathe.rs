//! A closed profile turned a full circle about an axis (Exact Curves 5b):
//! exact cones, spheres, tori, cylinders and flat rings. Used to build
//! shapes for tests and MCP until Follow Me (brief 26, step 5d) makes them
//! from the tools.
//!
//! As a cylinder from Push/Pull, every turned piece is cut in two halves
//! by seams at angle 0 and half a turn, so each face's loops close when
//! unrolled. The pieces meet in soft, smooth seams where they are tangent.

use prism_geom::{Cone, Curve, Cylinder, Sphere, Surface, Torus};
use prism_math::{DVec2, DVec3, Plane};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

/// One side of a profile: from its point to the next one's, straight, or
/// round the circle about `centre` (in the profile's (ρ, h) plane),
/// counter-clockwise there if `ccw`.
#[derive(Clone, Copy, Debug)]
pub enum Side {
    Line,
    Arc { centre: DVec2, ccw: bool },
}

/// Turn the closed profile `profile` (points as (ρ, h): distance from the
/// axis and height along it, each with the side to the next point; listed
/// counter-clockwise, so the solid is on the left) about the line through
/// `origin` along unit `axis`. Adds the shape's own vertices, edges and
/// faces (it does not join what is already there) and returns its faces,
/// or None if a side is not a profile this can turn (a circle that
/// crosses the axis, a point off the side's circle).
pub fn lathe(
    model: &mut Model,
    origin: DVec3,
    axis: DVec3,
    profile: &[(DVec2, Side)],
) -> Option<Vec<FaceId>> {
    let tol = model.tol().dist.max(1e-9);
    let axis = axis.normalize();
    let r0 = Plane::new(origin, axis).frame().u;
    let out_at = |half: usize| if half == 0 { r0 } else { -r0 };
    let place = |q: DVec2, half: usize| origin + out_at(half) * q.x + axis * q.y;
    let on_axis = |q: DVec2| q.x.abs() <= tol;
    let n = profile.len();
    if n < 2 {
        return None;
    }
    // Each point: one vertex on the axis, else one at each seam and the
    // two halves of its circle.
    let mut at: Vec<[VertexId; 2]> = Vec::with_capacity(n);
    let mut rings: Vec<Option<[EdgeId; 2]>> = Vec::with_capacity(n);
    for (q, _) in profile {
        if on_axis(*q) {
            let v = model.add_vertex(origin + axis * q.y);
            at.push([v, v]);
            rings.push(None);
            continue;
        }
        let vs = [model.add_vertex(place(*q, 0)), model.add_vertex(place(*q, 1))];
        let c = Curve::Arc {
            centre: origin + axis * q.y,
            axis,
            radius: q.x,
        };
        let id = model.new_curve_id();
        let a = model.add_curve_edge(vs[0], vs[1], c, Some(id))?;
        let b = model.add_curve_edge(vs[1], vs[0], c, Some(id))?;
        at.push(vs);
        rings.push(Some([a, b]));
    }
    let mut faces = Vec::new();
    let mut seams_of: Vec<[EdgeId; 2]> = Vec::new();
    for i in 0..n {
        let j = (i + 1) % n;
        let ((p, side), q) = (profile[i], profile[j].0);
        if on_axis(p) && on_axis(q) {
            continue;
        }
        let surface = surface(origin, axis, p, q, side, tol)?;
        // The two seams, one per half, along the side.
        let mut seams = [EdgeId(0); 2];
        for (half, seam) in seams.iter_mut().enumerate() {
            let (a, b) = (at[i][half], at[j][half]);
            *seam = match side {
                Side::Line => model.add_edge(a, b)?,
                Side::Arc { centre, ccw } => {
                    let out = out_at(half);
                    let normal = out.cross(axis) * if ccw { 1.0 } else { -1.0 };
                    let c = Curve::Arc {
                        centre: origin + out * centre.x + axis * centre.y,
                        axis: normal,
                        radius: p.distance(centre),
                    };
                    model.add_curve_edge(a, b, c, None)?
                }
            };
        }
        seams_of.push(seams);
        for half in 0..2 {
            let other = 1 - half;
            // Round at p from this seam to the other, along the other
            // seam, back round at q, and back along this seam.
            let mut verts = vec![at[i][half]];
            let mut edges = Vec::new();
            if let Some(r) = rings[i] {
                verts.push(at[i][other]);
                edges.push(r[half]);
            }
            edges.push(seams[other]);
            verts.push(at[j][other]);
            if let Some(r) = rings[j] {
                verts.push(at[j][half]);
                edges.push(r[half]);
            }
            edges.push(seams[half]);
            let id = model.add_face(Face::with_edges(verts, vec![], surface, vec![edges]));
            if winds_wrong(model, id) {
                model.edit_face(id, |f| {
                    let s = f.surface;
                    f.reverse();
                    f.surface = s;
                });
            }
            faces.push(id);
        }
    }
    // Seams inside a turned piece, and rings between pieces that meet
    // without a crease, are soft and smooth.
    for seams in &seams_of {
        for s in seams {
            soften_if_smooth(model, *s);
        }
    }
    for r in rings.iter().flatten() {
        for e in r {
            soften_if_smooth(model, *e);
        }
    }
    Some(faces)
}

/// The surface `side` turns into, from `p` to `q`, facing away from the
/// solid on its left.
fn surface(origin: DVec3, axis: DVec3, p: DVec2, q: DVec2, side: Side, tol: f64) -> Option<Surface> {
    let d = q - p;
    Some(match side {
        Side::Line if d.x.abs() <= tol => Surface::Cylinder(Cylinder {
            origin,
            axis,
            radius: p.x,
            outward: d.y > 0.0,
        }),
        Side::Line if d.y.abs() <= tol => {
            Surface::Plane(Plane::new(origin + axis * p.y, axis * -d.x.signum()))
        }
        Side::Line => {
            let slope = d.x / d.y;
            Surface::Cone(Cone {
                origin,
                axis,
                radius: p.x - slope * p.y,
                slope,
                outward: d.y > 0.0,
            })
        }
        Side::Arc { centre, ccw } => {
            let r = p.distance(centre);
            if (q.distance(centre) - r).abs() > tol || r <= tol {
                return None;
            }
            let at = origin + axis * centre.y;
            if centre.x.abs() <= tol {
                Surface::Sphere(Sphere {
                    centre: at,
                    axis,
                    radius: r,
                    outward: ccw,
                })
            } else if centre.x > r + tol {
                Surface::Torus(Torus {
                    centre: at,
                    axis,
                    major: centre.x,
                    minor: r,
                    outward: ccw,
                })
            } else {
                return None;
            }
        }
    })
}

/// Whether the face's outer loop winds against its normal.
fn winds_wrong(model: &Model, id: FaceId) -> bool {
    let Some(f) = model.face(id) else {
        return false;
    };
    if let Some(pl) = f.plane() {
        return prism_math::newell(&model.loop_points(f, 0)).dot(pl.normal) < 0.0;
    }
    if let Some(t) = f.surface.turned() {
        let p: Vec<DVec2> = model.unrolled(f, 0).into_iter().map(|x| x.1).collect();
        return t.loop_integrals(&p).0 < 0.0;
    }
    let p: Vec<DVec2> = model.unrolled(f, 0).into_iter().map(|x| x.1).collect();
    let area: f64 = (0..p.len())
        .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
        .sum();
    (area > 0.0) != f.surface.outward()
}

/// Soften and smooth an edge whose two faces meet without a crease.
fn soften_if_smooth(model: &mut Model, e: EdgeId) {
    let Some(edge) = model.edge(e).copied() else {
        return;
    };
    let faces = model.edge_faces().get(&e).cloned().unwrap_or_default();
    let [f, g] = faces.as_slice() else { return };
    let mid = edge
        .curve
        .point(model.vertex(edge.a), model.vertex(edge.b), 0.5);
    let n = |id: FaceId| model.face(id).map(|x| x.surface.normal_at(mid));
    if let (Some(a), Some(b)) = (n(*f), n(*g))
        && a.dot(b) > 1.0 - 1e-9
    {
        model.set_soft(e, true, true);
    }
}
