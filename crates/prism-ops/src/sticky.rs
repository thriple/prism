//! Sticky geometry: points snap to existing vertices and split the edges
//! they land on; segments split at every vertex and crossing; closed
//! coplanar loops become faces; overlapping coplanar faces split.

use std::collections::BTreeSet;

use std::f64::consts::TAU;

use prism_geom::{Curve, circle_circle, circle_distance, circle_line};
use prism_math::{DVec3, Plane, Tol};
use prism_topo::{Edge, EdgeId, FaceId, Model, VertexId};

use crate::regions::{Fill, rebuild_plane};

/// What adding a segment changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Added {
    /// The edges added (pieces of the segment that were not there already).
    pub edges: Vec<EdgeId>,
    /// The faces made.
    pub faces: Vec<FaceId>,
}

/// The tolerance for the model once `extra` points are added.
pub fn tol_with(model: &Model, extra: &[DVec3]) -> Tol {
    let pts: Vec<DVec3> = model
        .vertices()
        .map(|(_, p)| p)
        .chain(extra.iter().copied())
        .collect();
    Tol::for_points(&pts)
}

/// Where `p` falls along the segment `a`–`b`, if it is on it and clear of
/// both ends (0 < t < 1).
pub fn on_segment_inside(p: DVec3, a: DVec3, b: DVec3, tol: &Tol) -> Option<f64> {
    let d = b - a;
    let len2 = d.length_squared();
    if len2 <= tol.area() {
        return None;
    }
    let t = (p - a).dot(d) / len2;
    let len = len2.sqrt();
    let foot = a + d * t;
    (p.distance(foot) < tol.dist && t * len > tol.dist && (1.0 - t) * len > tol.dist).then_some(t)
}

/// Whether `p` lies on the edge `e`, clear of both its ends.
fn on_edge_inside(model: &Model, e: &Edge, p: DVec3, tol: &Tol) -> bool {
    on_curve_inside(e.curve, model.vertex(e.a), model.vertex(e.b), p, tol)
}

/// Whether `p` lies on `curve` from `a` to `b`, clear of both ends.
pub(crate) fn on_curve_inside(curve: Curve, a: DVec3, b: DVec3, p: DVec3, tol: &Tol) -> bool {
    match curve {
        Curve::Line => on_segment_inside(p, a, b, tol).is_some(),
        Curve::Arc { .. } => {
            let c = curve.closest_point(a, b, p);
            c.distance(p) < tol.dist && c.distance(a) > tol.dist && c.distance(b) > tol.dist
        }
    }
}

/// Where two curves (each with its ends) cross, clear of all four ends.
pub(crate) fn meet(
    (c1, a1, b1): (Curve, DVec3, DVec3),
    (c2, a2, b2): (Curve, DVec3, DVec3),
    tol: &Tol,
) -> Vec<DVec3> {
    let pts = match (c1, c2) {
        (Curve::Line, Curve::Line) => {
            return crossing(a1, b1, a2, b2, tol)
                .map(|(_, q)| q)
                .into_iter()
                .collect();
        }
        (
            Curve::Arc {
                centre,
                axis,
                radius,
            },
            Curve::Line,
        ) => circle_line(centre, axis, radius, a2, b2, tol.dist),
        (
            Curve::Line,
            Curve::Arc {
                centre,
                axis,
                radius,
            },
        ) => circle_line(centre, axis, radius, a1, b1, tol.dist),
        (
            Curve::Arc {
                centre: p,
                axis: n,
                radius: r,
            },
            Curve::Arc {
                centre: q,
                axis: m,
                radius: s,
            },
        ) => circle_circle((p, n, r), (q, m, s), tol.dist),
    };
    pts.into_iter()
        .filter(|x| on_curve_inside(c1, a1, b1, *x, tol) && on_curve_inside(c2, a2, b2, *x, tol))
        .collect()
}

