//! Prism's topology: vertices, edges and faces with holes, with stable ids.
//!
//! - Ids are never reused: each kind has a counter that only goes up.
//! - Everything is kept in ordered maps, so iteration order and the saved
//!   form are the same on every run.
//! - Non-manifold is allowed: loose vertices and edges, loose faces, and any
//!   number of faces on an edge.
//! - A face owns its surface (a plane for now), and its vertices lie on it.

use std::collections::BTreeMap;

use prism_geom::{Curve, Surface};
use prism_math::{DVec3, Plane, Tol, newell};
use serde::{Deserialize, Serialize};

macro_rules! id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub struct $name(pub u32);
    };
}
id!(VertexId);
id!(EdgeId);
id!(FaceId);
id!(GuideId);

/// A guide: a construction line or point the tools make (Tape Measure,
/// Protractor). Guides are not geometry: they bound no face and edges do
/// not stick to them, but inference snaps to them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Guide {
    /// An infinite line through `origin` along unit `dir`.
    Line {
        origin: DVec3,
        dir: DVec3,
    },
    Point {
        at: DVec3,
    },
}

/// An edge between two vertices. `a` is always the smaller id.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub a: VertexId,
    pub b: VertexId,
    pub curve: Curve,
}

impl Edge {
    pub fn has(&self, v: VertexId) -> bool {
        self.a == v || self.b == v
    }

    pub fn other(&self, v: VertexId) -> VertexId {
        if self.a == v { self.b } else { self.a }
    }
}

/// A face: an outer loop, any number of hole loops, and the surface.
/// The outer loop winds counter-clockwise about the surface normal; holes
/// wind the other way.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Face {
    pub outer: Vec<VertexId>,
    pub holes: Vec<Vec<VertexId>>,
    pub surface: Surface,
}

impl Face {
    /// The outer loop, then each hole.
    pub fn loops(&self) -> impl Iterator<Item = &Vec<VertexId>> {
        std::iter::once(&self.outer).chain(self.holes.iter())
    }

    pub fn plane(&self) -> &Plane {
        self.surface.plane()
    }

    pub fn uses_vertex(&self, v: VertexId) -> bool {
        self.loops().any(|l| l.contains(&v))
    }
}

/// The pairs of consecutive vertices of a loop, closing back to the start.
pub fn loop_pairs(l: &[VertexId]) -> impl Iterator<Item = (VertexId, VertexId)> + '_ {
    (0..l.len()).map(move |i| (l[i], l[(i + 1) % l.len()]))
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// The model: plain data, cloned to make undo snapshots.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Model {
    vertices: BTreeMap<VertexId, DVec3>,
    edges: BTreeMap<EdgeId, Edge>,
    faces: BTreeMap<FaceId, Face>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    guides: BTreeMap<GuideId, Guide>,
    next_vertex: u32,
    next_edge: u32,
    next_face: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    next_guide: u32,
}

/// A model taken apart: every item with its id, and the id counters.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelParts {
    pub vertices: Vec<(VertexId, DVec3)>,
    pub edges: Vec<(EdgeId, Edge)>,
    pub faces: Vec<(FaceId, Face)>,
    pub guides: Vec<(GuideId, Guide)>,
    pub counters: Counters,
}

