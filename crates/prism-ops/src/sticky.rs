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
pub(crate) fn on_edge_inside(model: &Model, e: &Edge, p: DVec3, tol: &Tol) -> bool {
    on_curve_inside(&e.curve, model.vertex(e.a), model.vertex(e.b), p, tol)
}

/// Whether `p` lies on `curve` from `a` to `b`, clear of both ends.
pub(crate) fn on_curve_inside(curve: &Curve, a: DVec3, b: DVec3, p: DVec3, tol: &Tol) -> bool {
    match curve {
        Curve::Line => on_segment_inside(p, a, b, tol).is_some(),
        Curve::Arc { .. } | Curve::Spline(_) => {
            let c = curve.closest_point(a, b, p);
            c.distance(p) < tol.dist && c.distance(a) > tol.dist && c.distance(b) > tol.dist
        }
    }
}

/// Where two curves (each with its ends) cross, clear of all four ends.
pub(crate) fn meet(
    (c1, a1, b1): (&Curve, DVec3, DVec3),
    (c2, a2, b2): (&Curve, DVec3, DVec3),
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
        ) => circle_line(*centre, *axis, *radius, a2, b2, tol.dist),
        (
            Curve::Line,
            Curve::Arc {
                centre,
                axis,
                radius,
            },
        ) => circle_line(*centre, *axis, *radius, a1, b1, tol.dist),
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
        ) => circle_circle((*p, *n, *r), (*q, *m, *s), tol.dist),
        // Anything with a spline: found by stepping along both.
        _ => prism_geom::curves_meet((c1, a1, b1), (c2, a2, b2), tol.dist),
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
        .filter(|(_, e)| !e.has(a) && !e.has(b) && !e.curve.is_curved())
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
                    Curve::Spline(_) => prism_geom::curves_meet(
                        (&e.curve, model.vertex(e.a), model.vertex(e.b)),
                        (&Curve::Line, pa, pb),
                        tol.dist,
                    ),
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
    // Drawn over edges already there: they close faces as if new, so
    // retracing a side of a flat loop with no face fills it (as in
    // SketchUp).
    let new: BTreeSet<EdgeId> = if added.is_empty() {
        chain
            .windows(2)
            .filter_map(|w| model.line_between(w[0], w[1]))
            .collect()
    } else {
        added.iter().copied().collect()
    };
    let mut faces = Vec::new();
    for plane in candidate_planes(model, &chain, pa, pb, &tol) {
        faces.extend(rebuild_plane(model, &plane, &new, &tol, Fill::Drawn, 0.0));
    }
    // Lying on a curved face (along a cylinder, through a cone's apex), it
    // splits it too.
    faces.extend(crate::cut::split_along(model, &added));
    faces.retain(|f| model.face(*f).is_some());
    Added {
        edges: added,
        faces,
    }
}

/// A closed polygon through `points`, its sides sharing one curve id (a
/// click on one side selects them all). Sides are added as lines are, so
/// they cut and are cut by what they cross; every straight piece along a
/// side takes the polygon's id.
pub fn add_polygon(model: &mut Model, points: &[DVec3]) {
    let n = points.len();
    if n < 3 {
        return;
    }
    for i in 0..n {
        add_segment(model, points[i], points[(i + 1) % n]);
    }
    let tol = model.tol();
    let on_side = |p: DVec3| {
        (0..n).find(|&i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            Curve::Line.closest_point(a, b, p).distance(p) <= tol.dist
        })
    };
    let sides: Vec<EdgeId> = model
        .edges()
        .filter(|(_, e)| !e.curve.is_curved())
        .filter(|(_, e)| {
            let (a, b) = (on_side(model.vertex(e.a)), on_side(model.vertex(e.b)));
            // Both ends on one side (a corner is on two: check the middle).
            a.is_some()
                && b.is_some()
                && on_side(model.vertex(e.a).lerp(model.vertex(e.b), 0.5)).is_some()
        })
        .map(|(id, _)| id)
        .collect();
    if sides.is_empty() {
        return;
    }
    let cid = model.new_curve_id();
    for id in sides {
        model.set_curve_id(id, Some(cid));
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
    if n == DVec3::ZERO {
        return Added::default();
    }
    let u = Plane::new(centre, n).frame().u;
    add_arc_from(model, centre, n, radius, u, TAU)
}

