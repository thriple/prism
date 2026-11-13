//! Sides for the Loft tool (brief 34): chains of edges picked by hand,
//! what they make together (a ring to fill, or sections to loft through),
//! and the surfaces.
//!
//! - [`chain`]: the edges that run on from a picked one without a sharp
//!   turn.
//! - [`side`]: a chain of edges as one exact curve.
//! - [`shape`]: whether the sides close a ring, are separate sections, or
//!   almost close a ring (and which end is open).
//! - [`surfaces`]: the surfaces they make, for a preview; [`made`] puts
//!   them into the model.
//!
//! A ring of four sides is one Coons patch. Three, five or more are split
//! from a centre point: a curve from the centre to the middle of each
//! side, and a four-sided patch in each corner. The patches are exact
//! along the ring and smooth across the inner curves (their control rows
//! either side of an inner curve are made mirror images, in homogeneous
//! coordinates, so the surface is smooth there).

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Curve, Nurbs, NurbsSurface};
use prism_math::{DVec3, newell};
use prism_topo::{EdgeId, FaceId, Model, VertexId};

use crate::loft::{DEGENERATE, middle};

/// Knots added to every curve of a split fill (0 is a side, 1 the centre).
const REFINE: [f64; 9] = [
    0.25, 0.5, 0.75, 0.875, 0.9375, 0.96875, 0.984375, 0.125, 0.375,
];

/// The default sharpest turn a side runs on through, in degrees.
pub const TURN_DEG: f64 = 30.0;
/// The most rows a preset can ask for.
pub const MAX_ROWS: usize = 8;

pub const NOT_JOINED: &str = "These edges do not join end to end in one run.";
pub const TOO_FEW: &str = "Pick two or more sides: sections to loft through, or a ring to fill.";
pub const RING_OF_TWO: &str =
    "Two sides that close a ring cannot be filled: split one so the ring has three or more sides.";
pub const OPEN: &str =
    "The sides neither close a ring nor stand apart: some touch and some do not.";
pub const CLOSED_SIDE: &str =
    "A closed curve can only be lofted through, not used as a side of a ring.";
pub const PERIODIC_PART: &str =
    "Part of a closed smooth curve cannot be a side yet; pick all of it.";

/// How the surface runs between sections, and how many rows of faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Preset {
    /// Straight between sections (else smooth through them).
    pub straight: bool,
    /// Rows of faces between each pair of sections, or in each patch of a
    /// fill (1 for none added).
    pub rows: usize,
}

impl Default for Preset {
    fn default() -> Preset {
        Preset {
            straight: false,
            rows: 1,
        }
    }
}

/// One side: its edges in order and the curve along them.
#[derive(Clone, Debug)]
pub struct Side {
    pub edges: Vec<EdgeId>,
    /// From the first edge's free end to the last's (for a closed run,
    /// round from the first vertex).
    pub curve: Nurbs,
}

impl Side {
    pub fn ends(&self) -> (DVec3, DVec3) {
        let (a, b) = self.curve.domain();
        (self.curve.eval(a), self.curve.eval(b))
    }
}

/// The unit direction an edge leaves `v` in.
fn leaving(model: &Model, e: EdgeId, v: VertexId) -> Option<DVec3> {
    let x = model.edge(e)?;
    let c = x.curve_from(v);
    Some(c.start_tangent(model.vertex(v), model.vertex(x.other(v))))
}

/// Whether two edges lie on one curve: the same spline, arcs of one
/// circle, or edges drawn as one curve.
fn same_curve(model: &Model, a: EdgeId, b: EdgeId) -> bool {
    let (Some(x), Some(y)) = (model.edge(a), model.edge(b)) else {
        return false;
    };
    if x.curve_id.is_some() && x.curve_id == y.curve_id {
        return true;
    }
    let tol = model.tol();
    match (&x.curve, &y.curve) {
        (Curve::Spline(p), Curve::Spline(q)) => {
            std::sync::Arc::ptr_eq(&p.curve, &q.curve) || p.curve == q.curve
        }
        (
            Curve::Arc {
                centre: c1,
                axis: n1,
                radius: r1,
            },
            Curve::Arc {
                centre: c2,
                axis: n2,
                radius: r2,
            },
        ) => {
            tol.same_point(*c1, *c2)
                && (r1 - r2).abs() <= tol.dist
                && n1.cross(*n2).length() <= 1e-9
        }
        _ => false,
    }
}

/// The edges joined to `start` that run on from it, end to end, while the
/// turn at each vertex is under `max_turn` radians. Where three or more
/// edges meet, only the same curve runs on. In order along the run;
/// `start` is among them.
pub fn chain(model: &Model, start: EdgeId, max_turn: f64) -> Vec<EdgeId> {
    let Some(first) = model.edge(start) else {
        return Vec::new();
    };
    let mut at: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for (id, e) in model.edges() {
        at.entry(e.a).or_default().push(id);
        at.entry(e.b).or_default().push(id);
    }
    let mut used: BTreeSet<EdgeId> = BTreeSet::from([start]);
    // Walk on from `v`, an end of the run.
    let walk = |from: EdgeId, v: VertexId, used: &mut BTreeSet<EdgeId>| {
        let mut out = Vec::new();
        let (mut e, mut v) = (from, v);
        // Arriving at `v` along `e`: the direction to carry on in.
        while let Some(arrive) = leaving(model, e, v).map(|d| -d) {
            // Where three or more edges meet, only the same curve runs on
            // (a line crossing a curve does not end it, and a curve does
            // not run into a box's edge it happens to touch).
            let junction = at.get(&v).map_or(0, Vec::len) > 2;
            let next = at
                .get(&v)
                .into_iter()
                .flatten()
                .filter(|k| !used.contains(k))
                .filter(|k| !junction || same_curve(model, e, **k))
                .filter_map(|k| Some((*k, leaving(model, *k, v)?)))
                .map(|(k, d)| (k, arrive.angle_between(d)))
                .filter(|(_, a)| *a < max_turn)
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
            let Some((k, _)) = next else { break };
            used.insert(k);
            out.push(k);
            let Some(x) = model.edge(k) else { break };
            v = x.other(v);
            e = k;
        }
        out
    };
    let ahead = walk(start, first.b, &mut used);
    let behind = walk(start, first.a, &mut used);
    behind
        .into_iter()
        .rev()
        .chain(std::iter::once(start))
        .chain(ahead)
        .collect()
}