/// The id counters, kept across undo so ids are never handed out twice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counters {
    pub vertex: u32,
    pub edge: u32,
    pub face: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub guide: u32,
}

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    // ---- vertices ----

    /// Add a vertex at `p`, without looking for an existing one.
    pub fn add_vertex(&mut self, p: DVec3) -> VertexId {
        let id = VertexId(self.next_vertex);
        self.next_vertex += 1;
        self.vertices.insert(id, p);
        id
    }

    pub fn vertex(&self, id: VertexId) -> DVec3 {
        self.vertices.get(&id).copied().unwrap_or(DVec3::NAN)
    }

    pub fn has_vertex(&self, id: VertexId) -> bool {
        self.vertices.contains_key(&id)
    }

    pub fn set_vertex(&mut self, id: VertexId, p: DVec3) {
        if let Some(v) = self.vertices.get_mut(&id) {
            *v = p;
        }
    }

    pub fn remove_vertex(&mut self, id: VertexId) {
        self.vertices.remove(&id);
    }

    pub fn vertices(&self) -> impl Iterator<Item = (VertexId, DVec3)> + '_ {
        self.vertices.iter().map(|(k, v)| (*k, *v))
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    // ---- edges ----

    /// The edge joining `a` and `b`, if there is one.
    pub fn edge_between(&self, a: VertexId, b: VertexId) -> Option<EdgeId> {
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        self.edges
            .iter()
            .find(|(_, e)| e.a == a && e.b == b)
            .map(|(id, _)| *id)
    }

    /// Join two vertices with a straight edge; an existing edge is reused.
    /// None if `a` and `b` are the same vertex.
    pub fn add_edge(&mut self, a: VertexId, b: VertexId) -> Option<EdgeId> {
        if a == b {
            return None;
        }
        if let Some(id) = self.edge_between(a, b) {
            return Some(id);
        }
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        let id = EdgeId(self.next_edge);
        self.next_edge += 1;
        self.edges.insert(
            id,
            Edge {
                a,
                b,
                curve: Curve::Line,
            },
        );
        Some(id)
    }

    /// Join two vertices with a new straight edge, without looking for an
    /// existing one (that scan is slow for big imports). The caller makes
    /// sure the edge is not there yet and `a` and `b` differ.
    pub fn add_new_edge(&mut self, a: VertexId, b: VertexId) -> EdgeId {
        debug_assert!(a != b);
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        let id = EdgeId(self.next_edge);
        self.next_edge += 1;
        self.edges.insert(
            id,
            Edge {
                a,
                b,
                curve: Curve::Line,
            },
        );
        id
    }

    pub fn edge(&self, id: EdgeId) -> Option<&Edge> {
        self.edges.get(&id)
    }

    pub fn edges(&self) -> impl Iterator<Item = (EdgeId, &Edge)> + '_ {
        self.edges.iter().map(|(k, v)| (*k, v))
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn remove_edge(&mut self, id: EdgeId) {
        self.edges.remove(&id);
    }

    /// Replace edge `id` by two edges through `mid`, and put `mid` into every
    /// face loop that ran along the edge. Returns the two new edges.
    pub fn split_edge(&mut self, id: EdgeId, mid: VertexId) -> Option<(EdgeId, EdgeId)> {
        let e = *self.edges.get(&id)?;
        self.edges.remove(&id);
        let first = self.add_edge(e.a, mid)?;
        let second = self.add_edge(mid, e.b)?;
        for face in self.faces.values_mut() {
            for l in std::iter::once(&mut face.outer).chain(face.holes.iter_mut()) {
                let n = l.len();
                for i in 0..n {
                    let (x, y) = (l[i], l[(i + 1) % n]);
                    if (x == e.a && y == e.b) || (x == e.b && y == e.a) {
                        l.insert(i + 1, mid);
                        break;
                    }
                }
            }
        }
        Some((first, second))
    }

    // ---- faces ----

    pub fn add_face(&mut self, face: Face) -> FaceId {
        let id = FaceId(self.next_face);
        self.next_face += 1;
        self.faces.insert(id, face);
        id
    }

    pub fn face(&self, id: FaceId) -> Option<&Face> {
        self.faces.get(&id)
    }

    pub fn face_mut(&mut self, id: FaceId) -> Option<&mut Face> {
        self.faces.get_mut(&id)
    }

    pub fn faces(&self) -> impl Iterator<Item = (FaceId, &Face)> + '_ {
        self.faces.iter().map(|(k, v)| (*k, v))
    }

    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    pub fn remove_face(&mut self, id: FaceId) -> Option<Face> {
        self.faces.remove(&id)
    }

    /// The positions of a loop's vertices.
    pub fn points(&self, l: &[VertexId]) -> Vec<DVec3> {
        l.iter().map(|v| self.vertex(*v)).collect()
    }

    /// A face's area: its outer loop minus its holes.
    pub fn face_area(&self, id: FaceId) -> f64 {
        let Some(f) = self.face(id) else { return 0.0 };
        let area = |l: &[VertexId]| newell(&self.points(l)).length() * 0.5;
        area(&f.outer) - f.holes.iter().map(|h| area(h)).sum::<f64>()
    }

    /// For every edge, the faces whose loops use it.
    pub fn edge_faces(&self) -> BTreeMap<EdgeId, Vec<FaceId>> {
        // Look edges up by their two vertices (a scan per pair was O(n²)).
        let by_ends: BTreeMap<(VertexId, VertexId), EdgeId> = self
            .edges()
            .map(|(id, e)| ((e.a.min(e.b), e.a.max(e.b)), id))
            .collect();
        let mut map: BTreeMap<EdgeId, Vec<FaceId>> = BTreeMap::new();
        for (fid, f) in &self.faces {
            for l in f.loops() {
                for (a, b) in loop_pairs(l) {
                    if let Some(e) = by_ends.get(&(a.min(b), a.max(b))) {
                        map.entry(*e).or_default().push(*fid);
                    }
                }
            }
        }
        map
    }

    // ---- guides ----

    pub fn add_guide(&mut self, g: Guide) -> GuideId {
        let id = GuideId(self.next_guide);
        self.next_guide += 1;
        self.guides.insert(id, g);
        id
    }

    pub fn guide(&self, id: GuideId) -> Option<&Guide> {
        self.guides.get(&id)
    }

    pub fn guides(&self) -> impl Iterator<Item = (GuideId, &Guide)> + '_ {
        self.guides.iter().map(|(k, v)| (*k, v))
    }

    pub fn remove_guide(&mut self, id: GuideId) {
        self.guides.remove(&id);
    }

    pub fn guide_count(&self) -> usize {
        self.guides.len()
    }

    // ---- whole model ----

    /// The tolerance for this model, from its size.
    pub fn tol(&self) -> Tol {
        Tol::for_points(self.vertices.values())
    }

    /// The bounding box of all vertices, or None if there are none.
    pub fn bbox(&self) -> Option<(DVec3, DVec3)> {
        let mut it = self.vertices.values();
        let first = *it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p))))
    }

    pub fn counters(&self) -> Counters {
        Counters {
            vertex: self.next_vertex,
            edge: self.next_edge,
            face: self.next_face,
            guide: self.next_guide,
        }
    }

    /// Never hand out an id below `c` again (after an undo).
    pub fn keep_counters_at_least(&mut self, c: Counters) {
        self.next_vertex = self.next_vertex.max(c.vertex);
        self.next_edge = self.next_edge.max(c.edge);
        self.next_face = self.next_face.max(c.face);
        self.next_guide = self.next_guide.max(c.guide);
    }

    /// Add everything in `other` beside what is here, with new ids (an
    /// import). Nothing is joined to what was here. Returns the new faces.
    pub fn append(&mut self, other: &Model) -> Vec<FaceId> {
        let mut vmap: BTreeMap<VertexId, VertexId> = BTreeMap::new();
        for (id, p) in other.vertices() {
            vmap.insert(id, self.add_vertex(p));
        }
        for (_, e) in other.edges() {
            // The vertices are all new, so the edges cannot be here yet.
            if let (Some(a), Some(b)) = (vmap.get(&e.a), vmap.get(&e.b))
                && a != b
            {
                let id = self.add_new_edge(*a, *b);
                if let Some(ne) = self.edges.get_mut(&id) {
                    ne.curve = e.curve;
                }
            }
        }
        let map = |l: &[VertexId]| l.iter().filter_map(|v| vmap.get(v).copied()).collect();
        let mut faces = Vec::new();
        for (_, f) in other.faces() {
            faces.push(self.add_face(Face {
                outer: map(&f.outer),
                holes: f.holes.iter().map(|h| map(h)).collect(),
                surface: f.surface,
            }));
        }
        for (_, g) in other.guides() {
            self.add_guide(*g);
        }
        faces
    }

    /// Everything in the model, with its ids and counters (for files).
    pub fn to_parts(&self) -> ModelParts {
        ModelParts {
            vertices: self.vertices().collect(),
            edges: self.edges.iter().map(|(k, v)| (*k, *v)).collect(),
            faces: self.faces.iter().map(|(k, v)| (*k, v.clone())).collect(),
            guides: self.guides.iter().map(|(k, v)| (*k, *v)).collect(),
            counters: self.counters(),
        }
    }

    /// A model from its parts. Refused if the parts do not fit together:
    /// a repeated id, an id at or above its counter, or the bookkeeping
    /// problems [`Model::bookkeeping_problems`] finds.
    pub fn from_parts(p: ModelParts) -> Result<Model, String> {
        fn ids<K: Ord + Copy + std::fmt::Debug, V>(
            items: Vec<(K, V)>,
            what: &str,
            below: impl Fn(K) -> bool,
        ) -> Result<BTreeMap<K, V>, String> {
            let mut map = BTreeMap::new();
            for (k, v) in items {
                if !below(k) {
                    return Err(format!("{what} {k:?} is not below its counter"));
                }
                if map.insert(k, v).is_some() {
                    return Err(format!("{what} {k:?} appears twice"));
                }
            }
            Ok(map)
        }
        let c = p.counters;
        let vertices = ids(p.vertices, "vertex", |k: VertexId| k.0 < c.vertex)?;
        if let Some((id, _)) = vertices.iter().find(|(_, v)| !v.is_finite()) {
            return Err(format!("vertex {id:?} is not a finite point"));
        }
        let finite = |g: &Guide| match *g {
            Guide::Line { origin, dir } => {
                origin.is_finite() && dir.is_finite() && dir != DVec3::ZERO
            }
            Guide::Point { at } => at.is_finite(),
        };
        if let Some((id, _)) = p.guides.iter().find(|(_, g)| !finite(g)) {
            return Err(format!("guide {id:?} is not finite"));
        }
        if let Some((id, _)) = p.faces.iter().find(|(_, f)| {
            let pl = f.plane();
            !(pl.origin.is_finite() && pl.normal.is_finite())
        }) {
            return Err(format!("face {id:?} has no finite plane"));
        }
        let m = Model {
            vertices,
            edges: ids(p.edges, "edge", |k: EdgeId| k.0 < c.edge)?,
            faces: ids(p.faces, "face", |k: FaceId| k.0 < c.face)?,
            guides: ids(p.guides, "guide", |k: GuideId| k.0 < c.guide)?,
            next_vertex: c.vertex,
            next_edge: c.edge,
            next_face: c.face,
            next_guide: c.guide,
        };
        let problems = m.bookkeeping_problems();
        if let Some(first) = problems.first() {
            return Err(first.clone());
        }
        Ok(m)
    }

    /// The saved form: deterministic JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<Model, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    /// The geometry alone, without the id counters, for comparing models.
    pub fn geometry_json(&self) -> String {
        let mut copy = self.clone();
        copy.next_vertex = 0;
        copy.next_edge = 0;
        copy.next_face = 0;
        copy.next_guide = 0;
        copy.to_json()
    }

    /// Bookkeeping problems (empty if none): face loops use existing edges
    /// and vertices, edges join existing vertices, no edge is repeated.
    pub fn bookkeeping_problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut pairs = std::collections::BTreeSet::new();
        for (id, e) in &self.edges {
            if !self.has_vertex(e.a) || !self.has_vertex(e.b) {
                out.push(format!("edge {id:?} uses a missing vertex"));
            }
            if e.a >= e.b {
                out.push(format!(
                    "edge {id:?} is not ordered or joins a vertex to itself"
                ));
            }
            if !pairs.insert((e.a, e.b)) {
                out.push(format!("edge {id:?} repeats another edge"));
            }
        }
        for (id, f) in &self.faces {
            for l in f.loops() {
                if l.len() < 3 {
                    out.push(format!("face {id:?} has a loop of {} vertices", l.len()));
                }
                for (a, b) in loop_pairs(l) {
                    if !pairs.contains(&(a.min(b), a.max(b))) {
                        out.push(format!("face {id:?} uses a missing edge {a:?}-{b:?}"));
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(m: &mut Model) -> [VertexId; 4] {
        let v = [
            m.add_vertex(DVec3::new(0.0, 0.0, 0.0)),
            m.add_vertex(DVec3::new(4.0, 0.0, 0.0)),
            m.add_vertex(DVec3::new(4.0, 4.0, 0.0)),
            m.add_vertex(DVec3::new(0.0, 4.0, 0.0)),
        ];
        for i in 0..4 {
            m.add_edge(v[i], v[(i + 1) % 4]);
        }
        m.add_face(Face {
            outer: v.to_vec(),
            holes: vec![],
            surface: Surface::Plane(Plane::new(DVec3::ZERO, DVec3::Z)),
        });
        v
    }

    #[test]
    fn edges_are_unique_and_ordered() {
        let mut m = Model::new();
        let v = square(&mut m);
        assert_eq!(m.add_edge(v[1], v[0]), m.edge_between(v[0], v[1]));
        assert_eq!(m.edge_count(), 4);
        assert_eq!(m.add_edge(v[0], v[0]), None);
        assert!(m.bookkeeping_problems().is_empty());
    }

    #[test]
    fn ids_are_never_reused() {
        let mut m = Model::new();
        let a = m.add_vertex(DVec3::ZERO);
        m.remove_vertex(a);
        let b = m.add_vertex(DVec3::ZERO);
        assert_ne!(a, b);
    }

    #[test]
    fn splitting_an_edge_updates_face_loops() {
        let mut m = Model::new();
        let v = square(&mut m);
        let e = m.edge_between(v[0], v[1]).expect("edge");
        let mid = m.add_vertex(DVec3::new(2.0, 0.0, 0.0));
        m.split_edge(e, mid);
        let f = m.faces().next().expect("face").1;
        assert_eq!(f.outer, vec![v[0], mid, v[1], v[2], v[3]]);
        assert!(m.bookkeeping_problems().is_empty());
        assert_eq!(m.face_area(FaceId(0)), 16.0);
    }

    #[test]
    fn json_round_trip_is_exact() {
        let mut m = Model::new();
        square(&mut m);
        let text = m.to_json();
        let back = Model::from_json(&text).expect("reads");
        assert_eq!(back, m);
        assert_eq!(back.to_json(), text);
    }
}
