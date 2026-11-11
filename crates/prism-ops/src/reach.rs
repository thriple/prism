//! How far Push/Pull may move a face (owner, 2026-11-09): the face moves
//! freely until it meets another face, and stops there (where letting go
//! merges them). It never passes through a face, and a neighbour that
//! leans over the face's path blocks it at once. Alt's new-loop pull is
//! not limited (it leaves the original loop behind).

use prism_math::{DVec3, Tol};
use prism_topo::{Face, FaceId, Model, VertexId, loop_pairs};

use crate::shell::Planar;

/// The distance (with `distance`'s sign, no longer than it) `face` can
/// move along its normal before it meets another face. A cylinder face
/// taking a new radius stops where it meets another face too.
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
    let Some(&plane) = f.plane() else {
        return grow_limit(model, face, distance);
    };
    let dir = plane.normal * sign;
    let origin = plane.origin;
    let f3: Vec<Vec<DVec3>> = f.loops().map(|l| model.points(l)).collect();
    let Some(fplan) = Planar::of(model, face) else {
        return distance;
    };
    let in_f = |p: DVec3| fplan.classify(p, tol.dist) == Some(true);
    let f_arcs = crate::meet::has_arcs(model, face);
    let f_lines = straight_edges(model, face);
    let mover = crate::meet::Mover::slide(model, face, dir);
    // The swept block's box, to skip faces far away.
    let Some((lo, hi)) = crate::meet::face_bounds(model, face) else {
        return distance;
    };
    let (lo, hi) = (
        lo.min(lo + dir * reach) - DVec3::splat(tol.dist),
        hi.max(hi + dir * reach) + DVec3::splat(tol.dist),
    );
    let mut best = reach;
    for (gid, g) in model.faces() {
        if gid == face {
            continue;
        }
        let Some((glo, ghi)) = crate::meet::face_bounds(model, gid) else {
            continue;
        };
        if glo.cmpgt(hi).any() || ghi.cmplt(lo).any() {
            continue;
        }
        let height = |p: DVec3| (p - origin).dot(dir);
        let Some(gp) = g.plane().copied() else {
            // A curved face: met at its edges, where the face's outline
            // runs into it, or where it faces the face head on.
            if let Some(mv) = &mover {
                best = crate::meet::first_meeting(model, mv, gid, best);
            }
            continue;
        };
        if leans_over(model, f, g, dir) {
            return 0.0;
        }
        if tol.parallel(gp.normal, dir) {
            // A parallel face ahead: met when the two overlap.
            let h = height(gp.origin);
            if h > tol.dist
                && h < best
                && Planar::of(model, gid)
                    .is_some_and(|gplan| overlap(model, face, &fplan, gid, &gplan, &tol))
            {
                take(&mut best, h, tol.dist);
            }
            continue;
        }
        let g3: Vec<Vec<DVec3>> = g.loops().map(|l| model.points(l)).collect();
        // Another face's corner inside the swept block.
        for p in g3.iter().flatten() {
            let h = height(*p);
            if h > tol.dist && h <= best && in_f(*p - dir * h) {
                take(&mut best, h, tol.dist);
            }
        }
        // The moving face's corner reaching the other face.
        if let Some(gplan) = Planar::of(model, gid) {
            let gn = gp.normal;
            let along = dir.dot(gn);
            if along.abs() > 1e-12 {
                for p in f3.iter().flatten() {
                    let t = (gp.origin - *p).dot(gn) / along;
                    if t > tol.dist
                        && t <= best
                        && gplan.classify(*p + dir * t, tol.dist) == Some(true)
                    {
                        take(&mut best, t, tol.dist);
                    }
                }
            }
        }
        // A moving straight edge crossing a straight edge of the other face.
        for (p, q) in &f_lines {
            for (a, b) in straight_edges(model, gid) {
                if let Some(t) = edge_cross(*p, *q, a, b, dir, &tol) {
                    take(&mut best, t, tol.dist);
                }
            }
        }
        // Arcs on either outline: met along them.
        if (f_arcs || crate::meet::has_arcs(model, gid))
            && let Some(mv) = &mover
        {
            best = crate::meet::first_meeting(model, mv, gid, best);
        }
    }
    sign * best
}

