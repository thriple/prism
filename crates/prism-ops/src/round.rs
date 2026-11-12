//! Fillet and chamfer of a solid's edge (Exact Curves 5a): a straight
//! edge between two flat faces is cut off by a flat face (chamfer) or
//! rounded by a cylinder face tangent to both (fillet). The faces at its
//! ends get a matching line or arc.

use prism_geom::{Curve, Cylinder, Surface};
use prism_math::{DVec3, Plane};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

use crate::select::{Selection, remove};
use crate::{add_arc, add_segment, insert_point};

/// Said when the edge is not a straight edge between two flat faces at an
/// angle, each end a plain corner.
pub const NOT_AN_EDGE: &str = "Only a straight edge between two flat faces at an angle, with plain corners at its ends, can be rounded.";
/// Said when the faces are too narrow for the size.
pub const TOO_WIDE: &str = "The faces beside this edge are too narrow for that size.";
/// Said when a fillet's end would be an ellipse.
pub const SLANTED_END: &str =
    "A fillet needs the faces at the edge's ends square to it (a slanted end would be an ellipse).";

/// One end of the edge: its corner, the cut points on each face's other
/// edge there, and the face across the end (if any).
struct End {
    v: VertexId,
    p1: DVec3,
    p2: DVec3,
    across: Option<FaceId>,
    /// The plane across the end, when it is not square to the edge: a
    /// fillet meets it in an ellipse, made of approximate segments
    /// (DECISIONS #32).
    slanted: Option<Plane>,
}

/// What a fillet or chamfer of an edge makes.
struct Plan {
    dir: DVec3,
    n1: DVec3,
    n2: DVec3,
    /// From the edge, into each face, square to the edge.
    u1: DVec3,
    u2: DVec3,
    ends: [End; 2],
    /// For a fillet: the radius and the angle between the faces.
    radius: Option<f64>,
}

/// The two faces on `edge`, with each one's direction into the face,
/// square to the edge (from `a` to `b`).
fn sides(model: &Model, edge: EdgeId) -> Option<[(FaceId, DVec3, DVec3); 2]> {
    let e = model.edge(edge)?;
    let dir = (model.vertex(e.b) - model.vertex(e.a)).normalize_or_zero();
    let mut out = Vec::new();
    for (id, f) in model.faces() {
        let Some(plane) = f.plane() else { continue };
        for k in 0..f.loops().count() {
            let Some(vs) = f.loops().nth(k) else { continue };
            for (i, id_e) in f.loop_edge_ids(k).iter().enumerate() {
                if *id_e == edge {
                    // The loop runs counter-clockwise about the normal, so
                    // the face lies to the left of its walk.
                    let walk = if vs[i] == e.a { dir } else { -dir };
                    out.push((id, plane.normal, plane.normal.cross(walk)));
                }
            }
        }
    }
    out.try_into().ok()
}