/// The vertex at `p`: an existing one within tolerance, or a new one that
/// splits the edge it lands on.
pub fn insert_point(model: &mut Model, p: DVec3, tol: &Tol) -> VertexId {
    let near = model
        .vertices()
        .map(|(id, q)| (id, q.distance(p)))
        .filter(|(_, d)| *d < tol.dist)
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    if let Some((id, _)) = near {
        return id;
    }
    let on_edge = model
        .edges()
        .find(|(_, e)| on_edge_inside(model, e, p, tol))
        .map(|(id, _)| id);
    let id = model.add_vertex(p);
    if let Some(e) = on_edge {
        model.split_edge(e, id);
    }
    id
}

/// Add the segment `p`–`q`, splitting it at every vertex it passes and every
/// edge it crosses, then make the faces it closes or splits.
pub fn add_segment(model: &mut Model, p: DVec3, q: DVec3) -> Added {
    let tol = tol_with(model, &[p, q]);
    let a = insert_point(model, p, &tol);
    let b = insert_point(model, q, &tol);
    if a == b {
        return Added::default();
    }
    let (pa, pb) = (model.vertex(a), model.vertex(b));
    let mut cuts: Vec<(f64, VertexId)> = model
        .vertices()
        .filter(|(id, _)| *id != a && *id != b)
        .filter_map(|(id, v)| on_segment_inside(v, pa, pb, &tol).map(|t| (t, id)))
        .collect();

    // Crossings with straight edges that do not touch the ends.
    let crossings: Vec<(EdgeId, f64, DVec3)> = model
        .edges()
        .filter(|(_, e)| !e.has(a) && !e.has(b) && !e.curve.is_arc())
        .filter_map(|(id, e)| {
            let (c, d) = (model.vertex(e.a), model.vertex(e.b));
            crossing(pa, pb, c, d, &tol).map(|(s, point)| (id, s, point))
        })
        .collect();
    for (edge, s, point) in crossings {
        // An earlier split may have used this point already.
        let existing = model
            .vertices()
            .find(|(_, v)| tol.same_point(*v, point))
            .map(|(id, _)| id);
        let id = match existing {
            Some(id) => id,
            None => {
                let id = model.add_vertex(point);
                model.split_edge(edge, id);
                id
            }
        };
        cuts.push((s, id));
    }
    // Crossings with arcs: up to two each, and an arc sharing an end with
    // the segment can still cross it elsewhere.
    let arc_points: Vec<DVec3> = {
        let model = &*model;
        model
            .edges()
            .flat_map(|(_, e)| {
                let pts = match e.curve {
                    Curve::Arc {
                        centre,
                        axis,
                        radius,
                    } => circle_line(centre, axis, radius, pa, pb, tol.dist),
                    Curve::Line => Vec::new(),
                };
                pts.into_iter().filter(move |x| {
                    on_edge_inside(model, e, *x, &tol)
                        && on_segment_inside(*x, pa, pb, &tol).is_some()
                })
            })
            .collect()
    };
    let d = pb - pa;
    for point in arc_points {
        let id = insert_point(model, point, &tol);
        cuts.push(((point - pa).dot(d) / d.length_squared(), id));
    }
    cuts.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    cuts.dedup_by_key(|c| c.1);

    let chain: Vec<VertexId> = std::iter::once(a)
        .chain(cuts.iter().map(|c| c.1))
        .chain(std::iter::once(b))
        .collect();
    let mut added = Vec::new();
    for w in chain.windows(2) {
        if model.line_between(w[0], w[1]).is_none()
            && let Some(e) = model.add_edge(w[0], w[1])
        {
            added.push(e);
        }
    }
    if added.is_empty() {
        return Added::default();
    }
    let new: BTreeSet<EdgeId> = added.iter().copied().collect();
    let mut faces = Vec::new();
    for plane in candidate_planes(model, &chain, pa, pb, &tol) {
        faces.extend(rebuild_plane(model, &plane, &new, &tol, Fill::Drawn, 0.0));
    }
    faces.retain(|f| model.face(*f).is_some());
    Added {
        edges: added,
        faces,
    }
}

