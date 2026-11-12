//! A mesh to a model: weld vertices, drop degenerate polygons, optionally
//! merge coplanar triangles that share edges into planar faces (with holes),
//! and build the model directly (not through sticky geometry one segment at
//! a time, which is far too slow for big meshes).

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::Surface;
use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3, Plane, Tol, newell};
use prism_topo::{EdgeId, Face, MaterialId, Model, UvMap, VertexId};

use super::{Mesh, into_model};

/// Two neighbouring triangles are one face when their normals are this
/// close (1 − cos), and the neighbour's corners are on the face's plane.
const MERGE_ANGLE: f64 = 1e-10;

/// Build a model from a mesh in file units: `metres_per_unit` scales it,
/// `y_up` turns a Y-up file into Prism's Z up. Materials are left out.
pub fn build(mesh: &Mesh, metres_per_unit: f64, y_up: bool, merge: bool) -> Model {
    let plain = Mesh {
        materials: Vec::new(),
        polygon_materials: Vec::new(),
        uvs: Vec::new(),
        ..mesh.clone()
    };
    build_painted(&plain, metres_per_unit, y_up, merge).0
}

/// The texture placement (metres, Prism's v up) that puts the corners
/// `p` at texture places `uv` (copies of the image, v down), for an image
/// `size` metres a copy. None for a sliver.
fn uv_map(p: [DVec3; 3], uv: [DVec2; 3], size: [f64; 2]) -> Option<UvMap> {
    let t = uv.map(|q| DVec2::new(q.x * size[0], -q.y * size[1]));
    let (e1, e2) = (p[1] - p[0], p[2] - p[0]);
    let (d1, d2) = (t[1] - t[0], t[2] - t[0]);
    let (a, b, c) = (e1.dot(e1), e1.dot(e2), e2.dot(e2));
    let det = a * c - b * b;
    if det <= 1e-24 * a.max(c).powi(2) {
        return None;
    }
    // The gradient g of each coordinate in the plane: g·e1 = d1, g·e2 = d2.
    let grad = |x1: f64, x2: f64| {
        let s = (x1 * c - x2 * b) / det;
        let r = (x2 * a - x1 * b) / det;
        e1 * s + e2 * r
    };
    let (u, v) = (grad(d1.x, d2.x), grad(d1.y, d2.y));
    // The origin: where both are zero, in the plane.
    let (ua, ub, uc) = (u.dot(u), u.dot(v), v.dot(v));
    let det2 = ua * uc - ub * ub;
    if det2 <= 1e-24 * ua.max(uc).powi(2) {
        return None;
    }
    let (x, y) = (t[0].x, t[0].y);
    let s = (x * uc - y * ub) / det2;
    let r = (y * ua - x * ub) / det2;
    let origin = p[0] - (u * s + v * r);
    Some(UvMap { origin, u, v })
}

