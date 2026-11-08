//! The coplanar rule after an edit (PLAN 1.9, "when the check runs").
//!
//! In every plane holding a face the edit touched, edges that cross or run
//! into each other are split, and the faces on the plane are rebuilt as one
//! set of regions, as drawing already does. Faces that overlap become one
//! set of faces, a face whose hole was pushed across its edge is rebuilt,
//! and an opening the edit left behind gets a face.
//!
//! Most edits change nothing that needs this, so each plane is checked
//! cheaply first (crossings, touches and containment, between pairs that
//! could have changed) and rebuilt only when something was found. Geometry
//! moved rigidly together (a Move, Rotate or Scale, or one copy of an
//! array) keeps its arrangement, so pairs inside one such group are skipped.

use std::collections::{BTreeMap, BTreeSet};

use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3, Plane, Tol};
use prism_topo::{EdgeId, FaceId, Model, VertexId, loop_pairs};

use crate::regions::{Fill, rebuild_plane};
use crate::sticky::{crossing, on_segment_inside};

/// After an edit that changed the touched vertices freely (Push/Pull).
pub fn settle(model: &mut Model, touched: &BTreeSet<VertexId>) {
    let tags = touched.iter().map(|v| (*v, Tag::Free)).collect();
    run(model, tags);
}

/// After an edit that moved each group of vertices by one affine map (a
/// Move, Rotate or Scale, or the copies of an array, one group each).
pub fn settle_rigid(model: &mut Model, groups: &[BTreeSet<VertexId>]) {
    let mut tags = BTreeMap::new();
    for (k, g) in groups.iter().enumerate() {
        for v in g {
            tags.insert(*v, Tag::Group(k));
        }
    }
    run(model, tags);
}

/// How a vertex, edge or face took part in the edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Still,
    Group(usize),
    Free,
}

fn combine(a: Tag, b: Tag) -> Tag {
    match (a, b) {
        (x, y) if x == y => x,
        _ => Tag::Free,
    }
}

/// Whether a pair could have changed how it meets.
fn worth_checking(a: Tag, b: Tag) -> bool {
    match (a, b) {
        (Tag::Still, Tag::Still) => false,
        (Tag::Group(x), Tag::Group(y)) => x != y,
        _ => true,
    }
}

/// A plane key on a coarse grid; neighbouring cells are searched too, so
/// planes within the tolerance of a cell border are still found.
type Cell = [i64; 4];

fn cell(p: &Plane, tol: &Tol) -> Cell {
    let (n, d) = p.canonical();
    let q = |x: f64, s: f64| (x / s).floor() as i64;
    let ds = (tol.dist * 64.0).max(f64::MIN_POSITIVE);
    [q(n.x, 1e-4), q(n.y, 1e-4), q(n.z, 1e-4), q(d, ds)]
}

fn around(c: Cell) -> impl Iterator<Item = Cell> {
    (0..81).map(move |i| {
        let o = |k: u32| (i / 3_i64.pow(k)) % 3 - 1;
        [c[0] + o(0), c[1] + o(1), c[2] + o(2), c[3] + o(3)]
    })
}

struct Seg {
    id: EdgeId,
    a: DVec3,
    b: DVec3,
    tag: Tag,
}

