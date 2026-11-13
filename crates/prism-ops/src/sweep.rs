//! Sweep and Patch (Freeform NURBS part 5).
//!
//! - **Sweep** of two open curves: the profile (the curve the path starts
//!   on) is carried along the path, turning with it as a pipe's circle
//!   does. Along a straight path that is a slide, and exact.
//! - **Sweep** of a closed curve along an open one (a pipe): the closed
//!   curve is carried along the path, turning with it and never twisting
//!   about it (rotation-minimising frames), and the kernel skins a surface
//!   through the copies. It passes through every copy; between them it is
//!   fitted, not exact.
//! - **Patch**: the surface inside a ring of four curves (a Coons patch),
//!   exact along all four.

use std::collections::BTreeSet;

use prism_geom::{Nurbs, NurbsSurface};
use prism_math::{DAffine3, DMat3, DVec3};
use prism_topo::{EdgeId, FaceId, Model};

use crate::loft::{DEGENERATE, Section, lay, middle, sections};

pub const TWO_FOR_SWEEP: &str =
    "Select two curves to sweep: a profile and a path (lines and arcs count as curves).";
pub const BOTH_CLOSED: &str =
    "Refused: two closed curves cannot be swept. One must be open (the path).";
pub const FOUR_FOR_PATCH: &str = "Select four curves joined end to end in a ring to patch (three-sided patches are not built yet).";