/// An edge as a curve of its own, walked from `v`.
fn piece(model: &Model, e: EdgeId, v: VertexId) -> Result<Nurbs, &'static str> {
    let x = model.edge(e).ok_or(NOT_JOINED)?;
    let (a, b) = (model.vertex(v), model.vertex(x.other(v)));
    let c = x.curve_from(v);
    match &c {
        Curve::Line => Nurbs::line(a, b).ok_or(DEGENERATE),
        Curve::Arc { centre, axis, .. } => {
            Nurbs::arc(*centre, *axis, a, c.sweep(a, b)).ok_or(DEGENERATE)
        }
        Curve::Spline(s) => {
            if s.is_whole() {
                let whole = (*s.curve).clone();
                return Ok(if s.t0 > s.t1 { whole.reversed() } else { whole });
            }
            if s.curve.is_periodic() {
                return Err(PERIODIC_PART);
            }
            let part = s
                .curve
                .between(s.t0.min(s.t1), s.t0.max(s.t1))
                .ok_or(DEGENERATE)?;
            Ok(if s.t0 > s.t1 { part.reversed() } else { part })
        }
    }
}

/// Edges (in any order) as one side: put end to end and joined into one
/// exact curve. A closed run starts at its lowest vertex id.
pub fn side(model: &Model, edges: &[EdgeId]) -> Result<Side, &'static str> {
    let set: BTreeSet<EdgeId> = edges.iter().copied().collect();
    if set.is_empty() {
        return Err(NOT_JOINED);
    }
    let mut count: BTreeMap<VertexId, usize> = BTreeMap::new();
    for e in &set {
        let x = model.edge(*e).ok_or(NOT_JOINED)?;
        if x.a == x.b {
            // A closed curve in one edge (a circle).
            if set.len() != 1 {
                return Err(NOT_JOINED);
            }
            return Ok(Side {
                edges: vec![*e],
                curve: piece(model, *e, x.a)?,
            });
        }
        *count.entry(x.a).or_default() += 1;
        *count.entry(x.b).or_default() += 1;
    }
    if count.values().any(|n| *n > 2) {
        return Err(NOT_JOINED);
    }
    let ends: Vec<VertexId> = count
        .iter()
        .filter(|(_, n)| **n == 1)
        .map(|(v, _)| *v)
        .collect();
    let start = match ends.as_slice() {
        [a, _] => *a,
        [] => *count.keys().next().ok_or(NOT_JOINED)?,
        _ => return Err(NOT_JOINED),
    };
    let mut order = Vec::new();
    let mut pieces = Vec::new();
    let mut left = set.clone();
    let mut v = start;
    while !left.is_empty() {
        let e = *left
            .iter()
            .find(|e| model.edge(**e).is_some_and(|x| x.has(v)))
            .ok_or(NOT_JOINED)?;
        left.remove(&e);
        pieces.push(piece(model, e, v)?);
        order.push(e);
        v = model.edge(e).ok_or(NOT_JOINED)?.other(v);
    }
    let curve = if pieces.len() == 1 {
        pieces.remove(0)
    } else {
        Nurbs::joined(&pieces).ok_or(NOT_JOINED)?
    };
    Ok(Side {
        edges: order,
        curve,
    })
}

/// What the sides make.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// Fewer than two sides.
    Few,
    /// They join end to end in a ring, in the order picked: each side's
    /// end meets the next one's. `turned[i]` says side `i` runs backwards
    /// round it.
    Ring { turned: Vec<bool> },
    /// They do not touch: sections to loft through, in the order picked.
    Sections,
    /// They join in a chain that does not close: the ring's two open ends.
    Open { ends: [DVec3; 2] },
    /// Anything else.
    Mixed,
}

/// Whether the sides close a ring (in the order picked), stand apart, or
/// almost close a ring.
pub fn shape(sides: &[Side], tol: f64) -> Shape {
    if sides.len() < 2 {
        return Shape::Few;
    }
    let ends: Vec<(DVec3, DVec3)> = sides.iter().map(Side::ends).collect();
    let near = |a: DVec3, b: DVec3| a.distance(b) <= tol;
    let touch = |i: usize, j: usize| {
        let ((a0, a1), (b0, b1)) = (ends[i], ends[j]);
        near(a0, b0) || near(a0, b1) || near(a1, b0) || near(a1, b1)
    };
    let closed = sides.iter().any(|s| s.curve.is_closed());
    let any_touch = (0..sides.len()).any(|i| (i + 1..sides.len()).any(|j| touch(i, j)));
    if !any_touch {
        return Shape::Sections;
    }
    if closed {
        return Shape::Mixed;
    }
    // Walk the chain in the order picked: side 0 turned so its end meets
    // side 1.
    let n = sides.len();
    let mut turned = vec![false; n];
    let (a0, a1) = ends[0];
    let (b0, b1) = ends[1];
    if !(near(a1, b0) || near(a1, b1)) && (near(a0, b0) || near(a0, b1)) {
        turned[0] = true;
    }
    let end_of = |i: usize, t: bool| if t { ends[i].0 } else { ends[i].1 };
    let start_of = |i: usize, t: bool| if t { ends[i].1 } else { ends[i].0 };
    let mut joined = 1;
    for i in 1..n {
        let at = end_of(i - 1, turned[i - 1]);
        if near(ends[i].0, at) {
            turned[i] = false;
        } else if near(ends[i].1, at) {
            turned[i] = true;
        } else {
            break;
        }
        joined += 1;
    }
    let first = start_of(0, turned[0]);
    let last = end_of(n - 1, turned[n - 1]);
    if joined < n {
        return Shape::Mixed;
    }
    if near(first, last) {
        Shape::Ring { turned }
    } else {
        Shape::Open {
            ends: [last, first],
        }
    }
}

