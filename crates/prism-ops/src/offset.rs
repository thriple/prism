//! Offset of a planar face: every loop (the outline and its holes) moves the
//! same distance into the face (positive) or out of it (negative), with
//! mitred corners.
//!
//! The offset never makes an invalid outline: if the distance asked for
//! would make an edge vanish or turn round, or two edges cross or touch,
//! it stops just short of that.

use std::f64::consts::{PI, TAU};

use prism_geom::{Curve, circle_circle, circle_line};
use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3};
use prism_topo::{FaceId, Model};

use crate::{add_arc, add_segment};

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
    let n = f.plane()?.normal;
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
    let (out, used) = largest_valid(distance, gap, tol.dist, at)?;
    Some((
        out.iter()
            .map(|l| l.iter().map(|p| flat.to_3d(*p)).collect())
            .collect(),
        used,
    ))
}

/// The result at `distance` if valid, or at the largest valid distance
/// short of it (to within `step`); None if that is within `gap` of zero.
fn largest_valid<T>(
    distance: f64,
    gap: f64,
    step: f64,
    at: impl Fn(f64) -> Option<T>,
) -> Option<(T, f64)> {
    if let Some(out) = at(distance) {
        return Some((out, distance));
    }
    let (mut lo, mut hi) = (0.0f64, distance.abs());
    while hi - lo > step {
        let mid = 0.5 * (lo + hi);
        if at(mid.copysign(distance)).is_some() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    if lo <= gap {
        return None;
    }
    let used = lo.copysign(distance);
    Some((at(used)?, used))
}

/// One piece of a loop, walked from `a` to `b`, the face on its left.
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub a: DVec3,
    pub b: DVec3,
    /// The curve walked from `a` to `b`.
    pub curve: Curve,
}

/// The loops of `face` as pieces (lines and arcs), outline first.
fn face_pieces(model: &Model, face: FaceId) -> Option<Vec<Vec<Piece>>> {
    let f = model.face(face)?;
    (0..f.loops().count())
        .map(|k| {
            let sides: Vec<_> = f.loop_edges(k).collect();
            (sides.len() == f.loops().nth(k)?.len()).then_some(())?;
            sides
                .iter()
                .map(|s| {
                    let e = model.edge(s.edge)?;
                    Some(Piece {
                        a: model.vertex(s.from),
                        b: model.vertex(s.to),
                        curve: if s.from == e.a {
                            e.curve
                        } else {
                            e.curve.reversed()
                        },
                    })
                })
                .collect()
        })
        .collect()
}

/// A piece's offset, as an unbounded line or circle: its curve, and where
/// its ends go if nothing trims them.
fn moved(p: &Piece, n: DVec3, d: f64) -> Option<Piece> {
    match p.curve {
        Curve::Line => {
            let left = n.cross(p.b - p.a).normalize_or_zero();
            Some(Piece {
                a: p.a + left * d,
                b: p.b + left * d,
                curve: Curve::Line,
            })
        }
        Curve::Arc {
            centre,
            axis,
            radius,
        } => {
            // Walking counter-clockwise about the face's normal, the face
            // (the left) is towards the centre.
            let r = if axis.dot(n) > 0.0 {
                radius - d
            } else {
                radius + d
            };
            (r > 0.0).then(|| Piece {
                a: centre + (p.a - centre).normalize() * r,
                b: centre + (p.b - centre).normalize() * r,
                curve: Curve::Arc {
                    centre,
                    axis,
                    radius: r,
                },
            })
        }
    }
}

