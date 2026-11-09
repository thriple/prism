//! How far Push/Pull may move a face (owner, 2026-11-09): the face moves
//! freely until it meets another face, and stops there (where letting go
//! merges them). It never passes through a face, and a neighbour that
//! leans over the face's path blocks it at once. Alt's new-loop pull is
//! not limited (it leaves the original loop behind).

use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3, Tol};
use prism_topo::{Face, FaceId, Model, VertexId, loop_pairs};

/// The distance (with `distance`'s sign, no longer than it) `face` can
/// move along its normal before it meets another face.
pub fn limit(model: &Model, face: FaceId, distance: f64) -> f64 {
    let Some(f) = model.face(face) else {
        return 0.0;
    };
    let tol = model.tol();
    let reach = distance.abs();
    if tol.is_zero(reach) {
        return distance;
    }
    let sign = distance.signum();
    let dir = f.plane().normal * sign;
    let origin = f.plane().origin;
    let frame = f.plane().frame();
    let flat = |l: &[VertexId]| -> Vec<DVec2> {
        l.iter().map(|v| frame.to_2d(model.vertex(*v))).collect()
    };
    let f2: Vec<Vec<DVec2>> = f.loops().map(|l| flat(l)).collect();
    let f3: Vec<Vec<DVec3>> = f.loops().map(|l| model.points(l)).collect();
    // The swept block's box, to skip faces far away.
    let (lo, hi) = bounds(f3.iter().flatten().copied());
    let (lo, hi) = (
        lo.min(lo + dir * reach) - DVec3::splat(tol.dist),
        hi.max(hi + dir * reach) + DVec3::splat(tol.dist),
    );
    let mut best = reach;
    for (gid, g) in model.faces() {
        if gid == face {
            continue;
        }
        let g3: Vec<Vec<DVec3>> = g.loops().map(|l| model.points(l)).collect();
        let (glo, ghi) = bounds(g3.iter().flatten().copied());
        if glo.cmpgt(hi).any() || ghi.cmplt(lo).any() {
            continue;
        }
        if leans_over(model, f, g, dir) {
            return 0.0;
        }
        let gn = g.plane().normal;
        let height = |p: DVec3| (p - origin).dot(dir);
        if tol.parallel(gn, dir) {
            // A parallel face ahead: met when the two overlap.
            let h = height(g.plane().origin);
            if h > tol.dist && h < best {
                let gframe: Vec<Vec<DVec2>> = g
                    .loops()
                    .map(|l| l.iter().map(|v| frame.to_2d(model.vertex(*v))).collect())
                    .collect();
                if overlap(&f2, &gframe, &tol) {
                    take(&mut best, h, tol.dist);
                }
            }
            continue;
        }
        // Another face's corner inside the swept block.
        for p in g3.iter().flatten() {
            let h = height(*p);
            if h > tol.dist && h <= best && inside(frame.to_2d(*p), &f2, &tol) {
                take(&mut best, h, tol.dist);
            }
        }
        // The moving face's corner reaching the other face.
        let gframe = g.plane().frame();
        let g2: Vec<Vec<DVec2>> = g3
            .iter()
            .map(|l| l.iter().map(|p| gframe.to_2d(*p)).collect())
            .collect();
        let along = dir.dot(gn);
        if along.abs() > 1e-12 {
            for p in f3.iter().flatten() {
                let t = (g.plane().origin - *p).dot(gn) / along;
                if t > tol.dist && t <= best && inside(gframe.to_2d(*p + dir * t), &g2, &tol) {
                    take(&mut best, t, tol.dist);
                }
            }
        }
        // A moving edge crossing an edge of the other face.
        for fl in &f3 {
            for (p, q) in ring(fl) {
                for gl in &g3 {
                    for (a, b) in ring(gl) {
                        if let Some(t) = edge_cross(p, q, a, b, dir, &tol) {
                            take(&mut best, t, tol.dist);
                        }
                    }
                }
            }
        }
    }
    sign * best
}

/// Keep `t` if it is ahead and nearer than `best`.
fn take(best: &mut f64, t: f64, eps: f64) {
    if t > eps && t < *best {
        *best = t;
    }
}

fn bounds(points: impl Iterator<Item = DVec3>) -> (DVec3, DVec3) {
    points.fold(
        (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)),
        |(lo, hi), p| (lo.min(p), hi.max(p)),
    )
}