fn plan(model: &Model, edge: EdgeId, size: f64, fillet: bool) -> Result<Plan, &'static str> {
    let tol = model.tol();
    let e = model.edge(edge).ok_or(NOT_AN_EDGE)?;
    if e.curve.is_curved() || size <= tol.dist {
        return Err(NOT_AN_EDGE);
    }
    let [(f1, n1, u1), (f2, n2, u2)] = sides(model, edge).ok_or(NOT_AN_EDGE)?;
    if f1 == f2 || tol.parallel(n1, n2) {
        return Err(NOT_AN_EDGE);
    }
    let (a, b) = (e.a, e.b);
    let dir = (model.vertex(b) - model.vertex(a)).normalize();
    let phi = u1.dot(u2).clamp(-1.0, 1.0).acos();
    // How far from the edge, square to it, each face is cut.
    let t = if fillet {
        size / (phi / 2.0).tan()
    } else {
        size
    };
    let f1v = model.face(f1).ok_or(NOT_AN_EDGE)?;
    let f2v = model.face(f2).ok_or(NOT_AN_EDGE)?;
    let end = |v: VertexId| -> Result<End, &'static str> {
        let at: Vec<_> = model.edges().filter(|(_, x)| x.has(v)).collect();
        if at.len() != 3 {
            return Err(NOT_AN_EDGE);
        }
        // Each face's other edge at this corner.
        let other = |f: &Face| -> Option<(EdgeId, VertexId)> {
            at.iter()
                .find(|(id, _)| *id != edge && f.edge_ids().any(|x| x == *id))
                .map(|(id, x)| (*id, x.other(v)))
        };
        let (Some((w1, q1)), Some((w2, q2))) = (other(f1v), other(f2v)) else {
            return Err(NOT_AN_EDGE);
        };
        if w1 == w2 || at.iter().any(|(_, x)| x.curve.is_curved()) {
            return Err(NOT_AN_EDGE);
        }
        let p = model.vertex(v);
        let cut = |q: VertexId, u: DVec3| -> Result<DVec3, &'static str> {
            let s = model.vertex(q) - p;
            let k = s.normalize_or_zero().dot(u);
            if k <= 1e-9 {
                return Err(NOT_AN_EDGE);
            }
            let len = t / k;
            if len >= s.length() - tol.dist {
                return Err(TOO_WIDE);
            }
            Ok(p + s.normalize() * len)
        };
        let (p1, p2) = (cut(q1, u1)?, cut(q2, u2)?);
        // The face across the end holds both other edges.
        let across: Vec<FaceId> = model
            .faces()
            .filter(|(id, f)| *id != f1 && *id != f2 && f.edge_ids().any(|x| x == w1 || x == w2))
            .map(|(id, _)| id)
            .collect();
        let across = match across.as_slice() {
            [] => None,
            [g] => {
                let g = model.face(*g).ok_or(NOT_AN_EDGE)?;
                if g.plane().is_none()
                    || !(g.edge_ids().any(|x| x == w1) && g.edge_ids().any(|x| x == w2))
                {
                    return Err(NOT_AN_EDGE);
                }
                Some(across[0])
            }
            _ => return Err(NOT_AN_EDGE),
        };
        // Slanted: the plane of the corner's other two edges leans.
        let (s1, s2) = (model.vertex(q1) - p, model.vertex(q2) - p);
        let slanted = (fillet
            && !(tol.perpendicular(s1.normalize(), dir) && tol.perpendicular(s2.normalize(), dir)))
        .then(|| Plane::new(p, s1.cross(s2).normalize_or_zero()))
        .filter(|pl| pl.normal != DVec3::ZERO);
        if slanted.is_some_and(|pl| pl.normal.dot(dir).abs() < 1e-9) {
            return Err(SLANTED_END);
        }
        Ok(End {
            v,
            p1,
            p2,
            across,
            slanted,
        })
    };
    let ends = [end(a)?, end(b)?];
    Ok(Plan {
        dir,
        n1,
        n2,
        u1,
        u2,
        ends,
        radius: fillet.then_some(size),
    })
}

/// Why `edge` cannot be filleted (`fillet`) or chamfered to `size`, if it
/// cannot. A chamfer's size is how far it cuts into each face; a fillet's
/// is its radius.
pub fn refusal(model: &Model, edge: EdgeId, size: f64, fillet: bool) -> Option<&'static str> {
    plan(model, edge, size, fillet).err()
}

/// A tenth of the narrower face beside `edge`, measured square to it (the
/// size when none was typed).
pub fn default_size(model: &Model, edge: EdgeId) -> Option<f64> {
    let e = model.edge(edge)?;
    let (a, dir) = (
        model.vertex(e.a),
        (model.vertex(e.b) - model.vertex(e.a)).normalize_or_zero(),
    );
    sides(model, edge)?
        .iter()
        .filter_map(|(f, _, u)| {
            let f = model.face(*f)?;
            f.outer
                .iter()
                .map(|v| (model.vertex(*v) - a).dot(*u))
                .max_by(f64::total_cmp)
        })
        .min_by(f64::total_cmp)
        .map(|w| w / 10.0)
        .filter(|w| *w > 0.0 && dir != DVec3::ZERO)
}