/// The values of a curve's parameter where the edges on it have vertices,
/// with its two ends, in order.
fn cuts(model: &Model, c: &Nurbs, edges: &[EdgeId]) -> Vec<f64> {
    let (lo, hi) = c.domain();
    let tol = model.tol().dist * 10.0;
    let mut out = vec![lo, hi];
    for e in edges {
        let Some(x) = model.edge(*e) else { continue };
        for p in [model.vertex(x.a), model.vertex(x.b)] {
            let t = c.closest(p, lo, hi);
            if c.eval(t).distance(p) <= tol
                && !out.iter().any(|k| (k - t).abs() <= (hi - lo) * 1e-9)
            {
                out.push(t);
            }
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

fn ends(c: &Nurbs) -> (DVec3, DVec3) {
    let (a, b) = c.domain();
    (c.eval(a), c.eval(b))
}

/// `a` slid along `b` (from `b`'s start), keeping its direction: u along
/// `a`, v along `b`.
fn slid(a: &Nurbs, b: &Nurbs) -> Option<NurbsSurface> {
    let from = ends(b).0;
    let (na, nb) = (a.points().len(), b.points().len());
    let mut points = Vec::with_capacity(na * nb);
    let mut weights = Vec::with_capacity(na * nb);
    for i in 0..na {
        for j in 0..nb {
            points.push(a.points()[i] + b.points()[j] - from);
            weights.push(a.weights()[i] * b.weights()[j]);
        }
    }
    NurbsSurface::new(
        [a.degree(), b.degree()],
        [a.knots().to_vec(), b.knots().to_vec()],
        [na, nb],
        points,
        weights,
        [a.is_periodic(), b.is_periodic()],
    )
}

/// Copies of `profile` carried along `path`, turning with it and not
/// twisting about it; the first is the profile itself.
fn carried(profile: &Nurbs, path: &Nurbs) -> Option<(Vec<Nurbs>, Vec<f64>)> {
    let (lo, hi) = path.domain();
    let n = (path.steps(lo, hi) / 4).clamp(4, 48);
    let at = |k: usize| lo + (hi - lo) * k as f64 / n as f64;
    let frame = |k: usize| {
        let (x, d) = path.eval_d(at(k));
        Some((x, d.try_normalize()?))
    };
    let (x0, t0) = frame(0)?;
    // Any direction square to the path's start.
    let seed = if t0.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let r0 = (seed - t0 * seed.dot(t0)).normalize();
    let base = DMat3::from_cols(t0, r0, t0.cross(r0));
    let (mut x, mut t, mut r) = (x0, t0, r0);
    let mut copies = vec![profile.clone()];
    let mut params = vec![0.0];
    let mut length = 0.0;
    for k in 1..=n {
        let (x1, t1) = frame(k)?;
        // The double reflection step (Wang, Jüttler, Zheng and Liu).
        let v1 = x1 - x;
        let c1 = v1.length_squared();
        if c1 <= 0.0 {
            return None;
        }
        let rl = r - v1 * (2.0 / c1 * v1.dot(r));
        let tl = t - v1 * (2.0 / c1 * v1.dot(t));
        let v2 = t1 - tl;
        let c2 = v2.length_squared();
        let r1 = if c2 > 0.0 {
            rl - v2 * (2.0 / c2 * v2.dot(rl))
        } else {
            rl
        };
        let r1 = (r1 - t1 * r1.dot(t1)).normalize();
        let turn = DMat3::from_cols(t1, r1, t1.cross(r1)) * base.transpose();
        let m = DAffine3::from_translation(x1)
            * DAffine3::from_mat3(turn)
            * DAffine3::from_translation(-x0);
        copies.push(profile.transformed(&m));
        length += v1.length();
        params.push(length);
        (x, t, r) = (x1, t1, r1);
    }
    for p in &mut params {
        *p /= length;
    }
    Some((copies, params))
}

/// Of two open curves, which is the profile and which the path (turned
/// to start at the profile): the path starts on the profile (its nearer
/// end is the closer to it). When both touch the other (they meet end to
/// end), the profile is the flat curve whose plane the other leaves most
/// squarely (a line has no plane of its own), then the shorter.
fn profile_and_path<'a>(a: &'a Section, b: &'a Section) -> (&'a Section, Nurbs) {
    let gap = |profile: &Nurbs, path: &Nurbs| {
        let (lo, hi) = profile.domain();
        let (p0, p1) = ends(path);
        let d0 = profile.eval(profile.closest(p0, lo, hi)).distance(p0);
        let d1 = profile.eval(profile.closest(p1, lo, hi)).distance(p1);
        (d0.min(d1), d1 < d0)
    };
    let size = |c: &Nurbs| {
        c.points()
            .windows(2)
            .map(|w| w[0].distance(w[1]))
            .sum::<f64>()
    };
    let (ga, flip_b) = gap(&a.curve, &b.curve);
    let (gb, flip_a) = gap(&b.curve, &a.curve);
    let scale = size(&a.curve).max(size(&b.curve)).max(1e-300);
    // How squarely `other` leaves `profile`'s plane where it starts.
    let square = |profile: &Nurbs, other: &Nurbs| {
        let Some(plane) = profile.plane(scale * 1e-9) else {
            return 0.0;
        };
        if straight(profile) {
            return 0.0;
        }
        let (lo, hi) = other.domain();
        let (p0, p1) = ends(other);
        let (plo, phi) = profile.domain();
        let near = |p: DVec3| profile.eval(profile.closest(p, plo, phi)).distance(p);
        let t = if near(p1) < near(p0) { hi } else { lo };
        other
            .eval_d(t)
            .1
            .normalize_or_zero()
            .dot(plane.normal)
            .abs()
    };
    let a_is_profile = if (ga - gb).abs() <= scale * 1e-9 {
        let (sa, sb) = (square(&a.curve, &b.curve), square(&b.curve, &a.curve));
        if (sa - sb).abs() > 1e-9 {
            sa > sb
        } else {
            size(&a.curve) <= size(&b.curve)
        }
    } else {
        ga < gb
    };
    let turned = |c: &Nurbs, flip: bool| if flip { c.reversed() } else { c.clone() };
    if a_is_profile {
        (a, turned(&b.curve, flip_b))
    } else {
        (b, turned(&a.curve, flip_a))
    }
}

/// Whether a curve is a straight line (all its control points on one).
fn straight(c: &Nurbs) -> bool {
    let pts = c.points();
    let (p0, p1) = (pts[0], pts[pts.len() - 1]);
    let d = p1 - p0;
    let len = d.length();
    len > 0.0
        && pts
            .iter()
            .all(|p| (*p - p0).cross(d).length() <= len * len * 1e-12)
}

/// The model with the two selected curves swept, and the faces made; or
/// why not.
pub fn swept(
    model: &Model,
    edges: &BTreeSet<EdgeId>,
) -> Result<(Model, Vec<FaceId>), &'static str> {
    let list = sections(model, edges);
    let [a, b] = list.as_slice() else {
        return Err(TWO_FOR_SWEEP);
    };
    let mut w = model.clone();
    let faces = match (a.curve.is_closed(), b.curve.is_closed()) {
        (true, true) => return Err(BOTH_CLOSED),
        (false, false) => {
            let (profile, path) = profile_and_path(a, b);
            if straight(&path) {
                // Along a straight path, sliding is turning: exact.
                let surface = slid(&profile.curve, &path).ok_or(DEGENERATE)?;
                let path_edges = if std::ptr::eq(profile, a) {
                    &b.edges
                } else {
                    &a.edges
                };
                let mut rows: Vec<(f64, Vec<EdgeId>)> = cuts(model, &path, path_edges)
                    .into_iter()
                    .map(|v| (v, Vec::new()))
                    .collect();
                rows[0].1 = profile.edges.clone();
                lay(&mut w, surface, &rows, false)?
            } else {
                // Along a curved path the profile turns with it, as a
                // pipe's circle does.
                let (copies, params) = carried(&profile.curve, &path).ok_or(DEGENERATE)?;
                let surface = prism_kernel::kernel().skin(&copies, &params)?;
                let rows = vec![(0.0, profile.edges.clone()), (1.0, Vec::new())];
                lay(&mut w, surface, &rows, false)?
            }
        }
        (pa, _) => {
            let (profile, path): (&Section, &Section) = if pa { (a, b) } else { (b, a) };
            let (copies, params) = carried(&profile.curve, &path.curve).ok_or(DEGENERATE)?;
            let surface = prism_kernel::kernel().skin(&copies, &params)?;
            // A pipe faces away from its middle.
            let [(u0, _), (v0, _)] = surface.domain();
            let p = surface.eval(u0, v0);
            let flipped = surface.normal(u0, v0).dot(p - middle(&profile.curve)) < 0.0;
            let rows = vec![(0.0, profile.edges.clone()), (1.0, Vec::new())];
            lay(&mut w, surface, &rows, flipped)?
        }
    };
    Ok((w, faces))
}

/// Sweep the two selected curves ([`swept`]). The model is left alone when
/// refused.
pub fn sweep(model: &mut Model, edges: &BTreeSet<EdgeId>) -> Result<Vec<FaceId>, &'static str> {
    let (w, faces) = swept(model, edges)?;
    *model = w;
    Ok(faces)
}

