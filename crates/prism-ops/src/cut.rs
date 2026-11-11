//! Cutting a curved face (Exact Curves 6b): edges that lie on a face's
//! turned surface, inside the face, split it. It is the region rebuild flat
//! faces use, done in (θ, s) (5b's `Revolution`): a circle round the axis
//! is a line s = constant there, a line or circle through the axis is
//! θ = constant, and any other curve (approximate segments, a slanted
//! circle on a sphere) is the polyline of its drawn points.
//!
//! A point on the axis (a sphere's pole, a cone's apex) has no angle of its
//! own: in (θ, s) it is a short stretch along s = constant, from the angle
//! of the side before it to the angle of the side after, and a cut that
//! ends there lands on that stretch at its own angle.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

use prism_geom::{Profile, Revolution};
use prism_math::DVec2;
use prism_math::predicates::{Inside, point_in_polygon};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

use crate::regions::{interior_point, same_cycle};

/// `x` moved by whole turns to be nearest `to`.
fn near(x: f64, to: f64) -> f64 {
    x + ((to - x) / TAU).round() * TAU
}

/// One side in (θ, s): an edge (None for the stretch across a pole) walked
/// from `from` to `to`, with its points there.
#[derive(Clone, Debug)]
struct Seg {
    edge: Option<EdgeId>,
    from: VertexId,
    to: VertexId,
    pts: Vec<DVec2>,
    inner: bool,
}

struct Places<'a> {
    model: &'a Model,
    rev: Revolution,
    circle: bool,
    eps: f64,
    outward: bool,
}

impl Places<'_> {
    fn on_axis(&self, p: prism_math::DVec3) -> bool {
        self.rev.axis_distance(p) <= self.eps
    }

    /// The sides of loop `k` in (θ, s), unwrapped so nothing jumps by a
    /// turn. None if the loop is not stored whole, or winds round the axis
    /// (a band: it has no outline in (θ, s)).
    fn loop_segs(&self, f: &Face, k: usize) -> Option<Vec<Seg>> {
        let m = self.model;
        let sides: Vec<_> = f.loop_edges(k).collect();
        if sides.is_empty() || sides.len() != f.loops().nth(k)?.len() {
            return None;
        }
        let n = sides.len();
        // Start at a vertex off the axis; with none (a sphere's half
        // between its poles), at the first side's first point off it.
        let start = sides
            .iter()
            .position(|s| !self.on_axis(m.vertex(s.from)))
            .unwrap_or(0);
        let from_pole = self.on_axis(m.vertex(sides[start].from));
        let (mut t0, mut s0) = self.rev.param(m.vertex(sides[start].from));
        if from_pole {
            let q = m
                .edge_points(sides[start].edge)
                .into_iter()
                .find(|q| !self.on_axis(*q))?;
            t0 = self.rev.param(q).0;
        }
        let first_t = t0;
        // The height of the last point off the axis, to tell which way a
        // pole is crossed.
        let mut s_off = s0;
        let mut out = Vec::new();
        let across = |t0: f64, tq: f64, top: bool| {
            let back = top == self.outward;
            let mut t1 = near(tq, t0);
            if back && t1 > t0 {
                t1 -= TAU;
            } else if !back && t1 < t0 {
                t1 += TAU;
            }
            t1
        };
        for j in 0..n {
            let sd = sides[(start + j) % n];
            let e = m.edge(sd.edge)?;
            let mut pts3 = m.edge_points(sd.edge);
            if e.a != sd.from {
                pts3.reverse();
            }
            if j > 0 && self.on_axis(pts3[0]) && pts3.len() > 1 {
                // Across the pole, to the angle of this side.
                let q = pts3[1..].iter().find(|q| !self.on_axis(**q))?;
                let tq = self.rev.param(*q).0;
                let t1 = across(t0, tq, s0 > s_off);
                if (t1 - t0).abs() > 1e-12 {
                    out.push(Seg {
                        edge: None,
                        from: sd.from,
                        to: sd.from,
                        pts: vec![DVec2::new(t0, s0), DVec2::new(t1, s0)],
                        inner: false,
                    });
                }
                t0 = t1;
            }
            let mut pts = vec![DVec2::new(t0, s0)];
            for p in &pts3[1..] {
                let (t, s) = self.rev.param(*p);
                let s = if self.circle { near(s, s0) } else { s };
                s0 = s;
                if self.on_axis(*p) {
                    pts.push(DVec2::new(t0, s));
                    continue;
                }
                t0 = near(t, t0);
                s_off = s;
                pts.push(DVec2::new(t0, s));
            }
            out.push(Seg {
                edge: Some(sd.edge),
                from: sd.from,
                to: sd.to,
                pts,
                inner: false,
            });
        }
        if from_pole {
            // Back across the starting pole.
            let v = sides[start].from;
            let t1 = across(t0, first_t, s0 > s_off);
            if (t1 - t0).abs() > 1e-12 {
                out.push(Seg {
                    edge: None,
                    from: v,
                    to: v,
                    pts: vec![DVec2::new(t0, s0), DVec2::new(t1, s0)],
                    inner: false,
                });
            }
            t0 = t1;
        }
        ((t0 - first_t).abs() < 1e-6).then_some(out)
    }

    /// An edge's points in (θ, s) from its `a` to its `b`, unwrapped along
    /// it (a point on the axis takes the angle of its neighbour).
    fn edge_pts(&self, id: EdgeId) -> Option<Vec<DVec2>> {
        let pts3 = self.model.edge_points(id);
        let first = pts3.iter().position(|p| !self.on_axis(*p))?;
        let (t, s) = self.rev.param(pts3[first]);
        let mut out = vec![DVec2::new(t, s); pts3.len()];
        // Forward from the first point off the axis, then back.
        let (mut t0, mut s0) = (t, s);
        for i in first + 1..pts3.len() {
            let (t, s) = self.rev.param(pts3[i]);
            s0 = if self.circle { near(s, s0) } else { s };
            if !self.on_axis(pts3[i]) {
                t0 = near(t, t0);
            }
            out[i] = DVec2::new(t0, s0);
        }
        let (mut t0, mut s0) = (t, s);
        for i in (0..first).rev() {
            let (t, s) = self.rev.param(pts3[i]);
            s0 = if self.circle { near(s, s0) } else { s };
            if !self.on_axis(pts3[i]) {
                t0 = near(t, t0);
            }
            out[i] = DVec2::new(t0, s0);
        }
        Some(out)
    }
}

