//! Faces in one plane, rebuilt from the plane's edges.
//!
//! All edges in the plane form a planar graph (edges are already split where
//! they cross). Tracing it with half-edges sorted by angle gives every atomic
//! region: an outer loop, and the outer boundaries of the islands inside it
//! as holes. A region becomes a face when an existing face covered it, or
//! when its outer loop uses an edge that was just added (a newly closed
//! loop). Faces whose region did not change keep their ids.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::Surface;
use prism_math::plane::Frame;
use prism_math::predicates::{Inside, orient2d, point_in_polygon, signed_area2};
use prism_math::{DVec2, DVec3, Plane, Tol};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId, loop_pairs};

/// One region of the plane's graph.
#[derive(Clone, Debug)]
struct Region {
    /// Counter-clockwise about the frame's normal.
    outer: Vec<VertexId>,
    /// Clockwise about the frame's normal.
    holes: Vec<Vec<VertexId>>,
}

/// How regions that no face covered become faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fill {
    /// Drawing: a region whose outer loop uses a new edge (a newly closed loop).
    Drawn,
    /// After an edit (Push/Pull, Move): a region that uses a new edge and
    /// closes an opening, so each of its edges has exactly one other face.
    Closing,
}

/// Rebuild the faces in `plane`. `new_edges` were just added. Returns the
/// faces made.
pub(crate) fn rebuild_plane(
    model: &mut Model,
    plane: &Plane,
    new_edges: &BTreeSet<EdgeId>,
    tol: &Tol,
    fill: Fill,
) -> Vec<FaceId> {
    let frame = plane.frame();
    let on: BTreeMap<VertexId, DVec2> = model
        .vertices()
        .filter(|(_, p)| plane.contains(*p, tol))
        .map(|(id, p)| (id, frame.to_2d(p)))
        .collect();
    let edges: Vec<(EdgeId, VertexId, VertexId)> = model
        .edges()
        .filter(|(_, e)| on.contains_key(&e.a) && on.contains_key(&e.b))
        .map(|(id, e)| (id, e.a, e.b))
        .collect();
    let old: Vec<FaceId> = model
        .faces()
        .filter(|(_, f)| {
            f.plane().same_surface(plane, tol) && f.loops().flatten().all(|v| on.contains_key(v))
        })
        .map(|(id, _)| id)
        .collect();

    let regions = trace(&on, &edges, tol);
    let edge_of = |a: VertexId, b: VertexId| model.edge_between(a, b);

    // Decide which regions are faces, and which way each faces.
    let mut keep: BTreeSet<FaceId> = BTreeSet::new();
    let mut made_faces: Vec<Face> = Vec::new();
    let mut openings: Vec<&Region> = Vec::new();
    for r in &regions {
        let loops: Vec<Vec<DVec2>> = std::iter::once(&r.outer)
            .chain(r.holes.iter())
            .map(|l| l.iter().map(|v| on[v]).collect())
            .collect();
        let Some(sample) = interior_point(&loops) else {
            continue;
        };
        let covers: Vec<FaceId> = old
            .iter()
            .copied()
            .filter(|f| face_contains_2d(model, *f, &frame, sample))
            .collect();
        let parent = covers.first().copied();
        // After an edit, faces covering a region back to back (a face pulled
        // flush against another) are inside one solid now: both go.
        if fill == Fill::Closing {
            let facing = |f: &FaceId| {
                model
                    .face(*f)
                    .is_some_and(|g| g.plane().normal.dot(frame.normal) > 0.0)
            };
            if covers.iter().any(facing) && !covers.iter().all(facing) {
                continue;
            }
        }
        let closes = loop_pairs(&r.outer)
            .any(|(a, b)| edge_of(a, b).is_some_and(|e| new_edges.contains(&e)));
        if parent.is_none() && (fill == Fill::Closing || !closes) {
            if fill == Fill::Closing && uses_edge(r, new_edges, &edge_of) {
                openings.push(r);
            }
            continue;
        }
        // Unchanged: keep the face as it is.
        if let Some(same) = old.iter().copied().find(|f| same_region(model.face(*f), r)) {
            keep.insert(same);
            continue;
        }
        let up = match parent {
            Some(p) => model
                .face(p)
                .is_some_and(|f| f.plane().normal.dot(frame.normal) > 0.0),
            None => neighbour_facing(model, &old, r, &frame).unwrap_or(true),
        };
        made_faces.push(make_face(model, r, &frame, up));
    }

    if !openings.is_empty() {
        let mut chosen: Vec<Vec<Vec<VertexId>>> = keep
            .iter()
            .filter_map(|f| model.face(*f))
            .map(|f| f.loops().cloned().collect())
            .chain(
                made_faces
                    .iter()
                    .map(|f| f.loops().cloned().collect::<Vec<_>>()),
            )
            .collect();
        let others = other_faces(model, &old);
        // One opening filled can make the next one an opening, so repeat.
        loop {
            let found = openings.iter().position(|r| {
                region_loops(r).flat_map(|l| loop_pairs(l)).all(|(a, b)| {
                    let key = (a.min(b), a.max(b));
                    let beside = others.get(&key).map_or(0, |d| d.len());
                    let inside = chosen
                        .iter()
                        .flatten()
                        .flat_map(|l| loop_pairs(l))
                        .filter(|(x, y)| (*x.min(y), *x.max(y)) == key)
                        .count();
                    beside + inside == 1
                })
            });
            let Some(i) = found else { break };
            let r = openings.remove(i);
            // Face the way that agrees with the faces around it: across a
            // shared edge, neighbours run the edge in opposite directions.
            let votes: i32 = region_loops(r)
                .flat_map(|l| loop_pairs(l))
                .map(|(a, b)| {
                    let key = (a.min(b), a.max(b));
                    others.get(&key).map_or(0, |dirs| {
                        dirs.iter()
                            .map(|from| if *from == a { -1 } else { 1 })
                            .sum::<i32>()
                    })
                })
                .sum();
            let face = make_face(model, r, &frame, votes >= 0);
            chosen.push(face.loops().cloned().collect());
            made_faces.push(face);
        }
    }

    for f in &old {
        if !keep.contains(f) {
            model.remove_face(*f);
        }
    }
    made_faces.into_iter().map(|f| model.add_face(f)).collect()
}