/// The mean of each run of `degree` knots: where a control point's
/// influence is centred.
fn greville(c: &Nurbs) -> Vec<f64> {
    let p = c.degree();
    (0..c.points().len())
        .map(|i| c.knots()[i + 1..=i + p].iter().sum::<f64>() / p as f64)
        .collect()
}

/// The surface inside four curves (a bilinearly blended Coons patch):
/// `bottom` and `top` run the same way along u, `left` and `right` the
/// same way along v, corners shared.
fn coons(bottom: &Nurbs, right: &Nurbs, top: &Nurbs, left: &Nurbs) -> Option<NurbsSurface> {
    let bt = Nurbs::alike(&[bottom.clone(), top.clone()])?;
    let lr = Nurbs::alike(&[left.clone(), right.clone()])?;
    let (b, t, l, r) = (&bt[0], &bt[1], &lr[0], &lr[1]);
    let (gu, gv) = (greville(b), greville(l));
    let (nu, nv) = (gu.len(), gv.len());
    // Blended in homogeneous coordinates (each point times its weight), so
    // arcs and other weighted curves are kept exactly along all four sides.
    let h = |c: &Nurbs, i: usize| {
        let w = c.weights()[i];
        (c.points()[i] * w, w)
    };
    let lerp = |a: (DVec3, f64), b: (DVec3, f64), t: f64| (a.0.lerp(b.0, t), a.1 + (b.1 - a.1) * t);
    let (p00, p10) = (h(b, 0), h(b, nu - 1));
    let (p01, p11) = (h(t, 0), h(t, nu - 1));
    let mut points = Vec::with_capacity(nu * nv);
    let mut weights = Vec::with_capacity(nu * nv);
    for (i, x) in gu.iter().enumerate() {
        for (j, y) in gv.iter().enumerate() {
            let across = lerp(h(b, i), h(t, i), *y);
            let along = lerp(h(l, j), h(r, j), *x);
            let flat = lerp(lerp(p00, p10, *x), lerp(p01, p11, *x), *y);
            let w = across.1 + along.1 - flat.1;
            if w.is_nan() || w <= 1e-9 {
                return None;
            }
            points.push((across.0 + along.0 - flat.0) / w);
            weights.push(w);
        }
    }
    NurbsSurface::new(
        [b.degree(), l.degree()],
        [b.knots().to_vec(), l.knots().to_vec()],
        [nu, nv],
        points,
        weights,
        [false, false],
    )
}