/// Where two moved pieces join: the point their lines or circles share
/// nearest to where their ends went. None if they do not meet.
fn join(p: &Piece, q: &Piece, n: DVec3, eps: f64) -> Option<DVec3> {
    let near = p.b.lerp(q.a, 0.5);
    if p.b.distance(q.a) <= eps {
        return Some(near);
    }
    let pts = match (p.curve, q.curve) {
        (Curve::Line, Curve::Line) => {
            let (u, w) = (p.b - p.a, q.b - q.a);
            let den = n.dot(u.cross(w));
            if den.abs() <= 1e-12 * u.length() * w.length() {
                return None;
            }
            let t = n.dot((q.a - p.a).cross(w)) / den;
            vec![p.a + u * t]
        }
        (
            Curve::Line,
            Curve::Arc {
                centre,
                axis,
                radius,
            },
        ) => circle_line(centre, axis, radius, p.a, p.b, eps),
        (
            Curve::Arc {
                centre,
                axis,
                radius,
            },
            Curve::Line,
        ) => circle_line(centre, axis, radius, q.a, q.b, eps),
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
        ) => circle_circle((c1, a1, r1), (c2, a2, r2), eps),
    };
    pts.into_iter()
        .min_by(|x, y| x.distance(near).total_cmp(&y.distance(near)))
}

/// The pieces of a loop moved `d` to their left and joined, or None if a
/// piece vanished or turned round. An arc whose radius would reach zero is
/// dropped, and its neighbours join directly.
fn offset_pieces(l: &[Piece], n: DVec3, d: f64, gap: f64) -> Option<Vec<Piece>> {
    let kept: Vec<(&Piece, Piece)> = l
        .iter()
        .filter_map(|p| match moved(p, n, d) {
            Some(m) => Some(Some((p, m))),
            // Only an arc can shrink away.
            None if p.curve.is_arc() => None,
            None => Some(None),
        })
        .collect::<Option<_>>()?;
    let k = kept.len();
    if k < 2 {
        return None;
    }
    let joints: Vec<DVec3> = (0..k)
        .map(|i| join(&kept[i].1, &kept[(i + 1) % k].1, n, gap * 0.1))
        .collect::<Option<_>>()?;
    let out: Vec<Piece> = (0..k)
        .map(|i| Piece {
            a: joints[(i + k - 1) % k],
            b: joints[i],
            curve: kept[i].1.curve,
        })
        .collect();
    for ((p, _), q) in kept.iter().zip(&out) {
        let ok = match q.curve {
            Curve::Line => (q.b - q.a).dot(p.b - p.a) > 0.0 && q.a.distance(q.b) >= gap,
            Curve::Arc { radius, .. } => {
                let (s0, s1) = (p.curve.sweep(p.a, p.b), q.curve.sweep(q.a, q.b));
                radius >= gap && s1 * radius >= gap && (s1 - s0).abs() < PI && s1 < TAU
            }
        };
        if !ok {
            return None;
        }
    }
    Some(out)
}

/// A piece drawn as points in the face's 2D frame.
fn piece_points(flat: &Flat, p: &Piece) -> Vec<DVec2> {
    std::iter::once(p.a)
        .chain(p.curve.inner_points(p.a, p.b))
        .chain(std::iter::once(p.b))
        .map(|x| flat.to_2d(x))
        .collect()
}