/// Add an arc of the circle about `centre` (unit `normal`, `radius`), from
/// `start` (projected onto the circle) counter-clockwise about `normal` by
/// `sweep` radians. A sweep of a full turn (or more) is a circle. Like a
/// segment, it cuts and is cut by what it crosses, and closes faces. The
/// pieces share one new curve id.
pub fn add_arc(
    model: &mut Model,
    centre: DVec3,
    normal: DVec3,
    radius: f64,
    start: DVec3,
    sweep: f64,
) -> Added {
    let n = normal.normalize_or_zero();
    let w = start - centre;
    let u = (w - n * w.dot(n)).normalize_or_zero();
    if n == DVec3::ZERO || u == DVec3::ZERO || sweep.is_nan() || sweep <= 0.0 {
        return Added::default();
    }
    add_arc_from(model, centre, n, radius, u, sweep.min(TAU))
}

/// A pie: [`add_arc`] and the two radius lines from `centre` to its ends.
pub fn add_pie(
    model: &mut Model,
    centre: DVec3,
    normal: DVec3,
    radius: f64,
    start: DVec3,
    sweep: f64,
) -> Added {
    let mut all = add_arc(model, centre, normal, radius, start, sweep);
    if all.edges.is_empty() || sweep >= TAU {
        return all;
    }
    let n = normal.normalize_or_zero();
    let w = start - centre;
    let u = (w - n * w.dot(n)).normalize_or_zero();
    let v = n.cross(u);
    for t in [0.0, sweep] {
        let r = add_segment(model, centre, centre + (u * t.cos() + v * t.sin()) * radius);
        all.edges.extend(r.edges);
        all.faces.extend(r.faces);
    }
    all.faces.retain(|f| model.face(*f).is_some());
    all
}

