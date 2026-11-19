//! Offset of a planar face: every loop (the outline and its holes) moves the
//! same distance into the face (positive) or out of it (negative), with
//! mitred corners.
//!
//! The offset goes as far as it is asked, even past where edges meet or turn
//! round: the new lines cross the old ones and each other, sticky geometry
//! splits them there, and tidying up is the user's job (as when an offset
//! is taken too far in SketchUp). The only limits are what cannot be built:
//! a corner with no mitre, a spline, an arc that would shrink to nothing.

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

/// The loops of `face` offset by `distance` (positive into the face), in
/// 3D, outline first, and the distance used, which is `distance`. None if
/// a corner has no mitre (an edge doubles straight back).
pub fn offset_loops(model: &Model, face: FaceId, distance: f64) -> Option<(Vec<Vec<DVec3>>, f64)> {
    let (flat, loops) = flat_loops(model, face)?;
    let out: Vec<Vec<DVec2>> = loops
        .iter()
        .map(|l| offset_loop(l, distance))
        .collect::<Option<_>>()?;
    Some((
        out.iter()
            .map(|l| l.iter().map(|p| flat.to_3d(*p)).collect())
            .collect(),
        distance,
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
#[derive(Clone, Debug)]
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
                        curve: e.curve_from(s.from),
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
        // A spline's offset is not a spline: not built.
        Curve::Spline(_) => None,
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
    let pts = match (p.curve.clone(), q.curve.clone()) {
        (Curve::Spline(_), _) | (_, Curve::Spline(_)) => return None,
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

/// The pieces of a loop moved `d` to their left and joined, or None if an
/// arc cannot be built (it would shrink to nothing or turn inside out). A
/// line may shrink away or turn round: that is the user's to tidy. An arc
/// whose radius would reach zero is dropped, and its neighbours join
/// directly.
fn offset_pieces(l: &[Piece], n: DVec3, d: f64, gap: f64) -> Option<Vec<Piece>> {
    let kept: Vec<(&Piece, Piece)> = l
        .iter()
        .filter_map(|p| match moved(p, n, d) {
            Some(m) => Some(Some((p, m))),
            // Only an arc can shrink away.
            None if p.curve.is_curved() => None,
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
            curve: kept[i].1.curve.clone(),
        })
        .collect();
    for ((p, _), q) in kept.iter().zip(&out) {
        let ok = match q.curve {
            Curve::Spline(_) => false,
            Curve::Line => q.a.distance(q.b) >= gap,
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

/// [`offset_loops`] for a face with arcs: its loops as pieces.
pub fn offset_arc_loops(
    model: &Model,
    face: FaceId,
    distance: f64,
) -> Option<(Vec<Vec<Piece>>, f64)> {
    let f = model.face(face)?;
    let n = f.plane()?.normal;
    let loops = face_pieces(model, face)?;
    let tol = model.tol();
    let gap = tol.dist * 10.0;
    let at = |d: f64| -> Option<Vec<Vec<Piece>>> {
        let out: Vec<Vec<Piece>> = loops
            .iter()
            .map(|l| offset_pieces(l, n, d, gap))
            .collect::<Option<_>>()?;
        Some(out)
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

/// The offset distance that puts the loops through `p`, measured from the
/// edge of the face nearest `start` (where the offset was begun): `p`'s
/// distance from that edge's line, positive into the face. This is how
/// Offset matches a point in another plane: the distance does not jump to
/// whichever edge `p` happens to lie on. Faces with arcs use
/// [`distance_through`].
pub fn distance_from_nearest_edge(
    model: &Model,
    face: FaceId,
    start: DVec3,
    p: DVec3,
) -> Option<f64> {
    if model.face_has_arcs(face) {
        return distance_through(model, face, p);
    }
    let (flat, loops) = flat_loops(model, face)?;
    let (s, q) = (flat.to_2d(start), flat.to_2d(p));
    let mut best: Option<(f64, DVec2, DVec2)> = None;
    for l in &loops {
        let n = l.len();
        for i in 0..n {
            let (a, b) = (l[i], l[(i + 1) % n]);
            let e = b - a;
            let t = ((s - a).dot(e) / e.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
            let gap = s.distance(a + e * t);
            if best.is_none_or(|(g, ..)| gap < g) {
                best = Some((gap, a, b));
            }
        }
    }
    let (_, a, b) = best?;
    let t = (b - a).normalize_or_zero();
    (t != DVec2::ZERO).then(|| DVec2::new(-t.y, t.x).dot(q - a))
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
            Curve::Spline(_) => Some(x.curve.closest_point(x.a, x.b, p).distance(p)),
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

/// Said when a face has a spline edge: its curve cannot be moved sideways yet.
pub const SPLINE_OFFSET: &str =
    "Offset cannot move a spline edge yet: draw that outline with lines and arcs.";

/// Said when no distance gives a valid outline.
pub const NO_OFFSET: &str = "That face cannot be offset: an edge would vanish or cross.";

/// Why `offset` would do nothing for this face and distance, if it would.
pub fn refusal(model: &Model, face: FaceId, distance: f64) -> Option<&'static str> {
    if model.face(face)?.is_curved() {
        return Some(CURVED_OFFSET);
    }
    if face_pieces(model, face).is_some_and(|loops| {
        loops
            .iter()
            .flatten()
            .any(|p| matches!(p.curve, Curve::Spline(_)))
    }) {
        return Some(SPLINE_OFFSET);
    }
    if model.tol().is_zero(distance) {
        return None;
    }
    let found = if model.face_has_arcs(face) {
        offset_arc_loops(model, face, distance).is_some()
    } else {
        offset_loops(model, face, distance).is_some()
    };
    (!found).then_some(NO_OFFSET)
}

pub fn offset(model: &mut Model, face: FaceId, distance: f64) -> Option<f64> {
    if model.tol().is_zero(distance) {
        return None;
    }
    if model.face_has_arcs(face) {
        let (loops, used) = offset_arc_loops(model, face, distance)?;
        for p in loops.iter().flatten() {
            match p.curve {
                Curve::Spline(_) => {}
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