/// Add an exact circle about `centre` in the plane with `normal`: arcs
/// sharing one curve id, between two seam points and every point where it
/// meets edges already there (which it splits). The plane's faces are
/// rebuilt as for a segment, so a circle on its own becomes a face, one
/// inside a face also makes a hole in it, and one across edges splits the
/// faces they bound.
pub fn add_circle(model: &mut Model, centre: DVec3, normal: DVec3, radius: f64) -> Added {
    let n = normal.normalize_or_zero();
    if n == DVec3::ZERO
        || radius.is_nan()
        || radius <= 0.0
        || !centre.is_finite()
        || !radius.is_finite()
    {
        return Added::default();
    }
    let plane = Plane::new(centre, n);
    let u = plane.frame().u;
    let (p, q) = (centre + u * radius, centre - u * radius);
    let tol = tol_with(model, &[p, q]);
    if radius <= tol.dist {
        return Added::default();
    }
    let a = insert_point(model, p, &tol);
    let b = insert_point(model, q, &tol);
    let arc = Curve::Arc {
        centre,
        axis: n,
        radius,
    };
    // Split the edges the circle crosses.
    let points: Vec<DVec3> = {
        let model = &*model;
        model
            .edges()
            .flat_map(|(_, e)| {
                let pts = match e.curve {
                    Curve::Line => circle_line(
                        centre,
                        n,
                        radius,
                        model.vertex(e.a),
                        model.vertex(e.b),
                        tol.dist,
                    ),
                    Curve::Arc {
                        centre: c,
                        axis,
                        radius: r,
                    } => circle_circle((centre, n, radius), (c, axis, r), tol.dist),
                };
                pts.into_iter()
                    .filter(move |x| on_edge_inside(model, e, *x, &tol))
            })
            .collect()
    };
    for x in points {
        insert_point(model, x, &tol);
    }
    // Every vertex on the circle, counter-clockwise from the seam.
    let v = n.cross(u);
    let angle = |x: DVec3| {
        let w = x - centre;
        w.dot(v).atan2(w.dot(u)).rem_euclid(TAU)
    };
    let mut on: Vec<(f64, VertexId)> = model
        .vertices()
        .filter(|(id, x)| *id == a || *id == b || circle_distance(centre, n, radius, *x) < tol.dist)
        .map(|(id, x)| (angle(x), id))
        .collect();
    on.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    // The same arc drawn again is the one already there.
    let same = |m: &Model, x: VertexId, y: VertexId| {
        let want = if x < y { arc } else { arc.reversed() };
        m.edges().any(|(_, e)| {
            e.a == x.min(y)
                && e.b == x.max(y)
                && match (e.curve, want) {
                    (
                        Curve::Arc {
                            centre: c1,
                            axis: a1,
                            radius: r1,
                        },
                        Curve::Arc {
                            centre: c2,
                            axis: a2,
                            radius: r2,
                        },
                    ) => tol.same_point(c1, c2) && a1.dot(a2) > 0.0 && (r1 - r2).abs() < tol.dist,
                    _ => false,
                }
        })
    };
    let missing: Vec<(VertexId, VertexId)> = (0..on.len())
        .map(|i| (on[i].1, on[(i + 1) % on.len()].1))
        .filter(|&(x, y)| !same(model, x, y))
        .collect();
    if missing.is_empty() {
        return Added::default();
    }
    let cid = model.new_curve_id();
    let added: Vec<EdgeId> = missing
        .into_iter()
        .filter_map(|(x, y)| model.add_curve_edge(x, y, arc, Some(cid)))
        .collect();
    let new: BTreeSet<EdgeId> = added.iter().copied().collect();
    let mut faces = rebuild_plane(model, &plane, &new, &tol, Fill::Drawn, 0.0);
    faces.retain(|f| model.face(*f).is_some());
    Added {
        edges: added,
        faces,
    }
}

/// Add an axis-aligned rectangle in the plane z = `z`, as four segments.
pub fn add_rect(model: &mut Model, x0: f64, y0: f64, x1: f64, y1: f64, z: f64) -> Added {
    let c = [
        DVec3::new(x0, y0, z),
        DVec3::new(x1, y0, z),
        DVec3::new(x1, y1, z),
        DVec3::new(x0, y1, z),
    ];
    let mut all = Added::default();
    for i in 0..4 {
        let r = add_segment(model, c[i], c[(i + 1) % 4]);
        all.edges.extend(r.edges);
        all.faces.extend(r.faces);
    }
    all.faces.retain(|f| model.face(*f).is_some());
    all
}