/// A face's straight edges, as their ends.
fn straight_edges(model: &Model, face: FaceId) -> Vec<(DVec3, DVec3)> {
    model
        .face(face)
        .map(|f| {
            f.sides()
                .filter_map(|s| model.edge(s.edge))
                .filter(|e| !e.curve.is_arc())
                .map(|e| (model.vertex(e.a), model.vertex(e.b)))
                .collect()
        })
        .unwrap_or_default()
}

/// How far a cylinder face's new radius can go (with `distance`'s sign,
/// no longer than it) before the faces on its cylinder meet another face.
fn grow_limit(model: &Model, face: FaceId, distance: f64) -> f64 {
    let Some((c, fs)) = crate::radius::faces(model, face) else {
        // Cones, spheres and tori take no new size yet.
        return distance;
    };
    let tol = model.tol();
    let reach = distance.abs();
    // The radius grows when an outward face is pulled out.
    let grows = c.outward == (distance > 0.0);
    let ids: Vec<FaceId> = fs.iter().copied().collect();
    let Some(mover) = crate::meet::Mover::grow(
        model,
        &ids,
        c.origin,
        c.axis,
        c.radius,
        if grows { 1.0 } else { -1.0 },
    ) else {
        return distance;
    };
    let boxes: Vec<(DVec3, DVec3)> = ids
        .iter()
        .filter_map(|f| crate::meet::face_bounds(model, *f))
        .collect();
    let Some(&(mut lo, mut hi)) = boxes.first() else {
        return distance;
    };
    for (a, b) in &boxes {
        lo = lo.min(*a);
        hi = hi.max(*b);
    }
    let pad = DVec3::splat(reach + tol.dist);
    let (lo, hi) = (lo - pad, hi + pad);
    let mut best = reach;
    for (gid, _) in model.faces() {
        if fs.contains(&gid) {
            continue;
        }
        let Some((glo, ghi)) = crate::meet::face_bounds(model, gid) else {
            continue;
        };
        if glo.cmpgt(hi).any() || ghi.cmplt(lo).any() {
            continue;
        }
        best = crate::meet::first_meeting(model, &mover, gid, best);
    }
    distance.signum() * best
}

/// Keep `t` if it is ahead and nearer than `best`.
fn take(best: &mut f64, t: f64, eps: f64) {
    if t > eps && t < *best {
        *best = t;
    }
}

/// Whether two faces in parallel planes cover some of the same area (seen
/// along the normal). Samples just inside each side of each drawn outline
/// (arcs by their drawn points), so shapes that match exactly count.
fn overlap(m: &Model, a: FaceId, pa: &Planar, b: FaceId, pb: &Planar, tol: &Tol) -> bool {
    let both =
        |p: DVec3| pa.classify(p, tol.dist) == Some(true) && pb.classify(p, tol.dist) == Some(true);
    [(a, pa.normal), (b, pb.normal)].iter().any(|(id, n)| {
        let Some(f) = m.face(*id) else {
            return false;
        };
        m.face_loop_points(f).iter().any(|l| {
            (0..l.len()).any(|i| {
                let (p, q) = (l[i], l[(i + 1) % l.len()]);
                let e = q - p;
                let len = e.length();
                if len <= tol.dist {
                    return false;
                }
                // Both sides of the side's middle: one of them is inside.
                let left = n.cross(e) / len;
                let step = (len * 1e-3).max(tol.dist * 4.0);
                let mid = (p + q) * 0.5;
                both(mid + left * step) || both(mid - left * step)
            })
        })
    })
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
    let (Some(fp), Some(gp)) = (f.plane(), g.plane()) else {
        return false;
    };
    let (fn_, gn) = (fp.normal, gp.normal);
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
            .find(|(_, f)| {
                f.plane().unwrap().normal.z > 0.9 && (m.points(&f.outer)[0].z - z).abs() < 1e-9
            })
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
        let up = if m.face(f).unwrap().plane().unwrap().normal.z > 0.0 {
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
        let up = if m.face(f).unwrap().plane().unwrap().normal.z > 0.0 {
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
        let up = if m.face(base).unwrap().plane().unwrap().normal.z > 0.0 {
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
        let up = if m.face(f).unwrap().plane().unwrap().normal.z > 0.0 {
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
        let up = if m.face(f).unwrap().plane().unwrap().normal.z > 0.0 {
            1.0
        } else {
            -1.0
        };
        assert_eq!(limit(&m, f, up * 3.0), 0.0);
        // Away from it (down), the floor moves freely.
        assert_eq!(limit(&m, f, -up * 3.0), -up * 3.0);
    }
}
