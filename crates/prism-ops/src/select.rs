//! Selections, removing them, copying them, and what is connected.

use std::collections::{BTreeMap, BTreeSet};

use prism_topo::{EdgeId, FaceId, GuideId, Model, VertexId};
use serde::{Deserialize, Serialize};

/// Some vertices, edges and faces.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub vertices: BTreeSet<VertexId>,
    pub edges: BTreeSet<EdgeId>,
    pub faces: BTreeSet<FaceId>,
    /// Guides (only the Eraser picks them, for now).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub guides: BTreeSet<GuideId>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
            && self.edges.is_empty()
            && self.faces.is_empty()
            && self.guides.is_empty()
    }

    pub fn edge(e: EdgeId) -> Self {
        Self {
            edges: [e].into(),
            ..Default::default()
        }
    }

    /// Every vertex the selection uses.
    pub fn all_vertices(&self, model: &Model) -> BTreeSet<VertexId> {
        let mut vs = self.vertices.clone();
        for e in &self.edges {
            if let Some(e) = model.edge(*e) {
                vs.insert(e.a);
                vs.insert(e.b);
            }
        }
        for f in &self.faces {
            if let Some(f) = model.face(*f) {
                vs.extend(f.loops().flatten().copied());
            }
        }
        vs
    }
}

/// Erase a selection. Removing an edge removes the faces whose outer loop
/// uses it; removing an edge of a hole closes the hole. Removing a vertex
/// removes its edges. Vertices left with no edges go too.
pub fn remove(model: &mut Model, sel: &Selection) {
    for g in &sel.guides {
        model.remove_guide(*g);
    }
    let mut sel = sel.clone();
    merge_across(model, &mut sel);
    let sel = &sel;
    let mut gone_edges = sel.edges.clone();
    for v in &sel.vertices {
        gone_edges.extend(model.edges().filter(|(_, e)| e.has(*v)).map(|(id, _)| id));
    }
    let uses =
        |f: &prism_topo::Face, k: usize| f.loop_edges(k).any(|s| gone_edges.contains(&s.edge));

    let mut gone_faces = sel.faces.clone();
    let ids: Vec<FaceId> = model.faces().map(|(id, _)| id).collect();
    for id in ids {
        let Some(f) = model.face(id) else {
            continue;
        };
        if uses(f, 0) || sel.vertices.iter().any(|v| f.outer.contains(v)) {
            gone_faces.insert(id);
        }
        // Holes that run along a gone edge go.
        let keep: Vec<bool> = (1..=f.holes.len()).map(|k| !uses(f, k)).collect();
        if keep.contains(&false) {
            model.edit_face(id, |f| {
                let mut keep = keep.into_iter();
                f.holes.retain(|_| keep.next().unwrap_or(true));
            });
        }
    }
    for f in gone_faces {
        model.remove_face(f);
    }
    for e in gone_edges {
        model.remove_edge(e);
    }
    let used: BTreeSet<VertexId> = model.edges().flat_map(|(_, e)| [e.a, e.b]).collect();
    let unused: Vec<VertexId> = model
        .vertices()
        .map(|(id, _)| id)
        .filter(|v| !used.contains(v))
        .collect();
    for v in unused {
        model.remove_vertex(v);
    }
}