/// The sides' curves round the ring, each from its corner to the next.
fn round(sides: &[Side], turned: &[bool]) -> Vec<Nurbs> {
    sides
        .iter()
        .zip(turned)
        .map(|(s, t)| {
            if *t {
                s.curve.reversed().normalized()
            } else {
                s.curve.normalized()
            }
        })
        .collect()
}

/// The mean of each run of `degree` knots: where a control point's
/// influence is centred.
fn greville(c: &Nurbs) -> Vec<f64> {
    let p = c.degree();
    (0..c.points().len())
        .map(|i| c.knots()[i + 1..=i + p].iter().sum::<f64>() / p as f64)
        .collect()
}

/// A point times its weight, and the weight.
type H = (DVec3, f64);

fn h(c: &Nurbs, i: usize) -> H {
    let w = c.weights()[i];
    (c.points()[i] * w, w)
}

fn lerp(a: H, b: H, t: f64) -> H {
    (a.0.lerp(b.0, t), a.1 + (b.1 - a.1) * t)
}

/// The control net (homogeneous, `[i][j]` along u then v) inside four
/// alike-in-pairs curves: `bottom` and `top` along u, `left` and `right`
/// along v, corners shared. A bilinearly blended Coons patch, so exact
/// along all four (weighted curves too).
fn coons_net(bottom: &Nurbs, right: &Nurbs, top: &Nurbs, left: &Nurbs) -> Vec<Vec<H>> {
    let (gu, gv) = (greville(bottom), greville(left));
    let (nu, nv) = (gu.len(), gv.len());
    let (p00, p10) = (h(bottom, 0), h(bottom, nu - 1));
    let (p01, p11) = (h(top, 0), h(top, nu - 1));
    (0..nu)
        .map(|i| {
            (0..nv)
                .map(|j| {
                    let (x, y) = (gu[i], gv[j]);
                    let across = lerp(h(bottom, i), h(top, i), y);
                    let along = lerp(h(left, j), h(right, j), x);
                    let flat = lerp(lerp(p00, p10, x), lerp(p01, p11, x), y);
                    (across.0 + along.0 - flat.0, across.1 + along.1 - flat.1)
                })
                .collect()
        })
        .collect()
}

fn surface_of(net: &[Vec<H>], ku: &Nurbs, kv: &Nurbs) -> Option<NurbsSurface> {
    let (nu, nv) = (net.len(), net[0].len());
    let mut points = Vec::with_capacity(nu * nv);
    let mut weights = Vec::with_capacity(nu * nv);
    for col in net {
        for (p, w) in col {
            if w.is_nan() || *w <= 1e-9 {
                return None;
            }
            points.push(*p / *w);
            weights.push(*w);
        }
    }
    NurbsSurface::new(
        [ku.degree(), kv.degree()],
        [ku.knots().to_vec(), kv.knots().to_vec()],
        [nu, nv],
        points,
        weights,
        [false, false],
    )
}

/// The Coons patch inside four curves round a ring (each from its corner
/// to the next).
fn four(ring: &[Nurbs]) -> Option<NurbsSurface> {
    let ring: Vec<Nurbs> = ring.iter().map(Nurbs::unit_ends).collect::<Option<_>>()?;
    let bt = Nurbs::alike(&[ring[0].clone(), ring[2].reversed()])?;
    let lr = Nurbs::alike(&[ring[3].reversed(), ring[1].clone()])?;
    let net = coons_net(&bt[0], &lr[1], &bt[1], &lr[0]);
    surface_of(&net, &bt[0], &lr[0])
}

/// The unit normal the fill should have at `p` on a side running along
/// `t` with curvature `k`: the ring's normal `n`, leaned towards the side's
/// own bend where the side bends out of the ring's plane (so a ring of
/// arcs on a sphere fills as the sphere).
fn normal_at(n: DVec3, t: DVec3, k: DVec3) -> DVec3 {
    let flat = (n - t * n.dot(t)).normalize_or_zero();
    let bend = (k - t * k.dot(t)).normalize_or_zero();
    let a = bend.dot(flat);
    let lean = a.abs().sqrt();
    (flat * (1.0 - lean) + bend * a.signum() * lean)
        .try_normalize()
        .unwrap_or(flat)
}