/// Fillet (`fillet`) or chamfer `edge` to `size`. Returns false, changing
/// nothing, if it cannot ([`refusal`] says why).
pub fn round(model: &mut Model, edge: EdgeId, size: f64, fillet: bool) -> bool {
    let Ok(plan) = plan(model, edge, size, fillet) else {
        return false;
    };
    // The new face takes the faces' colour: the one they share, else the
    // painted one (as SketchUp gives it a neighbour's material).
    let paint = sides(model, edge)
        .map(|[(f1, ..), (f2, ..)]| {
            let get = |f| model.face(f).map(|x| x.paint).unwrap_or_default();
            let (p1, p2) = (get(f1), get(f2));
            if p1.is_default() { p2 } else { p1 }
        })
        .unwrap_or_default();
    let tol = model.tol();
    let corner_a = model.vertex(plan.ends[0].v);
    let half = plan.u1.dot(plan.u2).clamp(-1.0, 1.0).acos() / 2.0;
    // A fillet's axis, square to the edge from a corner point `p`.
    let centre = |p: DVec3, r: f64| p + (plan.u1 + plan.u2).normalize() * (r / half.sin());
    // The cut points, and the lines along each face.
    let ids: Vec<[VertexId; 2]> = plan
        .ends
        .iter()
        .map(|end| {
            [
                insert_point(model, end.p1, &tol),
                insert_point(model, end.p2, &tol),
            ]
        })
        .collect();
    let (a1, a2, b1, b2) = (ids[0][0], ids[0][1], ids[1][0], ids[1][1]);
    let [ea, eb] = &plan.ends;
    add_segment(model, ea.p1, eb.p1);
    add_segment(model, ea.p2, eb.p2);
    // Across each end: a line, an arc about the fillet's axis, or (at a
    // slanted end) an ellipse of approximate segments. Each is a chain of
    // points from p1 to p2 and the edges between them.
    let mut across: Vec<Option<(Vec<VertexId>, Vec<EdgeId>)>> = Vec::new();
    for (end, (p1, p2)) in plan.ends.iter().zip([(a1, a2), (b1, b2)]) {
        let one = |e: Option<EdgeId>| e.map(|e| (vec![p1, p2], vec![e]));
        let joined = match (plan.radius, end.slanted) {
            (None, _) => one(if end.across.is_some() {
                add_segment(model, end.p1, end.p2);
                model.line_between(p1, p2)
            } else {
                model.add_edge(p1, p2)
            }),
            (Some(r), None) => {
                let c = centre(model.vertex(end.v), r);
                // The short way round, from p1 to p2.
                let n = (end.p1 - c).cross(end.p2 - c).normalize();
                let sweep = std::f64::consts::PI - 2.0 * half;
                one(if end.across.is_some() {
                    add_arc(model, c, n, r, end.p1, sweep);
                    model
                        .edges()
                        .find(|(_, x)| x.has(p1) && x.has(p2) && x.curve.is_curved())
                        .map(|(id, _)| id)
                } else {
                    let cid = model.new_curve_id();
                    let curve = Curve::Arc {
                        centre: c,
                        axis: n,
                        radius: r,
                    };
                    model.add_curve_edge(p1, p2, curve, Some(cid))
                })
            }
            (Some(r), Some(pl)) => {
                // The circle square to the edge at the corner, each point
                // carried along the edge onto the slanted plane.
                let corner = model.vertex(end.v);
                let c = centre(corner, r);
                let t = r / half.tan();
                let (q1, q2) = (corner + plan.u1 * t, corner + plan.u2 * t);
                let n = (q1 - c).cross(q2 - c).normalize();
                let arc = Curve::Arc {
                    centre: c,
                    axis: n,
                    radius: r,
                };
                let steps = crate::approx::fine_count(arc.sweep(q1, q2));
                let onto = |x: DVec3| {
                    x + plan.dir * ((pl.origin - x).dot(pl.normal) / plan.dir.dot(pl.normal))
                };
                let mut chain = vec![p1];
                for i in 1..steps {
                    let x = onto(arc.point(q1, q2, i as f64 / steps as f64));
                    chain.push(if end.across.is_some() {
                        insert_point(model, x, &tol)
                    } else {
                        model.add_vertex(x)
                    });
                }
                chain.push(p2);
                let cid = model.new_curve_id();
                let mut es = Vec::new();
                for w in chain.windows(2) {
                    let e = if end.across.is_some() {
                        add_segment(model, model.vertex(w[0]), model.vertex(w[1]));
                        model.line_between(w[0], w[1])
                    } else {
                        model.add_edge(w[0], w[1])
                    };
                    let Some(e) = e else { break };
                    model.set_approx(e, true);
                    model.set_curve_id(e, Some(cid));
                    es.push(e);
                }
                (es.len() + 1 == chain.len()).then_some((chain, es))
            }
        };
        across.push(joined);
    }
    // The corner pieces go: the edge, and the short pieces at its ends.
    let mut gone = Selection::default();
    gone.edges.insert(edge);
    for end in &plan.ends {
        for (id, x) in model.edges() {
            if x.has(end.v) && id != edge {
                gone.edges.insert(id);
            }
        }
    }
    remove(model, &gone);
    let (Some(top), Some(bottom), [Some(at_a), Some(at_b)]) = (
        model.line_between(a1, b1),
        model.line_between(a2, b2),
        across.as_slice(),
    ) else {
        return true;
    };
    // The new face: along face 1, across b, back along face 2, across a.
    let mut outer = vec![a1];
    let mut edges = vec![top];
    outer.extend(at_b.0.iter().copied());
    edges.extend(at_b.1.iter().copied());
    outer.extend(at_a.0.iter().rev().copied().take(at_a.0.len() - 1));
    edges.push(bottom);
    edges.extend(at_a.1.iter().rev().copied());
    let edges = vec![edges];
    let out = (plan.n1 + plan.n2).normalize_or_zero();
    let made = match plan.radius {
        None => {
            // Drawing its last side may have closed the flat loop already.
            let corners: std::collections::BTreeSet<VertexId> = outer.iter().copied().collect();
            let made = model
                .faces()
                .find(|(_, f)| {
                    f.holes.is_empty()
                        && f.outer
                            .iter()
                            .copied()
                            .collect::<std::collections::BTreeSet<_>>()
                            == corners
                })
                .map(|(id, _)| id);
            let id = match made {
                Some(id) => id,
                None => {
                    let pts = model.points(&outer);
                    let Some(plane) = Plane::fit(&pts, &tol) else {
                        return true;
                    };
                    model.add_face(Face::with_edges(
                        outer,
                        vec![],
                        Surface::Plane(plane),
                        edges,
                    ))
                }
            };
            if model
                .face(id)
                .and_then(|f| f.plane())
                .is_some_and(|p| p.normal.dot(out) < 0.0)
            {
                model.edit_face(id, |f| f.reverse());
            }
            id
        }
        Some(r) => {
            let origin = centre(corner_a, r);
            let outward = (model.vertex(a1) - origin).dot(plan.n1) > 0.0;
            let cyl = Cylinder {
                origin,
                axis: plan.dir,
                radius: r,
                outward: true,
            };
            let id = model.add_face(Face::with_edges(
                outer,
                vec![],
                Surface::Cylinder(cyl),
                edges,
            ));
            // On an outward cylinder, a loop counter-clockwise unrolled
            // faces out; turn the loop round where the face should not.
            let ccw = model.face(id).is_some_and(|g| {
                let p = model.unrolled(g, 0);
                (0..p.len())
                    .map(|i| p[i].1.perp_dot(p[(i + 1) % p.len()].1))
                    .sum::<f64>()
                    > 0.0
            });
            model.edit_face(id, |f| {
                if ccw != outward {
                    f.reverse();
                }
                f.surface = Surface::Cylinder(Cylinder { outward, ..cyl });
            });
            model.set_soft(top, true, true);
            model.set_soft(bottom, true, true);
            id
        }
    };
    // Set after it was turned to face out.
    model.edit_face(made, |f| f.paint = paint);
    true
}