/// A traced region: its cycle of half-edges, its polygon, and the cycles
/// of its holes.
type Region = (Vec<usize>, Vec<DVec2>, Vec<Vec<usize>>);

/// The node for vertex `v` at `p`: one it has there already, or a new one.
fn node(
    nodes: &mut Vec<DVec2>,
    at: &mut BTreeMap<VertexId, Vec<usize>>,
    v: VertexId,
    p: DVec2,
) -> usize {
    let list = at.entry(v).or_default();
    if let Some(i) = list.iter().find(|i| nodes[**i].distance(p) < 1e-9) {
        return *i;
    }
    nodes.push(p);
    list.push(nodes.len() - 1);
    nodes.len() - 1
}

fn area(poly: &[DVec2]) -> f64 {
    (0..poly.len())
        .map(|i| poly[i].perp_dot(poly[(i + 1) % poly.len()]))
        .sum::<f64>()
        / 2.0
}

/// The polygon of a run of segments (each without its last point).
fn polygon(segs: &[&Seg], dirs: &[bool]) -> Vec<DVec2> {
    let mut out = Vec::new();
    for (s, fwd) in segs.iter().zip(dirs) {
        let n = s.pts.len();
        if *fwd {
            out.extend_from_slice(&s.pts[..n - 1]);
        } else {
            out.extend(s.pts[1..].iter().rev());
        }
    }
    out
}

/// How far each hole must move, by whole turns (θ, and s on a circle
/// profile), to lie inside the outer loop: each loop is unwrapped from its
/// own start, so a hole can come out a turn away.
pub(crate) fn hole_shifts(polys: &[Vec<DVec2>], circle: bool) -> Vec<DVec2> {
    let s_turns: &[f64] = if circle { &[0.0, -1.0, 1.0] } else { &[0.0] };
    polys
        .iter()
        .skip(1)
        .map(|h| {
            [0.0, -1.0, 1.0, -2.0, 2.0]
                .iter()
                .find_map(|kt| {
                    s_turns.iter().find_map(|ks| {
                        let d = DVec2::new(kt * TAU, ks * TAU);
                        h.iter()
                            .any(|p| point_in_polygon(*p + d, &polys[0]) == Inside::Inside)
                            .then_some(d)
                    })
                })
                .unwrap_or(DVec2::ZERO)
        })
        .collect()
}