/// Three, five or more sides: a centre point, a curve from it to the
/// middle of each side, and a four-sided patch in each corner, smooth
/// across the inner curves. Patch `i` has corner `i` (where side `i - 1`
/// ends and side `i` starts) at (0, 0), runs along side `i` in u and back
/// along side `i - 1` in v; the centre is at (1, 1).
fn split(ring: &[Nurbs]) -> Option<Vec<NurbsSurface>> {
    let n = ring.len();
    let halves: Vec<(Nurbs, Nurbs)> = ring
        .iter()
        .map(|c| {
            // Weight 1 at the corners; at the middle, the weight the split
            // gave, the same both sides, so the halves stay mirror images
            // there.
            let a = c
                .between(0.0, 0.5)?
                .reversed()
                .reweighted_end(1.0)?
                .reversed();
            let b = c.between(0.5, 1.0)?.reweighted_end(1.0)?;
            Some((a, b))
        })
        .collect::<Option<_>>()?;
    let mids: Vec<DVec3> = ring.iter().map(|c| c.eval(0.5)).collect();
    let tangents: Vec<DVec3> = ring
        .iter()
        .map(|c| c.eval_d(0.5).1.normalize_or_zero())
        .collect();
    let mut samples = Vec::new();
    for c in ring {
        for k in 0..8 {
            samples.push(c.eval(k as f64 / 8.0));
        }
    }
    let up = newell(&samples).try_normalize()?;
    let base = mids.iter().copied().sum::<DVec3>() / n as f64;
    // Into the ring at each middle: square to the side, in the plane the
    // fill should have there.
    let inward: Vec<DVec3> = (0..n)
        .map(|i| {
            let nm = normal_at(up, tangents[i], ring[i].curvature(0.5));
            let d = nm.cross(tangents[i]).normalize_or_zero();
            if d.dot(base - mids[i]) < 0.0 { -d } else { d }
        })
        .collect();
    // The centre: above the middles' mean by as much as a circular arc
    // leaving each middle along `inward` rises before it levels off.
    let height = (0..n)
        .map(|i| {
            let off = mids[i] - base;
            let e = off.dot(up);
            let d = (off - up * e).length();
            let theta = inward[i].dot(up).clamp(-1.0, 1.0).asin();
            e + d * (theta / 2.0).tan()
        })
        .sum::<f64>()
        / n as f64;
    let centre = base + up * height;
    // The inner curves, centre-level at the centre so every patch's normal
    // there is `up` (no crease at the centre).
    let inner: Vec<Nurbs> = (0..n)
        .map(|i| {
            let chord = centre.distance(mids[i]);
            let flat = (mids[i] - centre) - up * (mids[i] - centre).dot(up);
            let out = flat.try_normalize()?;
            Nurbs::open(
                &[
                    mids[i],
                    mids[i] + inward[i] * (chord / 3.0),
                    centre + out * (chord / 3.0),
                    centre,
                ],
                3,
            )
        })
        .collect::<Option<_>>()?;
    // Each inner curve weighted to match the side at its middle, and 1 at
    // the centre.
    let inner: Vec<Nurbs> = inner
        .iter()
        .zip(&halves)
        .map(|(c, (a, _))| {
            let wm = a.weights()[a.weights().len() - 1];
            c.scaled_weights(wm).reweighted_end(1.0)
        })
        .collect::<Option<_>>()?;
    // Everything alike, so every inner curve has the same knots in both
    // patches beside it.
    let mut all: Vec<Nurbs> = Vec::with_capacity(3 * n);
    for (a, b) in &halves {
        all.push(a.clone());
        all.push(b.reversed());
    }
    all.extend(inner.iter().cloned());
    let mut all = Nurbs::alike(&all)?;
    // Finer towards the centre, where the patches can only meet smoothly
    // on average: the more rows there, the smaller the area that is not
    // quite smooth (and near the centre every patch faces `up` anyway).
    for t in REFINE {
        if all[0].knots().iter().all(|k| (k - t).abs() > 1e-6) {
            all = all
                .iter()
                .map(|c| c.insert_knot(t))
                .collect::<Option<_>>()?;
        }
    }
    let first = |i: usize| &all[2 * i];
    let second_rev = |i: usize| &all[2 * i + 1];
    let seam = |i: usize| &all[2 * n + i];
    let prev = |i: usize| (i + n - 1) % n;
    let mut nets: Vec<Vec<Vec<H>>> = (0..n)
        .map(|i| coons_net(first(i), seam(i), seam(prev(i)), second_rev(prev(i))))
        .collect();
    let (nu, nv) = (nets[0].len(), nets[0][0].len());
    if nu < 4 || nv < 4 || nu != nv {
        return None;
    }
    // Smooth across each inner curve `i`: patch `i`'s column next to it
    // (u = 1) and patch `i + 1`'s row next to it (v = 1) made mirror images
    // about it. The rows at the ring and at the centre are left alone.
    let wanted: Vec<Vec<(usize, usize, usize, H)>> = (0..n)
        .map(|i| {
            let j = (i + 1) % n;
            let mut out = Vec::new();
            for k in 1..nv - 1 {
                let s = nets[i][nu - 1][k];
                let a = nets[i][nu - 2][k];
                let b = nets[j][k][nv - 2];
                let d = ((a.0 - b.0) * 0.5, (a.1 - b.1) * 0.5);
                out.push((i, nu - 2, k, (s.0 + d.0, s.1 + d.1)));
                out.push((j, k, nv - 2, (s.0 - d.0, s.1 - d.1)));
            }
            out
        })
        .collect();
    // A point next to two inner curves (beside the centre) takes the mean
    // of what each asks.
    let mut sum: BTreeMap<(usize, usize, usize), (DVec3, f64, f64)> = BTreeMap::new();
    for (p, i, j, x) in wanted.into_iter().flatten() {
        let e = sum.entry((p, i, j)).or_insert((DVec3::ZERO, 0.0, 0.0));
        e.0 += x.0;
        e.1 += x.1;
        e.2 += 1.0;
    }
    for ((p, i, j), (x, w, k)) in sum {
        nets[p][i][j] = (x / k, w / k);
    }
    nets.iter()
        .map(|net| surface_of(net, first(0), first(0)))
        .collect()
}