/// Erasing an edge between two faces on one plane merges them into one
/// face, as in SketchUp (DECISIONS #15): the edge goes, and the plane's
/// faces are rebuilt, so the two become one. Those edges are taken out of
/// `sel`; erasing the rest works as before.
fn merge_across(model: &mut Model, sel: &mut Selection) {
    if sel.edges.is_empty() {
        return;
    }
    let tol = model.tol();
    let edge_faces = model.edge_faces();
    let mut planes: Vec<prism_math::Plane> = Vec::new();
    let mut merged = Vec::new();
    let mut others = Vec::new();
    for e in sel.edges.clone() {
        let Some(fs) = edge_faces.get(&e) else {
            continue;
        };
        let ends_kept = model
            .edge(e)
            .is_some_and(|ed| !sel.vertices.contains(&ed.a) && !sel.vertices.contains(&ed.b));
        if !ends_kept {
            continue;
        }
        // Exactly two of the edge's faces on one plane, facing the same way
        // (with two boxes side by side, a wall between them can share the
        // edge too: it goes, as erasing an edge always takes its faces).
        let flat: Vec<(FaceId, prism_math::Plane)> = fs
            .iter()
            .filter_map(|f| model.face(*f).and_then(|x| Some((*f, *x.plane()?))))
            .collect();
        let mut pair = None;
        for i in 0..flat.len() {
            for j in i + 1..flat.len() {
                let (pa, pb) = (flat[i].1, flat[j].1);
                if pa.same_surface(&pb, &tol) && pa.normal.dot(pb.normal) > 0.0 {
                    pair = Some(if pair.is_none() {
                        (i, j)
                    } else {
                        (usize::MAX, 0)
                    });
                }
            }
        }
        let Some((i, j)) = pair.filter(|p| p.0 != usize::MAX) else {
            continue;
        };
        let pa = flat[i].1;
        for (k, (f, _)) in flat.iter().enumerate() {
            if k != i && k != j {
                others.push(*f);
            }
        }
        merged.push(e);
        if !planes.iter().any(|p| p.same_surface(&pa, &tol)) {
            planes.push(pa);
        }
    }
    if merged.is_empty() {
        return;
    }
    for f in others {
        model.remove_face(f);
    }
    for e in &merged {
        sel.edges.remove(e);
        model.remove_edge(*e);
    }
    for p in planes {
        crate::regions::rebuild_plane(
            model,
            &p,
            &BTreeSet::new(),
            &tol,
            crate::regions::Fill::Drawn,
            0.0,
        );
    }
}

/// Everything reachable from `start` along edges, and across faces: a face
/// with a hole joins its outer loop to the hole, so a pocket comes with the
/// box it is cut into.
pub fn connected(model: &Model, start: VertexId) -> Selection {
    let mut adj: BTreeMap<VertexId, Vec<VertexId>> = BTreeMap::new();
    for (_, e) in model.edges() {
        adj.entry(e.a).or_default().push(e.b);
        adj.entry(e.b).or_default().push(e.a);
    }
    for (_, f) in model.faces() {
        let Some(&first) = f.outer.first() else {
            continue;
        };
        for h in &f.holes {
            if let Some(&v) = h.first() {
                adj.entry(first).or_default().push(v);
                adj.entry(v).or_default().push(first);
            }
        }
    }
    let mut seen: BTreeSet<VertexId> = [start].into();
    let mut stack = vec![start];
    while let Some(v) = stack.pop() {
        for n in adj.get(&v).into_iter().flatten() {
            if seen.insert(*n) {
                stack.push(*n);
            }
        }
    }
    Selection {
        edges: model
            .edges()
            .filter(|(_, e)| seen.contains(&e.a))
            .map(|(id, _)| id)
            .collect(),
        faces: model
            .faces()
            .filter(|(_, f)| seen.contains(&f.outer[0]))
            .map(|(id, _)| id)
            .collect(),
        vertices: seen,
        guides: BTreeSet::new(),
    }
}

/// Copy a selection in place (the copy is not stuck to anything). Returns
/// the copy.
pub fn duplicate(model: &mut Model, sel: &Selection) -> Selection {
    let vs = sel.all_vertices(model);
    let map: BTreeMap<VertexId, VertexId> = vs
        .iter()
        .map(|v| (*v, model.add_vertex(model.vertex(*v))))
        .collect();
    let faces: Vec<_> = sel
        .faces
        .iter()
        .filter_map(|f| model.face(*f).cloned())
        .collect();
    let mut edges: BTreeSet<EdgeId> = sel.edges.clone();
    for f in &faces {
        edges.extend(f.edge_ids());
    }
    let mut out = Selection::default();
    let mut emap: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut cmap = BTreeMap::new();
    for e in edges {
        let Some(ed) = model.edge(e).copied() else {
            continue;
        };
        if let Some(c) = copy_edge(model, &ed, map[&ed.a], map[&ed.b], &mut cmap) {
            out.edges.insert(c);
            emap.insert(e, c);
        }
    }
    out.vertices = sel.vertices.iter().map(|v| map[v]).collect();
    for f in faces {
        out.faces.insert(model.add_face(copy_face(&f, &map, &emap)));
    }
    out
}

/// Copy edge `e` between `a` and `b` (its ends' copies) into `to`: a line
/// is reused if there, an arc keeps its curve, and a curve id gets a new
/// one per curve (in `cmap`).
fn copy_edge(
    to: &mut Model,
    e: &prism_topo::Edge,
    a: VertexId,
    b: VertexId,
    cmap: &mut BTreeMap<prism_topo::CurveId, prism_topo::CurveId>,
) -> Option<EdgeId> {
    let cid = e
        .curve_id
        .map(|c| *cmap.entry(c).or_insert_with(|| to.new_curve_id()));
    let id = if e.curve.is_arc() {
        // `a` and `b` stand for the edge's own `a` and `b`, so the arc runs
        // from `a` to `b` as before.
        to.add_curve_edge(a, b, e.curve, cid)?
    } else {
        let id = to.add_edge(a, b)?;
        to.set_curve_id(id, cid);
        id
    };
    to.set_soft(id, e.soft, e.smooth);
    Some(id)
}

