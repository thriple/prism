//! Selections, removing them, copying them, and what is connected.

use std::collections::{BTreeMap, BTreeSet};

use prism_topo::{EdgeId, FaceId, GuideId, Model, VertexId, loop_pairs};
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
    let pairs: BTreeSet<(VertexId, VertexId)> = gone_edges
        .iter()
        .filter_map(|e| model.edge(*e))
        .map(|e| (e.a, e.b))
        .collect();
    let uses = |a: VertexId, b: VertexId| pairs.contains(&(a.min(b), a.max(b)));

    let mut gone_faces = sel.faces.clone();
    let ids: Vec<FaceId> = model.faces().map(|(id, _)| id).collect();
    for id in ids {
        let Some(f) = model.face_mut(id) else {
            continue;
        };
        if loop_pairs(&f.outer).any(|(a, b)| uses(a, b))
            || sel.vertices.iter().any(|v| f.outer.contains(v))
        {
            gone_faces.insert(id);
        }
        f.holes.retain(|h| !loop_pairs(h).any(|(a, b)| uses(a, b)));
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
            .filter_map(|f| model.face(*f).map(|x| (*f, *x.plane())))
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
    let mut pairs: BTreeSet<(VertexId, VertexId)> = sel
        .edges
        .iter()
        .filter_map(|e| model.edge(*e))
        .map(|e| (e.a, e.b))
        .collect();
    let faces: Vec<_> = sel
        .faces
        .iter()
        .filter_map(|f| model.face(*f).cloned())
        .collect();
    for f in &faces {
        for l in f.loops() {
            pairs.extend(loop_pairs(l).map(|(a, b)| (a.min(b), a.max(b))));
        }
    }
    let mut out = Selection::default();
    for (a, b) in pairs {
        if let Some(e) = model.add_edge(map[&a], map[&b]) {
            out.edges.insert(e);
        }
    }
    out.vertices = sel.vertices.iter().map(|v| map[v]).collect();
    for mut f in faces {
        f.outer = f.outer.iter().map(|v| map[v]).collect();
        for h in &mut f.holes {
            *h = h.iter().map(|v| map[v]).collect();
        }
        out.faces.insert(model.add_face(f));
    }
    out
}