fn region_loops(r: &Region) -> impl Iterator<Item = &Vec<VertexId>> {
    std::iter::once(&r.outer).chain(r.holes.iter())
}

/// Whether any loop of the region runs along one of `edges`.
fn uses_edge(
    r: &Region,
    edges: &BTreeSet<EdgeId>,
    edge_of: &impl Fn(VertexId, VertexId) -> Option<EdgeId>,
) -> bool {
    region_loops(r)
        .flat_map(|l| loop_pairs(l))
        .any(|(a, b)| edge_of(a, b).is_some_and(|e| edges.contains(&e)))
}

/// For each edge (as a sorted vertex pair), the start vertex of each run
/// along it by a face that is not one of `except`.
fn other_faces(model: &Model, except: &[FaceId]) -> BTreeMap<(VertexId, VertexId), Vec<VertexId>> {
    let mut out: BTreeMap<(VertexId, VertexId), Vec<VertexId>> = BTreeMap::new();
    for (id, f) in model.faces() {
        if except.contains(&id) {
            continue;
        }
        for (a, b) in f.loops().flat_map(|l| loop_pairs(l)) {
            out.entry((a.min(b), a.max(b))).or_default().push(a);
        }
    }
    out
}

/// Build a face from a region, facing along the frame normal if `up`.
fn make_face(model: &Model, r: &Region, frame: &Frame, up: bool) -> Face {
    let (mut outer, mut holes) = (r.outer.clone(), r.holes.clone());
    if !up {
        outer.reverse();
        for h in &mut holes {
            h.reverse();
        }
    }
    let pts = model.points(&outer);
    let centroid = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
    let normal = if up { frame.normal } else { -frame.normal };
    Face {
        outer,
        holes,
        surface: Surface::Plane(Plane::new(centroid, normal)),
    }
}

/// Which way a coplanar face sharing an edge with the region faces, if any.
fn neighbour_facing(model: &Model, old: &[FaceId], r: &Region, frame: &Frame) -> Option<bool> {
    let pairs: BTreeSet<(VertexId, VertexId)> = loop_pairs(&r.outer)
        .map(|(a, b)| (a.min(b), a.max(b)))
        .collect();
    old.iter().find_map(|f| {
        let face = model.face(*f)?;
        let shares = face
            .loops()
            .any(|l| loop_pairs(l).any(|(a, b)| pairs.contains(&(a.min(b), a.max(b)))));
        shares.then(|| face.plane().normal.dot(frame.normal) > 0.0)
    })
}

