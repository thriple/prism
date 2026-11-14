//! Prism's solids to the kernel and back (Fillet Joins step A, brief 32).
//!
//! - [`solid_of`]: the faces of the closed solid a face belongs to.
//! - [`region`]: those faces as a `prism_kernel::solid::Region`, every
//!   vertex, edge and face keyed by its Prism id.
//! - [`put_back`]: a region the kernel returned, written into the model in
//!   place of the old faces. What the kernel kept keeps its id, paint and
//!   texture placement; a face whose loops and surface did not change is
//!   not touched at all. New faces and edges get new ids.
//!
//! Ids are kept by the keys the kernel carries through, never by matching
//! positions (brief 32, "Risks").

use std::collections::{BTreeMap, BTreeSet};

use prism_kernel::solid::{self, Edge, Face as RFace, NOT_CLOSED, Region, Vertex};
use prism_math::DVec3;
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

/// Said for an edge Prism only approximates (DECISIONS #32).
pub const APPROXIMATE: &str =
    "Refused: an edge of this solid is an approximation, which the kernel cannot take.";

/// The faces of the closed solid `seed` is a face of: the faces reached
/// from it across its edges. Where more than two faces meet at an edge (a
/// disc left on the ground in a block's round hole), the one met first
/// turning about the edge from the face into the solid is the next, as
/// long as it runs the edge the other way. An edge with no such face
/// leaves the solid open.
pub fn solid_of(model: &Model, seed: FaceId) -> Result<BTreeSet<FaceId>, &'static str> {
    let edge_faces = model.edge_faces();
    let mut out = BTreeSet::new();
    let mut todo = vec![seed];
    model.face(seed).ok_or(NOT_CLOSED)?;
    while let Some(f) = todo.pop() {
        if !out.insert(f) {
            continue;
        }
        let face = model.face(f).ok_or(NOT_CLOSED)?;
        for side in face.sides() {
            let others: Vec<FaceId> = edge_faces
                .get(&side.edge)
                .map(|v| v.iter().copied().filter(|g| *g != f).collect())
                .unwrap_or_default();
            let next = match others.as_slice() {
                [] => return Err(NOT_CLOSED),
                [g] => *g,
                _ => partner(model, f, side, &others).ok_or(NOT_CLOSED)?,
            };
            todo.push(next);
        }
    }
    Ok(out)
}

/// The middle of an edge, and its direction there from `a` to `b`.
fn edge_middle(model: &Model, e: &prism_topo::Edge) -> (DVec3, DVec3) {
    let (a, b) = (model.vertex(e.a), model.vertex(e.b));
    match e.curve {
        prism_geom::Curve::Arc { centre, axis, .. } => {
            let (xa, xb) = (a - centre, b - centre);
            let mut sweep = axis.dot(xa.cross(xb)).atan2(xa.dot(xb));
            if sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
            let m = centre + prism_math::DQuat::from_axis_angle(axis, sweep / 2.0) * xa;
            (m, axis.cross(m - centre).normalize_or_zero())
        }
        _ => ((a + b) * 0.5, (b - a).normalize_or_zero()),
    }
}

/// A face's outward normal at `p` on it (flat and cylindrical faces).
fn normal_at(s: &prism_geom::Surface, p: DVec3) -> Option<DVec3> {
    match s {
        prism_geom::Surface::Plane(pl) => Some(pl.normal),
        prism_geom::Surface::Cylinder(c) => Some(c.normal_at(p)),
        _ => None,
    }
}

/// Of `others` meeting face `f` at its side `side`, the one met first
/// turning about the edge from `f` into the solid behind it, among those
/// running the edge the other way.
fn partner(model: &Model, f: FaceId, side: prism_topo::Side, others: &[FaceId]) -> Option<FaceId> {
    let e = model.edge(side.edge)?;
    let (mid, along) = edge_middle(model, e);
    let run = |g: FaceId| {
        model
            .face(g)?
            .sides()
            .find(|s| s.edge == side.edge)
            .map(|s| if s.from == e.a { along } else { -along })
    };
    // Into the face from the edge: to the left of the way its loop runs.
    let into = |g: FaceId, t: DVec3| Some(normal_at(&model.face(g)?.surface, mid)?.cross(t));
    let t = run(f)?;
    let x = into(f, t)?;
    let y = -normal_at(&model.face(f)?.surface, mid)?;
    others
        .iter()
        .filter_map(|&g| {
            let tg = run(g)?;
            if tg.dot(t) >= 0.0 {
                return None;
            }
            let w = into(g, tg)?;
            let mut a = w.dot(y).atan2(w.dot(x));
            if a <= 1e-12 {
                a += std::f64::consts::TAU;
            }
            Some((a, g))
        })
        .min_by(|p, q| p.0.total_cmp(&q.0))
        .map(|(_, g)| g)
}

