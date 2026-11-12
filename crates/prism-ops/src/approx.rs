//! The segment fallback (DECISIONS #32, Exact Curves 6a): where an edit
//! would make a curve or surface Prism cannot store exactly, it is replaced
//! by fine straight segments and flat facets marked approximate. Their
//! points are exact; the chords and facets between them are not. Model
//! Health lists them, and NURBS can replace them later.
//!
//! How fine: a chord never strays from its curve by more than 1/1000 of
//! the curve's radius. A curve has never fewer segments than an arc of the
//! same turn is drawn with; a surface's facets follow the chord rule alone.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Curve, Surface, segments};
use prism_math::{DAffine3, DVec2, DVec3, Plane, newell};
use prism_topo::{CurveId, EdgeId, Face, FaceId, Model, VertexId};

/// The largest turn one segment may cover: its chord then strays by at
/// most 1/1000 of the radius (1 - cos(φ/2) = 1/1000).
fn max_step() -> f64 {
    2.0 * (1.0 - 1e-3f64).acos()
}

/// How many segments a curve turning `sweep` radians takes: fine enough
/// for the chord rule, and never fewer than an arc is drawn with.
pub fn fine_count(sweep: f64) -> usize {
    facet_count(sweep).max(segments(sweep.abs()))
}

/// How many facets a surface turning `sweep` radians takes one way: the
/// chord rule alone (in two directions the drawing count would make a
/// sphere thousands of facets).
fn facet_count(sweep: f64) -> usize {
    ((sweep.abs() / max_step()).ceil() as usize).max(1)
}

/// Approximate segments made from one curve share one new curve id.
type Ids = BTreeMap<Option<CurveId>, CurveId>;

fn new_id(model: &mut Model, ids: &mut Ids, old: Option<CurveId>, e: EdgeId) -> CurveId {
    // Edges with no curve id each get their own.
    let key = old.or(Some(CurveId(u32::MAX - e.0)));
    *ids.entry(key).or_insert_with(|| model.new_curve_id())
}

/// Replace arc `e` by fine straight segments along it, marked approximate.
/// Returns the points from the arc's `a` to its `b`.
pub fn segment_arc(model: &mut Model, e: EdgeId, ids: &mut Ids) -> Vec<VertexId> {
    let Some(x) = model.edge(e).cloned() else {
        return Vec::new();
    };
    if !x.curve.is_curved() {
        return vec![x.a, x.b];
    }
    let (pa, pb) = (model.vertex(x.a), model.vertex(x.b));
    let n = fine_count(x.curve.sweep(pa, pb));
    let pts: Vec<DVec3> = (1..n)
        .map(|i| x.curve.point(pa, pb, i as f64 / n as f64))
        .collect();
    split_into(model, e, &pts, ids)
}

/// Split edge `e` at `pts` (strictly between its ends, in order from its
/// `a`), making every piece a straight approximate segment. Returns the
/// points from `a` to `b`.
fn split_into(model: &mut Model, e: EdgeId, pts: &[DVec3], ids: &mut Ids) -> Vec<VertexId> {
    let Some(x) = model.edge(e).cloned() else {
        return Vec::new();
    };
    let cid = new_id(model, ids, x.curve_id, e);
    let mut chain = vec![x.a];
    let mut rest = e;
    for p in pts {
        let v = model.add_vertex(*p);
        let Some((first, second)) = model.split_edge(rest, v) else {
            break;
        };
        // The rest runs on from `v`, whichever way it was stored.
        let (done, more) = if model
            .edge(first)
            .is_some_and(|y| y.has(*chain.last().unwrap()))
        {
            (first, second)
        } else {
            (second, first)
        };
        mark(model, done, cid);
        chain.push(v);
        rest = more;
    }
    mark(model, rest, cid);
    chain.push(x.b);
    chain
}

fn mark(model: &mut Model, e: EdgeId, cid: CurveId) {
    model.set_curve(e, Curve::Line);
    model.set_approx(e, true);
    model.set_curve_id(e, Some(cid));
}