fn run(model: &mut Model, tags: BTreeMap<VertexId, Tag>) {
    if tags.is_empty() {
        return;
    }
    let tol = model.tol();
    let tag_of = |v: &VertexId| tags.get(v).copied().unwrap_or(Tag::Still);
    let face_tag = |f: &prism_topo::Face| {
        f.loops()
            .flatten()
            .map(tag_of)
            .reduce(combine)
            .unwrap_or(Tag::Still)
    };

    // Faces by plane cell, and the planes the edit touched.
    let mut cells: BTreeMap<Cell, Vec<FaceId>> = BTreeMap::new();
    let mut planes: Vec<Plane> = Vec::new();
    let mut plane_cells: BTreeMap<Cell, Vec<usize>> = BTreeMap::new();
    for (id, f) in model.faces() {
        let c = cell(f.plane(), &tol);
        cells.entry(c).or_default().push(id);
        if face_tag(f) == Tag::Still {
            continue;
        }
        let p = *f.plane();
        let known = around(c).any(|k| {
            plane_cells
                .get(&k)
                .is_some_and(|ix| ix.iter().any(|i| planes[*i].same_surface(&p, &tol)))
        });
        if !known {
            plane_cells.entry(c).or_default().push(planes.len());
            planes.push(p);
        }
    }

    let edge_ids: BTreeMap<(VertexId, VertexId), EdgeId> =
        model.edges().map(|(id, e)| ((e.a, e.b), id)).collect();
    let mut in_faces: BTreeSet<EdgeId> = BTreeSet::new();
    for (_, f) in model.faces() {
        for (a, b) in f.loops().flat_map(|l| loop_pairs(l)) {
            if let Some(e) = edge_ids.get(&(a.min(b), a.max(b))) {
                in_faces.insert(*e);
            }
        }
    }
    let loose: Vec<EdgeId> = model
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !in_faces.contains(id))
        .collect();

    // Find the planes with something to do, before changing anything.
    let mut work: Vec<(Plane, Vec<DVec3>)> = Vec::new();
    for plane in &planes {
        let faces: Vec<FaceId> = around(cell(plane, &tol))
            .filter_map(|k| cells.get(&k))
            .flatten()
            .copied()
            .filter(|f| {
                model.face(*f).is_some_and(|face| {
                    face.plane().same_surface(plane, &tol)
                        && face
                            .loops()
                            .flatten()
                            .all(|v| plane.contains(model.vertex(*v), &tol))
                })
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut edges: BTreeSet<EdgeId> = BTreeSet::new();
        for f in &faces {
            let face = model.face(*f).expect("face");
            for (a, b) in face.loops().flat_map(|l| loop_pairs(l)) {
                if let Some(e) = edge_ids.get(&(a.min(b), a.max(b))) {
                    edges.insert(*e);
                }
            }
        }
        for e in &loose {
            let ed = model.edge(*e).expect("edge");
            if plane.contains(model.vertex(ed.a), &tol) && plane.contains(model.vertex(ed.b), &tol)
            {
                edges.insert(*e);
            }
        }
        let segs: Vec<Seg> = edges
            .iter()
            .map(|id| {
                let e = model.edge(*id).expect("edge");
                Seg {
                    id: *id,
                    a: model.vertex(e.a),
                    b: model.vertex(e.b),
                    tag: combine(tag_of(&e.a), tag_of(&e.b)),
                }
            })
            .collect();
        // Everything on the plane moved together: nothing can have changed.
        let mut kinds = segs.iter().map(|s| s.tag);
        let first = kinds.next();
        if let Some(Tag::Group(k)) = first
            && kinds.all(|t| t == Tag::Group(k))
        {
            continue;
        }
        let points = meeting_points(&segs, &tol);
        if !points.is_empty() || contains_another(model, &faces, plane, &face_tag) {
            work.push((*plane, points));
        }
    }

    for (plane, points) in work {
        let before: BTreeSet<EdgeId> = model.edges().map(|(id, _)| id).collect();
        for p in points {
            split_at(model, p, &tol);
        }
        let new: BTreeSet<EdgeId> = model
            .edges()
            .map(|(id, _)| id)
            .filter(|id| !before.contains(id))
            .collect();
        rebuild_plane(model, &plane, &new, &tol, Fill::Closing);
    }
}

/// Where pairs of segments that could have changed cross or touch.
fn meeting_points(segs: &[Seg], tol: &Tol) -> Vec<DVec3> {
    let pad = DVec3::splat(tol.dist);
    let boxes: Vec<(DVec3, DVec3)> = segs
        .iter()
        .map(|s| (s.a.min(s.b) - pad, s.a.max(s.b) + pad))
        .collect();
    let mut points = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        if s.tag == Tag::Still {
            continue;
        }
        for (j, t) in segs.iter().enumerate() {
            if i == j || s.id == t.id || !worth_checking(s.tag, t.tag) {
                continue;
            }
            // Each unordered pair once, unless only one side is moving.
            if t.tag != Tag::Still && j < i {
                continue;
            }
            let (lo, hi) = (boxes[i], boxes[j]);
            if lo.0.cmpgt(hi.1).any() || hi.0.cmpgt(lo.1).any() {
                continue;
            }
            if let Some((_, q)) = crossing(s.a, s.b, t.a, t.b, tol) {
                points.push(q);
            }
            for p in [t.a, t.b] {
                if on_segment_inside(p, s.a, s.b, tol).is_some() {
                    points.push(p);
                }
            }
            for p in [s.a, s.b] {
                if on_segment_inside(p, t.a, t.b, tol).is_some() {
                    points.push(p);
                }
            }
        }
    }
    points
}