/// `faces` as a region for the kernel.
pub fn region(model: &Model, faces: &BTreeSet<FaceId>) -> Result<Region, &'static str> {
    let mut r = Region::default();
    let mut vix: BTreeMap<VertexId, usize> = BTreeMap::new();
    let mut eix: BTreeMap<EdgeId, usize> = BTreeMap::new();
    for &fid in faces {
        let f = model.face(fid).ok_or(NOT_CLOSED)?;
        let mut loops = Vec::new();
        for k in 0..f.loops().count() {
            let mut lp = Vec::new();
            for side in f.loop_edges(k) {
                let e = model.edge(side.edge).ok_or(NOT_CLOSED)?;
                if e.approx {
                    return Err(APPROXIMATE);
                }
                let i = match eix.get(&side.edge) {
                    Some(i) => *i,
                    None => {
                        let mut corner = |v: VertexId| {
                            *vix.entry(v).or_insert_with(|| {
                                r.vertices.push(Vertex {
                                    key: Some(u64::from(v.0)),
                                    at: model.vertex(v),
                                });
                                r.vertices.len() - 1
                            })
                        };
                        let (a, b) = (corner(e.a), corner(e.b));
                        r.edges.push(Edge {
                            key: Some(u64::from(side.edge.0)),
                            a,
                            b,
                            curve: e.curve.clone(),
                        });
                        eix.insert(side.edge, r.edges.len() - 1);
                        r.edges.len() - 1
                    }
                };
                lp.push((i, side.from == e.a));
            }
            loops.push(lp);
        }
        r.faces.push(RFace {
            key: Some(u64::from(fid.0)),
            surface: f.surface.clone(),
            loops,
        });
    }
    Ok(r)
}

/// The closed solid `seed` belongs to, through the kernel and back with no
/// change: what step A checks (the same faces, ids and volume return).
/// Returns the kernel's volume of it.
pub fn round_trip(model: &mut Model, seed: FaceId) -> Result<f64, &'static str> {
    let faces = solid_of(model, seed)?;
    let r = region(model, &faces)?;
    let s = solid::Solid::build(&r, model.tol().dist)?;
    let volume = s.volume()?;
    let back = s.region()?;
    put_back(model, &faces, &back)?;
    Ok(volume)
}

/// Whether two loops are the same edges in the same order, from any start.
fn same_loop(a: &[EdgeId], b: &[EdgeId]) -> bool {
    a.len() == b.len()
        && (a.is_empty()
            || (0..b.len()).any(|s| (0..a.len()).all(|i| a[i] == b[(s + i) % b.len()])))
}

