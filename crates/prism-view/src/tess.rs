//! Display triangulation of planar faces with holes (PLAN 1.1: for drawing
//! only, never stored in the model).
//!
//! Ear clipping in the face's own 2D frame. Holes are first joined to the
//! outer loop by bridge edges, which turns the face into one loop that
//! touches itself along each bridge. Every orientation and inside test uses
//! the exact predicates, so a face with collinear vertices (common, since
//! sticky geometry splits edges) or a hole that lines up with a corner
//! still triangulates correctly.

use prism_math::DVec2;
use prism_math::predicates::{orient2d, signed_area2};

/// Triangles as indices into the outer loop followed by each hole, in that
/// order. Every triangle turns counter-clockwise in the 2D frame.
pub fn triangulate(outer: &[DVec2], holes: &[Vec<DVec2>]) -> Vec<[usize; 3]> {
    if outer.len() < 3 {
        return Vec::new();
    }
    let mut pts: Vec<DVec2> = outer.to_vec();
    let mut ring: Vec<usize> = (0..outer.len()).collect();
    if signed_area2(outer) < 0.0 {
        ring.reverse();
    }
    // Holes: clockwise, then bridged in from the one furthest right.
    let mut hole_rings: Vec<Vec<usize>> = Vec::new();
    for h in holes {
        let start = pts.len();
        pts.extend_from_slice(h);
        if h.len() < 3 {
            continue;
        }
        let mut r: Vec<usize> = (start..start + h.len()).collect();
        if signed_area2(h) > 0.0 {
            r.reverse();
        }
        hole_rings.push(r);
    }
    let key = |r: &Vec<usize>| {
        r.iter()
            .map(|&i| pts[i].x)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    hole_rings.sort_by(|a, b| key(b).total_cmp(&key(a)));
    for k in 0..hole_rings.len() {
        let (done, rest) = hole_rings.split_at(k + 1);
        let hole = &done[k];
        ring = bridge(&pts, ring, hole, rest);
    }
    clip(&pts, &ring)
}

/// A cone, sphere or torus face drawn (Exact Curves 5b): points on the
/// exact surface with its normals there, and triangles that turn
/// counter-clockwise seen from the face's front.
#[derive(Clone, Debug, Default)]
pub struct Turned {
    pub points: Vec<prism_math::DVec3>,
    pub normals: Vec<prism_math::DVec3>,
    /// Each point's (θ, s) on the surface, without jumps across the seam
    /// (for texture coordinates).
    pub params: Vec<DVec2>,
    pub tris: Vec<[usize; 3]>,
}

/// Draw a face on a turned surface from a grid: the face's loops as (θ, s)
/// places (`Model::param_loop`) are triangulated, and each triangle is cut
/// by a grid of θ (and, for a profile circle, of s) so no piece spans much
/// of the curve. Every corner is then put on the surface. None for a flat
/// face or a cylinder (cylinders are drawn unrolled).
pub fn turned(model: &prism_topo::Model, face: &prism_topo::Face) -> Option<Turned> {
    turned_with(model, face, true)
}

/// [`turned`], with the fast path for rectangles in (θ, s) on or off (off
/// only to test the general path).
#[doc(hidden)]
pub fn turned_with(
    model: &prism_topo::Model,
    face: &prism_topo::Face,
    fast: bool,
) -> Option<Turned> {
    use prism_geom::{Profile, Surface};
    if matches!(face.surface, Surface::Plane(_) | Surface::Cylinder(_)) {
        return None;
    }
    let rev = face.surface.revolution()?;
    let loops: Vec<Vec<DVec2>> = (0..face.loops().count())
        .map(|k| model.param_loop(face, k).into_iter().map(|x| x.1).collect())
        .collect();
    let outer = loops.first()?;
    let pts: Vec<DVec2> = loops.iter().flatten().copied().collect();
    // The grid spans the face's own extent in equal steps no longer than
    // a 64th of a turn, so a face bounded by circles and profile lines
    // (every face Follow Me makes) is whole cells. A cone's profile is
    // straight: one row.
    let (mut lo, mut hi) = (outer[0], outer[0]);
    for q in outer {
        lo = lo.min(*q);
        hi = hi.max(*q);
    }
    // As many steps as an arc of that sweep is drawn with, so the grid's
    // lines meet the face's edges at the edges' own drawn points.
    let cols = prism_geom::segments(hi.x - lo.x);
    let rows = match rev.profile {
        Profile::Line { .. } => 1,
        Profile::Circle { .. } => prism_geom::segments(hi.y - lo.y),
    };
    let (dx, dy) = ((hi.x - lo.x) / cols as f64, (hi.y - lo.y) / rows as f64);
    let cell_of = |q: DVec2| {
        let i = (((q.x - lo.x) / dx).floor().max(0.0) as usize).min(cols - 1);
        let j = (((q.y - lo.y) / dy).floor().max(0.0) as usize).min(rows - 1);
        (i, j)
    };
    let edge = |i: usize, n: usize, lo: f64, d: f64, hi: f64| {
        if i == n { hi } else { lo + d * i as f64 }
    };
    let bounds = |i: usize, j: usize| {
        (
            edge(i, cols, lo.x, dx, hi.x),
            edge(i + 1, cols, lo.x, dx, hi.x),
            edge(j, rows, lo.y, dy, hi.y),
            edge(j + 1, rows, lo.y, dy, hi.y),
        )
    };
    // The face's outline as drawn: a grid point on it takes the outline's
    // own point, so the faces either side share points exactly (no cracks).
    let outline: Vec<(DVec2, prism_math::DVec3)> = (0..face.loops().count())
        .flat_map(|k| model.param_loop(face, k))
        .map(|(p, q)| (q, p))
        .collect();
    let eps = model.tol().dist;
    let mut bins: std::collections::BTreeMap<(usize, usize), Vec<prism_math::DVec3>> =
        std::collections::BTreeMap::new();
    let mut poles: Vec<prism_math::DVec3> = Vec::new();
    for (q, p) in &outline {
        bins.entry(cell_of(*q)).or_default().push(*p);
        if rev.axis_distance(*p) <= eps {
            poles.push(*p);
        }
    }
    let place = |q: DVec2| -> prism_math::DVec3 {
        let p = rev.point(q.x, q.y);
        let (i, j) = cell_of(q);
        let near = |x: &&prism_math::DVec3| x.distance(p) <= eps;
        for di in [0, 1, 2] {
            for dj in [0, 1, 2] {
                let (Some(a), Some(b)) = ((i + di).checked_sub(1), (j + dj).checked_sub(1)) else {
                    continue;
                };
                if let Some(x) = bins.get(&(a, b)).and_then(|v| v.iter().find(near)) {
                    return *x;
                }
            }
        }
        poles.iter().find(near).copied().unwrap_or(p)
    };
    let mut out = Turned::default();
    let flip = !face.surface.outward();
    let normal = |q: DVec2| {
        let n = rev.normal(q.x, q.y);
        if flip { -n } else { n }
    };
    // Faces round the axis and along the profile, with no holes, are
    // rectangles in (θ, s) (every face Follow Me makes): one shared grid,
    // with only its rim matched to the outline.
    let w = 2.0 * (hi.x - lo.x) * (hi.y - lo.y);
    if fast && loops.len() == 1 && (signed_area2(outer).abs() - w).abs() <= 1e-9 * w {
        let at = |i: usize, j: usize| {
            DVec2::new(edge(i, cols, lo.x, dx, hi.x), edge(j, rows, lo.y, dy, hi.y))
        };
        for j in 0..=rows {
            for i in 0..=cols {
                let q = at(i, j);
                let rim = i == 0 || j == 0 || i == cols || j == rows;
                out.points
                    .push(if rim { place(q) } else { rev.point(q.x, q.y) });
                out.normals.push(normal(q));
                out.params.push(q);
            }
        }
        let id = |i: usize, j: usize| j * (cols + 1) + i;
        for j in 0..rows {
            for i in 0..cols {
                let quad = [id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)];
                for t in [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]] {
                    let [a, b, c] = t.map(|k| out.points[k]);
                    let n = (b - a).cross(c - a);
                    if n.length_squared() == 0.0 {
                        continue; // two corners at a pole
                    }
                    let front = out.normals[t[0]] + out.normals[t[1]] + out.normals[t[2]];
                    out.tris.push(if n.dot(front) < 0.0 {
                        [t[0], t[2], t[1]]
                    } else {
                        t
                    });
                }
            }
        }
        return Some(out);
    }
    // Each triangle of the face cut into the cells it covers.
    let mut cells: std::collections::BTreeMap<(usize, usize), Vec<Vec<DVec2>>> =
        std::collections::BTreeMap::new();
    for t in triangulate(outer, &loops[1..]) {
        let tri = t.map(|i| pts[i]);
        let a = cell_of(tri[0].min(tri[1]).min(tri[2]));
        let b = cell_of(tri[0].max(tri[1]).max(tri[2]));
        for i in a.0..=b.0 {
            for j in a.1..=b.1 {
                let (x0, x1, y0, y1) = bounds(i, j);
                let piece = clip_box(&tri, x0, x1, y0, y1);
                if piece.len() >= 3 && signed_area2(&piece) > 0.0 {
                    cells.entry((i, j)).or_default().push(piece);
                }
            }
        }
    }
    let emit = |poly: &[DVec2], out: &mut Turned| {
        let base = out.points.len();
        for q in poly {
            out.points.push(place(*q));
            out.normals.push(normal(*q));
            out.params.push(*q);
        }
        for k in 1..poly.len() - 1 {
            let mut t = [base, base + k, base + k + 1];
            let [a, b, c] = t.map(|i| out.points[i]);
            if (b - a).cross(c - a).length_squared() == 0.0 {
                continue; // two corners at a pole
            }
            // Near a pole the map to space pinches, and a sliver can come
            // out turned over: face it the surface's way.
            let front = out.normals[t[0]] + out.normals[t[1]] + out.normals[t[2]];
            if (b - a).cross(c - a).dot(front) < 0.0 {
                t.swap(1, 2);
            }
            out.tris.push(t);
        }
    };
    for ((i, j), pieces) in cells {
        let (x0, x1, y0, y1) = bounds(i, j);
        let area: f64 = pieces.iter().map(|p| signed_area2(p)).sum();
        let full = 2.0 * (x1 - x0) * (y1 - y0);
        if (area - full).abs() <= 1e-9 * full {
            // A whole cell: one clean quad.
            let quad = [
                DVec2::new(x0, y0),
                DVec2::new(x1, y0),
                DVec2::new(x1, y1),
                DVec2::new(x0, y1),
            ];
            emit(&quad, &mut out);
        } else {
            for p in &pieces {
                emit(p, &mut out);
            }
        }
    }
    Some(out)
}