/// Replace each arc of `edges` by fine approximate segments, the pieces of
/// one curve sharing one new curve id.
pub fn segment_arcs(model: &mut Model, edges: &[EdgeId]) {
    let mut ids = Ids::new();
    for e in edges {
        segment_arc(model, *e, &mut ids);
    }
}

/// Whether [`facet`] can facet curved face `id`: any face that can be
/// laid out in (θ, s) (not a band round the axis, which Prism never makes).
pub fn can_facet(model: &Model, id: FaceId) -> bool {
    crate::cut::laid_segs(model, id).is_some()
}

/// `n` + 1 evenly spaced values from `lo` to `hi`.
fn steps(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    (0..=n)
        .map(|i| lo + (hi - lo) * i as f64 / n as f64)
        .collect()
}

/// Where along edge `e` (0 at its `a`, 1 at its `b`) its place in (θ, s)
/// reaches `target` in coordinate `y` (0 for θ, 1 for s), between `u0`
/// and `u1`, where it changes side. `guess` keeps angles unwrapped.
#[allow(clippy::too_many_arguments)]
fn cross_at(
    rev: &prism_geom::Revolution,
    curve: Curve,
    pa: DVec3,
    pb: DVec3,
    (mut u0, mut u1): (f64, f64),
    y: usize,
    target: f64,
    guess: f64,
) -> f64 {
    let turn = std::f64::consts::TAU;
    let at = |u: f64| {
        let (t, s) = rev.param(curve.point(pa, pb, u));
        let v = if y == 0 { t } else { s };
        v + ((guess - v) / turn).round() * turn - target
    };
    let mut g0 = at(u0);
    for _ in 0..60 {
        let m = (u0 + u1) / 2.0;
        let gm = at(m);
        if (gm < 0.0) == (g0 < 0.0) {
            u0 = m;
            g0 = gm;
        } else {
            u1 = m;
        }
    }
    (u0 + u1) / 2.0
}