/// Where segment `a`–`b` crosses `c`–`d`: its parameter along `a`–`b` and
/// the point. Only true crossings clear of all four ends count.
pub(crate) fn crossing(a: DVec3, b: DVec3, c: DVec3, d: DVec3, tol: &Tol) -> Option<(f64, DVec3)> {
    let (u, v) = (b - a, d - c);
    let (lu, lv) = (u.length(), v.length());
    if lu <= tol.dist || lv <= tol.dist || tol.parallel(u / lu, v / lv) {
        return None;
    }
    let w = a - c;
    let (uu, uv, vv, uw, vw) = (u.dot(u), u.dot(v), v.dot(v), u.dot(w), v.dot(w));
    let den = uu * vv - uv * uv;
    let s = (uv * vw - vv * uw) / den;
    let t = (uu * vw - uv * uw) / den;
    let (p, q) = (a + u * s, c + v * t);
    let clear = |x: f64, len: f64| x * len > tol.dist && (1.0 - x) * len > tol.dist;
    (p.distance(q) < tol.dist && clear(s, lu) && clear(t, lv)).then_some((s, q))
}

/// The planes a new segment could close or split faces in: the planes of
/// faces it lies in, and the planes it makes with each edge at its vertices.
fn candidate_planes(
    model: &Model,
    chain: &[VertexId],
    pa: DVec3,
    pb: DVec3,
    tol: &Tol,
) -> Vec<Plane> {
    let dir = (pb - pa).normalize();
    let mut planes: Vec<Plane> = Vec::new();
    let mut push = |p: Plane| {
        if !planes.iter().any(|q| q.same_surface(&p, tol)) {
            planes.push(p);
        }
    };
    for (_, f) in model.faces() {
        let pl = f.plane();
        if pl.contains(pa, tol) && pl.contains(pb, tol) {
            push(*pl);
        }
    }
    for v in chain {
        let at = model.vertex(*v);
        for (_, e) in model.edges().filter(|(_, e)| e.has(*v)) {
            if let Some(pl) = e.curve.plane() {
                if pl.contains(pa, tol) && pl.contains(pb, tol) {
                    push(pl);
                }
                continue;
            }
            let side = model.vertex(e.other(*v)) - at;
            if side.length() > tol.dist && !tol.parallel(dir, side.normalize()) {
                push(Plane::new(at, dir.cross(side)));
            }
        }
    }
    // A deterministic order: by the canonical normal and offset.
    planes.sort_by(|p, q| {
        let (a, da) = p.canonical();
        let (b, db) = q.canonical();
        a.to_array()
            .partial_cmp(&b.to_array())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(da.total_cmp(&db))
    });
    planes
}

/// The face containing `p`: on its plane, inside its outer loop and outside
/// its holes. The smallest if several do.
pub fn face_at(model: &Model, p: DVec3) -> Option<FaceId> {
    use prism_math::predicates::{Inside, point_in_polygon};
    let tol = model.tol();
    model
        .faces()
        .filter(|(_, f)| f.plane().contains(p, &tol))
        .filter(|(_, f)| {
            let fr = f.plane().frame();
            let poly = |l: &[VertexId]| -> Vec<prism_math::DVec2> {
                l.iter().map(|v| fr.to_2d(model.vertex(*v))).collect()
            };
            let q = fr.to_2d(p);
            point_in_polygon(q, &poly(&f.outer)) == Inside::Inside
                && f.holes
                    .iter()
                    .all(|h| point_in_polygon(q, &poly(h)) != Inside::Inside)
        })
        .map(|(id, _)| (id, model.face_area(id)))
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
        .map(|(id, _)| id)
}

/// The vertex at `p`, if there is one within tolerance.
pub fn vertex_at(model: &Model, p: DVec3) -> Option<VertexId> {
    let tol = model.tol();
    model
        .vertices()
        .find(|(_, v)| tol.same_point(*v, p))
        .map(|(id, _)| id)
}