fn ring(l: &[DVec3]) -> impl Iterator<Item = (DVec3, DVec3)> + '_ {
    (0..l.len()).map(move |i| (l[i], l[(i + 1) % l.len()]))
}

/// Distance from `p` to the segment `a`–`b` (2D).
fn seg_dist(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Strictly inside a face (outer loop first, then holes): not within
/// tolerance of any of its edges.
fn inside(p: DVec2, loops: &[Vec<DVec2>], tol: &Tol) -> bool {
    let Some((outer, holes)) = loops.split_first() else {
        return false;
    };
    let near = loops
        .iter()
        .any(|l| (0..l.len()).any(|i| seg_dist(p, l[i], l[(i + 1) % l.len()]) <= tol.dist));
    !near
        && point_in_polygon(p, outer) == Inside::Inside
        && holes
            .iter()
            .all(|h| point_in_polygon(p, h) == Inside::Outside)
}

/// Whether two faces in parallel planes cover some of the same area (seen
/// along the normal). Samples just inside each edge of each, so shapes that
/// match exactly count.
fn overlap(a: &[Vec<DVec2>], b: &[Vec<DVec2>], tol: &Tol) -> bool {
    let samples = |loops: &[Vec<DVec2>]| -> Vec<DVec2> {
        let mut out = Vec::new();
        for l in loops {
            // Both sides of each edge's middle: one of them is inside.
            for i in 0..l.len() {
                let (p, q) = (l[i], l[(i + 1) % l.len()]);
                let e = q - p;
                let len = e.length();
                if len <= tol.dist {
                    continue;
                }
                let left = DVec2::new(-e.y, e.x) / len;
                let step = (len * 1e-3).max(tol.dist * 4.0);
                out.push((p + q) * 0.5 + left * step);
                out.push((p + q) * 0.5 - left * step);
            }
        }
        out
    };
    samples(a)
        .into_iter()
        .any(|p| inside(p, a, tol) && inside(p, b, tol))
        || samples(b)
            .into_iter()
            .any(|p| inside(p, a, tol) && inside(p, b, tol))
}

/// When the edge `p`–`q`, moving along `dir`, crosses the edge `a`–`b`
/// (both inside their ends), the distance moved.
fn edge_cross(p: DVec3, q: DVec3, a: DVec3, b: DVec3, dir: DVec3, tol: &Tol) -> Option<f64> {
    // p + s (q - p) + t dir = a + u (b - a)
    let (e, g) = (q - p, b - a);
    let rhs = a - p;
    let det = e.dot(dir.cross(-g));
    let scale = e.length() * g.length();
    if det.abs() <= 1e-12 * scale.max(f64::MIN_POSITIVE) {
        return None;
    }
    let s = rhs.dot(dir.cross(-g)) / det;
    let t = e.dot(rhs.cross(-g)) / det;
    let u = e.dot(dir.cross(rhs)) / det;
    let margin_e = tol.dist / e.length().max(f64::MIN_POSITIVE);
    let margin_g = tol.dist / g.length().max(f64::MIN_POSITIVE);
    (s > margin_e && s < 1.0 - margin_e && u > margin_g && u < 1.0 - margin_g && t > tol.dist)
        .then_some(t)
}

/// Whether neighbour `g` shares an edge with `f` and leans over the path
/// `f` moves along (its inside heads both along `dir` and back over `f`).
fn leans_over(model: &Model, f: &Face, g: &Face, dir: DVec3) -> bool {
    let fn_ = f.plane().normal;
    let gn = g.plane().normal;
    let pairs = |face: &Face| -> Vec<(VertexId, VertexId)> {
        face.loops()
            .flat_map(|l| loop_pairs(l).collect::<Vec<_>>())
            .collect()
    };
    let gp = pairs(g);
    for (a, b) in pairs(f) {
        // The same edge, in either direction.
        let Some(&(ga, gb)) = gp
            .iter()
            .find(|(x, y)| (*x == a && *y == b) || (*x == b && *y == a))
        else {
            continue;
        };
        let (pa, pb) = (model.vertex(a), model.vertex(b));
        let e = (pb - pa).normalize_or_zero();
        // Inside a face is to the left of its loops' edges.
        let v = fn_.cross(e);
        let ge = (model.vertex(gb) - model.vertex(ga)).normalize_or_zero();
        let w = gn.cross(ge);
        if w.dot(dir) > 1e-9 && w.dot(v) > 1e-9 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sticky::add_rect;

    fn top_face(m: &Model, z: f64) -> FaceId {
        m.faces()
            .find(|(_, f)| f.plane().normal.z > 0.9 && (m.points(&f.outer)[0].z - z).abs() < 1e-9)
            .map(|(id, _)| id)
            .expect("a face facing up")
    }

    #[test]
    fn a_free_face_moves_any_distance() {
        let mut m = Model::new();
        add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
        let f = m.faces().next().unwrap().0;
        assert_eq!(limit(&m, f, 5.0), 5.0);
        assert_eq!(limit(&m, f, -5.0), -5.0);
    }

    #[test]
    fn a_face_stops_at_a_parallel_face_in_its_way() {
        let mut m = Model::new();
        add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
        add_rect(&mut m, 0.5, 0.5, 1.5, 1.5, 3.0);
        let f = top_face(&m, 0.0);
        // The floor faces up or down: push it toward the plate above.
        let up = if m.face(f).unwrap().plane().normal.z > 0.0 {
            1.0
        } else {
            -1.0
        };
        assert!((limit(&m, f, up * 10.0) - up * 3.0).abs() < 1e-9);
        assert_eq!(limit(&m, f, up * 2.0), up * 2.0, "short of it: free");
        // A plate off to the side is not in the way.
        let mut m = Model::new();
        add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
        add_rect(&mut m, 5.0, 0.0, 6.0, 1.0, 3.0);
        let f = top_face(&m, 0.0);
        let up = if m.face(f).unwrap().plane().normal.z > 0.0 {
            1.0
        } else {
            -1.0
        };
        assert_eq!(limit(&m, f, up * 10.0), up * 10.0);
    }

    #[test]
    fn a_box_top_pushed_down_stops_at_the_bottom() {
        let mut m = Model::new();
        add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
        let base = m.faces().next().unwrap().0;
        let up = if m.face(base).unwrap().plane().normal.z > 0.0 {
            1.0
        } else {
            -1.0
        };
        crate::push_pull(&mut m, base, up, false);
        let top = top_face(&m, 1.0);
        assert!((limit(&m, top, -5.0) + 1.0).abs() < 1e-9);
        // Up, nothing is in the way.
        assert_eq!(limit(&m, top, 5.0), 5.0);
    }

    #[test]
    fn a_fin_across_the_path_stops_it_where_the_edges_cross() {
        let mut m = Model::new();
        // A long thin floor along x, and a vertical fin along y above it,
        // whose bottom edge at height 2 crosses over the floor's middle.
        add_rect(&mut m, 0.0, 0.0, 4.0, 1.0, 0.0);
        let fin = [
            DVec3::new(2.0, -1.0, 2.0),
            DVec3::new(2.0, 2.0, 2.0),
            DVec3::new(2.0, 2.0, 3.0),
            DVec3::new(2.0, -1.0, 3.0),
        ];
        for i in 0..4 {
            crate::sticky::add_segment(&mut m, fin[i], fin[(i + 1) % 4]);
        }
        let f = top_face(&m, 0.0);
        let up = if m.face(f).unwrap().plane().normal.z > 0.0 {
            1.0
        } else {
            -1.0
        };
        assert!((limit(&m, f, up * 10.0) - up * 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_neighbour_leaning_over_the_path_blocks_at_once() {
        let mut m = Model::new();
        // A floor from x 0 to 2, and a face hinged on its x = 2 edge that
        // leans back over it (up and toward x = 1).
        let p = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(2.0, 1.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
        ];
        for i in 0..4 {
            crate::sticky::add_segment(&mut m, p[i], p[(i + 1) % 4]);
        }
        let lean = [
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 1.0),
            DVec3::new(1.0, 1.0, 1.0),
            DVec3::new(2.0, 1.0, 0.0),
        ];
        for i in 0..4 {
            crate::sticky::add_segment(&mut m, lean[i], lean[(i + 1) % 4]);
        }
        let f = top_face(&m, 0.0);
        let up = if m.face(f).unwrap().plane().normal.z > 0.0 {
            1.0
        } else {
            -1.0
        };
        assert_eq!(limit(&m, f, up * 3.0), 0.0);
        // Away from it (down), the floor moves freely.
        assert_eq!(limit(&m, f, -up * 3.0), -up * 3.0);
    }
}