/// The surfaces a ring of sides fills with (one for four sides).
fn ring_surfaces(sides: &[Side], turned: &[bool]) -> Result<Vec<NurbsSurface>, &'static str> {
    if sides.len() < 3 {
        return Err(RING_OF_TWO);
    }
    let ring = round(sides, turned);
    if sides.len() == 4 {
        four(&ring).map(|s| vec![s]).ok_or(DEGENERATE)
    } else {
        split(&ring).ok_or(DEGENERATE)
    }
}

/// The surface through sections, in the order given, and the parameter
/// of each section across it.
fn loft_surface(sides: &[Side], preset: Preset) -> Result<(NurbsSurface, Vec<f64>), &'static str> {
    let mut curves: Vec<Nurbs> = sides.iter().map(|s| s.curve.clone()).collect();
    let closed = curves[0].is_closed();
    if curves.iter().any(|c| c.is_closed() != closed) {
        return Err(crate::loft::MIXED);
    }
    // Each turned to run the same way as the one before and, where they
    // are closed, started beside it.
    for j in 1..curves.len() {
        let prev = curves[j - 1].clone();
        let c = &mut curves[j];
        if closed {
            if newell(c.cage()).dot(newell(prev.cage())) < 0.0 {
                *c = c.reversed();
            }
            *c = crate::loft::beside(&prev, c);
        } else {
            let ends = |x: &Nurbs| {
                let (a, b) = x.domain();
                (x.eval(a), x.eval(b))
            };
            let ((a0, a1), (b0, b1)) = (ends(&prev), ends(c));
            if a0.distance(b0) + a1.distance(b1) > a0.distance(b1) + a1.distance(b0) {
                *c = c.reversed();
            }
        }
    }
    let alike = Nurbs::alike(&curves).ok_or(crate::loft::NOT_ALIKE)?;
    let mut params = vec![0.0];
    for w in alike.windows(2) {
        let d = middle(&w[0]).distance(middle(&w[1]));
        params.push(params[params.len() - 1] + d);
    }
    let total = params[params.len() - 1];
    if total <= 0.0 || params.windows(2).any(|w| w[1] <= w[0]) {
        return Err(DEGENERATE);
    }
    for p in &mut params {
        *p /= total;
    }
    let surface = if preset.straight || alike.len() == 2 {
        // Straight between sections: the control rows are the sections'.
        let c = &alike[0];
        let mut knots = vec![0.0];
        knots.extend(params.iter().copied());
        knots.push(1.0);
        let (nu, nv) = (c.points().len(), alike.len());
        let mut points = Vec::with_capacity(nu * nv);
        let mut weights = Vec::with_capacity(nu * nv);
        for i in 0..nu {
            for s in &alike {
                points.push(s.points()[i]);
                weights.push(s.weights()[i]);
            }
        }
        NurbsSurface::new(
            [c.degree(), 1],
            [c.knots().to_vec(), knots],
            [nu, nv],
            points,
            weights,
            [c.is_periodic(), false],
        )
        .ok_or(DEGENERATE)?
    } else {
        prism_kernel::kernel().skin(&alike, &params)?
    };
    Ok((surface, params))
}

/// The surfaces the sides make, for a preview: the patches of a ring, or
/// the loft through sections.
pub fn surfaces(
    model: &Model,
    sides: &[Side],
    preset: Preset,
) -> Result<Vec<NurbsSurface>, &'static str> {
    match shape(sides, model.tol().dist * 10.0) {
        Shape::Ring { turned } => ring_surfaces(sides, &turned),
        Shape::Sections => Ok(vec![loft_surface(sides, preset)?.0]),
        Shape::Few => Err(TOO_FEW),
        Shape::Open { .. } | Shape::Mixed => Err(OPEN),
    }
}

/// Values from 0 to 1 cutting it into `rows` equal parts, added to `cuts`.
fn with_rows(mut cuts: Vec<f64>, lo: f64, hi: f64, rows: usize) -> Vec<f64> {
    for k in 1..rows.clamp(1, MAX_ROWS) {
        let t = lo + (hi - lo) * k as f64 / rows as f64;
        if !cuts.iter().any(|c| (c - t).abs() <= (hi - lo) * 1e-6) {
            cuts.push(t);
        }
    }
    cuts.sort_by(f64::total_cmp);
    cuts
}

