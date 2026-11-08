//! Offset of a planar face: every loop (the outline and its holes) moves the
//! same distance into the face (positive) or out of it (negative), with
//! mitred corners.
//!
//! The offset never makes an invalid outline: if the distance asked for
//! would make an edge vanish or turn round, or two edges cross or touch,
//! it stops just short of that.

use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3};
use prism_topo::{FaceId, Model};

use crate::add_segment;

/// The 2D frame of a face: the outline winds counter-clockwise in it.
struct Flat {
    origin: DVec3,
    u: DVec3,
    v: DVec3,
}

impl Flat {
    fn new(origin: DVec3, n: DVec3) -> Self {
        let helper = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
        let u = n.cross(helper).normalize();
        let v = n.cross(u);
        Self { origin, u, v }
    }

    fn to_2d(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        DVec2::new(d.dot(self.u), d.dot(self.v))
    }

    fn to_3d(&self, p: DVec2) -> DVec3 {
        self.origin + self.u * p.x + self.v * p.y
    }
}

/// The face's loops in 2D (outline first) and its frame. Every loop has
/// the face on its left.
fn flat_loops(model: &Model, face: FaceId) -> Option<(Flat, Vec<Vec<DVec2>>)> {
    let f = model.face(face)?;
    let n = f.plane().normal;
    let origin = model.vertex(*f.outer.first()?);
    let flat = Flat::new(origin, n);
    let loops = f
        .loops()
        .map(|l| l.iter().map(|v| flat.to_2d(model.vertex(*v))).collect())
        .collect();
    Some((flat, loops))
}

/// One loop moved `d` to its left, with mitred corners. Points where the
/// loop doubles straight back have no mitre: None.
fn offset_loop(l: &[DVec2], d: f64) -> Option<Vec<DVec2>> {
    let n = l.len();
    let left = |a: DVec2, b: DVec2| {
        let t = (b - a).normalize();
        DVec2::new(-t.y, t.x)
    };
    (0..n)
        .map(|i| {
            let (p0, p, p1) = (l[(i + n - 1) % n], l[i], l[(i + 1) % n]);
            let (n0, n1) = (left(p0, p), left(p, p1));
            let k = 1.0 + n0.dot(n1);
            (k > 1e-9).then(|| p + (n0 + n1) * (d / k))
        })
        .collect()
}