/// Whether offset pieces keep apart: no two pieces that are not neighbours
/// come within `gap`.
fn pieces_apart(flat: &Flat, loops: &[Vec<Piece>], gap: f64) -> bool {
    let mut segs: Vec<(usize, usize, DVec2, DVec2)> = Vec::new();
    for (li, l) in loops.iter().enumerate() {
        for (pi, p) in l.iter().enumerate() {
            let pts = piece_points(flat, p);
            for w in pts.windows(2) {
                segs.push((li, pi, w[0], w[1]));
            }
        }
    }
    for x in 0..segs.len() {
        for y in x + 1..segs.len() {
            let (lx, px, a, b) = segs[x];
            let (ly, py, c, d) = segs[y];
            if lx == ly {
                let k = loops[lx].len();
                if px == py || (px + 1) % k == py || (py + 1) % k == px {
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

/// [`offset_loops`] for a face with arcs: its loops as pieces.
pub fn offset_arc_loops(
    model: &Model,
    face: FaceId,
    distance: f64,
) -> Option<(Vec<Vec<Piece>>, f64)> {
    let f = model.face(face)?;
    let n = f.plane()?.normal;
    let flat = Flat::new(model.vertex(*f.outer.first()?), n);
    let loops = face_pieces(model, face)?;
    let tol = model.tol();
    let gap = tol.dist * 10.0;
    let at = |d: f64| -> Option<Vec<Vec<Piece>>> {
        let out: Vec<Vec<Piece>> = loops
            .iter()
            .map(|l| offset_pieces(l, n, d, gap))
            .collect::<Option<_>>()?;
        pieces_apart(&flat, &out, gap).then_some(out)
    };
    largest_valid(distance, gap, tol.dist, at)
}

/// The signed distance from `p` (on or near the face's plane) to the face's
/// loops: positive inside the face, negative outside.
pub fn signed_distance(model: &Model, face: FaceId, p: DVec3) -> Option<f64> {
    if model.face_has_arcs(face) {
        // Along the curves, and inside by the drawn outline.
        let f = model.face(face)?;
        let flat = Flat::new(model.vertex(*f.outer.first()?), f.plane()?.normal);
        let best = face_pieces(model, face)?
            .iter()
            .flatten()
            .map(|x| x.curve.closest_point(x.a, x.b, p).distance(p))
            .fold(f64::INFINITY, f64::min);
        let q = flat.to_2d(p);
        let poly = |k: usize| -> Vec<DVec2> {
            model
                .loop_points(f, k)
                .iter()
                .map(|x| flat.to_2d(*x))
                .collect()
        };
        let inside = point_in_polygon(q, &poly(0)) != Inside::Outside
            && (1..f.loops().count()).all(|k| point_in_polygon(q, &poly(k)) != Inside::Inside);
        return Some(if inside { best } else { -best });
    }
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
    if model.face_has_arcs(face) {
        return arc_distance_through(model, face, p).or_else(|| signed_distance(model, face, p));
    }
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

/// [`distance_through`] for a face with arcs: the nearest piece that `p`
/// lies beside (within its length, or its sweep), by how far it is to
/// that piece's left.
fn arc_distance_through(model: &Model, face: FaceId, p: DVec3) -> Option<f64> {
    let n = model.face(face)?.plane()?.normal;
    let slack = model.tol().dist;
    face_pieces(model, face)?
        .iter()
        .flatten()
        .filter_map(|x| match x.curve {
            Curve::Line => {
                let e = x.b - x.a;
                let along = (p - x.a).dot(e) / e.length_squared().max(f64::MIN_POSITIVE);
                let len = e.length();
                (along * len >= -slack && along * len <= len + slack)
                    .then(|| n.cross(e).normalize_or_zero().dot(p - x.a))
            }
            Curve::Arc {
                centre,
                axis,
                radius,
            } => {
                let w = p - centre;
                let off = (w - axis * w.dot(axis)).length();
                // Beside the arc: the closest point on it is not an end.
                let q = x.curve.closest_point(x.a, x.b, p);
                let on = q.distance(centre + (w - axis * w.dot(axis)).normalize_or_zero() * radius)
                    <= slack;
                let d = if axis.dot(n) > 0.0 {
                    radius - off
                } else {
                    off - radius
                };
                on.then_some(d)
            }
        })
        .min_by(|a, b| a.abs().total_cmp(&b.abs()))
}

/// Offset a face: add its offset loops as edges (they split the face, or
/// add a ring round it). Returns the distance used, if anything was added.
/// Said when Offset is asked to offset a curved face.
pub const CURVED_OFFSET: &str = "Curved faces cannot be offset yet.";

pub fn offset(model: &mut Model, face: FaceId, distance: f64) -> Option<f64> {
    if model.tol().is_zero(distance) {
        return None;
    }
    if model.face_has_arcs(face) {
        let (loops, used) = offset_arc_loops(model, face, distance)?;
        for p in loops.iter().flatten() {
            match p.curve {
                Curve::Line => {
                    add_segment(model, p.a, p.b);
                }
                Curve::Arc {
                    centre,
                    axis,
                    radius,
                } => {
                    add_arc(model, centre, axis, radius, p.a, p.curve.sweep(p.a, p.b));
                }
            }
        }
        return Some(used);
    }
    let (loops, used) = offset_loops(model, face, distance)?;
    for l in &loops {
        for i in 0..l.len() {
            add_segment(model, l[i], l[(i + 1) % l.len()]);
        }
    }
    Some(used)
}