/// Where the vertices of `edges` fall on `c` (its ends included).
fn cuts_on(model: &Model, c: &Nurbs, edges: &[EdgeId], tol: f64) -> Vec<f64> {
    let (lo, hi) = c.domain();
    let mut out = vec![lo, hi];
    for e in edges {
        let Some(x) = model.edge(*e) else { continue };
        for p in [model.vertex(x.a), model.vertex(x.b)] {
            let t = c.closest(p, lo, hi);
            if c.eval(t).distance(p) <= tol
                && !out.iter().any(|k| (k - t).abs() <= (hi - lo) * 1e-6)
            {
                out.push(t);
            }
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

/// Whether some face not in `new` walks `a` to `b` (so a new face beside
/// it should walk `b` to `a`).
fn walked_by_others(model: &Model, new: &BTreeSet<FaceId>, a: VertexId, b: VertexId) -> bool {
    model
        .faces()
        .any(|(id, f)| !new.contains(&id) && f.sides().any(|s| s.from == a && s.to == b))
}

/// The model with the sides' surface made, and its faces: a ring filled,
/// or a loft through sections. Edges inside the new surface are soft,
/// except where they lie on a picked side. Faces turn to match a face
/// already beside them.
pub fn made(
    model: &Model,
    sides: &[Side],
    preset: Preset,
) -> Result<(Model, Vec<FaceId>), &'static str> {
    let tol10 = model.tol().dist * 10.0;
    let picked: BTreeSet<EdgeId> = sides.iter().flat_map(|s| s.edges.clone()).collect();
    let mut w = model.clone();
    let mut faces = Vec::new();
    match shape(sides, tol10) {
        Shape::Ring { turned } => {
            let surfs = ring_surfaces(sides, &turned)?;
            let n = sides.len();
            let ring = round(sides, &turned);
            // Edges of each side, as fractions of the side round the ring.
            let side_cuts: Vec<Vec<f64>> = (0..n)
                .map(|i| cuts_on(model, &ring[i], &sides[i].edges, tol10))
                .collect();
            let lay_one = |w: &mut Model, s: NurbsSurface, us: Vec<f64>, vs: Vec<f64>, flip| {
                crate::loft::lay_at(w, s, us, &vs, flip)
            };
            // Which way round: against a face already on the first side.
            let corner = crate::sticky::vertex_at;
            let flip = {
                let c0 = ring[0].eval(0.0);
                let c1 = ring[0].eval(side_cuts[0].get(1).copied().unwrap_or(1.0));
                match (corner(model, c0), corner(model, c1)) {
                    (Some(a), Some(b)) => walked_by_others(model, &BTreeSet::new(), a, b),
                    _ => false,
                }
            };
            if n == 4 {
                // The top runs backwards round the ring.
                let mut us = side_cuts[0].clone();
                for t in &side_cuts[2] {
                    let t = 1.0 - t;
                    if !us.iter().any(|k| (k - t).abs() <= 1e-6) {
                        us.push(t);
                    }
                }
                us.sort_by(f64::total_cmp);
                let mut vs: Vec<f64> = side_cuts[1].clone();
                for t in &side_cuts[3] {
                    let t = 1.0 - t;
                    if !vs.iter().any(|k| (k - t).abs() <= 1e-6) {
                        vs.push(t);
                    }
                }
                vs.sort_by(f64::total_cmp);
                let vs = with_rows(vs, 0.0, 1.0, preset.rows);
                let s = surfs.into_iter().next().ok_or(DEGENERATE)?;
                let s = reparam(s);
                faces.extend(lay_one(&mut w, s, us, vs, flip)?);
            } else {
                // Patch i: u along the first half of side i, v back along
                // the second half of side i - 1. A cut on one half runs
                // across to an inner curve, which the next patch shares,
                // so both halves' cuts go into both directions.
                let half = |cuts: &[f64], second: bool| -> Vec<f64> {
                    cuts.iter()
                        .filter_map(|t| {
                            let x = if second { (1.0 - t) * 2.0 } else { t * 2.0 };
                            (x > 1e-6 && x < 1.0 - 1e-6).then_some(x)
                        })
                        .collect()
                };
                let own_u = |i: usize| half(&side_cuts[i], false);
                let own_v = |i: usize| half(&side_cuts[(i + n - 1) % n], true);
                let both = |a: Vec<f64>, b: Vec<f64>| {
                    let mut c = vec![0.0, 1.0];
                    c.extend(a);
                    c.extend(b);
                    c.sort_by(f64::total_cmp);
                    c.dedup_by(|a, b| (*a - *b).abs() <= 1e-6);
                    with_rows(c, 0.0, 1.0, preset.rows)
                };
                for (i, s) in surfs.into_iter().enumerate() {
                    // The top is the inner curve the patch before shares
                    // as its right; the right is the next patch's top.
                    let us = both(own_u(i), own_v((i + n - 1) % n));
                    let vs = both(own_v(i), own_u((i + 1) % n));
                    faces.extend(lay_one(&mut w, reparam(s), us, vs, flip)?);
                }
            }
        }
        Shape::Sections => {
            let (surface, params) = loft_surface(sides, preset)?;
            let mut vs = Vec::new();
            for k in 0..params.len() {
                vs.push(params[k]);
                if k + 1 < params.len() {
                    let extra = with_rows(Vec::new(), params[k], params[k + 1], preset.rows);
                    vs.extend(extra);
                }
            }
            let closed = sides[0].curve.is_closed();
            let flipped = closed && {
                let [(a, _), (b, _)] = surface.domain();
                let p = surface.eval(a, b);
                surface.normal(a, b).dot(p - middle(&sides[0].curve)) < 0.0
            };
            let rows: Vec<(f64, Vec<EdgeId>)> = vs
                .iter()
                .map(|v| {
                    let on = params
                        .iter()
                        .position(|q| (q - v).abs() <= 1e-12)
                        .map(|k| sides[k].edges.clone())
                        .unwrap_or_default();
                    (*v, on)
                })
                .collect();
            faces.extend(crate::loft::lay(&mut w, surface, &rows, flipped)?);
        }
        Shape::Few => return Err(TOO_FEW),
        Shape::Open { .. } | Shape::Mixed => return Err(OPEN),
    }
    // Soft inside: an edge between two new faces that is not a picked side.
    let new: BTreeSet<FaceId> = faces.iter().copied().collect();
    let on_picked = |w: &Model, e: EdgeId| {
        picked.contains(&e)
            || w.edge(e).is_some_and(|x| {
                let (a, b) = (w.vertex(x.a), w.vertex(x.b));
                let mid = x.curve.point(a, b, 0.5);
                sides.iter().any(|s| {
                    let (lo, hi) = s.curve.domain();
                    s.curve.eval(s.curve.closest(mid, lo, hi)).distance(mid) <= tol10
                })
            })
    };
    let inner: Vec<EdgeId> = w
        .edge_faces()
        .into_iter()
        .filter(|(_, fs)| fs.len() == 2 && fs.iter().all(|f| new.contains(f)))
        .map(|(e, _)| e)
        .collect();
    for e in inner {
        if !on_picked(&w, e) {
            w.set_soft(e, true, true);
        }
    }
    Ok((w, faces))
}

/// The model with the surface the sides (each a list of edges, in the
/// order picked) make ([`made`]), or why not.
pub fn made_from(
    model: &Model,
    sides: &[Vec<EdgeId>],
    preset: Preset,
) -> Result<(Model, Vec<FaceId>), &'static str> {
    let list: Vec<Side> = sides
        .iter()
        .map(|e| side(model, e))
        .collect::<Result<_, _>>()?;
    made(model, &list, preset)
}

/// Make the surface the sides make ([`made_from`]). The model is left
/// alone when refused.
pub fn make(
    model: &mut Model,
    sides: &[Vec<EdgeId>],
    preset: Preset,
) -> Result<Vec<FaceId>, &'static str> {
    let (w, faces) = made_from(model, sides, preset)?;
    *model = w;
    Ok(faces)
}