/// A curved face laid out in (θ, s): its surface's turned form and the
/// polygon of each loop (counter-clockwise when the face points out of its
/// surface). None for a flat face or one that cannot be laid out.
pub(crate) fn layout(model: &Model, id: FaceId) -> Option<(Revolution, Vec<Vec<DVec2>>)> {
    let face = model.face(id)?;
    let rev = face.surface.revolution()?;
    let pl = Places {
        model,
        rev,
        circle: matches!(rev.profile, Profile::Circle { .. }),
        eps: model.tol().dist,
        outward: face.surface.outward(),
    };
    let mut polys = (0..=face.holes.len())
        .map(|k| {
            let l = pl.loop_segs(face, k)?;
            Some(polygon(&l.iter().collect::<Vec<_>>(), &vec![true; l.len()]))
        })
        .collect::<Option<Vec<_>>>()?;
    for (h, d) in hole_shifts(&polys, pl.circle).into_iter().enumerate() {
        for p in &mut polys[h + 1] {
            *p += d;
        }
    }
    Some((rev, polys))
}

fn inside(q: DVec2, polys: &[Vec<DVec2>]) -> bool {
    point_in_polygon(q, &polys[0]) == Inside::Inside
        && polys[1..]
            .iter()
            .all(|h| point_in_polygon(q, h) == Inside::Outside)
}