/// [`add_arc`] with the start direction `u` (unit, in the plane).
fn add_arc_from(
    model: &mut Model,
    centre: DVec3,
    n: DVec3,
    radius: f64,
    u: DVec3,
    sweep: f64,
) -> Added {
    if radius.is_nan() || radius <= 0.0 || !centre.is_finite() || !radius.is_finite() {
        return Added::default();
    }
    let full = sweep >= TAU;
    // Almost on an existing arc's circle (same centre and axis, radius
    // within ten tolerances, a near miss): on it exactly, as a point near
    // a vertex lands on it. Otherwise the two would lie a hair apart, and
    // merge badly once the model grows and its tolerance with it.
    let (centre, radius) = {
        let near = tol_with(model, &[centre]).dist * 10.0;
        model
            .edges()
            .find_map(|(_, e)| match e.curve {
                Curve::Arc {
                    centre: c,
                    axis,
                    radius: r,
                } if c.distance(centre) < near
                    && 1.0 - axis.dot(n).abs() < 1e-12
                    && (r - radius).abs() < near =>
                {
                    Some((c, r))
                }
                _ => None,
            })
            .unwrap_or((centre, radius))
    };
    let plane = Plane::new(centre, n);
    let v = n.cross(u);
    let at = |t: f64| centre + (u * t.cos() + v * t.sin()) * radius;
    let (p, q) = (at(0.0), at(if full { TAU / 2.0 } else { sweep }));
    let tol = tol_with(model, &[p, q]);
    if radius <= tol.dist || p.distance(q) <= tol.dist && !full {
        return Added::default();
    }
    let a = insert_point(model, p, &tol);
    let b = insert_point(model, q, &tol);
    let arc = Curve::Arc {
        centre,
        axis: n,
        radius,
    };
    // The angle about the centre from the start.
    let angle = |x: DVec3| {
        let w = x - centre;
        w.dot(v).atan2(w.dot(u)).rem_euclid(TAU)
    };
    let (pa, pb) = (model.vertex(a), model.vertex(b));
    let on_this = |x: DVec3| full || on_curve_inside(&arc, pa, pb, x, &tol);
    // Split the edges the arc crosses.
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
                    Curve::Spline(_) => {
                        let ends = (&e.curve, model.vertex(e.a), model.vertex(e.b));
                        let mut pts = prism_geom::curves_meet(ends, (&arc, pa, pb), tol.dist);
                        if full {
                            pts.extend(prism_geom::curves_meet(ends, (&arc, pb, pa), tol.dist));
                        }
                        pts
                    }
                };
                pts.into_iter()
                    .filter(move |x| on_edge_inside(model, e, *x, &tol))
            })
            .filter(|x| on_this(*x))
            .collect()
    };
    for x in points {
        insert_point(model, x, &tol);
    }
    // Every vertex on the arc, counter-clockwise from the start.
    let mut on: Vec<(f64, VertexId)> = model
        .vertices()
        .filter(|(id, x)| {
            *id == a
                || *id == b
                || (circle_distance(centre, n, radius, *x) < tol.dist && on_this(*x))
        })
        .map(|(id, x)| {
            (
                if id == a {
                    0.0
                } else if id == b && !full {
                    sweep
                } else {
                    angle(x)
                },
                id,
            )
        })
        .collect();
    on.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    on.dedup();
    // The same arc drawn again is the one already there.
    let same = |m: &Model, x: VertexId, y: VertexId| {
        let want = if x < y { arc.clone() } else { arc.reversed() };
        m.edges().any(|(_, e)| {
            e.a == x.min(y)
                && e.b == x.max(y)
                && match (&e.curve, &want) {
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
                    ) => {
                        tol.same_point(*c1, *c2)
                            && a1.dot(*a2) > 1.0 - 1e-9
                            && (r1 - r2).abs() < tol.dist
                    }
                    _ => false,
                }
        })
    };
    let pieces = if full {
        on.len()
    } else {
        on.len().saturating_sub(1)
    };
    let missing: Vec<(VertexId, VertexId)> = (0..pieces)
        .map(|i| (on[i].1, on[(i + 1) % on.len()].1))
        .filter(|&(x, y)| x != y && !same(model, x, y))
        .collect();
    if missing.is_empty() {
        return Added::default();
    }
    let cid = model.new_curve_id();
    let added: Vec<EdgeId> = missing
        .into_iter()
        .filter_map(|(x, y)| model.add_curve_edge(x, y, arc.clone(), Some(cid)))
        .collect();
    let new: BTreeSet<EdgeId> = added.iter().copied().collect();
    let mut faces = rebuild_plane(model, &plane, &new, &tol, Fill::Drawn, 0.0);
    // On a curved face (a circle round a cylinder, any circle on a
    // sphere), it splits it too.
    faces.extend(crate::cut::split_along(model, &added));
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
        if let Some(pl) = f.plane()
            && pl.contains(pa, tol)
            && pl.contains(pb, tol)
        {
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
        .filter(|(_, f)| f.plane().is_some_and(|pl| pl.contains(p, &tol)))
        .filter(|(_, f)| {
            let Some(pl) = f.plane() else { return false };
            let fr = pl.frame();
            // The drawn outline (arcs as their points): a disc has only two
            // corners.
            let loops: Vec<Vec<prism_math::DVec2>> = model
                .face_loop_points(f)
                .iter()
                .map(|l| l.iter().map(|x| fr.to_2d(*x)).collect())
                .collect();
            let q = fr.to_2d(p);
            point_in_polygon(q, &loops[0]) == Inside::Inside
                && loops[1..]
                    .iter()
                    .all(|h| point_in_polygon(q, h) != Inside::Inside)
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

/// The places where a curve crosses itself, as pairs of parameters
/// `s < t`. The curve is cut into parts that turn less than half a turn
/// (such a part cannot cross itself), and each pair of parts is met; the
/// joint two neighbouring parts share is not a crossing.
fn self_crossings(curve: &prism_geom::Nurbs, tol: &Tol) -> Vec<(f64, f64)> {
    let (d0, d1) = curve.domain();
    let mut cuts = vec![d0, d1];
    let mut i = 0;
    while i + 1 < cuts.len() {
        let (t0, t1) = (cuts[i], cuts[i + 1]);
        if curve.turning(t0, t1) > 2.5 && cuts.len() < 256 {
            cuts.insert(i + 1, (t0 + t1) / 2.0);
        } else {
            i += 1;
        }
    }
    let n = cuts.len() - 1;
    if n < 2 {
        return Vec::new();
    }
    let closed = curve.is_closed();
    let piece = |k: usize| {
        let (t0, t1) = (cuts[k], cuts[k + 1]);
        let c = Curve::Spline(prism_geom::Spline {
            curve: std::sync::Arc::new(curve.clone()),
            t0,
            t1,
        });
        (c, curve.eval(t0), curve.eval(t1))
    };
    let pieces: Vec<_> = (0..n).map(piece).collect();
    let mut out: Vec<(f64, f64)> = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            let (ci, ai, bi) = &pieces[i];
            let (cj, aj, bj) = &pieces[j];
            // Joints the two parts share.
            let mut joints = Vec::new();
            if j == i + 1 {
                joints.push(*bi);
            }
            if closed && i == 0 && j == n - 1 {
                joints.push(*ai);
            }
            for x in prism_geom::curves_meet((ci, *ai, *bi), (cj, *aj, *bj), tol.dist) {
                if joints.iter().any(|q| tol.same_point(*q, x)) {
                    continue;
                }
                let s = curve.closest(x, cuts[i], cuts[i + 1]);
                let t = curve.closest(x, cuts[j], cuts[j + 1]);
                let p = curve.eval(s);
                if out.iter().any(|&(s0, _)| tol.same_point(curve.eval(s0), p)) {
                    continue;
                }
                // Strands that only come within tolerance (the tip of a
                // curve that doubles back, two strands brushing past) are
                // not a crossing: the strands must truly meet, at an angle,
                // and the loop between must go somewhere.
                let (ds, dt) = (curve.eval_d(s).1, curve.eval_d(t).1);
                let sin = ds
                    .normalize_or_zero()
                    .cross(dt.normalize_or_zero())
                    .length();
                let away = (1..16)
                    .map(|k| curve.eval(s + (t - s) * k as f64 / 16.0).distance(p))
                    .fold(0.0, f64::max);
                if p.distance(curve.eval(t)) > 1e-3 * tol.dist
                    || sin < 1e-3
                    || away < 10.0 * tol.dist
                {
                    continue;
                }
                out.push((s, t));
            }
        }
    }
    out
}

/// Add a NURBS curve (Freeform NURBS part 1). Like a segment or an arc, it
/// cuts and is cut by what it crosses, takes in the vertices it passes
/// through, and closes faces when it lies in a plane. Its pieces share the
/// curve and one new curve id. A closed curve is two pieces at least, as a
/// circle is. A curve that crosses itself is cut at the crossing, and the
/// loop it makes closes a face when it lies in a plane.
pub fn add_spline(model: &mut Model, curve: &prism_geom::Nurbs) -> Added {
    let (d0, d1) = curve.domain();
    let closed = curve.is_closed();
    let shared = std::sync::Arc::new(curve.clone());
    let part = |t0: f64, t1: f64| {
        Curve::Spline(prism_geom::Spline {
            curve: shared.clone(),
            t0,
            t1,
        })
    };
    let half = (d0 + d1) / 2.0;
    let (start, far) = (curve.eval(d0), curve.eval(if closed { half } else { d1 }));
    let mut extent: Vec<DVec3> = curve.points().to_vec();
    extent.extend([start, far]);
    let tol = tol_with(model, &extent);
    if !start.is_finite() || !far.is_finite() || start.distance(far) <= tol.dist {
        return Added::default();
    }
    let a = insert_point(model, start, &tol);
    let b = insert_point(model, far, &tol);
    if a == b {
        return Added::default();
    }
    // Split the edges it crosses.
    let ranges: Vec<(f64, f64)> = if closed {
        vec![(d0, half), (half, d1)]
    } else {
        vec![(d0, d1)]
    };
    let crossings: Vec<DVec3> = {
        let model = &*model;
        ranges
            .iter()
            .flat_map(|&(t0, t1)| {
                let piece = part(t0, t1);
                let (pa, pb) = (curve.eval(t0), curve.eval(t1));
                model
                    .edges()
                    .flat_map(|(_, e)| {
                        prism_geom::curves_meet(
                            (&piece, pa, pb),
                            (&e.curve, model.vertex(e.a), model.vertex(e.b)),
                            tol.dist,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    for x in crossings {
        insert_point(model, x, &tol);
    }
    // Where the curve crosses itself: one vertex, met at two places.
    let selfs: Vec<(f64, f64, VertexId)> = self_crossings(curve, &tol)
        .into_iter()
        .map(|(s, t)| (s, t, insert_point(model, curve.eval(s), &tol)))
        .collect();
    // Every vertex on the curve, in order along it.
    let mut on: Vec<(f64, VertexId)> = model
        .vertices()
        .filter_map(|(id, x)| {
            if id == a {
                return Some((d0, id));
            }
            let t = curve.closest(x, d0, d1);
            (curve.eval(t).distance(x) < tol.dist).then_some((t, id))
        })
        .collect();
    for &(s, t, v) in &selfs {
        on.retain(|x| x.1 != v);
        on.extend([(s, v), (t, v)]);
    }
    on.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    on.dedup();
    if closed {
        on.push((d1, a));
    } else if let Some(last) = on.iter_mut().find(|x| x.1 == b) {
        last.0 = d1;
    }
    on.sort_by(|x, y| x.0.total_cmp(&y.0));
    // A loop from a crossing back to it is two pieces, as a circle is.
    let mut k = 0;
    while k + 1 < on.len() {
        let ((t0, x), (t1, y)) = (on[k], on[k + 1]);
        if x == y && t1 > t0 {
            let t = (t0 + t1) / 2.0;
            on.insert(k + 1, (t, insert_point(model, curve.eval(t), &tol)));
        }
        k += 1;
    }
    // The same piece drawn again is the one already there.
    let same = |m: &Model, x: VertexId, y: VertexId, mid: DVec3| {
        m.edges().any(|(_, e)| {
            e.has(x)
                && e.has(y)
                && e.curve.is_spline()
                && e.curve
                    .closest_point(m.vertex(e.a), m.vertex(e.b), mid)
                    .distance(mid)
                    < tol.dist
        })
    };
    let cid = model.new_curve_id();
    let mut added = Vec::new();
    for w in on.windows(2) {
        let ((t0, x), (t1, y)) = (w[0], w[1]);
        if x == y || t1 <= t0 || same(model, x, y, curve.eval((t0 + t1) / 2.0)) {
            continue;
        }
        added.extend(model.add_curve_edge(x, y, part(t0, t1), Some(cid)));
    }
    if added.is_empty() {
        return Added::default();
    }
    let new: BTreeSet<EdgeId> = added.iter().copied().collect();
    let mut faces = Vec::new();
    if let Some(plane) = curve.plane(tol.dist) {
        faces = rebuild_plane(model, &plane, &new, &tol, Fill::Drawn, 0.0);
    }
    faces.retain(|f| model.face(*f).is_some());
    Added {
        edges: added,
        faces,
    }
}