/// A convex polygon clipped to the box `x0..x1`, `y0..y1` (Sutherland and
/// Hodgman).
fn clip_box(poly: &[DVec2], x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<DVec2> {
    let mut p = poly.to_vec();
    let sides: [(usize, f64, bool); 4] =
        [(0, x0, true), (0, x1, false), (1, y0, true), (1, y1, false)];
    for (axis, at, above) in sides {
        if !at.is_finite() || p.is_empty() {
            continue;
        }
        let keep = |q: DVec2| if above { q[axis] >= at } else { q[axis] <= at };
        let mut next = Vec::with_capacity(p.len() + 2);
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            if keep(a) {
                next.push(a);
            }
            if keep(a) != keep(b) {
                let t = (at - a[axis]) / (b[axis] - a[axis]);
                let mut q = a.lerp(b, t);
                q[axis] = at;
                next.push(q);
            }
        }
        p = next;
    }
    p
}

/// Join one hole to the ring with a bridge from the hole's rightmost vertex
/// to the nearest ring vertex it can see.
/// [`triangulate`] for a curved face unrolled flat (Exact Curves part 4),
/// with every triangle spanning as little of the curve as the points
/// allow: a triangle spanning much of it would cut through the curved
/// surface on its chord. Triangles index the points as given and turn
/// counter-clockwise in the flat frame.
///
/// A loop that runs one way round and comes back (a cylinder face pulled
/// from an arc: two chains along the curve, joined by straight seams) is
/// zipped from one chain to the other. Any other shape is triangulated
/// with each straight run bowed a little into the face first: unrolled,
/// an arc's points are in a line that is not a line on the surface, and
/// bowed, no triangle can join two of them that are not neighbours.
pub fn triangulate_unrolled(outer: &[DVec2], holes: &[Vec<DVec2>]) -> Vec<[usize; 3]> {
    if holes.is_empty()
        && let Some(t) = zip(outer)
    {
        return t;
    }
    // The face is on the left of a counter-clockwise outer loop and of a
    // clockwise hole.
    let o = bow(outer, signed_area2(outer) > 0.0);
    let hs: Vec<Vec<DVec2>> = holes
        .iter()
        .map(|h| bow(h, signed_area2(h) < 0.0))
        .collect();
    triangulate(&o, &hs)
}