/// Whether a face has exactly the region's loops (any start, either way round).
fn same_region(face: Option<&Face>, r: &Region) -> bool {
    let Some(f) = face else { return false };
    if !same_cycle(&f.outer, &r.outer) || f.holes.len() != r.holes.len() {
        return false;
    }
    let mut used = vec![false; r.holes.len()];
    f.holes.iter().all(|h| {
        r.holes.iter().enumerate().any(|(i, rh)| {
            if !used[i] && same_cycle(h, rh) {
                used[i] = true;
                true
            } else {
                false
            }
        })
    })
}

/// Whether two loops are the same cycle, from any start and in either direction.
pub(crate) fn same_cycle(a: &[VertexId], b: &[VertexId]) -> bool {
    if a.len() != b.len() || a.is_empty() {
        return false;
    }
    let n = a.len();
    let Some(start) = b.iter().position(|v| *v == a[0]) else {
        return false;
    };
    let fwd = (0..n).all(|i| a[i] == b[(start + i) % n]);
    let back = (0..n).all(|i| a[i] == b[(start + n - i) % n]);
    fwd || back
}

/// Whether the 2D point (in `frame`) is inside a face: in its outer loop and
/// not in a hole.
fn face_contains_2d(model: &Model, f: FaceId, frame: &Frame, p: DVec2) -> bool {
    let Some(face) = model.face(f) else {
        return false;
    };
    let poly = |l: &[VertexId]| -> Vec<DVec2> {
        l.iter().map(|v| frame.to_2d(model.vertex(*v))).collect()
    };
    point_in_polygon(p, &poly(&face.outer)) == Inside::Inside
        && face
            .holes
            .iter()
            .all(|h| point_in_polygon(p, &poly(h)) == Inside::Outside)
}

/// Trace the planar graph into regions.
fn trace(
    on: &BTreeMap<VertexId, DVec2>,
    edges: &[(EdgeId, VertexId, VertexId)],
    tol: &Tol,
) -> Vec<Region> {
    // Neighbours of each vertex, sorted counter-clockwise by direction.
    let mut around: BTreeMap<VertexId, Vec<VertexId>> = BTreeMap::new();
    for (_, a, b) in edges {
        around.entry(*a).or_default().push(*b);
        around.entry(*b).or_default().push(*a);
    }
    for (v, list) in around.iter_mut() {
        let o = on[v];
        list.sort_by(|x, y| ccw_order(o, on[x], on[y]));
    }

    // Each half-edge u→v continues to the edge just clockwise of v→u at v,
    // which keeps the region on its left.
    let mut seen: BTreeSet<(VertexId, VertexId)> = BTreeSet::new();
    let mut cycles: Vec<Vec<VertexId>> = Vec::new();
    for (u, list) in &around {
        for v in list {
            if seen.contains(&(*u, *v)) {
                continue;
            }
            let mut cycle = Vec::new();
            let (mut a, mut b) = (*u, *v);
            while seen.insert((a, b)) {
                cycle.push(a);
                let at_b = &around[&b];
                let i = at_b.iter().position(|x| *x == a).unwrap_or(0);
                let next = at_b[(i + at_b.len() - 1) % at_b.len()];
                a = b;
                b = next;
            }
            cycles.push(cycle);
        }
    }

    // Bounded regions wind counter-clockwise; the outside of each connected
    // piece winds clockwise and is a hole of whichever region contains it.
    let mut outers: Vec<(Vec<VertexId>, Vec<DVec2>, f64)> = Vec::new();
    let mut islands: Vec<Vec<VertexId>> = Vec::new();
    for c in cycles {
        let pts: Vec<DVec2> = c.iter().map(|v| on[v]).collect();
        let area = signed_area2(&pts) * 0.5;
        let clean = remove_spurs(c);
        if clean.len() < 3 {
            continue;
        }
        if area > tol.area() {
            let poly = clean.iter().map(|v| on[v]).collect();
            outers.push((clean, poly, area));
        } else if area < -tol.area() {
            islands.push(clean);
        }
    }
    let mut regions: Vec<Region> = outers
        .iter()
        .map(|(l, _, _)| Region {
            outer: l.clone(),
            holes: vec![],
        })
        .collect();
    for island in islands {
        let probe = on[&island[0]];
        let host = outers
            .iter()
            .enumerate()
            .filter(|(_, (l, poly, _))| {
                !l.contains(&island[0]) && point_in_polygon(probe, poly) == Inside::Inside
            })
            .min_by(|a, b| a.1.2.total_cmp(&b.1.2))
            .map(|(i, _)| i);
        if let Some(i) = host {
            regions[i].holes.push(island);
        }
    }
    regions
}