/// Write `r` (from the kernel) into `model` in place of the faces `old`.
/// Returns the faces of the result.
pub fn put_back(
    model: &mut Model,
    old: &BTreeSet<FaceId>,
    r: &Region,
) -> Result<Vec<FaceId>, &'static str> {
    let tol = model.tol().dist;
    let old_edges: BTreeSet<EdgeId> = old
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.edge_ids().collect::<Vec<_>>())
        .collect();
    // Corners: a kept one keeps its id (and moves only if the kernel moved
    // it).
    let mut vids = Vec::with_capacity(r.vertices.len());
    for v in &r.vertices {
        let kept = v
            .key
            .and_then(|k| u32::try_from(k).ok())
            .map(VertexId)
            .filter(|id| model.vertices().any(|(x, _)| x == *id));
        let id = match kept {
            Some(id) => {
                if model.vertex(id).distance(v.at) > tol {
                    model.set_vertex(id, v.at);
                }
                id
            }
            None => model.add_vertex(v.at),
        };
        vids.push(id);
    }
    // Edges: a kept one keeps its id when it still joins the same corners.
    let mut eids: Vec<(EdgeId, bool)> = Vec::with_capacity(r.edges.len());
    for e in &r.edges {
        let (a, b) = (vids[e.a], vids[e.b]);
        let kept = e
            .key
            .and_then(|k| u32::try_from(k).ok())
            .map(EdgeId)
            .and_then(|id| model.edge(id).map(|x| (id, x.a, x.b)))
            .filter(|(_, xa, xb)| (*xa, *xb) == (a, b) || (*xa, *xb) == (b, a));
        // `true` when the model's edge runs the region's way.
        eids.push(match kept {
            Some((id, xa, _)) => (id, xa == a),
            None => {
                let id = match e.curve {
                    prism_geom::Curve::Line => model.add_new_edge(a, b),
                    ref c => model
                        .add_curve_edge(a, b, c.clone(), None)
                        .ok_or(solid::CANNOT_STORE)?,
                };
                (id, true)
            }
        });
    }
    let mut result = Vec::with_capacity(r.faces.len());
    for f in &r.faces {
        // Each loop as Prism stores it: the corners, and the edge from
        // each to the next.
        let mut loops: Vec<(Vec<VertexId>, Vec<EdgeId>)> = Vec::new();
        for l in &f.loops {
            let mut corners = Vec::with_capacity(l.len());
            let mut edges = Vec::with_capacity(l.len());
            for &(i, forward) in l {
                let e = &r.edges[i];
                corners.push(vids[if forward { e.a } else { e.b }]);
                edges.push(eids[i].0);
            }
            loops.push((corners, edges));
        }
        let mut it = loops.into_iter();
        let (outer, outer_edges) = it.next().ok_or(NOT_CLOSED)?;
        let (holes, hole_edges): (Vec<_>, Vec<_>) = it.unzip();
        let mut edges = vec![outer_edges];
        edges.extend(hole_edges);
        let kept = f
            .key
            .and_then(|k| u32::try_from(k).ok())
            .map(FaceId)
            .filter(|id| old.contains(id) && model.face(*id).is_some());
        match kept {
            Some(id) => {
                let same = model.face(id).is_some_and(|x| {
                    x.loops().count() == edges.len()
                        && (0..edges.len()).all(|k| same_loop(x.loop_edge_ids(k), &edges[k]))
                        && same_surface(&x.surface, &f.surface, tol)
                });
                if !same {
                    let surface = f.surface.clone();
                    model.edit_face(id, |x| {
                        let paint = x.paint;
                        *x = Face::with_edges(outer, holes, surface, edges);
                        x.paint = paint;
                    });
                }
                result.push(id);
            }
            None => result.push(model.add_face(Face::with_edges(
                outer,
                holes,
                f.surface.clone(),
                edges,
            ))),
        }
    }
    // Faces, edges and corners of the old solid that are gone.
    let keep: BTreeSet<FaceId> = result.iter().copied().collect();
    for f in old {
        if !keep.contains(f) {
            model.remove_face(*f);
        }
    }
    let used: BTreeSet<EdgeId> = model
        .faces()
        .flat_map(|(_, f)| f.edge_ids().collect::<Vec<_>>())
        .collect();
    let new_edges: BTreeSet<EdgeId> = eids.iter().map(|(e, _)| *e).collect();
    for e in old_edges {
        if !used.contains(&e) && !new_edges.contains(&e) {
            model.remove_edge(e);
        }
    }
    Ok(result)
}

/// The same surface within `tol` (and facing the same way).
fn same_surface(a: &prism_geom::Surface, b: &prism_geom::Surface, tol: f64) -> bool {
    use prism_geom::Surface as S;
    let near = |p: DVec3, q: DVec3| p.distance(q) <= tol;
    match (a, b) {
        (S::Plane(p), S::Plane(q)) => {
            p.normal.distance(q.normal) <= 1e-9 && (q.origin - p.origin).dot(p.normal).abs() <= tol
        }
        (S::Cylinder(p), S::Cylinder(q)) => {
            p.outward == q.outward
                && (p.radius - q.radius).abs() <= tol
                && p.axis.dot(q.axis).abs() >= 1.0 - 1e-9
                && near(p.axis_point(q.origin), q.origin)
        }
        _ => false,
    }
}