/// Triangles between the two chains of a loop that runs along x and back
/// (x never turns back within a chain), or None if it does not.
fn zip(l: &[DVec2]) -> Option<Vec<[usize; 3]>> {
    let n = l.len();
    if n < 3 {
        return None;
    }
    // Walk counter-clockwise, from the lowest of the leftmost points.
    let ccw = signed_area2(l) > 0.0;
    let start = (0..n).min_by(|a, b| {
        l[*a]
            .x
            .total_cmp(&l[*b].x)
            .then(l[*a].y.total_cmp(&l[*b].y))
    })?;
    let walk: Vec<usize> = (0..n)
        .map(|k| {
            if ccw {
                (start + k) % n
            } else {
                (start + n - k) % n
            }
        })
        .collect();
    // Along x (the lower chain), then back (the upper one).
    let mut turn = 0;
    while turn + 1 < n && l[walk[turn + 1]].x >= l[walk[turn]].x {
        turn += 1;
    }
    let lower: Vec<usize> = walk[..=turn].to_vec();
    let mut upper: Vec<usize> = walk[turn..].to_vec();
    upper.push(start);
    if upper.windows(2).any(|w| l[w[1]].x > l[w[0]].x) {
        return None;
    }
    upper.reverse();
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::with_capacity(n);
    while i + 1 < lower.len() || j + 1 < upper.len() {
        let lower_next = (i + 1 < lower.len()).then(|| l[lower[i + 1]].x);
        let upper_next = (j + 1 < upper.len()).then(|| l[upper[j + 1]].x);
        let step_lower = match (lower_next, upper_next) {
            (Some(a), Some(b)) => a < b || (a == b && i <= j),
            (Some(_), None) => true,
            _ => false,
        };
        let t = if step_lower {
            i += 1;
            [lower[i - 1], lower[i], upper[j]]
        } else {
            j += 1;
            [lower[i], upper[j], upper[j - 1]]
        };
        if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
            out.push(t);
        }
    }
    Some(out)
}

