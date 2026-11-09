//! A mesh to a model: weld vertices, drop degenerate polygons, optionally
//! merge coplanar triangles that share edges into planar faces (with holes),
//! and build the model directly (not through sticky geometry one segment at
//! a time, which is far too slow for big meshes).

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::Surface;
use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3, Plane, Tol, newell};
use prism_topo::{EdgeId, Face, Model, VertexId};

use super::{Mesh, into_model};

/// Two neighbouring triangles are one face when their normals are this
/// close (1 − cos), and the neighbour's corners are on the face's plane.
const MERGE_ANGLE: f64 = 1e-10;

/// Build a model from a mesh in file units: `metres_per_unit` scales it,
/// `y_up` turns a Y-up file into Prism's Z up.
pub fn build(mesh: &Mesh, metres_per_unit: f64, y_up: bool, merge: bool) -> Model {
    let pts: Vec<DVec3> = mesh
        .positions
        .iter()
        .map(|p| into_model(*p, metres_per_unit, y_up))
        .collect();
    let tol = Tol::for_points(&pts);
    let (welded, map) = weld(&pts, &tol);
    // Polygons on welded vertices, without repeats or zero area.
    let mut polys: Vec<Vec<usize>> = Vec::new();
    for p in &mesh.polygons {
        let mut q: Vec<usize> = p.iter().map(|i| map[*i as usize]).collect();
        q.dedup();
        while q.len() > 1 && q.first() == q.last() {
            q.pop();
        }
        let distinct: BTreeSet<usize> = q.iter().copied().collect();
        if distinct.len() < 3 || distinct.len() != q.len() {
            continue;
        }
        let ring: Vec<DVec3> = q.iter().map(|i| welded[*i]).collect();
        if newell(&ring).length() * 0.5 <= tol.area() {
            continue;
        }
        polys.push(q);
    }
    let regions = if merge {
        merged(&welded, &polys, &tol)
    } else {
        polys.into_iter().map(|p| (p, Vec::new())).collect()
    };
    let mut m = Model::new();
    let ids: Vec<VertexId> = welded.iter().map(|p| m.add_vertex(*p)).collect();
    let mut used = vec![false; welded.len()];
    let mut edges: BTreeMap<(usize, usize), EdgeId> = BTreeMap::new();
    for (outer, holes) in &regions {
        let mut loop_edges = Vec::new();
        for l in std::iter::once(outer).chain(holes.iter()) {
            let mut le = Vec::new();
            for k in 0..l.len() {
                let (a, b) = (l[k], l[(k + 1) % l.len()]);
                let e = *edges
                    .entry((a.min(b), a.max(b)))
                    .or_insert_with(|| m.add_new_edge(ids[a], ids[b]));
                le.push(e);
                used[a] = true;
            }
            loop_edges.push(le);
        }
        let ring: Vec<DVec3> = outer.iter().map(|i| welded[*i]).collect();
        let n = newell(&ring).normalize_or_zero();
        let centre = ring.iter().copied().sum::<DVec3>() / ring.len() as f64;
        m.add_face(Face::with_edges(
            outer.iter().map(|i| ids[*i]).collect(),
            holes
                .iter()
                .map(|h| h.iter().map(|i| ids[*i]).collect())
                .collect(),
            Surface::Plane(Plane::new(centre, n)),
            loop_edges,
        ));
    }
    for (k, id) in ids.iter().enumerate() {
        if !used[k] {
            m.remove_vertex(*id);
        }
    }
    m
}

/// Points within the tolerance become one. Returns the points kept and,
/// for each input point, the one it became.
fn weld(pts: &[DVec3], tol: &Tol) -> (Vec<DVec3>, Vec<usize>) {
    let cell = tol.dist * 2.0;
    let key = |p: DVec3| {
        (
            (p.x / cell).floor() as i64,
            (p.y / cell).floor() as i64,
            (p.z / cell).floor() as i64,
        )
    };
    let mut grid: BTreeMap<(i64, i64, i64), Vec<usize>> = BTreeMap::new();
    let mut kept: Vec<DVec3> = Vec::new();
    let mut map = Vec::with_capacity(pts.len());
    for p in pts {
        let (x, y, z) = key(*p);
        let mut found = None;
        'look: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let cell = (
                        x.saturating_add(dx),
                        y.saturating_add(dy),
                        z.saturating_add(dz),
                    );
                    for &k in grid.get(&cell).into_iter().flatten() {
                        if kept[k].distance(*p) < tol.dist {
                            found = Some(k);
                            break 'look;
                        }
                    }
                }
            }
        }
        let k = found.unwrap_or_else(|| {
            kept.push(*p);
            grid.entry((x, y, z)).or_default().push(kept.len() - 1);
            kept.len() - 1
        });
        map.push(k);
    }
    (kept, map)
}