/// A face with its vertices and edges swapped for their copies.
fn copy_face(
    f: &prism_topo::Face,
    vmap: &BTreeMap<VertexId, VertexId>,
    emap: &BTreeMap<EdgeId, EdgeId>,
) -> prism_topo::Face {
    let vs = |l: &[VertexId]| -> Vec<VertexId> { l.iter().map(|v| vmap[v]).collect() };
    let edges = (0..=f.holes.len())
        .map(|k| {
            f.loop_edge_ids(k)
                .iter()
                .filter_map(|e| emap.get(e).copied())
                .collect()
        })
        .collect();
    let mut g = prism_topo::Face::with_edges(
        vs(&f.outer),
        f.holes.iter().map(|h| vs(h)).collect(),
        f.surface,
        edges,
    );
    g.paint = f.paint;
    g
}

/// Move a selection out into a new model (Make Group). Edges and points
/// that geometry left behind still uses are copied, not moved, so what
/// stays is whole. The new model has new ids.
pub fn take(model: &mut Model, sel: &Selection) -> Model {
    let mut edges: BTreeSet<EdgeId> = sel.edges.clone();
    for f in &sel.faces {
        if let Some(face) = model.face(*f) {
            edges.extend(face.edge_ids());
        }
    }
    let mut verts = sel.all_vertices(model);
    for e in &edges {
        if let Some(ed) = model.edge(*e) {
            verts.insert(ed.a);
            verts.insert(ed.b);
        }
    }

    let mut out = Model::new();
    let vmap: BTreeMap<VertexId, VertexId> = verts
        .iter()
        .map(|v| (*v, out.add_vertex(model.vertex(*v))))
        .collect();
    let mut emap: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut cmap = BTreeMap::new();
    for e in &edges {
        if let Some(ed) = model.edge(*e)
            && let Some(c) = copy_edge(&mut out, ed, vmap[&ed.a], vmap[&ed.b], &mut cmap)
        {
            emap.insert(*e, c);
        }
    }
    for f in &sel.faces {
        if let Some(face) = model.face(*f) {
            out.add_face(copy_face(face, &vmap, &emap));
        }
    }
    for g in &sel.guides {
        if let Some(guide) = model.guide(*g) {
            out.add_guide(*guide);
        }
        model.remove_guide(*g);
    }

    // What stays: drop the faces, then edges and points nothing else uses.
    for f in &sel.faces {
        model.remove_face(*f);
    }
    let mut used: BTreeSet<EdgeId> = BTreeSet::new();
    let mut used_verts: BTreeSet<VertexId> = BTreeSet::new();
    for (_, f) in model.faces() {
        used_verts.extend(f.loops().flatten().copied());
        used.extend(f.edge_ids());
    }
    for e in &edges {
        if !used.contains(e) {
            model.remove_edge(*e);
        }
    }
    let on_edges: BTreeSet<VertexId> = model.edges().flat_map(|(_, e)| [e.a, e.b]).collect();
    for v in &verts {
        if !on_edges.contains(v) && !used_verts.contains(v) {
            model.remove_vertex(*v);
        }
    }
    out
}

/// The faces of the smooth surface `face` is on: it and every face joined
/// to it through soft edges (Exact Curves 4c). Just `face` when none are.
pub fn surface(model: &Model, face: FaceId) -> BTreeSet<FaceId> {
    let mut out: BTreeSet<FaceId> = [face].into();
    if model.face(face).is_none() {
        return BTreeSet::new();
    }
    if !model.edges().any(|(_, e)| e.soft) {
        return out;
    }
    let edge_faces = model.edge_faces();
    let mut todo = vec![face];
    while let Some(f) = todo.pop() {
        let Some(x) = model.face(f) else { continue };
        for e in x.edge_ids() {
            if !model.edge(e).is_some_and(|x| x.soft) {
                continue;
            }
            for g in edge_faces.get(&e).into_iter().flatten() {
                if out.insert(*g) {
                    todo.push(*g);
                }
            }
        }
    }
    out
}