/// A loop with each straight run of three or more points bowed towards
/// the face (`face_left`: the face is on its left).
fn bow(l: &[DVec2], face_left: bool) -> Vec<DVec2> {
    let n = l.len();
    let mut out = l.to_vec();
    if n < 3 {
        return out;
    }
    let corner = |i: usize| {
        let (a, b, c) = (l[(i + n - 1) % n], l[i], l[(i + 1) % n]);
        let (u, v) = (b - a, c - b);
        u.perp_dot(v).abs() > 1e-9 * u.length() * v.length() || u.dot(v) <= 0.0
    };
    let corners: Vec<usize> = (0..n).filter(|i| corner(*i)).collect();
    if corners.len() < 2 {
        return out;
    }
    let (lo, hi) = l
        .iter()
        .fold((l[0], l[0]), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let span = (hi - lo).min_element();
    for (k, &c) in corners.iter().enumerate() {
        let next = corners[(k + 1) % corners.len()];
        let (p, q) = (l[c], l[next]);
        let len = p.distance(q);
        if len <= 0.0 {
            continue;
        }
        let d = (q - p) / len;
        let into = if face_left {
            DVec2::new(-d.y, d.x)
        } else {
            DVec2::new(d.y, -d.x)
        };
        let amp = 0.04 * len.min(span);
        let mut j = (c + 1) % n;
        while j != next {
            let t = l[j].distance(p) / len;
            out[j] += into * amp * t * (1.0 - t);
            j = (j + 1) % n;
        }
    }
    out
}

fn bridge(pts: &[DVec2], ring: Vec<usize>, hole: &[usize], later: &[Vec<usize>]) -> Vec<usize> {
    let hm = (0..hole.len())
        .max_by(|&a, &b| {
            let (pa, pb) = (pts[hole[a]], pts[hole[b]]);
            pa.x.total_cmp(&pb.x).then(pb.y.total_cmp(&pa.y))
        })
        .expect("a hole has vertices");
    let m = pts[hole[hm]];
    let n = ring.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        pts[ring[a]]
            .distance_squared(m)
            .total_cmp(&pts[ring[b]].distance_squared(m))
            .then(a.cmp(&b))
    });
    // Every edge the bridge must not cross.
    let mut edges: Vec<(DVec2, DVec2)> = Vec::with_capacity(n + hole.len());
    let loop_edges = |r: &[usize], out: &mut Vec<(DVec2, DVec2)>| {
        for i in 0..r.len() {
            out.push((pts[r[i]], pts[r[(i + 1) % r.len()]]));
        }
    };
    loop_edges(&ring, &mut edges);
    loop_edges(hole, &mut edges);
    for h in later {
        loop_edges(h, &mut edges);
    }
    let chosen = order
        .iter()
        .copied()
        .find(|&i| {
            let v = pts[ring[i]];
            if v == m {
                return true;
            }
            let a = pts[ring[(i + n - 1) % n]];
            let c = pts[ring[(i + 1) % n]];
            in_wedge(a, v, c, m) && edges.iter().all(|&(p, q)| !blocks(m, v, p, q))
        })
        // No clear bridge (bad input): use the nearest vertex anyway.
        .unwrap_or(order[0]);
    let mut out = Vec::with_capacity(n + hole.len() + 2);
    out.extend_from_slice(&ring[..=chosen]);
    for k in 0..=hole.len() {
        out.push(hole[(hm + k) % hole.len()]);
    }
    out.push(ring[chosen]);
    out.extend_from_slice(&ring[chosen + 1..]);
    out
}