/// Order two directions from `o` counter-clockwise, starting from +x (exact).
fn ccw_order(o: DVec2, p: DVec2, q: DVec2) -> std::cmp::Ordering {
    let half = |d: DVec2| {
        if d.y > 0.0 || (d.y == 0.0 && d.x > 0.0) {
            0
        } else {
            1
        }
    };
    let (hp, hq) = (half(p - o), half(q - o));
    if hp != hq {
        return hp.cmp(&hq);
    }
    let s = orient2d(o, p, q);
    if s > 0.0 {
        std::cmp::Ordering::Less
    } else if s < 0.0 {
        std::cmp::Ordering::Greater
    } else {
        std::cmp::Ordering::Equal
    }
}

/// Remove back-and-forth visits along loose edges inside a region
/// (…, x, y, x, … becomes …, x, …).
fn remove_spurs(mut c: Vec<VertexId>) -> Vec<VertexId> {
    loop {
        let n = c.len();
        if n < 3 {
            return c;
        }
        let spur = (0..n).find(|&i| c[(i + n - 1) % n] == c[(i + 1) % n]);
        match spur {
            Some(i) => {
                // Drop c[i] and the repeated c[i+1].
                let j = (i + 1) % n;
                let (first, second) = if i < j { (j, i) } else { (i, j) };
                c.remove(first);
                c.remove(second);
            }
            None => return c,
        }
    }
}

/// A point strictly inside the first loop and outside the others. A
/// horizontal line between two vertex heights crosses the loops at known
/// places; the middle of the widest inside stretch is used.
pub(crate) fn interior_point(loops: &[Vec<DVec2>]) -> Option<DVec2> {
    let outer = loops.first()?;
    let (lo, hi) = outer
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        });
    let mut ys: Vec<f64> = loops
        .iter()
        .flatten()
        .map(|p| p.y)
        .filter(|y| *y >= lo && *y <= hi)
        .collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup();
    // Try heights between consecutive vertex heights, widest gap first.
    let mut gaps: Vec<(f64, f64)> = ys
        .windows(2)
        .map(|w| (w[1] - w[0], (w[0] + w[1]) * 0.5))
        .collect();
    gaps.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, y) in gaps {
        let mut xs: Vec<f64> = Vec::new();
        for l in loops {
            for i in 0..l.len() {
                let (a, b) = (l[i], l[(i + 1) % l.len()]);
                if (a.y < y) != (b.y < y) {
                    xs.push(a.x + (y - a.y) * (b.x - a.x) / (b.y - a.y));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        let best = xs
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| (p[1] - p[0], (p[0] + p[1]) * 0.5))
            .max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((w, x)) = best
            && w > 0.0
        {
            return Some(DVec2::new(x, y));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spurs_are_removed() {
        let v = |i| VertexId(i);
        assert_eq!(
            remove_spurs(vec![v(0), v(1), v(2), v(9), v(2), v(3)]),
            vec![v(0), v(1), v(2), v(3)]
        );
        assert_eq!(remove_spurs(vec![v(0), v(1), v(0), v(2)]).len(), 2);
    }

    #[test]
    fn cycles_match_from_any_start_either_way() {
        let v = |i| VertexId(i);
        let a = [v(1), v(2), v(3), v(4)];
        assert!(same_cycle(&a, &[v(3), v(4), v(1), v(2)]));
        assert!(same_cycle(&a, &[v(4), v(3), v(2), v(1)]));
        assert!(!same_cycle(&a, &[v(1), v(3), v(2), v(4)]));
    }

    #[test]
    fn interior_points_avoid_holes() {
        let sq = |a: f64, b: f64| {
            vec![
                DVec2::new(a, a),
                DVec2::new(b, a),
                DVec2::new(b, b),
                DVec2::new(a, b),
            ]
        };
        let p = interior_point(&[sq(0.0, 4.0), sq(1.0, 3.0)]).expect("point");
        assert_eq!(point_in_polygon(p, &sq(0.0, 4.0)), Inside::Inside);
        assert_eq!(point_in_polygon(p, &sq(1.0, 3.0)), Inside::Outside);
    }
}