/// A face's tag, outer loop in 2D, its vertices and its 2D box.
type FlatFace = (Tag, Vec<DVec2>, Vec<VertexId>, (DVec2, DVec2));

/// Whether a face that could have moved now has a corner strictly inside
/// another face on the plane, or the other way round (a face moved wholly
/// inside another, touching no edge).
fn contains_another(
    model: &Model,
    faces: &[FaceId],
    plane: &Plane,
    face_tag: &impl Fn(&prism_topo::Face) -> Tag,
) -> bool {
    let frame = plane.frame();
    let flat: Vec<FlatFace> = faces
        .iter()
        .filter_map(|f| model.face(*f))
        .map(|f| {
            let pts: Vec<DVec2> = f
                .outer
                .iter()
                .map(|v| frame.to_2d(model.vertex(*v)))
                .collect();
            let lo = pts.iter().fold(DVec2::INFINITY, |a, p| a.min(*p));
            let hi = pts.iter().fold(DVec2::NEG_INFINITY, |a, p| a.max(*p));
            (face_tag(f), pts, f.outer.clone(), (lo, hi))
        })
        .collect();
    for (i, (ta, pa, va, ba)) in flat.iter().enumerate() {
        if *ta == Tag::Still {
            continue;
        }
        for (j, (tb, pb, vb, bb)) in flat.iter().enumerate() {
            if i == j || !worth_checking(*ta, *tb) {
                continue;
            }
            if ba.0.cmpgt(bb.1).any() || bb.0.cmpgt(ba.1).any() {
                continue;
            }
            let inside = pa
                .iter()
                .zip(va)
                .any(|(p, v)| !vb.contains(v) && point_in_polygon(*p, pb) == Inside::Inside);
            let around = pb
                .iter()
                .zip(vb)
                .any(|(p, v)| !va.contains(v) && point_in_polygon(*p, pa) == Inside::Inside);
            if inside || around {
                return true;
            }
        }
    }
    false
}

/// Make sure a vertex is at `p` (reusing one within tolerance) and split
/// every edge that passes through it.
fn split_at(model: &mut Model, p: DVec3, tol: &Tol) {
    let near = model
        .vertices()
        .find(|(_, q)| tol.same_point(p, *q))
        .map(|(id, _)| id);
    let v = near.unwrap_or_else(|| model.add_vertex(p));
    let at = model.vertex(v);
    let through: Vec<EdgeId> = model
        .edges()
        .filter(|(_, e)| !e.has(v))
        .filter(|(_, e)| on_segment_inside(at, model.vertex(e.a), model.vertex(e.b), tol).is_some())
        .map(|(id, _)| id)
        .collect();
    for e in through {
        model.split_edge(e, v);
    }
}