/// Whether direction `m` from `v` points into the polygon's interior at the
/// corner `a`, `v`, `c` (interior on the left of a counter-clockwise ring).
fn in_wedge(a: DVec2, v: DVec2, c: DVec2, m: DVec2) -> bool {
    let left_ab = orient2d(a, v, m) > 0.0;
    let left_bc = orient2d(v, c, m) > 0.0;
    if orient2d(a, v, c) >= 0.0 {
        left_ab && left_bc
    } else {
        left_ab || left_bc
    }
}

/// Whether edge `p`–`q` blocks the bridge `m`–`v`: they cross, or the edge
/// touches the bridge anywhere except at the bridge's own ends.
fn blocks(m: DVec2, v: DVec2, p: DVec2, q: DVec2) -> bool {
    let at_end = |x: DVec2| x == m || x == v;
    if at_end(p) && at_end(q) {
        return false;
    }
    let o1 = orient2d(m, v, p);
    let o2 = orient2d(m, v, q);
    let o3 = orient2d(p, q, m);
    let o4 = orient2d(p, q, v);
    if o1 * o2 < 0.0 && o3 * o4 < 0.0 {
        return true;
    }
    // Touching: an edge end lying on the bridge (not at its ends), or a
    // bridge end lying inside the edge.
    let on = |a: DVec2, b: DVec2, x: DVec2| {
        orient2d(a, b, x) == 0.0
            && x.x >= a.x.min(b.x)
            && x.x <= a.x.max(b.x)
            && x.y >= a.y.min(b.y)
            && x.y <= a.y.max(b.y)
    };
    (!at_end(p) && on(m, v, p))
        || (!at_end(q) && on(m, v, q))
        || (p != m && q != m && on(p, q, m))
        || (p != v && q != v && on(p, q, v))
}