/// Ear clipping of a simple polygon: triangles as indices.
fn ears(p: &[DVec2]) -> Vec<[usize; 3]> {
    use prism_math::predicates::orient2d;
    let mut idx: Vec<usize> = (0..p.len()).collect();
    let area: f64 = (0..p.len())
        .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
        .sum();
    if area < 0.0 {
        idx.reverse();
    }
    let mut out = Vec::new();
    while idx.len() > 3 {
        let n = idx.len();
        let ear = (0..n).find(|&i| {
            let (a, b, c) = (p[idx[(i + n - 1) % n]], p[idx[i]], p[idx[(i + 1) % n]]);
            orient2d(a, b, c) > 0.0
                && idx.iter().all(|&k| {
                    let q = p[k];
                    q == a
                        || q == b
                        || q == c
                        || !(orient2d(a, b, q) > 0.0
                            && orient2d(b, c, q) > 0.0
                            && orient2d(c, a, q) > 0.0)
                })
        });
        // Nothing convex left (rounding): cut anywhere.
        let i = ear.unwrap_or(0);
        out.push([idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]]);
        idx.remove(i);
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

/// Replace curved face `id` by flat facets. Lines of a grid in (θ, s)
/// (the chord rule's step each way, s one step on a straight profile) are
/// laid across it as soft, smooth, approximate edges, the face's own edges
/// split exactly where they cross them, and the face cut along them
/// ([`crate::cut::split_face`]). A whole cell is flat on every turned
/// surface (two points on each of two circles about the axis, at the same
/// two angles, make an isosceles trapezoid), so it stays flat under any
/// affine map; a cell the face's outline crosses is cut into triangles.
/// Every point is on the surface. The face's own edges become approximate
/// segments. False if the face cannot be laid out (it is left as it is).
pub fn facet(model: &mut Model, id: FaceId, ids: &mut Ids) -> bool {
    use crate::cut::{laid_segs, seg_polygon};
    use prism_math::predicates::{Inside, point_in_polygon};
    let Some((rev, loops)) = laid_segs(model, id) else {
        return false;
    };
    let circle = matches!(rev.profile, prism_geom::Profile::Circle { .. });
    let polys: Vec<Vec<DVec2>> = loops.iter().map(|l| seg_polygon(l)).collect();
    let (lo, hi) = polys[0]
        .iter()
        .fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(*p), hi.max(*p))
        });
    if hi.x - lo.x <= 1e-9 || hi.y - lo.y <= 1e-12 {
        return false;
    }
    let ang = 1e-9;
    let mut ts = steps(lo.x, hi.x, facet_count(hi.x - lo.x));
    let ss = steps(
        lo.y,
        hi.y,
        if circle { facet_count(hi.y - lo.y) } else { 1 },
    );
    // A line through each hole, so no piece is left holding one.
    for h in &polys[1..] {
        if let Some(p) = h.iter().min_by(|a, b| a.x.total_cmp(&b.x)) {
            ts.push(p.x);
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|a, b| (*a - *b).abs() < ang);

    // The face's own edges, split exactly where grid lines cross them;
    // arcs become straight segments.
    for seg in loops.iter().flatten() {
        let Some(e) = seg.edge else { continue };
        let Some(x) = model.edge(e).cloned() else {
            continue;
        };
        let (pa, pb) = (model.vertex(x.a), model.vertex(x.b));
        let mut p2 = seg.pts.clone();
        if seg.from != x.a {
            p2.reverse();
        }
        let n = p2.len() - 1;
        let mut us: Vec<f64> = Vec::new();
        for k in 0..n {
            let (a, b) = (p2[k], p2[k + 1]);
            let span = (k as f64 / n as f64, (k + 1) as f64 / n as f64);
            for (y, lines) in [(0usize, &ts), (1, &ss)] {
                let (va, vb) = if y == 0 { (a.x, b.x) } else { (a.y, b.y) };
                for g in lines.iter() {
                    if (va - g) * (vb - g) < 0.0 && (va - g).abs() > ang && (vb - g).abs() > ang {
                        let r = (g - va) / (vb - va);
                        let guess = if y == 0 {
                            a.x + (b.x - a.x) * r
                        } else {
                            a.y + (b.y - a.y) * r
                        };
                        us.push(cross_at(&rev, x.curve.clone(), pa, pb, span, y, *g, guess));
                    } else if (vb - g).abs() <= ang && k + 1 < n && {
                        // Crossing there, not running along the line.
                        let c = p2[k + 2];
                        let vc = if y == 0 { c.x } else { c.y };
                        (va - g) * (vc - g) < 0.0
                    } {
                        // Through a drawn point of an arc: the crossing is
                        // there, found again on the curve about it.
                        let around = ((k as f64 + 0.5) / n as f64, (k as f64 + 1.5) / n as f64);
                        us.push(cross_at(&rev, x.curve.clone(), pa, pb, around, y, *g, vb));
                    }
                }
            }
        }
        us.sort_by(f64::total_cmp);
        let pts: Vec<DVec3> = us
            .iter()
            .map(|u| x.curve.point(pa, pb, *u))
            .filter(|q| q.distance(pa) > model.tol().dist && q.distance(pb) > model.tol().dist)
            .collect();
        if !pts.is_empty() || x.curve.is_curved() {
            split_into(model, e, &pts, ids);
        }
    }

    // Laid out again: every side is straight now.
    let Some((_, loops)) = laid_segs(model, id) else {
        return false;
    };
    let polys: Vec<Vec<DVec2>> = loops.iter().map(|l| seg_polygon(l)).collect();
    let inside = |q: DVec2| {
        point_in_polygon(q, &polys[0]) == Inside::Inside
            && polys[1..]
                .iter()
                .all(|h| point_in_polygon(q, h) == Inside::Outside)
    };
    let border: Vec<(VertexId, DVec2)> = loops
        .iter()
        .flatten()
        .filter(|s| s.edge.is_some())
        .map(|s| (s.from, s.pts[0]))
        .collect();
    // Stretches across a pole: (pole, s, from θ, to θ).
    let poles: Vec<(VertexId, f64, f64, f64)> = loops
        .iter()
        .flatten()
        .filter(|s| s.edge.is_none())
        .map(|s| (s.from, s.pts[0].y, s.pts[0].x, s.pts[1].x))
        .collect();
    let mut corner: BTreeMap<(usize, usize), VertexId> = BTreeMap::new();
    let mut lines: Vec<(VertexId, VertexId)> = Vec::new();
    for pass in 0..2 {
        let (along, across) = if pass == 0 { (&ts, &ss) } else { (&ss, &ts) };
        for (i, g) in along.iter().enumerate() {
            let coord = |q: DVec2| if pass == 0 { (q.x, q.y) } else { (q.y, q.x) };
            let mut on: Vec<(f64, VertexId)> = border
                .iter()
                .filter(|(_, q)| (coord(*q).0 - g).abs() < ang)
                .map(|(v, q)| (coord(*q).1, *v))
                .collect();
            if pass == 0 {
                for (v, s, ta, tb) in &poles {
                    if (g - ta) * (g - tb) < 0.0 {
                        on.push((*s, *v));
                    }
                }
            }
            for (j, h) in across.iter().enumerate() {
                if on.iter().any(|(y, _)| (y - h).abs() < ang) {
                    continue;
                }
                let q = if pass == 0 {
                    DVec2::new(*g, *h)
                } else {
                    DVec2::new(*h, *g)
                };
                if inside(q) {
                    let key = if pass == 0 { (i, j) } else { (j, i) };
                    let v = *corner
                        .entry(key)
                        .or_insert_with(|| model.add_vertex(rev.point(q.x, q.y)));
                    on.push((*h, v));
                }
            }
            on.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            for w in on.windows(2) {
                let m = (w[0].0 + w[1].0) / 2.0;
                let q = if pass == 0 {
                    DVec2::new(*g, m)
                } else {
                    DVec2::new(m, *g)
                };
                if w[0].1 != w[1].1 && inside(q) {
                    lines.push((w[0].1, w[1].1));
                }
            }
        }
    }
    let mut made: BTreeMap<(VertexId, VertexId), EdgeId> =
        model.edges().map(|(e, x)| ((x.a, x.b), e)).collect();
    let mut line = |model: &mut Model, a: VertexId, b: VertexId| -> EdgeId {
        let key = (a.min(b), a.max(b));
        if let Some(e) = made.get(&key) {
            return *e;
        }
        let e = model.add_new_edge(a, b);
        model.set_soft(e, true, true);
        model.set_approx(e, true);
        made.insert(key, e);
        e
    };
    for (a, b) in lines {
        line(model, a, b);
    }
    let pieces = crate::cut::split_face(model, id);

    // Each piece flat: as it is when its points lie in a plane (a whole
    // cell, a triangle), else in triangles.
    let tol = model.tol();
    for p in pieces {
        let Some(face) = model.face(p).cloned() else {
            continue;
        };
        if !face.holes.is_empty() {
            continue;
        }
        let pts: Vec<DVec3> = face.outer.iter().map(|v| model.vertex(*v)).collect();
        let c = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
        let want = face.surface.normal_at(face.surface.project(c));
        let flat = |pts: &[DVec3]| {
            Plane::fit(pts, &tol).filter(|pl| pts.iter().all(|q| pl.distance(*q).abs() <= tol.dist))
        };
        let make = |l: Vec<VertexId>, es: Vec<EdgeId>, pts: &[DVec3]| -> Face {
            let nn = newell(pts).normalize_or_zero();
            let c = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
            let mut g = Face::with_edges(l, vec![], Surface::Plane(Plane::new(c, nn)), vec![es]);
            if nn.dot(want) < 0.0 {
                g.reverse();
            }
            // Facing as the face does, so it keeps its paint (#36).
            g.paint = face.paint;
            g
        };
        let mut faces = Vec::new();
        // Flat already, and facing about the way the surface does (a piece
        // of the cut's points alone can be flat in the cutting plane).
        let facing = newell(&pts).normalize_or_zero().dot(want).abs() >= 0.5;
        if (pts.len() == 3 || flat(&pts).is_some()) && facing {
            faces.push(make(
                face.outer.clone(),
                face.loop_edge_ids(0).to_vec(),
                &pts,
            ));
        } else {
            let Some((_, l)) = laid_segs(model, p) else {
                continue;
            };
            let mut ring: Vec<(VertexId, DVec2)> =
                l[0].iter().map(|s| (s.from, s.pts[0])).collect();
            let area: f64 = (0..ring.len())
                .map(|i| ring[i].1.perp_dot(ring[(i + 1) % ring.len()].1))
                .sum();
            if area < 0.0 {
                ring.reverse();
            }
            let q2: Vec<DVec2> = ring.iter().map(|r| r.1).collect();
            // Fanned from one of its points, or else from a new point
            // inside: three points of the cut's curve alone would make a
            // triangle in the cutting plane, over the face there.
            let p3: Vec<DVec3> = ring.iter().map(|r| model.vertex(r.0)).collect();
            // A fan from `apex` (2D and 3D; `skip` is its index in the ring)
            // whose triangles all turn the right way and face about as the
            // surface does.
            let fan_ok = |apex: DVec2, at: DVec3, skip: Option<usize>| {
                (0..q2.len()).all(|i| {
                    let j = (i + 1) % q2.len();
                    if skip.is_some_and(|k| k == i || k == j) {
                        return true;
                    }
                    let n = (p3[i] - at).cross(p3[j] - at).normalize_or_zero();
                    prism_math::predicates::orient2d(apex, q2[i], q2[j]) > 0.0
                        && (n == DVec3::ZERO || n.dot(want).abs() >= 0.5)
                })
            };
            let corner_apex = (0..q2.len()).find(|&k| fan_ok(q2[k], p3[k], Some(k)));
            let tris: Vec<[usize; 3]> = if let Some(k) = corner_apex {
                (0..q2.len())
                    .filter(|&i| i != k && (i + 1) % q2.len() != k)
                    .map(|i| [k, i, (i + 1) % q2.len()])
                    .collect()
            } else if let Some(c) = crate::regions::interior_point(std::slice::from_ref(&q2))
                .filter(|c| fan_ok(*c, rev.point(c.x, c.y), None))
            {
                let v = model.add_vertex(rev.point(c.x, c.y));
                ring.push((v, c));
                let k = ring.len() - 1;
                (0..q2.len()).map(|i| [k, i, (i + 1) % q2.len()]).collect()
            } else {
                ears(&q2)
            };
            for [a, b, c] in tris {
                let (va, vb, vc) = (ring[a].0, ring[b].0, ring[c].0);
                if va == vb || vb == vc || vc == va {
                    continue;
                }
                let es = vec![
                    line(model, va, vb),
                    line(model, vb, vc),
                    line(model, vc, va),
                ];
                let tp = [model.vertex(va), model.vertex(vb), model.vertex(vc)];
                faces.push(make(vec![va, vb, vc], es, &tp));
            }
        }
        model.remove_face(p);
        for g in faces {
            model.add_face(g);
        }
    }
    true
}