/// The edges not in `face` that lie on its surface inside it.
fn inner_edges(model: &Model, face: &Face, eps: f64) -> Vec<EdgeId> {
    let used: BTreeSet<EdgeId> = face.edge_ids().collect();
    let pts: Vec<_> = model.face_loop_points(face).into_iter().flatten().collect();
    if pts.is_empty() {
        return Vec::new();
    }
    let pad = prism_math::DVec3::splat(eps);
    let (lo, hi) = pts
        .iter()
        .fold((pts[0], pts[0]), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let (lo, hi) = (lo - pad, hi + pad);
    let s = face.surface;
    model
        .edges()
        .filter(|(id, _)| !used.contains(id))
        .filter(|(_, e)| {
            let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
            let boxed = |p: prism_math::DVec3| p.cmpge(lo).all() && p.cmple(hi).all();
            boxed(pa)
                && boxed(pb)
                && s.distance(pa).abs() <= eps
                && s.distance(pb).abs() <= eps
                && (e.approx || s.holds(&e.curve, pa, pb, eps))
        })
        .map(|(id, _)| id)
        .collect()
}

/// Split curved face `id` along the edges on its surface inside it.
/// Returns the faces it became (itself if nothing cuts it, or if it cannot
/// be laid out in (θ, s)).
pub fn split_face(model: &mut Model, id: FaceId) -> Vec<FaceId> {
    let Some(face) = model.face(id).cloned() else {
        return Vec::new();
    };
    let Some(rev) = face.surface.revolution() else {
        return vec![id];
    };
    let eps = model.tol().dist;
    let candidates = inner_edges(model, &face, eps);
    if candidates.is_empty() {
        return vec![id];
    }
    let pl = Places {
        model,
        rev,
        circle: matches!(rev.profile, Profile::Circle { .. }),
        eps,
        outward: face.surface.outward(),
    };
    let Some(mut loops): Option<Vec<Vec<Seg>>> = (0..=face.holes.len())
        .map(|k| pl.loop_segs(&face, k))
        .collect()
    else {
        return vec![id];
    };
    let mut polys: Vec<Vec<DVec2>> = loops
        .iter()
        .map(|l| polygon(&l.iter().collect::<Vec<_>>(), &vec![true; l.len()]))
        .collect();
    for (h, d) in hole_shifts(&polys, pl.circle).into_iter().enumerate() {
        for p in &mut polys[h + 1] {
            *p += d;
        }
        for sg in &mut loops[h + 1] {
            for p in &mut sg.pts {
                *p += d;
            }
        }
    }
    // Laid out clockwise (a face pointing into its surface): mirror θ so
    // the tracing below always sees its outline counter-clockwise.
    let mirror = area(&polys[0]) < 0.0;

    // Nodes: a vertex's place; a pole can have several.
    let mut nodes: Vec<DVec2> = Vec::new();
    let mut at: BTreeMap<VertexId, Vec<usize>> = BTreeMap::new();
    let mut segs: Vec<Seg> = loops.into_iter().flatten().collect();
    for s in &segs {
        node(&mut nodes, &mut at, s.from, s.pts[0]);
        node(&mut nodes, &mut at, s.to, s.pts[s.pts.len() - 1]);
    }
    let boundary_vs: BTreeSet<VertexId> = face.loops().flatten().copied().collect();

    for e in candidates {
        let Some(x) = pl.model.edge(e).copied() else {
            continue;
        };
        let Some(raw) = pl.edge_pts(e) else { continue };
        // Moved by whole turns (θ, and s on a circle profile) to lie in
        // the face, judged at its middle.
        let mid = |p: &[DVec2]| {
            if p.len() >= 3 {
                p[p.len() / 2]
            } else {
                p[0].lerp(p[p.len() - 1], 0.5)
            }
        };
        let s_turns: &[f64] = if pl.circle { &[0.0, -1.0, 1.0] } else { &[0.0] };
        let shift = [0.0, -1.0, 1.0, -2.0, 2.0].iter().find_map(|kt| {
            s_turns.iter().find_map(|ks| {
                let d = DVec2::new(kt * TAU, ks * TAU);
                inside(mid(&raw) + d, &polys).then_some(d)
            })
        });
        let Some(d) = shift else { continue };
        let pts: Vec<DVec2> = raw.iter().map(|p| *p + d).collect();
        // Its ends: an existing node of that vertex, or a new one; at a
        // pole, a place on the stretch across it.
        let end = |nodes: &mut Vec<DVec2>,
                   at: &mut BTreeMap<VertexId, Vec<usize>>,
                   segs: &mut Vec<Seg>,
                   v: VertexId,
                   p: DVec2| {
            let list = at.get(&v).cloned().unwrap_or_default();
            if let Some(i) = list.iter().find(|i| nodes[**i].distance(p) < 1e-6) {
                return Some(*i);
            }
            if !pl.on_axis(pl.model.vertex(v)) {
                // A boundary vertex placed elsewhere: not this face's.
                return (!boundary_vs.contains(&v) && list.is_empty())
                    .then(|| node(nodes, at, v, p));
            }
            // Split the stretch across the pole that holds this angle.
            let k = segs.iter().position(|s| {
                s.edge.is_none()
                    && s.from == v
                    && (s.pts[0].y - p.y).abs() < 1e-9
                    && (p.x - s.pts[0].x) * (p.x - s.pts[1].x) < 0.0
            });
            match k {
                Some(k) => {
                    let n = node(nodes, at, v, p);
                    let s = segs[k].clone();
                    segs[k].pts[1] = p;
                    segs.push(Seg {
                        pts: vec![p, s.pts[1]],
                        ..s
                    });
                    Some(n)
                }
                None => list.is_empty().then(|| node(nodes, at, v, p)),
            }
        };
        let a = end(&mut nodes, &mut at, &mut segs, x.a, pts[0]);
        let b = end(&mut nodes, &mut at, &mut segs, x.b, pts[pts.len() - 1]);
        if a.is_none() || b.is_none() {
            continue;
        }
        segs.push(Seg {
            edge: Some(e),
            from: x.a,
            to: x.b,
            pts,
            inner: true,
        });
    }
    // Every segment's end nodes, again (splitting stretches moved some).
    let node_at = |v: VertexId, p: DVec2| -> Option<usize> {
        at.get(&v)?
            .iter()
            .copied()
            .min_by(|i, j| nodes[*i].distance(p).total_cmp(&nodes[*j].distance(p)))
    };
    let mut ends: Vec<(usize, usize)> = Vec::new();
    for s in &segs {
        let (Some(a), Some(b)) = (
            node_at(s.from, s.pts[0]),
            node_at(s.to, s.pts[s.pts.len() - 1]),
        ) else {
            return vec![id];
        };
        ends.push((a, b));
    }
    // Cuts that end nowhere split nothing.
    let mut alive = vec![true; segs.len()];
    loop {
        let mut degree = vec![0usize; nodes.len()];
        for (i, (a, b)) in ends.iter().enumerate() {
            if alive[i] {
                degree[*a] += 1;
                degree[*b] += 1;
            }
        }
        let dead: Vec<usize> = (0..segs.len())
            .filter(|i| {
                alive[*i] && segs[*i].inner && {
                    let (a, b) = ends[*i];
                    degree[a] < 2 || degree[b] < 2 || a == b
                }
            })
            .collect();
        if dead.is_empty() {
            break;
        }
        for i in dead {
            alive[i] = false;
        }
    }
    if !(0..segs.len()).any(|i| alive[i] && segs[i].inner) {
        return vec![id];
    }
    if mirror {
        for s in &mut segs {
            for p in &mut s.pts {
                p.x = -p.x;
            }
        }
    }
    let polys: Vec<Vec<DVec2>> = if mirror {
        polys
            .into_iter()
            .map(|l| l.into_iter().map(|p| DVec2::new(-p.x, p.y)).collect())
            .collect()
    } else {
        polys
    };

    // Half-edges 2i (a to b) and 2i + 1 (b to a), sorted round each node
    // counter-clockwise by the direction they leave it.
    let live: Vec<usize> = (0..segs.len()).filter(|i| alive[*i]).collect();
    let mut around: BTreeMap<usize, Vec<(f64, usize)>> = BTreeMap::new();
    let leave = |h: usize| -> DVec2 {
        let p = &segs[h / 2].pts;
        if h.is_multiple_of(2) {
            p[1] - p[0]
        } else {
            p[p.len() - 2] - p[p.len() - 1]
        }
    };
    let start_of = |h: usize| {
        if h.is_multiple_of(2) {
            ends[h / 2].0
        } else {
            ends[h / 2].1
        }
    };
    let end_of = |h: usize| {
        if h.is_multiple_of(2) {
            ends[h / 2].1
        } else {
            ends[h / 2].0
        }
    };
    for &i in &live {
        for h in [2 * i, 2 * i + 1] {
            let d = leave(h);
            around
                .entry(start_of(h))
                .or_default()
                .push((d.y.atan2(d.x), h));
        }
    }
    for list in around.values_mut() {
        list.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    }
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut cycles: Vec<Vec<usize>> = Vec::new();
    for &i in &live {
        for h0 in [2 * i, 2 * i + 1] {
            if seen.contains(&h0) {
                continue;
            }
            let mut cycle = Vec::new();
            let mut h = h0;
            while seen.insert(h) {
                cycle.push(h);
                let list = &around[&end_of(h)];
                let twin = h ^ 1;
                let k = list.iter().position(|x| x.1 == twin).unwrap_or(0);
                h = list[(k + list.len() - 1) % list.len()].1;
            }
            cycles.push(cycle);
        }
    }
    let poly_of = |c: &[usize]| -> Vec<DVec2> {
        let s: Vec<&Seg> = c.iter().map(|h| &segs[h / 2]).collect();
        let d: Vec<bool> = c.iter().map(|h| h % 2 == 0).collect();
        polygon(&s, &d)
    };
    let mut regions: Vec<Region> = Vec::new();
    let mut hole_cycles: Vec<(Vec<usize>, Vec<DVec2>)> = Vec::new();
    for c in cycles {
        let p = poly_of(&c);
        if area(&p) > 0.0 {
            if let Some(q) = interior_point(std::slice::from_ref(&p))
                && inside(q, &polys)
            {
                regions.push((c, p, Vec::new()));
            }
        } else {
            hole_cycles.push((c, p));
        }
    }
    for (c, p) in hole_cycles {
        let best = regions
            .iter()
            .enumerate()
            .filter(|(_, r)| point_in_polygon(p[0], &r.1) == Inside::Inside)
            .min_by(|a, b| area(&a.1.1).total_cmp(&area(&b.1.1)))
            .map(|(i, _)| i);
        if let Some(i) = best {
            regions[i].2.push(c);
        }
    }
    let to_loop = |c: &[usize]| -> (Vec<VertexId>, Vec<EdgeId>) {
        c.iter()
            .filter_map(|h| {
                let s = &segs[h / 2];
                let e = s.edge?;
                Some((if h.is_multiple_of(2) { s.from } else { s.to }, e))
            })
            .unzip()
    };
    let made: Vec<Face> = regions
        .iter()
        .map(|(c, _, holes)| {
            let (outer, oe) = to_loop(c);
            let (hv, he): (Vec<_>, Vec<_>) = holes.iter().map(|h| to_loop(h)).unzip();
            let mut edges = vec![oe];
            edges.extend(he);
            Face::with_edges(outer, hv, face.surface, edges)
        })
        .collect();
    if made.len() == 1
        && same_cycle(made[0].loop_edge_ids(0), face.loop_edge_ids(0))
        && made[0].holes.len() == face.holes.len()
    {
        return vec![id];
    }
    if made.is_empty() {
        return vec![id];
    }
    model.remove_face(id);
    made.into_iter().map(|f| model.add_face(f)).collect()
}

/// Split every curved face in `faces` (all curved faces if None) along the
/// edges on it. Returns the faces made.
pub fn split_curved(model: &mut Model, faces: Option<&BTreeSet<FaceId>>) -> Vec<FaceId> {
    let ids: Vec<FaceId> = model
        .faces()
        .filter(|(id, f)| f.is_curved() && faces.is_none_or(|s| s.contains(id)))
        .map(|(id, _)| id)
        .collect();
    let mut out = Vec::new();
    for id in ids {
        let made = split_face(model, id);
        if made != [id] {
            out.extend(made);
        }
    }
    out
}