/// The distance between segments `a`–`b` and `c`–`d` (0 if they cross).
fn segment_gap(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> f64 {
    let cross = |o: DVec2, p: DVec2, q: DVec2| (p - o).perp_dot(q - o);
    let (d1, d2) = (cross(a, b, c), cross(a, b, d));
    let (d3, d4) = (cross(c, d, a), cross(c, d, b));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return 0.0;
    }
    let to_seg = |p: DVec2, a: DVec2, b: DVec2| {
        let e = b - a;
        let t = ((p - a).dot(e) / e.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
        p.distance(a + e * t)
    };
    to_seg(a, c, d)
        .min(to_seg(b, c, d))
        .min(to_seg(c, a, b))
        .min(to_seg(d, a, b))
}

/// Whether offset loops are a valid result: no edge vanished or turned
/// round, and no two edges that are not neighbours come within `gap`.
fn valid(before: &[Vec<DVec2>], after: &[Vec<DVec2>], gap: f64) -> bool {
    let mut segs: Vec<(usize, usize, DVec2, DVec2)> = Vec::new();
    for (li, (l0, l1)) in before.iter().zip(after).enumerate() {
        let n = l0.len();
        for i in 0..n {
            let (a0, b0) = (l0[i], l0[(i + 1) % n]);
            let (a1, b1) = (l1[i], l1[(i + 1) % n]);
            let (e0, e1) = (b0 - a0, b1 - a1);
            // The edge must keep its direction and a usable length.
            if e1.dot(e0) <= 0.0 || e1.length() < gap {
                return false;
            }
            segs.push((li, i, a1, b1));
        }
    }
    for x in 0..segs.len() {
        for y in x + 1..segs.len() {
            let (lx, ix, a, b) = segs[x];
            let (ly, iy, c, d) = segs[y];
            if lx == ly {
                let n = after[lx].len();
                if (ix + 1) % n == iy || (iy + 1) % n == ix {
                    continue;
                }
            }
            if segment_gap(a, b, c, d) < gap {
                return false;
            }
        }
    }
    true
}

/// The loops of `face` offset by `distance` (positive into the face), in
/// 3D, outline first, and the distance actually used: `distance`, or less
/// if that would make an invalid outline. None if the face has no valid
/// offset at all.
pub fn offset_loops(model: &Model, face: FaceId, distance: f64) -> Option<(Vec<Vec<DVec3>>, f64)> {
    let (flat, loops) = flat_loops(model, face)?;
    let tol = model.tol();
    let gap = tol.dist * 10.0;
    let at = |d: f64| -> Option<Vec<Vec<DVec2>>> {
        let out: Option<Vec<_>> = loops.iter().map(|l| offset_loop(l, d)).collect();
        out.filter(|o| valid(&loops, o, gap))
    };
    let mut used = distance;
    let mut out = at(distance);
    if out.is_none() {
        // The largest valid distance, to within the tolerance.
        let (mut lo, mut hi) = (0.0f64, distance.abs());
        while hi - lo > tol.dist {
            let mid = 0.5 * (lo + hi);
            if at(mid.copysign(distance)).is_some() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        used = lo.copysign(distance);
        if lo <= gap {
            return None;
        }
        out = at(used);
    }
    let out = out?;
    Some((
        out.iter()
            .map(|l| l.iter().map(|p| flat.to_3d(*p)).collect())
            .collect(),
        used,
    ))
}

/// The signed distance from `p` (on or near the face's plane) to the face's
/// loops: positive inside the face, negative outside.
pub fn signed_distance(model: &Model, face: FaceId, p: DVec3) -> Option<f64> {
    let (flat, loops) = flat_loops(model, face)?;
    let q = flat.to_2d(p);
    let mut best = f64::INFINITY;
    for l in &loops {
        let n = l.len();
        for i in 0..n {
            let (a, b) = (l[i], l[(i + 1) % n]);
            let e = b - a;
            let t = ((q - a).dot(e) / e.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
            best = best.min(q.distance(a + e * t));
        }
    }
    let inside = point_in_polygon(q, &loops[0]) != Inside::Outside
        && loops[1..]
            .iter()
            .all(|h| point_in_polygon(q, h) != Inside::Inside);
    Some(if inside { best } else { -best })
}

/// The offset distance whose loops pass through `p` (on or near the face's
/// plane): positive into the face. Each edge, moved its own distance from
/// `p`'s line, is tried; the smallest that reaches `p` between its mitred
/// ends wins. Falls back to [`signed_distance`].
pub fn distance_through(model: &Model, face: FaceId, p: DVec3) -> Option<f64> {
    let (flat, loops) = flat_loops(model, face)?;
    let q = flat.to_2d(p);
    let slack = model.tol().dist;
    let mut best: Option<f64> = None;
    for l in &loops {
        let n = l.len();
        for i in 0..n {
            let (a, b) = (l[i], l[(i + 1) % n]);
            let t = (b - a).normalize_or_zero();
            let d = DVec2::new(-t.y, t.x).dot(q - a);
            let Some(moved) = offset_loop(l, d) else {
                continue;
            };
            let (a1, b1) = (moved[i], moved[(i + 1) % n]);
            let e = b1 - a1;
            let len = e.length();
            let along = (q - a1).dot(e) / len.max(f64::MIN_POSITIVE);
            if e.dot(b - a) > 0.0
                && along >= -slack
                && along <= len + slack
                && best.is_none_or(|x| d.abs() < x.abs())
            {
                best = Some(d);
            }
        }
    }
    best.or_else(|| signed_distance(model, face, p))
}

/// Offset a face: add its offset loops as edges (they split the face, or
/// add a ring round it). Returns the distance used, if anything was added.
pub fn offset(model: &mut Model, face: FaceId, distance: f64) -> Option<f64> {
    if model.tol().is_zero(distance) {
        return None;
    }
    let (loops, used) = offset_loops(model, face, distance)?;
    for l in &loops {
        for i in 0..l.len() {
            add_segment(model, l[i], l[(i + 1) % l.len()]);
        }
    }
    Some(used)
}
