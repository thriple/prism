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

use prism_geom::Curve;
use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3, Plane, Tol};
use prism_topo::{EdgeId, FaceId, Model, VertexId};

use crate::regions::{Fill, InsideTest, rebuild_plane};
use crate::sticky::{meet, on_curve_inside};

/// After an edit that changed the touched vertices freely (Push/Pull).
/// Overlapping faces are left for the solid rule; call [`dedupe_faces`]
/// after it.
pub fn settle(model: &mut Model, touched: &BTreeSet<VertexId>) {
    settle_with(model, touched, &[], None);
}

/// [`settle`], knowing exactly what is inside the solid the edit makes
/// (Push/Pull on a solid, [`crate::solid::Swept`]): every touched plane is
/// rebuilt, and a region gets a face where the solid is on one side only.
/// `planes` are rebuilt too (planes of faces the edit may have removed).
pub(crate) fn settle_with(
    model: &mut Model,
    touched: &BTreeSet<VertexId>,
    planes: &[Plane],
    inside: Option<InsideTest>,
) {
    let tags = touched.iter().map(|v| (*v, Tag::Free)).collect();
    run(model, tags, planes, inside);
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
    run(model, tags, &[], None);
    dedupe_faces(model);
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
    curve: Curve,
    a: DVec3,
    b: DVec3,
    tag: Tag,
}