/// [`build`] with each polygon's material on its face's front, as
/// `MaterialId(k)` for the mesh's material `k`, and textured faces given
/// their placement. Also returns each material's texture size (metres a
/// copy): as the file says, else worked out from its coordinates.
pub fn build_painted(
    mesh: &Mesh,
    metres_per_unit: f64,
    y_up: bool,
    merge: bool,
) -> (Model, Vec<[f64; 2]>) {
    let pts: Vec<DVec3> = mesh
        .positions
        .iter()
        .map(|p| into_model(*p, metres_per_unit, y_up))
        .collect();
    let tol = Tol::for_points(&pts);
    let (welded, map) = weld(&pts, &tol);
    let mat_of = |k: usize| mesh.polygon_materials.get(k).copied().flatten();
    let has_uv = mesh.uvs.len() == mesh.positions.len();
    let corners = |p: &[u32]| -> Option<([DVec3; 3], [DVec2; 3])> {
        if !has_uv || p.len() < 3 {
            return None;
        }
        let i = [p[0], p[1], p[2]].map(|i| i as usize);
        Some((i.map(|i| pts[i]), i.map(|i| mesh.uvs[i])))
    };
    // Each textured material's size: the file's, else one copy's size
    // where it is first used (the coordinates' rate along the face).
    let sizes: Vec<[f64; 2]> = (0..mesh.materials.len())
        .map(|k| {
            let t = mesh.materials[k].texture.as_ref();
            if let Some(s) = t.and_then(|t| t.size) {
                return s;
            }
            let found = t.and_then(|_| {
                (0..mesh.polygons.len())
                    .filter(|j| mat_of(*j) == Some(k as u32))
                    .find_map(|j| {
                        let (p, uv) = corners(&mesh.polygons[j])?;
                        let m = uv_map(p, uv, [1.0, 1.0])?;
                        let s = [1.0 / m.u.length(), 1.0 / m.v.length()];
                        s.iter().all(|x| x.is_finite() && *x > 0.0).then_some(s)
                    })
            });
            found.unwrap_or([1.0, 1.0])
        })
        .collect();
    let textured = |k: Option<u32>| k.filter(|k| mesh.materials[*k as usize].texture.is_some());
    // Polygons on welded vertices, without repeats or zero area, with
    // their material and placement.
    let mut polys: Vec<Vec<usize>> = Vec::new();
    let mut looks: Vec<(Option<u32>, Option<UvMap>)> = Vec::new();
    for (j, p) in mesh.polygons.iter().enumerate() {
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
        let mat = mat_of(j);
        let uv = textured(mat).and_then(|k| {
            let (c, uv) = corners(p)?;
            uv_map(c, uv, sizes[k as usize])
        });
        polys.push(q);
        looks.push((mat, uv));
    }
    // Neighbours join only with the same material and placement.
    let same = |a: usize, b: usize| {
        let ((ma, ua), (mb, ub)) = (looks[a], looks[b]);
        ma == mb
            && match (ua, ub) {
                (None, None) => true,
                (Some(x), Some(y)) => polys[b].iter().all(|i| {
                    let (p, q) = (x.at(welded[*i]), y.at(welded[*i]));
                    p.distance(q) <= 1e-6 * (1.0 + p.length())
                }),
                _ => false,
            }
    };
    let regions: Vec<(Region, usize)> = if merge {
        merged(&welded, &polys, &tol, same)
    } else {
        (0..polys.len())
            .map(|k| ((polys[k].clone(), Vec::new()), k))
            .collect()
    };
    let mut m = Model::new();
    let ids: Vec<VertexId> = welded.iter().map(|p| m.add_vertex(*p)).collect();
    let mut used = vec![false; welded.len()];
    let mut edges: BTreeMap<(usize, usize), EdgeId> = BTreeMap::new();
    for ((outer, holes), seed) in &regions {
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
        let mut face = Face::with_edges(
            outer.iter().map(|i| ids[*i]).collect(),
            holes
                .iter()
                .map(|h| h.iter().map(|i| ids[*i]).collect())
                .collect(),
            Surface::Plane(Plane::new(centre, n)),
            loop_edges,
        );
        let (mat, uv) = looks[*seed];
        face.paint.front = mat.map(MaterialId);
        face.paint.uv = uv;
        m.add_face(face);
    }
    for (k, id) in ids.iter().enumerate() {
        if !used[k] {
            m.remove_vertex(*id);
        }
    }
    (m, sizes)
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
fn merged(
    pts: &[DVec3],
    polys: &[Vec<usize>],
    tol: &Tol,
    same: impl Fn(usize, usize) -> bool,
) -> Vec<(Region, usize)> {
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
                        || !same(seed, j)
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
        out.push((
            region_loops(pts, polys, &members, n).unwrap_or_else(|| {
                // A region that does not make one clean face stays as it was.
                (polys[seed].clone(), Vec::new())
            }),
            seed,
        ));
        // Members of a region that fell back become their own faces.
        if out
            .last()
            .is_some_and(|r| r.0.0 == polys[seed] && members.len() > 1)
        {
            out.pop();
            for &k in &members {
                out.push(((polys[k].clone(), Vec::new()), k));
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