/// The same surface over 0 to 1 both ways.
fn reparam(s: NurbsSurface) -> NurbsSurface {
    let [(u0, u1), (v0, v1)] = s.domain();
    let [ku, kv] = s.knots();
    let ku: Vec<f64> = ku.iter().map(|k| (k - u0) / (u1 - u0)).collect();
    let kv: Vec<f64> = kv.iter().map(|k| (k - v0) / (v1 - v0)).collect();
    NurbsSurface::new(
        s.degree(),
        [ku, kv],
        s.count(),
        s.points().to_vec(),
        s.weights().to_vec(),
        s.periodic(),
    )
    .unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sticky::{add_arc, add_segment};
    use std::f64::consts::FRAC_PI_2;

    /// Three quarter circles round the corner of a unit sphere.
    fn octant() -> (Model, Vec<Side>) {
        let mut m = Model::new();
        let mut sides = Vec::new();
        for (axis, start) in [
            (DVec3::X, DVec3::Y),
            (DVec3::Y, DVec3::Z),
            (DVec3::Z, DVec3::X),
        ] {
            let added = add_arc(&mut m, DVec3::ZERO, axis, 1.0, start, FRAC_PI_2);
            sides.push(added.edges);
        }
        let sides = sides.iter().map(|e| side(&m, e).unwrap()).collect();
        (m, sides)
    }

    /// Every edge of `faces` that only one of them uses lies on `on`;
    /// across every edge two of them share, their normals agree within
    /// `max_deg`.
    fn exact_and_smooth(m: &Model, faces: &[FaceId], on: impl Fn(DVec3) -> f64, max_deg: f64) {
        let ef = m.edge_faces();
        let new: BTreeSet<FaceId> = faces.iter().copied().collect();
        let mut seams = 0;
        for (e, fs) in ef {
            let fs: Vec<FaceId> = fs.into_iter().filter(|f| new.contains(f)).collect();
            let x = m.edge(e).unwrap();
            let (a, b) = (m.vertex(x.a), m.vertex(x.b));
            let pts: Vec<DVec3> = (0..=10)
                .map(|k| x.curve.point(a, b, k as f64 / 10.0))
                .collect();
            match fs.as_slice() {
                [_] => {
                    for p in &pts {
                        assert!(on(*p).abs() < 1e-9, "a side keeps its shape: {}", on(*p));
                    }
                }
                [f, g] => {
                    seams += 1;
                    assert!(x.soft, "an inner edge is soft");
                    let s1 = m.face(*f).unwrap().surface.clone();
                    let s2 = m.face(*g).unwrap().surface.clone();
                    for p in &pts {
                        let (n1, n2) = (s1.normal_at(*p), s2.normal_at(*p));
                        let deg = n1.angle_between(n2).to_degrees();
                        assert!(deg < max_deg, "a crease of {deg:.2} degrees at {p}");
                    }
                }
                _ => {}
            }
        }
        assert!(seams > 0);
    }

    #[test]
    fn three_arcs_round_a_corner_fill_with_three_faces_meeting_in_the_middle() {
        let (m, sides) = octant();
        assert!(matches!(shape(&sides, 1e-6), Shape::Ring { .. }));
        let (w, faces) = made(&m, &sides, Preset::default()).unwrap();
        assert_eq!(faces.len(), 3);
        assert!(
            crate::check::problems(&w).is_empty(),
            "{:?}",
            crate::check::problems(&w)
        );
        exact_and_smooth(&w, &faces, |p| p.length() - 1.0, 2.0);
        // Close to the sphere inside, too.
        for f in &faces {
            let face = w.face(*f).unwrap();
            let patch = face.surface.patch().unwrap();
            let [(u0, u1), (v0, v1)] = patch.range();
            let p = patch.surface.eval((u0 + u1) / 2.0, (v0 + v1) / 2.0);
            assert!((p.length() - 1.0).abs() < 0.02, "{}", p.length());
        }
        // The three faces share one centre vertex.
        let shared = w
            .vertices()
            .filter(|(v, _)| faces.iter().all(|f| w.face(*f).unwrap().uses_vertex(*v)))
            .count();
        assert_eq!(shared, 1);
    }

    #[test]
    fn a_ring_of_four_stays_one_patch_and_five_fill_smoothly() {
        let mut m = Model::new();
        let sq = [
            DVec3::ZERO,
            DVec3::new(2.0, 0.0, 0.5),
            DVec3::new(2.0, 2.0, 0.0),
            DVec3::new(0.0, 2.0, 0.5),
        ];
        let mut sides = Vec::new();
        for i in 0..4 {
            let e = add_segment(&mut m, sq[i], sq[(i + 1) % 4]).edges;
            sides.push(side(&m, &e).unwrap());
        }
        let (w, faces) = made(&m, &sides, Preset::default()).unwrap();
        assert_eq!(faces.len(), 1);
        assert!(crate::check::problems(&w).is_empty());

        // Five arcs round a cap of a sphere.
        let mut m = Model::new();
        let corners: Vec<DVec3> = (0..5)
            .map(|k| {
                let a = k as f64 * std::f64::consts::TAU / 5.0;
                DVec3::new(a.cos() * 0.8, a.sin() * 0.8, 0.6)
            })
            .collect();
        let mut sides = Vec::new();
        for i in 0..5 {
            let (a, b) = (corners[i], corners[(i + 1) % 5]);
            // The great circle through a and b.
            let axis = a.cross(b).normalize();
            let added = add_arc(&mut m, DVec3::ZERO, axis, 1.0, a, a.angle_between(b));
            sides.push(side(&m, &added.edges).unwrap());
        }
        let (w, faces) = made(&m, &sides, Preset::default()).unwrap();
        assert_eq!(faces.len(), 5);
        assert!(
            crate::check::problems(&w).is_empty(),
            "{:?}",
            crate::check::problems(&w)
        );
        exact_and_smooth(&w, &faces, |p| p.length() - 1.0, 2.0);
    }

    #[test]
    fn separate_curves_loft_in_the_order_picked_smooth_or_straight() {
        let mut m = Model::new();
        let mut sides = Vec::new();
        for (y, z) in [(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)] {
            let e = add_segment(&mut m, DVec3::new(0.0, y, z), DVec3::new(3.0, y, z)).edges;
            sides.push(side(&m, &e).unwrap());
        }
        assert_eq!(shape(&sides, 1e-6), Shape::Sections);
        let smooth = surfaces(&m, &sides, Preset::default()).unwrap();
        let straight = surfaces(
            &m,
            &sides,
            Preset {
                straight: true,
                rows: 1,
            },
        )
        .unwrap();
        // Halfway between the first two sections: the smooth one bulges
        // above the straight line.
        let at = |s: &NurbsSurface| {
            let v = s.closest(DVec3::new(1.5, 0.5, 2.0), s.domain());
            s.eval(v.0, v.1)
        };
        assert!(at(&smooth[0]).z > at(&straight[0]).z + 0.05);
        assert!((straight[0].degree()[1]) == 1);
        let (w, faces) = made(
            &m,
            &sides,
            Preset {
                straight: false,
                rows: 3,
            },
        )
        .unwrap();
        assert_eq!(faces.len(), 6, "two gaps, three rows each");
        assert!(crate::check::problems(&w).is_empty());
        // The middle section stays a hard edge; the added rows are soft.
        let hard_inside = w
            .edges()
            .filter(|(_, e)| {
                !e.soft
                    && [e.a, e.b]
                        .iter()
                        .all(|v| (w.vertex(*v).y - 1.0).abs() < 1e-9)
            })
            .count();
        assert_eq!(hard_inside, 1);
    }

    #[test]
    fn circles_started_at_different_places_loft_without_a_twist() {
        let mut m = Model::new();
        let tau = std::f64::consts::TAU;
        let a = crate::add_arc(&mut m, DVec3::ZERO, DVec3::Z, 1.0, DVec3::X, tau).edges;
        let b = crate::add_arc(
            &mut m,
            DVec3::new(0.0, 0.0, 2.0),
            DVec3::Z,
            1.0,
            DVec3::new(0.0, 1.0, 2.0),
            tau,
        )
        .edges;
        let sides = vec![side(&m, &a).unwrap(), side(&m, &b).unwrap()];
        assert_eq!(shape(&sides, 1e-6), Shape::Sections);
        let s = &surfaces(&m, &sides, Preset::default()).unwrap()[0];
        let [(u0, u1), (v0, v1)] = s.domain();
        for k in 0..8 {
            let u = u0 + (u1 - u0) * k as f64 / 8.0;
            let (p, q) = (s.eval(u, v0), s.eval(u, v1));
            assert!(
                p.truncate().distance(q.truncate()) < 1e-9,
                "at {k}: {p} below {q}"
            );
        }
        let (w, faces) = made(&m, &sides, Preset::default()).unwrap();
        assert!(!faces.is_empty());
        assert!(crate::check::problems(&w).is_empty());
    }

    #[test]
    fn a_chain_runs_on_through_gentle_turns_and_stops_at_sharp_ones() {
        let mut m = Model::new();
        let pts = [
            DVec3::ZERO,
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(2.0, 0.2, 0.0),
            DVec3::new(2.0, 1.2, 0.0),
        ];
        let mut ids = Vec::new();
        for w in pts.windows(2) {
            ids.extend(add_segment(&mut m, w[0], w[1]).edges);
        }
        let c = chain(&m, ids[0], TURN_DEG.to_radians());
        assert_eq!(c, vec![ids[0], ids[1]], "the 79 degree turn ends it");
        let all = chain(&m, ids[1], 85f64.to_radians());
        assert_eq!(all.len(), 3);
        let s = side(&m, &all).unwrap();
        let (a, b) = s.ends();
        assert!(a.distance(pts[0]) < 1e-12 || a.distance(pts[3]) < 1e-12);
        assert!(a.distance(b) > 1.0);
    }

    #[test]
    fn an_open_ring_says_where_it_is_open() {
        let (m, sides) = octant();
        let two = &sides[..2];
        let Shape::Open { ends } = shape(two, 1e-6) else {
            panic!("{:?}", shape(two, 1e-6));
        };
        let gap: BTreeSet<[i64; 3]> = ends
            .iter()
            .map(|p| (*p * 1e6).round().as_i64vec3().to_array())
            .collect();
        let want: BTreeSet<[i64; 3]> = [DVec3::X, DVec3::Y]
            .iter()
            .map(|p| (*p * 1e6).round().as_i64vec3().to_array())
            .collect();
        assert_eq!(gap, want);
        assert_eq!(made(&m, two, Preset::default()).unwrap_err(), OPEN);
    }
}