fn run(
    model: &mut Model,
    tags: BTreeMap<VertexId, Tag>,
    extra: &[Plane],
    inside: Option<InsideTest>,
) {
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
        // The coplanar rule is for flat faces.
        let Some(&p) = f.plane() else { continue };
        let c = cell(&p, &tol);
        cells.entry(c).or_default().push(id);
        if face_tag(f) == Tag::Still {
            continue;
        }
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

    for p in extra {
        let c = cell(p, &tol);
        let known = around(c).any(|k| {
            plane_cells
                .get(&k)
                .is_some_and(|ix| ix.iter().any(|i| planes[*i].same_surface(p, &tol)))
        });
        if !known {
            plane_cells.entry(c).or_default().push(planes.len());
            planes.push(*p);
        }
    }

    let in_faces: BTreeSet<EdgeId> = model.faces().flat_map(|(_, f)| f.edge_ids()).collect();
    let mut edges_at: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for (id, e) in model.edges() {
        edges_at.entry(e.a).or_default().push(id);
        edges_at.entry(e.b).or_default().push(id);
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
                    face.plane().is_some_and(|p| p.same_surface(plane, &tol))
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
            edges.extend(model.face(*f).expect("face").edge_ids());
        }
        // Edges of faces in other planes can lie in this one too (a wall's
        // corner edge along this plane): take those at this plane's points.
        let mut at: BTreeSet<VertexId> = BTreeSet::new();
        for f in &faces {
            at.extend(model.face(*f).expect("face").loops().flatten().copied());
        }
        for v in &at {
            for e in edges_at.get(v).into_iter().flatten() {
                let ed = model.edge(*e).expect("edge");
                if plane.contains(model.vertex(ed.a), &tol)
                    && plane.contains(model.vertex(ed.b), &tol)
                {
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
                    curve: e.curve,
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
        if inside.is_some()
            || !points.is_empty()
            || contains_another(model, &faces, plane, &face_tag)
        {
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
        rebuild_plane(
            model,
            &plane,
            &new,
            &tol,
            Fill::Closing(inside),
            crate::solid::step(&tol),
        );
    }
}

/// Where pairs of segments that could have changed cross or touch.
fn meeting_points(segs: &[Seg], tol: &Tol) -> Vec<DVec3> {
    let pad = DVec3::splat(tol.dist);
    // An arc's box is its whole circle's, which holds it.
    let boxes: Vec<(DVec3, DVec3)> = segs
        .iter()
        .map(|s| match s.curve {
            Curve::Arc { centre, radius, .. } => {
                let r = DVec3::splat(radius) + pad;
                (centre - r, centre + r)
            }
            Curve::Line => (s.a.min(s.b) - pad, s.a.max(s.b) + pad),
        })
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
            points.extend(meet((s.curve, s.a, s.b), (t.curve, t.a, t.b), tol));
            for p in [t.a, t.b] {
                if on_curve_inside(s.curve, s.a, s.b, p, tol) {
                    points.push(p);
                }
            }
            for p in [s.a, s.b] {
                if on_curve_inside(t.curve, t.a, t.b, p, tol) {
                    points.push(p);
                }
            }
        }
    }
    points
}

/// A face's tag, its outer vertices in 2D, the outer loop's polygon along
/// its edges (arcs drawn), its outer vertices and its 2D box.
type FlatFace = (Tag, Vec<DVec2>, Vec<DVec2>, Vec<VertexId>, (DVec2, DVec2));

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
            let poly: Vec<DVec2> = model
                .loop_points(f, 0)
                .iter()
                .map(|p| frame.to_2d(*p))
                .collect();
            let lo = poly.iter().fold(DVec2::INFINITY, |a, p| a.min(*p));
            let hi = poly.iter().fold(DVec2::NEG_INFINITY, |a, p| a.max(*p));
            (face_tag(f), pts, poly, f.outer.clone(), (lo, hi))
        })
        .collect();
    for (i, (ta, pa, polya, va, ba)) in flat.iter().enumerate() {
        if *ta == Tag::Still {
            continue;
        }
        for (j, (tb, pb, polyb, vb, bb)) in flat.iter().enumerate() {
            if i == j || !worth_checking(*ta, *tb) {
                continue;
            }
            if ba.0.cmpgt(bb.1).any() || bb.0.cmpgt(ba.1).any() {
                continue;
            }
            let inside = pa
                .iter()
                .zip(va)
                .any(|(p, v)| !vb.contains(v) && point_in_polygon(*p, polyb) == Inside::Inside);
            let around = pb
                .iter()
                .zip(vb)
                .any(|(p, v)| !va.contains(v) && point_in_polygon(*p, polya) == Inside::Inside);
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
        .filter(|(_, e)| on_curve_inside(e.curve, model.vertex(e.a), model.vertex(e.b), at, tol))
        .map(|(id, _)| id)
        .collect();
    for e in through {
        model.split_edge(e, v);
    }
}

/// Faces with the same loops facing the same way (overlapping faces left
/// by an edit) become one: the lowest id stays.
pub fn dedupe_faces(model: &mut Model) {
    // Keyed by sides (start and edge), not corners: two arcs can join the
    // same two points (the halves of a sphere both run pole to pole).
    type Key = Vec<(VertexId, EdgeId)>;
    let mut seen: BTreeMap<Key, FaceId> = BTreeMap::new();
    let mut extra = Vec::new();
    for (id, f) in model.faces() {
        // The outer loop from its smallest side, in its own direction.
        let outer: Key = f.loop_edges(0).map(|s| (s.from, s.edge)).collect();
        let n = outer.len();
        let start = (0..n).min_by_key(|i| outer[*i]).unwrap_or(0);
        let mut key: Key = (0..n).map(|i| outer[(start + i) % n]).collect();
        let mut holes: Vec<Key> = (1..=f.holes.len())
            .map(|k| {
                let mut h: Key = f.loop_edges(k).map(|s| (s.from, s.edge)).collect();
                h.sort();
                h
            })
            .collect();
        holes.sort();
        for h in holes {
            key.push((VertexId(u32::MAX), EdgeId(u32::MAX)));
            key.extend(h);
        }
        if let std::collections::btree_map::Entry::Vacant(e) = seen.entry(key) {
            e.insert(id);
        } else {
            extra.push(id);
        }
    }
    for id in extra {
        model.remove_face(id);
    }
}
