//! Faces in one plane, rebuilt from the plane's edges.
//!
//! All edges in the plane form a planar graph (edges are already split where
//! they cross). Tracing it with half-edges sorted by the direction they
//! leave each vertex (an arc's tangent; curvature breaks ties) gives every
//! atomic region: an outer loop, and the outer boundaries of the islands
//! inside it as holes. Regions are kept as edges, since two arcs can join
//! the same two vertices (a circle). A region becomes a face when an existing face covered it, or
//! when its outer loop uses an edge that was just added (a newly closed
//! loop). Faces whose region did not change keep their ids.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Curve, Surface};
use prism_math::plane::Frame;
use prism_math::predicates::{Inside, orient2d, point_in_polygon, signed_area2};
use prism_math::{DVec2, DVec3, Plane, Tol};
use prism_topo::{EdgeId, Face, FaceId, Model, Paint, VertexId};

/// One side of a traced loop: leaving `from` along `edge`.
type Side = (VertexId, EdgeId);

/// One region of the plane's graph.
#[derive(Clone, Debug)]
struct Region {
    /// Counter-clockwise about the frame's normal.
    outer: Vec<Side>,
    /// Clockwise about the frame's normal.
    holes: Vec<Vec<Side>>,
    /// The polygons of the loops in the frame (arcs as their drawn points).
    polys: Vec<Vec<DVec2>>,
}

impl Region {
    fn loops(&self) -> impl Iterator<Item = &Vec<Side>> {
        std::iter::once(&self.outer).chain(self.holes.iter())
    }

    fn edges(&self) -> impl Iterator<Item = EdgeId> + '_ {
        self.loops().flatten().map(|s| s.1)
    }
}

/// The points of a loop of sides in the frame: each side's start and, for
/// an arc, its drawn points in walking order.
fn side_polygon(model: &Model, frame: &Frame, sides: &[Side]) -> Vec<DVec2> {
    let mut out = Vec::new();
    for (from, e) in sides {
        let p = model.vertex(*from);
        out.push(frame.to_2d(p));
        if let Some(ed) = model.edge(*e)
            && ed.curve.is_curved()
        {
            let (pa, pb) = (model.vertex(ed.a), model.vertex(ed.b));
            let mut inner = ed.curve.inner_points(pa, pb);
            if *from != ed.a {
                inner.reverse();
            }
            out.extend(inner.into_iter().map(|q| frame.to_2d(q)));
        }
    }
    out
}

/// A face's loops as polygons in the frame.
fn face_polys(model: &Model, f: &Face, frame: &Frame) -> Vec<Vec<DVec2>> {
    model
        .face_loop_points(f)
        .iter()
        .map(|l| l.iter().map(|p| frame.to_2d(*p)).collect())
        .collect()
}

/// Whether a point is inside the solid an edit makes (None: cannot tell).
pub(crate) type InsideTest<'a> = &'a dyn Fn(DVec3) -> Option<bool>;

/// How regions that no face covered become faces.
#[derive(Clone, Copy)]
pub(crate) enum Fill<'a> {
    /// Drawing: a region whose outer loop uses a new edge (a newly closed loop).
    Drawn,
    /// After an edit. With an inside test (Push/Pull on a solid): a region
    /// with the solid on exactly one side, facing out of it. Without: a
    /// region that uses a new edge and closes an opening, so each of its
    /// edges has exactly one other face.
    Closing(Option<InsideTest<'a>>),
}