type Region = (Vec<usize>, Vec<Vec<usize>>);

/// Merge polygons that share an edge and lie on one plane into faces.
fn merged(pts: &[DVec3], polys: &[Vec<usize>], tol: &Tol) -> Vec<Region> {
    let normal = |p: &Vec<usize>| {
        let ring: Vec<DVec3> = p.iter().map(|i| pts[*i]).collect();
        newell(&ring).normalize_or_zero()
    };
    let normals: Vec<DVec3> = polys.iter().map(normal).collect();
    let mut by_edge: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    for (k, p) in polys.iter().enumerate() {
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            by_edge.entry((a.min(b), a.max(b))).or_default().push(k);
        }
    }
    let mut region_of = vec![usize::MAX; polys.len()];
    let mut out = Vec::new();
    for seed in 0..polys.len() {
        if region_of[seed] != usize::MAX {
            continue;
        }
        let n = normals[seed];
        let plane = Plane::new(pts[polys[seed][0]], n);
        let id = out.len();
        let mut members = vec![seed];
        region_of[seed] = id;
        let mut stack = vec![seed];
        while let Some(k) = stack.pop() {
            let p = &polys[k];
            for i in 0..p.len() {
                let (a, b) = (p[i], p[(i + 1) % p.len()]);
                let Some(next) = by_edge.get(&(a.min(b), a.max(b))) else {
                    continue;
                };
                // Only an edge with exactly two polygons joins them.
                if next.len() != 2 {
                    continue;
                }
                for &j in next {
                    if region_of[j] != usize::MAX
                        || 1.0 - normals[j].dot(n) > MERGE_ANGLE
                        || polys[j]
                            .iter()
                            .any(|v| plane.distance(pts[*v]).abs() > tol.dist)
                    {
                        continue;
                    }
                    region_of[j] = id;
                    members.push(j);
                    stack.push(j);
                }
            }
        }
        out.push(region_loops(pts, polys, &members, n).unwrap_or_else(|| {
            // A region that does not make one clean face stays as it was.
            (polys[seed].clone(), Vec::new())
        }));
        // Members of a region that fell back become their own faces.
        if out
            .last()
            .is_some_and(|r| r.0 == polys[seed] && members.len() > 1)
        {
            out.pop();
            for &k in &members {
                out.push((polys[k].clone(), Vec::new()));
            }
        }
    }
    out
}

/// The outline and holes of a set of polygons: the edges used once, in
/// order. None if they do not make one outline with holes inside it.
fn region_loops(
    pts: &[DVec3],
    polys: &[Vec<usize>],
    members: &[usize],
    n: DVec3,
) -> Option<Region> {
    if members.len() == 1 {
        return Some((polys[members[0]].clone(), Vec::new()));
    }
    let mut count: BTreeMap<(usize, usize), i32> = BTreeMap::new();
    let mut directed: Vec<(usize, usize)> = Vec::new();
    for &k in members {
        let p = &polys[k];
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            *count.entry((a.min(b), a.max(b))).or_default() += 1;
            directed.push((a, b));
        }
    }
    let mut next: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (a, b) in directed {
        if count[&(a.min(b), a.max(b))] == 1 {
            next.entry(a).or_default().push(b);
        }
    }
    // A vertex where the boundary touches itself: not one clean face.
    if next.values().any(|v| v.len() != 1) {
        return None;
    }
    let mut loops: Vec<Vec<usize>> = Vec::new();
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    for &start in next.keys() {
        if seen.contains(&start) {
            continue;
        }
        let mut l = vec![start];
        seen.insert(start);
        let mut at = start;
        loop {
            let b = *next.get(&at)?.first()?;
            if b == start {
                break;
            }
            if !seen.insert(b) {
                return None;
            }
            l.push(b);
            at = b;
        }
        if l.len() < 3 {
            return None;
        }
        loops.push(l);
    }
    // The outline turns with the normal; holes turn against it.
    let frame = Plane::new(pts[loops[0][0]], n).frame();
    let flip = frame.normal.dot(n) < 0.0;
    let flat = |l: &Vec<usize>| -> Vec<DVec2> { l.iter().map(|i| frame.to_2d(pts[*i])).collect() };
    let area = |l: &Vec<usize>| {
        let a = prism_math::predicates::signed_area2(&flat(l));
        if flip { -a } else { a }
    };
    let (outers, holes): (Vec<_>, Vec<_>) = loops.into_iter().partition(|l| area(l) > 0.0);
    if outers.len() != 1 {
        return None;
    }
    let outer = outers.into_iter().next()?;
    let o2 = flat(&outer);
    if holes
        .iter()
        .any(|h| point_in_polygon(frame.to_2d(pts[h[0]]), &o2) == Inside::Outside)
    {
        return None;
    }
    Some((outer, holes))
}
