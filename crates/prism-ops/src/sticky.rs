//! Sticky geometry: points snap to existing vertices and split the edges
//! they land on; segments split at every vertex and crossing; closed
//! coplanar loops become faces; overlapping coplanar faces split.

use std::collections::BTreeSet;

use prism_math::{DVec3, Plane, Tol};
use prism_topo::{EdgeId, FaceId, Model, VertexId};

use crate::regions::rebuild_plane;

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
        .find(|(_, e)| on_segment_inside(p, model.vertex(e.a), model.vertex(e.b), tol).is_some())
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

    // Crossings with edges that do not touch the ends.
    let crossings: Vec<(EdgeId, f64, DVec3)> = model
        .edges()
        .filter(|(_, e)| !e.has(a) && !e.has(b))
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
    cuts.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    cuts.dedup_by_key(|c| c.1);

    let chain: Vec<VertexId> = std::iter::once(a)
        .chain(cuts.iter().map(|c| c.1))
        .chain(std::iter::once(b))
        .collect();
    let mut added = Vec::new();
    for w in chain.windows(2) {
        if model.edge_between(w[0], w[1]).is_none()
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
        faces.extend(rebuild_plane(model, &plane, &new, &tol));
    }
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
fn crossing(a: DVec3, b: DVec3, c: DVec3, d: DVec3, tol: &Tol) -> Option<(f64, DVec3)> {
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