/// Rebuild the faces in `plane`. `new_edges` were just added. Returns the
/// faces made.
pub(crate) fn rebuild_plane(
    model: &mut Model,
    plane: &Plane,
    new_edges: &BTreeSet<EdgeId>,
    tol: &Tol,
    fill: Fill,
    step: f64,
) -> Vec<FaceId> {
    let frame = plane.frame();
    let on: BTreeMap<VertexId, DVec2> = model
        .vertices()
        .filter(|(_, p)| plane.contains(*p, tol))
        .map(|(id, p)| (id, frame.to_2d(p)))
        .collect();
    // Arcs belong to the plane only when they lie in it.
    let edges: Vec<(EdgeId, VertexId, VertexId)> = model
        .edges()
        .filter(|(_, e)| on.contains_key(&e.a) && on.contains_key(&e.b))
        .filter(|(_, e)| match e.curve {
            Curve::Line => true,
            Curve::Arc { centre, axis, .. } => {
                plane.contains(centre, tol) && tol.parallel(axis, plane.normal)
            }
            Curve::Spline(ref s) => s.curve.points().iter().all(|p| plane.contains(*p, tol)),
        })
        .map(|(id, e)| (id, e.a, e.b))
        .collect();
    let old: Vec<FaceId> = model
        .faces()
        .filter(|(_, f)| {
            f.plane().is_some_and(|p| p.same_surface(plane, tol))
                && f.loops().flatten().all(|v| on.contains_key(v))
        })
        .map(|(id, _)| id)
        .collect();

    let regions = trace(model, &frame, &on, &edges, tol);
    // Each old face's box in the plane, so a region only tests the faces
    // that could cover it (a ground plane can hold thousands).
    let old_boxes: Vec<(FaceId, DVec2, DVec2)> = old
        .iter()
        .filter_map(|f| {
            let face = model.face(*f)?;
            let pts: Vec<DVec2> = face_polys(model, face, &frame).swap_remove(0);
            let lo = pts.iter().fold(DVec2::INFINITY, |a, p| a.min(*p));
            let hi = pts.iter().fold(DVec2::NEG_INFINITY, |a, p| a.max(*p));
            Some((*f, lo, hi))
        })
        .collect();

    // Decide which regions are faces, and which way each faces.
    let mut keep: BTreeSet<FaceId> = BTreeSet::new();
    let mut made_faces: Vec<Face> = Vec::new();
    let mut openings: Vec<&Region> = Vec::new();
    for r in &regions {
        let Some(sample) = interior_point(&r.polys) else {
            continue;
        };
        let covers: Vec<FaceId> = old_boxes
            .iter()
            .filter(|(_, lo, hi)| sample.cmpge(*lo).all() && sample.cmple(*hi).all())
            .map(|(f, _, _)| *f)
            .filter(|f| face_contains_2d(model, *f, &frame, sample))
            .collect();
        // The sample can sit on an old face's edge (the middle of two faces
        // just merged is on the line erased between them) and so be inside
        // neither. Then go by the edges: a face runs along each of its
        // edges with itself on the left (about its normal), so an old face
        // running along one of the region's edges with the region on that
        // side covers it. One is enough, as a sample finds one.
        let covers = if covers.is_empty() {
            old.iter()
                .copied()
                .find(|f| {
                    model.face(*f).is_some_and(|face| {
                        let up = face
                            .plane()
                            .is_some_and(|p| p.normal.dot(frame.normal) > 0.0);
                        face.sides().any(|s| {
                            r.outer
                                .iter()
                                .any(|(from, e)| s.edge == *e && (s.from == *from) == up)
                        })
                    })
                })
                .into_iter()
                .collect()
        } else {
            covers
        };
        let parent = covers.first().copied();
        // After an edit, faces covering a region back to back (a face pulled
        // flush against another) are inside one solid now: both go.
        let closing = matches!(fill, Fill::Closing(_));
        if closing {
            let facing = |f: &FaceId| {
                model
                    .face(*f)
                    .and_then(|g| g.plane())
                    .is_some_and(|p| p.normal.dot(frame.normal) > 0.0)
            };
            if covers.iter().any(facing) && !covers.iter().all(facing) {
                continue;
            }
        }
        let closes = r.outer.iter().any(|(_, e)| new_edges.contains(e));
        if parent.is_none() && (closing || !closes) {
            match fill {
                Fill::Closing(Some(inside)) => {
                    // Exactly: a face where the solid is on one side only.
                    let c = frame.origin + frame.u * sample.x + frame.v * sample.y;
                    let (above, below) = (
                        inside(c + frame.normal * step),
                        inside(c - frame.normal * step),
                    );
                    if let (Some(a), Some(b)) = (above, below)
                        && a != b
                    {
                        made_faces.push(make_face(model, r, &frame, b));
                    }
                }
                Fill::Closing(None) if r.edges().any(|e| new_edges.contains(&e)) => {
                    openings.push(r)
                }
                _ => {}
            }
            continue;
        }
        if covers.is_empty() {
            // Facing as a neighbour in the same plane does, else as the
            // faces round its edges agree (a hole in a solid closed), else
            // along the frame.
            let up = neighbour_facing(model, &old, r, &frame)
                .or_else(|| wall_facing(model, r, &frame))
                .unwrap_or(true);
            made_faces.push(make_face(model, r, &frame, up));
            continue;
        }
        // One face for each face that covered the region. Several only
        // while faces overlap in the middle of an edit: the solid rule needs
        // to count each solid's own face, and `dedupe_faces` then keeps one.
        for c in &covers {
            if same_region(model.face(*c), r) {
                keep.insert(*c);
            } else {
                let up = model
                    .face(*c)
                    .and_then(|f| f.plane())
                    .is_some_and(|p| p.normal.dot(frame.normal) > 0.0);
                let mut face = make_face(model, r, &frame, up);
                face.paint = inherited(model, &old, *c, r, &frame);
                made_faces.push(face);
            }
        }
    }

    if !openings.is_empty() {
        // The edges of the faces chosen so far, once per side.
        let mut chosen: Vec<EdgeId> = keep
            .iter()
            .filter_map(|f| model.face(*f))
            .flat_map(|f| f.edge_ids().collect::<Vec<_>>())
            .chain(
                made_faces
                    .iter()
                    .flat_map(|f| f.edge_ids().collect::<Vec<_>>()),
            )
            .collect();
        let others = other_faces(model, &old);
        // One opening filled can make the next one an opening, so repeat.
        loop {
            let found = openings.iter().position(|r| {
                r.edges().all(|e| {
                    let beside = others.get(&e).map_or(0, |d| d.len());
                    let inside = chosen.iter().filter(|x| **x == e).count();
                    beside + inside == 1
                })
            });
            let Some(i) = found else { break };
            let r = openings.remove(i);
            // Face the way that agrees with the faces around it: across a
            // shared edge, neighbours run the edge in opposite directions.
            let votes: i32 = r
                .loops()
                .flatten()
                .map(|(from, e)| {
                    others.get(e).map_or(0, |dirs| {
                        dirs.iter()
                            .map(|x| if x == from { -1 } else { 1 })
                            .sum::<i32>()
                    })
                })
                .sum();
            let face = make_face(model, r, &frame, votes >= 0);
            chosen.extend(face.edge_ids());
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

/// For each edge, the start vertex of each run along it by a face that is
/// not one of `except`.
fn other_faces(model: &Model, except: &[FaceId]) -> BTreeMap<EdgeId, Vec<VertexId>> {
    let mut out: BTreeMap<EdgeId, Vec<VertexId>> = BTreeMap::new();
    for (id, f) in model.faces() {
        if except.contains(&id) {
            continue;
        }
        for s in f.sides() {
            out.entry(s.edge).or_default().push(s.from);
        }
    }
    out
}

/// The paint a region made from `cover` takes (DECISIONS #36): a piece of
/// a split face keeps the face's; where faces merged, the largest of the
/// old faces facing the same way inside the region gives its own.
fn inherited(model: &Model, old: &[FaceId], cover: FaceId, r: &Region, frame: &Frame) -> Paint {
    let Some(c) = model.face(cover) else {
        return Paint::default();
    };
    let up = |f: &Face| f.plane().is_some_and(|p| p.normal.dot(frame.normal) > 0.0);
    let inside_region = |q: DVec2| {
        point_in_polygon(q, &r.polys[0]) == Inside::Inside
            && r.polys[1..]
                .iter()
                .all(|h| point_in_polygon(q, h) == Inside::Outside)
    };
    let area = |f: &Face| -> f64 {
        let polys = face_polys(model, f, frame);
        let a = |l: &[DVec2]| -> f64 {
            (0..l.len())
                .map(|i| l[i].perp_dot(l[(i + 1) % l.len()]))
                .sum::<f64>()
                .abs()
                / 2.0
        };
        polys.first().map_or(0.0, |o| a(o)) - polys[1..].iter().map(|h| a(h)).sum::<f64>()
    };
    let mut best = (area(c), c.paint);
    for id in old {
        let Some(f) = model.face(*id) else { continue };
        if *id == cover || up(f) != up(c) || f.paint == best.1 {
            continue;
        }
        let polys = face_polys(model, f, frame);
        if interior_point(&polys).is_some_and(inside_region) {
            let a = area(f);
            if a > best.0 {
                best = (a, f.paint);
            }
        }
    }
    best.1
}

/// Build a face from a region, facing along the frame normal if `up`.
fn make_face(model: &Model, r: &Region, frame: &Frame, up: bool) -> Face {
    let verts = |l: &[Side]| -> Vec<VertexId> { l.iter().map(|s| s.0).collect() };
    let outer = verts(&r.outer);
    let pts = model.points(&outer);
    let centroid = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
    let mut f = Face::with_edges(
        outer,
        r.holes.iter().map(|h| verts(h)).collect(),
        Surface::Plane(Plane::new(centroid, frame.normal)),
        r.loops().map(|l| l.iter().map(|s| s.1).collect()).collect(),
    );
    if !up {
        f.reverse();
    }
    f
}

/// Which way a coplanar face sharing an edge with the region faces, if any.
fn neighbour_facing(model: &Model, old: &[FaceId], r: &Region, frame: &Frame) -> Option<bool> {
    let edges: BTreeSet<EdgeId> = r.outer.iter().map(|s| s.1).collect();
    old.iter().find_map(|f| {
        let face = model.face(*f)?;
        let shares = face.edge_ids().any(|e| edges.contains(&e));
        let n = face.plane()?.normal;
        shares.then(|| n.dot(frame.normal) > 0.0)
    })
}

/// Which way a new face must face to agree with the faces that already
/// use its edges: on a consistent solid, two faces sharing an edge run
/// along it in opposite directions. Each edge used by exactly one other
/// face votes; None without a vote or on a tie (owner, 2026-11-12: a face
/// closing a hole in a body came out reversed at random).
fn wall_facing(model: &Model, r: &Region, frame: &Frame) -> Option<bool> {
    let face = make_face(model, r, frame, true);
    let mut votes = 0i32;
    for side in face.sides() {
        let users: Vec<&Face> = model
            .faces()
            .map(|(_, g)| g)
            .filter(|g| g.uses_edge(side.edge))
            .collect();
        let [g] = users.as_slice() else { continue };
        // Its plane: a face in the same plane is not a wall.
        if g.plane()
            .is_some_and(|p| p.normal.cross(frame.normal).length() < 1e-9)
        {
            continue;
        }
        for s in g.sides().filter(|s| s.edge == side.edge) {
            votes += if s.from == side.from { -1 } else { 1 };
        }
    }
    (votes != 0).then_some(votes > 0)
}

/// Whether a face has exactly the region's loops (the same edges, any
/// start, either way round).
fn same_region(face: Option<&Face>, r: &Region) -> bool {
    let Some(f) = face else { return false };
    let edges = |l: &[Side]| -> Vec<EdgeId> { l.iter().map(|s| s.1).collect() };
    if !same_cycle(f.loop_edge_ids(0), &edges(&r.outer)) || f.holes.len() != r.holes.len() {
        return false;
    }
    let mut used = vec![false; r.holes.len()];
    (1..=f.holes.len()).all(|k| {
        let h = f.loop_edge_ids(k);
        r.holes.iter().enumerate().any(|(i, rh)| {
            if !used[i] && same_cycle(h, &edges(rh)) {
                used[i] = true;
                true
            } else {
                false
            }
        })
    })
}

/// Whether two loops are the same cycle, from any start and in either direction.
pub(crate) fn same_cycle<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    if a.len() != b.len() || a.is_empty() {
        return false;
    }
    let n = a.len();
    // An edge can appear twice in a loop (a line into an island), so try
    // every start that matches.
    (0..n).filter(|&start| b[start] == a[0]).any(|start| {
        let fwd = (0..n).all(|i| a[i] == b[(start + i) % n]);
        let back = (0..n).all(|i| a[i] == b[(start + n - i) % n]);
        fwd || back
    })
}

/// Whether the 2D point (in `frame`) is inside a face: in its outer loop and
/// not in a hole.
fn face_contains_2d(model: &Model, f: FaceId, frame: &Frame, p: DVec2) -> bool {
    let Some(face) = model.face(f) else {
        return false;
    };
    let polys = face_polys(model, face, frame);
    point_in_polygon(p, &polys[0]) == Inside::Inside
        && polys[1..]
            .iter()
            .all(|h| point_in_polygon(p, h) == Inside::Outside)
}

/// Trace the planar graph into regions.
fn trace(
    model: &Model,
    frame: &Frame,
    on: &BTreeMap<VertexId, DVec2>,
    edges: &[(EdgeId, VertexId, VertexId)],
    tol: &Tol,
) -> Vec<Region> {
    // How each half-edge leaves its start: a point along its direction
    // there (the far end for a line, so lines compare exactly), and its
    // curvature turning left (0 for a line).
    let leave = |e: EdgeId, from: VertexId, to: VertexId| -> (DVec2, f64) {
        let o = on[&from];
        let Some(ed) = model.edge(e) else {
            return (on[&to], 0.0);
        };
        match ed.curve.clone() {
            Curve::Line => (on[&to], 0.0),
            Curve::Spline(_) => {
                // Its direction at this end, and how it bends to the left.
                let c = ed.curve_from(from);
                let Some(s) = c.spline() else {
                    return (on[&to], 0.0);
                };
                let t = s.tangent(0.0);
                let t2 = DVec2::new(t.dot(frame.u), t.dot(frame.v));
                let bend = s.curve.curvature(s.t0).dot(frame.normal.cross(t));
                (o + t2 * model.vertex(from).distance(model.vertex(to)), bend)
            }
            c @ Curve::Arc { axis, radius, .. } => {
                let (pa, pb) = (model.vertex(ed.a), model.vertex(ed.b));
                let (c, p, q) = if from == ed.a {
                    (c, pa, pb)
                } else {
                    (c.reversed(), pb, pa)
                };
                let t = c.start_tangent(p, q);
                let t2 = DVec2::new(t.dot(frame.u), t.dot(frame.v));
                let turn = if from == ed.a { axis } else { -axis };
                (o + t2 * radius, turn.dot(frame.normal).signum() / radius)
            }
        }
    };
    // Half-edges leaving each vertex, sorted counter-clockwise by direction,
    // then by curvature (curving left is further round).
    let mut around: BTreeMap<VertexId, Vec<(EdgeId, VertexId, DVec2, f64)>> = BTreeMap::new();
    for (e, a, b) in edges {
        let (pa, ka) = leave(*e, *a, *b);
        let (pb, kb) = leave(*e, *b, *a);
        around.entry(*a).or_default().push((*e, *b, pa, ka));
        around.entry(*b).or_default().push((*e, *a, pb, kb));
    }
    for (v, list) in around.iter_mut() {
        let o = on[v];
        list.sort_by(|x, y| {
            ccw_order(o, x.2, y.2)
                .then(x.3.total_cmp(&y.3))
                .then(x.0.cmp(&y.0))
        });
        sort_tangent_runs(o, list, tol);
    }

    // Each half-edge u→v continues to the half-edge just clockwise of v→u
    // at v, which keeps the region on its left.
    let mut seen: BTreeSet<(EdgeId, VertexId)> = BTreeSet::new();
    let mut cycles: Vec<Vec<Side>> = Vec::new();
    for (u, list) in &around {
        for (e, v, _, _) in list {
            if seen.contains(&(*e, *u)) {
                continue;
            }
            let mut cycle = Vec::new();
            let (mut from, mut edge, mut to) = (*u, *e, *v);
            while seen.insert((edge, from)) {
                cycle.push((from, edge));
                let at = &around[&to];
                let i = at
                    .iter()
                    .position(|x| x.0 == edge && x.1 == from)
                    .unwrap_or(0);
                let next = at[(i + at.len() - 1) % at.len()];
                from = to;
                edge = next.0;
                to = next.1;
            }
            cycles.push(cycle);
        }
    }

    // Bounded regions wind counter-clockwise; the outside of each connected
    // piece winds clockwise and is a hole of whichever region contains it.
    let mut outers: Vec<(Vec<Side>, Vec<DVec2>, f64)> = Vec::new();
    let mut islands: Vec<(Vec<Side>, Vec<DVec2>)> = Vec::new();
    for c in cycles {
        let area = signed_area2(&side_polygon(model, frame, &c)) * 0.5;
        let clean = remove_spurs(c);
        if clean.len() < 2 {
            continue;
        }
        let poly = side_polygon(model, frame, &clean);
        if area > tol.area() {
            outers.push((clean, poly, area));
        } else if area < -tol.area() {
            islands.push((clean, poly));
        }
    }
    let mut regions: Vec<Region> = outers
        .iter()
        .map(|(l, poly, _)| Region {
            outer: l.clone(),
            holes: vec![],
            polys: vec![poly.clone()],
        })
        .collect();
    let boxes: Vec<(DVec2, DVec2)> = outers
        .iter()
        .map(|(_, poly, _)| {
            let lo = poly.iter().fold(DVec2::INFINITY, |a, p| a.min(*p));
            let hi = poly.iter().fold(DVec2::NEG_INFINITY, |a, p| a.max(*p));
            (lo, hi)
        })
        .collect();
    for (island, poly) in islands {
        let first = island[0].0;
        let probe = on[&first];
        let host = outers
            .iter()
            .enumerate()
            .filter(|(i, _)| probe.cmpge(boxes[*i].0).all() && probe.cmple(boxes[*i].1).all())
            .filter(|(_, (l, poly, _))| {
                !l.iter().any(|s| s.0 == first) && point_in_polygon(probe, poly) == Inside::Inside
            })
            .min_by(|a, b| a.1.2.total_cmp(&b.1.2))
            .map(|(i, _)| i);
        if let Some(i) = host {
            regions[i].holes.push(island);
            regions[i].polys.push(poly);
        }
    }
    regions
}

/// Order two directions from `o` counter-clockwise, starting from +x (exact).
/// Half-edges leaving one way within tolerance (an arc and a tangent line
/// or arc) sort by rounding noise; put each such run of neighbours in order
/// of curvature instead. Two directions agree when, as far out as the
/// shorter one reaches (its length, or an arc's radius), they are less than
/// the tolerance apart.
fn sort_tangent_runs(o: DVec2, list: &mut [(EdgeId, VertexId, DVec2, f64)], tol: &Tol) {
    let n = list.len();
    let same = |x: DVec2, y: DVec2| {
        let (dx, dy) = (x - o, y - o);
        let (lx, ly) = (dx.length(), dy.length());
        lx > 0.0
            && ly > 0.0
            && dx.dot(dy) > 0.0
            && (dx / lx).perp_dot(dy / ly).abs() * lx.min(ly) < tol.dist
    };
    if n < 2 || !list.iter().any(|x| x.3 != 0.0) {
        return;
    }
    // Start just after a gap between neighbours, so no run wraps around.
    let Some(start) = (0..n).find(|&i| !same(list[(i + n - 1) % n].2, list[i].2)) else {
        list.sort_by(|x, y| x.3.total_cmp(&y.3).then(x.0.cmp(&y.0)));
        return;
    };
    list.rotate_left(start);
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && same(list[j - 1].2, list[j].2) {
            j += 1;
        }
        list[i..j].sort_by(|x, y| x.3.total_cmp(&y.3).then(x.0.cmp(&y.0)));
        i = j;
    }
}

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

/// Remove back-and-forth visits along loose edges inside a region (a side
/// along an edge straight followed by the way back along the same edge).
fn remove_spurs(mut c: Vec<Side>) -> Vec<Side> {
    loop {
        let n = c.len();
        if n < 2 {
            return c;
        }
        let spur = (0..n).find(|&i| c[i].1 == c[(i + 1) % n].1);
        match spur {
            Some(i) => {
                // Drop side i and the way back, side i+1.
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
    interior_points(loops).into_iter().next()
}

/// Points strictly inside the first loop and outside the others, best
/// first: the middle of the widest stretch on each scan line, ranked by the
/// box that fits there (the smaller of its width and the gap's height), so
/// the best point is not in a thin spot (such as where two arcs touch).
pub(crate) fn interior_points(loops: &[Vec<DVec2>]) -> Vec<DVec2> {
    let mut out = Vec::new();
    let Some(outer) = loops.first() else {
        return out;
    };
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
    let mut found: Vec<(f64, DVec2)> = Vec::new();
    for (h, y) in gaps {
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
            found.push((w.min(h), DVec2::new(x, y)));
        }
    }
    // Stable, so equal boxes keep the widest-gap order.
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.extend(found.into_iter().map(|(_, p)| p));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spurs_are_removed() {
        let v = |i| VertexId(i);
        let e = |i| EdgeId(i);
        // 0 -a- 1 -b- 2 -c(spur)- 9 -c- 2 -d- 3 ...
        let c = vec![
            (v(0), e(0)),
            (v(1), e(1)),
            (v(2), e(2)),
            (v(9), e(2)),
            (v(2), e(3)),
        ];
        assert_eq!(
            remove_spurs(c),
            vec![(v(0), e(0)), (v(1), e(1)), (v(2), e(3))]
        );
        assert!(remove_spurs(vec![(v(0), e(0)), (v(1), e(0))]).is_empty());
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