/// Ear clipping of one counter-clockwise ring (which may touch itself along
/// bridges).
fn clip(pts: &[DVec2], ring: &[usize]) -> Vec<[usize; 3]> {
    let n = ring.len();
    let mut out = Vec::with_capacity(n.saturating_sub(2));
    if n < 3 {
        return out;
    }
    let mut next: Vec<usize> = (0..n).map(|i| (i + 1) % n).collect();
    let mut prev: Vec<usize> = (0..n).map(|i| (i + n - 1) % n).collect();
    let p = |i: usize| pts[ring[i]];
    let turn = |prev: &[usize], next: &[usize], i: usize| orient2d(p(prev[i]), p(i), p(next[i]));
    let mut reflex: Vec<bool> = (0..n).map(|i| turn(&prev, &next, i) < 0.0).collect();
    let mut left = n;
    let mut cur = 0usize;
    let mut stuck = 0usize;
    while left > 3 {
        let (a, c) = (prev[cur], next[cur]);
        let o = orient2d(p(a), p(cur), p(c));
        let remove = |cur: usize, next: &mut Vec<usize>, prev: &mut Vec<usize>| {
            let (a, c) = (prev[cur], next[cur]);
            next[a] = c;
            prev[c] = a;
        };
        if o == 0.0 {
            // A straight (or zero-width) corner: drop it, no triangle.
            remove(cur, &mut next, &mut prev);
            left -= 1;
            reflex[a] = turn(&prev, &next, a) < 0.0;
            reflex[c] = turn(&prev, &next, c) < 0.0;
            cur = a;
            stuck = 0;
            continue;
        }
        let ear = o > 0.0 && is_ear(pts, ring, &next, &reflex, a, cur, c);
        if ear || (stuck > left && o > 0.0) {
            out.push([ring[a], ring[cur], ring[c]]);
            remove(cur, &mut next, &mut prev);
            left -= 1;
            reflex[a] = turn(&prev, &next, a) < 0.0;
            reflex[c] = turn(&prev, &next, c) < 0.0;
            cur = c;
            stuck = 0;
            continue;
        }
        cur = next[cur];
        stuck += 1;
        if stuck > 2 * left + 2 {
            // Nothing convex is left (the input was not a simple polygon).
            break;
        }
    }
    if left == 3 {
        let a = prev[cur];
        let c = next[cur];
        if orient2d(p(a), p(cur), p(c)) > 0.0 {
            out.push([ring[a], ring[cur], ring[c]]);
        }
    }
    out
}

/// Whether no reflex vertex lies in the triangle `a`, `b`, `c` (or on its
/// boundary), apart from copies of its own corners.
fn is_ear(
    pts: &[DVec2],
    ring: &[usize],
    next: &[usize],
    reflex: &[bool],
    a: usize,
    b: usize,
    c: usize,
) -> bool {
    let (pa, pb, pc) = (pts[ring[a]], pts[ring[b]], pts[ring[c]]);
    let mut i = next[c];
    while i != a {
        if reflex[i] {
            let q = pts[ring[i]];
            if q != pa
                && q != pb
                && q != pc
                && orient2d(pa, pb, q) >= 0.0
                && orient2d(pb, pc, q) >= 0.0
                && orient2d(pc, pa, q) >= 0.0
            {
                return false;
            }
        }
        i = next[i];
    }
    true
}