/// Before `m` (an uneven scale) moves `vs`: facet every curved face it
/// moves whole and segment every arc it moves whole, since neither would
/// stay a cylinder, cone, sphere, torus or circle. Returns `vs` with the
/// new points.
pub fn flatten_for(model: &mut Model, vs: &BTreeSet<VertexId>, m: &DAffine3) -> BTreeSet<VertexId> {
    let before: BTreeSet<VertexId> = model.vertices().map(|(id, _)| id).collect();
    let mut ids = Ids::new();
    let faces: Vec<FaceId> = model
        .faces()
        .filter(|(_, f)| {
            f.is_curved()
                && f.surface.transformed(m).is_none()
                && f.loops().flatten().all(|v| vs.contains(v))
        })
        .map(|(id, _)| id)
        .collect();
    for id in faces {
        facet(model, id, &mut ids);
    }
    let arcs: Vec<EdgeId> = model
        .edges()
        .filter(|(_, e)| {
            e.curve.is_curved()
                && e.curve.transformed(m).is_none()
                && vs.contains(&e.a)
                && vs.contains(&e.b)
        })
        .map(|(id, _)| id)
        .collect();
    for e in arcs {
        segment_arc(model, e, &mut ids);
    }
    let mut out = vs.clone();
    out.extend(
        model
            .vertices()
            .map(|(id, _)| id)
            .filter(|v| !before.contains(v)),
    );
    out
}