/// The model with the ring of four selected curves patched, and the faces
/// made; or why not.
pub fn patched(
    model: &Model,
    edges: &BTreeSet<EdgeId>,
) -> Result<(Model, Vec<FaceId>), &'static str> {
    let mut list = sections(model, edges);
    if list.len() != 4 || list.iter().any(|s| s.curve.is_closed()) {
        return Err(FOUR_FOR_PATCH);
    }
    let tol = model.tol().dist * 10.0;
    // Round the ring: each next curve starts where the last one ended.
    let mut ring = vec![list.remove(0)];
    while !list.is_empty() {
        let end = ends(&ring[ring.len() - 1].curve).1;
        let k = list
            .iter()
            .position(|s| {
                let (a, b) = ends(&s.curve);
                a.distance(end) <= tol || b.distance(end) <= tol
            })
            .ok_or(FOUR_FOR_PATCH)?;
        let mut next = list.remove(k);
        if ends(&next.curve).0.distance(end) > tol {
            next.curve = next.curve.reversed();
        }
        ring.push(next);
    }
    if ends(&ring[3].curve).1.distance(ends(&ring[0].curve).0) > tol {
        return Err(FOUR_FOR_PATCH);
    }
    let (bottom, right) = (ring[0].curve.clone(), ring[1].curve.clone());
    let (top, left) = (ring[2].curve.reversed(), ring[3].curve.reversed());
    let surface = coons(&bottom, &right, &top, &left).ok_or(DEGENERATE)?;
    // Cut across where the left and right curves have vertices.
    let [_, (v0, v1)] = surface.domain();
    let side = |c: &Nurbs, e: &[EdgeId]| -> Vec<f64> {
        let n = c.normalized();
        cuts(model, &n, e)
            .into_iter()
            .map(|t| v0 + (v1 - v0) * t)
            .collect()
    };
    let mut vs = side(&left, &ring[3].edges);
    for v in side(&right, &ring[1].edges) {
        if !vs.iter().any(|k| (k - v).abs() <= (v1 - v0) * 1e-9) {
            vs.push(v);
        }
    }
    vs.sort_by(f64::total_cmp);
    let last = vs.len() - 1;
    let mut rows: Vec<(f64, Vec<EdgeId>)> = vs.into_iter().map(|v| (v, Vec::new())).collect();
    rows[0].1 = ring[0].edges.clone();
    rows[last].1 = ring[2].edges.clone();
    let mut w = model.clone();
    let faces = lay(&mut w, surface, &rows, false)?;
    Ok((w, faces))
}

/// Patch the ring of four selected curves ([`patched`]). The model is left
/// alone when refused.
pub fn patch(model: &mut Model, edges: &BTreeSet<EdgeId>) -> Result<Vec<FaceId>, &'static str> {
    let (w, faces) = patched(model, edges)?;
    *model = w;
    Ok(faces)
}