/// A freeform face drawn (Freeform NURBS part 2): a grid over the patch,
/// finer where the surface bends (`NurbsSurface::grid`), each point on the
/// surface with its normal. `params` are distances over the surface in
/// metres (along its middle lines), for texture coordinates.
pub fn patch(p: &prism_geom::Patch) -> Turned {
    let s = &p.surface;
    let us = s.grid(0, p.u.0, p.u.1, p.v);
    let vs = s.grid(1, p.v.0, p.v.1, p.u);
    let (nu, nv) = (us.len(), vs.len());
    let mut out = Turned::default();
    for u in &us {
        for v in &vs {
            out.points.push(s.eval(*u, *v));
            out.normals.push(p.normal(*u, *v));
        }
    }
    // Distance along the middle row and the middle column.
    let along = |n: usize, at: &dyn Fn(usize) -> usize| -> Vec<f64> {
        let mut d = vec![0.0];
        for k in 1..n {
            let step = out.points[at(k - 1)].distance(out.points[at(k)]);
            d.push(d[k - 1] + step);
        }
        d
    };
    let du = along(nu, &|i| i * nv + nv / 2);
    let dv = along(nv, &|j| (nu / 2) * nv + j);
    for x in &du {
        for y in &dv {
            out.params.push(DVec2::new(*x, *y));
        }
    }
    // Counter-clockwise seen from the front.
    let forward = ((p.u.1 >= p.u.0) == (p.v.1 >= p.v.0)) != p.flipped;
    for i in 0..nu - 1 {
        for j in 0..nv - 1 {
            let (a, b, c, d) = (
                i * nv + j,
                (i + 1) * nv + j,
                (i + 1) * nv + j + 1,
                i * nv + j + 1,
            );
            // Split along the shorter diagonal.
            let ac = out.points[a].distance_squared(out.points[c]);
            let bd = out.points[b].distance_squared(out.points[d]);
            let tris = if ac <= bd {
                [[a, b, c], [a, c, d]]
            } else {
                [[a, b, d], [b, c, d]]
            };
            for t in tris {
                // Skip pieces with no area (a pole).
                let n = (out.points[t[1]] - out.points[t[0]])
                    .cross(out.points[t[2]] - out.points[t[0]]);
                if n.length_squared() <= 0.0 {
                    continue;
                }
                out.tris.push(if forward { t } else { [t[0], t[2], t[1]] });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A band unrolled from a cylinder: the bottom in 5 steps, the top in 3.
    fn band() -> Vec<DVec2> {
        let mut l: Vec<DVec2> = (0..=5).map(|i| DVec2::new(f64::from(i), 0.0)).collect();
        l.extend(
            (0..=3)
                .rev()
                .map(|i| DVec2::new(f64::from(i) * 5.0 / 3.0, 2.0)),
        );
        l
    }

    fn tri_area(l: &[DVec2], t: &[[usize; 3]]) -> f64 {
        t.iter()
            .map(|t| (l[t[1]] - l[t[0]]).perp_dot(l[t[2]] - l[t[0]]) / 2.0)
            .sum()
    }

    #[test]
    fn unrolled_bands_zip_in_short_steps() {
        for l in [band(), band().into_iter().rev().collect()] {
            let t = triangulate_unrolled(&l, &[]);
            assert_eq!(t.len(), l.len() - 2);
            assert!((tri_area(&l, &t) - 10.0).abs() < 1e-12, "covers it once");
            for t in &t {
                let xs = t.map(|i| l[i].x);
                let span = xs.iter().fold(0.0f64, |m, x| m.max(*x))
                    - xs.iter().fold(9.0f64, |m, x| m.min(*x));
                assert!(span <= 5.0 / 3.0 + 1e-12, "a short step: {t:?}");
                // No triangle lies along one chain.
                let ys = t.map(|i| l[i].y);
                assert!(ys.contains(&0.0) && ys.contains(&2.0));
            }
        }
    }

    #[test]
    fn other_unrolled_shapes_never_join_three_points_of_a_run() {
        // A C opening right: x turns back, so it is not a band.
        let mut l: Vec<DVec2> = (0..=3).map(|i| DVec2::new(f64::from(i), 0.0)).collect();
        l.extend(
            [
                (3.0, 1.0),
                (1.0, 1.0),
                (1.0, 2.0),
                (3.0, 2.0),
                (3.0, 3.0),
                (0.0, 3.0),
            ]
            .map(|(x, y)| DVec2::new(x, y)),
        );
        let t = triangulate_unrolled(&l, &[]);
        assert!(zip(&l).is_none());
        assert!((tri_area(&l, &t) - 7.0).abs() < 1e-12, "{t:?}");
        for t in &t {
            let on_bottom = t.iter().filter(|i| l[**i].y == 0.0).count();
            assert!(on_bottom < 3, "{t:?}");
        }
    }

    fn area(pts: &[DVec2], tris: &[[usize; 3]]) -> f64 {
        tris.iter()
            .map(|t| {
                let o = orient2d(pts[t[0]], pts[t[1]], pts[t[2]]);
                assert!(o > 0.0, "triangle {t:?} is not counter-clockwise");
                o * 0.5
            })
            .sum()
    }

    fn sq(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<DVec2> {
        vec![
            DVec2::new(x0, y0),
            DVec2::new(x1, y0),
            DVec2::new(x1, y1),
            DVec2::new(x0, y1),
        ]
    }

    #[test]
    fn square_is_two_triangles() {
        let s = sq(0.0, 0.0, 2.0, 1.0);
        let t = triangulate(&s, &[]);
        assert_eq!(t.len(), 2);
        assert!((area(&s, &t) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn clockwise_input_and_collinear_points() {
        let mut s = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(1.0, 0.0),
            DVec2::new(2.0, 0.0),
            DVec2::new(2.0, 2.0),
            DVec2::new(0.0, 2.0),
            DVec2::new(0.0, 1.0),
        ];
        s.reverse();
        let t = triangulate(&s, &[]);
        assert!((area(&s, &t) - 4.0).abs() < 1e-12);
    }

    #[test]
    fn square_with_hole() {
        let s = sq(0.0, 0.0, 4.0, 4.0);
        let h = sq(1.0, 1.0, 3.0, 3.0);
        let t = triangulate(&s, std::slice::from_ref(&h));
        let all: Vec<DVec2> = s.iter().chain(h.iter()).copied().collect();
        assert!((area(&all, &t) - 12.0).abs() < 1e-12);
    }

    #[test]
    fn hole_lined_up_with_corners_and_several_holes() {
        let s = sq(0.0, 0.0, 10.0, 4.0);
        let holes = vec![
            sq(1.0, 1.0, 3.0, 3.0),
            sq(4.0, 1.0, 6.0, 3.0),
            sq(7.0, 1.0, 9.0, 3.0),
        ];
        let t = triangulate(&s, &holes);
        let all: Vec<DVec2> = s.iter().chain(holes.iter().flatten()).copied().collect();
        assert!((area(&all, &t) - (40.0 - 12.0)).abs() < 1e-12);
    }

    #[test]
    fn concave_comb() {
        // A comb: many reflex corners.
        let mut s = vec![DVec2::new(0.0, 0.0), DVec2::new(20.0, 0.0)];
        for i in (0..10).rev() {
            let x = f64::from(i) * 2.0;
            s.push(DVec2::new(x + 2.0, 5.0));
            s.push(DVec2::new(x + 1.0, 1.0));
            s.push(DVec2::new(x, 5.0));
        }
        s.dedup();
        let t = triangulate(&s, &[]);
        let expect = signed_area2(&s) * 0.5;
        assert!((area(&s, &t) - expect).abs() < 1e-9);
    }
}
