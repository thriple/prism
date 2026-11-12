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
use prism_math::{DAffine3, DVec2, DVec3, Plane, Tol, newell};
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
id!(CurveId);
// A material in the document (Basic Materials): the document keeps the
// table, shared by every group's model; a face only names it.
id!(MaterialId);

/// What a face is painted with, front and back (Basic Materials, DECISIONS
/// #35). None is Default: drawn in the style's front or back colour, or a
/// painted group's material.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Paint {
    pub front: Option<MaterialId>,
    pub back: Option<MaterialId>,
    /// Where a flat face's texture sits (Basic Materials part 2), once the
    /// face has been moved, turned or scaled; None is the default placement
    /// ([`UvMap::default_for`]). Both sides share it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv: Option<UvMap>,
}

/// A flat face's texture placement: a point at the texture's corner and
/// the directions of its u and v, so a point `p` sits at
/// `((p - origin) · u, (p - origin) · v)` metres into the texture
/// (DECISIONS #35). Divided by the material's texture size, that is the
/// place in the image.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UvMap {
    pub origin: DVec3,
    pub u: DVec3,
    pub v: DVec3,
}

impl UvMap {
    /// The default placement on a plane, anchored to the model: u runs
    /// level along the face (along red on floors and ceilings), v runs up
    /// it (along green on floors), and the origin is the model's origin
    /// dropped onto the plane. Coplanar faces line up, and the rows of a
    /// texture stay level round a box's corners.
    pub fn default_for(plane: &Plane) -> UvMap {
        let n = plane.normal;
        let u = if n.z.abs() > 1.0 - 1e-9 {
            DVec3::X
        } else {
            DVec3::Z.cross(n).normalize()
        };
        UvMap {
            origin: n * n.dot(plane.origin),
            u,
            v: n.cross(u),
        }
    }

    /// Where `p` sits in the texture, in metres.
    pub fn at(&self, p: DVec3) -> DVec2 {
        let w = p - self.origin;
        DVec2::new(w.dot(self.u), w.dot(self.v))
    }

    /// The placement after the face moved by `m`: every point keeps its
    /// place in the texture.
    pub fn transformed(&self, m: &DAffine3) -> UvMap {
        // (m p - m o) · u' = (p - o) · u for all p: u' = M⁻ᵀ u.
        let inv_t = m.matrix3.inverse().transpose();
        UvMap {
            origin: m.transform_point3(self.origin),
            u: inv_t * self.u,
            v: inv_t * self.v,
        }
    }
}

impl Paint {
    pub fn is_default(&self) -> bool {
        self.front.is_none() && self.back.is_none()
    }

    /// The same paint seen from the other side.
    pub fn flipped(self) -> Paint {
        Paint {
            front: self.back,
            back: self.front,
            uv: self.uv,
        }
    }
}

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

/// An edge between two vertices. `a` is always the smaller id; an arc runs
/// from `a` to `b` (counter-clockwise about its axis).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub a: VertexId,
    pub b: VertexId,
    pub curve: Curve,
    /// The edges of one circle, arc or polygon share a curve id: they are
    /// picked, moved and erased together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curve_id: Option<CurveId>,
    /// Not drawn: a hidden line (Exact Curves 4c). The seams of a curved
    /// surface are soft.
    #[serde(default, skip_serializing_if = "is_false")]
    pub soft: bool,
    /// Shading runs smoothly across it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub smooth: bool,
    /// A fine straight segment standing in for a curve Prism cannot store
    /// exactly (an ellipse, a crossing of two cylinders): DECISIONS #32.
    /// Its ends are exact; the chord between them is not.
    #[serde(default, skip_serializing_if = "is_false")]
    pub approx: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Edge {
    /// The curve walked starting from end `v`.
    pub fn curve_from(&self, v: VertexId) -> Curve {
        if v == self.a {
            self.curve.clone()
        } else {
            self.curve.reversed()
        }
    }

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
///
/// Each loop also names its edges (Exact Curves part 1): edge `i` of a loop
/// joins vertex `i` to vertex `i + 1`. The model keeps them in step with
/// the vertices when a face is added or edited, and when an edge is split.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Face {
    pub outer: Vec<VertexId>,
    pub holes: Vec<Vec<VertexId>>,
    pub surface: Surface,
    /// One list per loop, the outer loop first. Empty until the face is in
    /// a model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    edges: Vec<Vec<EdgeId>>,
    /// Its materials (Basic Materials); Default unless painted.
    #[serde(default, skip_serializing_if = "Paint::is_default")]
    pub paint: Paint,
}

/// Stands for an edge a loop needs but the model does not have. Ids are
/// never this high; the model check reports it.
const NO_EDGE: EdgeId = EdgeId(u32::MAX);

/// One side of a loop: its edge, walked from `from` to `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Side {
    pub edge: EdgeId,
    pub from: VertexId,
    pub to: VertexId,
}

impl Face {
    /// A face from its loops and surface; the model finds its edges when
    /// it is added.
    pub fn new(outer: Vec<VertexId>, holes: Vec<Vec<VertexId>>, surface: Surface) -> Face {
        Face {
            outer,
            holes,
            surface,
            edges: Vec::new(),
            paint: Paint::default(),
        }
    }

    /// A face whose loops name their edges already (one list per loop, the
    /// outer loop first), which saves looking them up.
    pub fn with_edges(
        outer: Vec<VertexId>,
        holes: Vec<Vec<VertexId>>,
        surface: Surface,
        edges: Vec<Vec<EdgeId>>,
    ) -> Face {
        Face {
            outer,
            holes,
            surface,
            edges,
            paint: Paint::default(),
        }
    }

    /// The edges of loop `k` as stored, one per side.
    pub fn loop_edge_ids(&self, k: usize) -> &[EdgeId] {
        self.edges.get(k).map(|e| e.as_slice()).unwrap_or(&[])
    }

    /// The outer loop, then each hole.
    pub fn loops(&self) -> impl Iterator<Item = &Vec<VertexId>> {
        std::iter::once(&self.outer).chain(self.holes.iter())
    }

    /// The plane of a flat face; None for a curved one.
    pub fn plane(&self) -> Option<&Plane> {
        self.surface.plane()
    }

    pub fn is_curved(&self) -> bool {
        self.surface.is_curved()
    }

    /// A flat face's texture placement: the stored one, else the default
    /// for its plane. None for a curved face, which uses its surface's own
    /// coordinates (`Revolution::texture_at`).
    pub fn texture_map(&self) -> Option<UvMap> {
        let plane = self.plane()?;
        Some(self.paint.uv.unwrap_or_else(|| UvMap::default_for(plane)))
    }

    pub fn uses_vertex(&self, v: VertexId) -> bool {
        self.loops().any(|l| l.contains(&v))
    }

    /// The sides of loop `k` (0 is the outer loop, then the holes), in
    /// order. A side whose edge is missing is left out.
    pub fn loop_edges(&self, k: usize) -> impl Iterator<Item = Side> + '_ {
        let l = self.loops().nth(k).map(|l| l.as_slice()).unwrap_or(&[]);
        let edges = self.edges.get(k).map(|e| e.as_slice()).unwrap_or(&[]);
        (0..l.len()).filter_map(move |i| {
            let edge = *edges.get(i)?;
            (edge != NO_EDGE).then(|| Side {
                edge,
                from: l[i],
                to: l[(i + 1) % l.len()],
            })
        })
    }

    /// The sides of every loop, the outer loop first.
    pub fn sides(&self) -> impl Iterator<Item = Side> + '_ {
        (0..1 + self.holes.len()).flat_map(move |k| self.loop_edges(k))
    }

    /// The edges of every loop.
    pub fn edge_ids(&self) -> impl Iterator<Item = EdgeId> + '_ {
        self.sides().map(|s| s.edge)
    }

    /// Turn the face round: every loop runs the other way, with its edges,
    /// and the surface flips.
    pub fn reverse(&mut self) {
        let Face {
            outer,
            holes,
            edges,
            surface,
            paint,
        } = self;
        // Its front is now its back.
        *paint = paint.flipped();
        for (k, l) in std::iter::once(outer).chain(holes.iter_mut()).enumerate() {
            l.reverse();
            // Side i joined l[i] to l[i+1]; walked back, side i joins the
            // old l[n-1-i] to l[n-2-i], which was side n-2-i.
            if let Some(le) = edges.get_mut(k)
                && le.len() == l.len()
            {
                let n = le.len();
                let old = le.clone();
                for (i, e) in le.iter_mut().enumerate() {
                    *e = old[(2 * n - 2 - i) % n];
                }
            }
        }
        *surface = surface.flipped();
    }

    pub fn uses_edge(&self, e: EdgeId) -> bool {
        self.edges.iter().flatten().any(|x| *x == e)
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
#[serde(try_from = "ModelData")]
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
    #[serde(default, skip_serializing_if = "is_zero")]
    next_curve: u32,
}

/// The model as saved in JSON (undo journals, schema 0 files). Faces saved
/// before loops named their edges find them when read.
#[derive(Deserialize)]
struct ModelData {
    vertices: BTreeMap<VertexId, DVec3>,
    edges: BTreeMap<EdgeId, Edge>,
    faces: BTreeMap<FaceId, Face>,
    #[serde(default)]
    guides: BTreeMap<GuideId, Guide>,
    next_vertex: u32,
    next_edge: u32,
    next_face: u32,
    #[serde(default)]
    next_guide: u32,
    #[serde(default)]
    next_curve: u32,
}

impl TryFrom<ModelData> for Model {
    type Error = String;

    fn try_from(d: ModelData) -> Result<Model, String> {
        let mut m = Model {
            vertices: d.vertices,
            edges: d.edges,
            faces: d.faces,
            guides: d.guides,
            next_vertex: d.next_vertex,
            next_edge: d.next_edge,
            next_face: d.next_face,
            next_guide: d.next_guide,
            next_curve: d.next_curve,
        };
        m.link_unlinked()?;
        Ok(m)
    }
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
    #[serde(default, skip_serializing_if = "is_zero")]
    pub curve: u32,
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

    /// The straight edge joining `a` and `b`, if there is one.
    pub fn line_between(&self, a: VertexId, b: VertexId) -> Option<EdgeId> {
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        self.edges
            .iter()
            .find(|(_, e)| e.a == a && e.b == b && !e.curve.is_curved())
            .map(|(id, _)| *id)
    }

    /// Join two vertices with a straight edge; an existing straight edge is
    /// reused. None if `a` and `b` are the same vertex.
    pub fn add_edge(&mut self, a: VertexId, b: VertexId) -> Option<EdgeId> {
        if a == b {
            return None;
        }
        if let Some(id) = self.line_between(a, b) {
            return Some(id);
        }
        Some(self.push_edge(a, b, Curve::Line, None))
    }

    /// Join two vertices with a new straight edge, without looking for an
    /// existing one (that scan is slow for big imports). The caller makes
    /// sure the edge is not there yet and `a` and `b` differ.
    pub fn add_new_edge(&mut self, a: VertexId, b: VertexId) -> EdgeId {
        debug_assert!(a != b);
        self.push_edge(a, b, Curve::Line, None)
    }

    /// Join two vertices with `curve` walked from `a` to `b` (an arc), as a
    /// new edge: several arcs may join the same two vertices (a circle's two
    /// halves). None if `a` and `b` are the same vertex.
    pub fn add_curve_edge(
        &mut self,
        a: VertexId,
        b: VertexId,
        curve: Curve,
        curve_id: Option<CurveId>,
    ) -> Option<EdgeId> {
        (a != b).then(|| self.push_edge(a, b, curve, curve_id))
    }

    /// A new id for a curve (a circle, arc or polygon).
    pub fn new_curve_id(&mut self) -> CurveId {
        let id = CurveId(self.next_curve);
        self.next_curve += 1;
        id
    }

    /// Every edge of the curve `e` belongs to (a circle's arcs, a polygon's
    /// sides), in id order; just `e` if it has no curve id.
    pub fn curve_edges(&self, e: EdgeId) -> Vec<EdgeId> {
        match self.edge(e).and_then(|x| x.curve_id) {
            Some(c) => self
                .edges
                .iter()
                .filter(|(_, x)| x.curve_id == Some(c))
                .map(|(&id, _)| id)
                .collect(),
            None if self.edge(e).is_some() => vec![e],
            None => Vec::new(),
        }
    }

    /// Soften and smooth an edge, or not (Exact Curves 4c).
    pub fn set_soft(&mut self, e: EdgeId, soft: bool, smooth: bool) {
        if let Some(x) = self.edges.get_mut(&e) {
            x.soft = soft;
            x.smooth = smooth;
        }
    }

    /// Mark an edge approximate, or not (DECISIONS #32).
    pub fn set_approx(&mut self, e: EdgeId, approx: bool) {
        if let Some(x) = self.edges.get_mut(&e) {
            x.approx = approx;
        }
    }

    /// Fine straight segments through `points` (in order; `closed` joins
    /// the last to the first) standing in for one curve Prism cannot store
    /// exactly: new edges marked approximate, sharing a new curve id
    /// (DECISIONS #32).
    pub fn add_approx_curve(&mut self, points: &[VertexId], closed: bool) -> Vec<EdgeId> {
        let cid = self.new_curve_id();
        let n = points.len();
        let segs = if closed { n } else { n.saturating_sub(1) };
        let mut out = Vec::with_capacity(segs);
        for i in 0..segs {
            let (a, b) = (points[i], points[(i + 1) % n]);
            if a == b {
                continue;
            }
            let id = self.push_edge(a, b, Curve::Line, Some(cid));
            self.set_approx(id, true);
            out.push(id);
        }
        out
    }

    /// Set the curve id of an edge (a polygon's straight edges share one).
    pub fn set_curve_id(&mut self, e: EdgeId, curve_id: Option<CurveId>) {
        if let Some(x) = self.edges.get_mut(&e) {
            x.curve_id = curve_id;
        }
    }

    /// Change the curve of an edge, given walked from `a` to `b` (after a
    /// transform).
    pub fn set_curve(&mut self, e: EdgeId, curve: Curve) {
        if let Some(x) = self.edges.get_mut(&e) {
            x.curve = curve;
        }
    }

    /// Store an edge with its ends in order (an arc turns round with them).
    fn push_edge(
        &mut self,
        a: VertexId,
        b: VertexId,
        curve: Curve,
        curve_id: Option<CurveId>,
    ) -> EdgeId {
        let (a, b, curve) = if a < b {
            (a, b, curve)
        } else {
            (b, a, curve.reversed())
        };
        let id = EdgeId(self.next_edge);
        self.next_edge += 1;
        self.edges.insert(
            id,
            Edge {
                a,
                b,
                curve,
                curve_id,
                soft: false,
                smooth: false,
                approx: false,
            },
        );
        id
    }

    /// The points along an edge from `a` to `b`, both ends included (the
    /// drawn points of an arc).
    pub fn edge_points(&self, id: EdgeId) -> Vec<DVec3> {
        let Some(e) = self.edges.get(&id) else {
            return Vec::new();
        };
        let (pa, pb) = (self.vertex(e.a), self.vertex(e.b));
        let mut out = vec![pa];
        out.extend(e.curve.inner_points(pa, pb));
        out.push(pb);
        out
    }

    /// Whether a face has an arc in any loop.
    pub fn face_has_arcs(&self, id: FaceId) -> bool {
        self.face(id).is_some_and(|f| {
            f.edge_ids()
                .any(|e| self.edges.get(&e).is_some_and(|x| x.curve.is_curved()))
        })
    }

    /// Whether any edge is an arc.
    pub fn has_arcs(&self) -> bool {
        self.edges.values().any(|e| e.curve.is_curved())
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

    /// Replace edge `id` by two edges through `mid`, and put `mid` and both
    /// halves into every face loop that ran along the edge. Returns the two
    /// new edges.
    pub fn split_edge(&mut self, id: EdgeId, mid: VertexId) -> Option<(EdgeId, EdgeId)> {
        let e = self.edges.get(&id)?.clone();
        if mid == e.a || mid == e.b {
            return None;
        }
        self.edges.remove(&id);
        // Arcs split into two arcs of the same circle and curve; splines
        // into the two parts of the same curve.
        let (first, second) = if e.curve.is_curved() {
            let (c1, c2) = e.curve.split_at(self.vertex(mid));
            (
                self.push_edge(e.a, mid, c1, e.curve_id),
                self.push_edge(mid, e.b, c2, e.curve_id),
            )
        } else {
            let first = self.add_edge(e.a, mid)?;
            let second = self.add_edge(mid, e.b)?;
            for h in [first, second] {
                self.set_curve_id(h, e.curve_id);
            }
            (first, second)
        };
        // Both halves keep the edge's look.
        for h in [first, second] {
            self.set_soft(h, e.soft, e.smooth);
        }
        for face in self.faces.values_mut() {
            let Face {
                outer,
                holes,
                edges,
                ..
            } = face;
            for (l, le) in std::iter::once(outer).chain(holes.iter_mut()).zip(edges) {
                // Every run along the edge: a loop can run along one edge
                // twice (a bridge to an island). Last first, so earlier
                // places stay put.
                let runs: Vec<usize> = (0..le.len()).filter(|&i| le[i] == id).collect();
                for &i in runs.iter().rev() {
                    // Walked a to b, the halves come first then second.
                    let halves = if l[i] == e.a {
                        [first, second]
                    } else {
                        [second, first]
                    };
                    l.insert(i + 1, mid);
                    le.splice(i..=i, halves);
                }
            }
        }
        Some((first, second))
    }

    // ---- faces ----

    /// Add a face. Its loops find their edges: edges it was given that
    /// still join the right vertices are kept, the rest are looked up.
    pub fn add_face(&mut self, mut face: Face) -> FaceId {
        self.link(&mut face);
        let id = FaceId(self.next_face);
        self.next_face += 1;
        self.faces.insert(id, face);
        id
    }

    pub fn face(&self, id: FaceId) -> Option<&Face> {
        self.faces.get(&id)
    }

    /// Change a face (its loops or surface); its loops find their edges
    /// again afterwards, as in [`Model::add_face`].
    pub fn edit_face<R>(&mut self, id: FaceId, change: impl FnOnce(&mut Face) -> R) -> Option<R> {
        let mut face = self.faces.remove(&id)?;
        let r = change(&mut face);
        self.link(&mut face);
        self.faces.insert(id, face);
        Some(r)
    }

    /// Find a face's edges again, after edges it needed were added.
    pub fn relink_face(&mut self, id: FaceId) {
        self.edit_face(id, |_| ());
    }

    /// Give each loop of `face` its edges: an edge it already names that
    /// joins the same two vertices is kept, else the edge between them.
    fn link(&self, face: &mut Face) {
        let mut old = std::mem::take(&mut face.edges);
        // A point repeated next to itself (a loop closed back onto its
        // start) names no edge: drop the repeat, and the side it made.
        let mut drop_repeats = |k: usize, l: &mut Vec<VertexId>| {
            let mut i = 0;
            while l.len() > 1 && i < l.len() {
                let j = (i + 1) % l.len();
                if l[i] == l[j] {
                    l.remove(j);
                    if let Some(le) = old.get_mut(k)
                        && i < le.len()
                    {
                        le.remove(i);
                    }
                } else {
                    i += 1;
                }
            }
        };
        drop_repeats(0, &mut face.outer);
        for (k, h) in face.holes.iter_mut().enumerate() {
            drop_repeats(k + 1, h);
        }
        let named: BTreeMap<(VertexId, VertexId), EdgeId> = old
            .iter()
            .flatten()
            .filter_map(|e| self.edges.get(e).map(|x| ((x.a, x.b), *e)))
            .collect();
        let joins = |e: EdgeId, a: VertexId, b: VertexId| {
            self.edges.get(&e).is_some_and(|x| x.has(a) && x.has(b))
        };
        face.edges = face
            .loops()
            .enumerate()
            .map(|(k, l)| {
                loop_pairs(l)
                    .enumerate()
                    .map(|(i, (a, b))| {
                        // The edge named at this side, then one named
                        // elsewhere, then a straight edge, then any.
                        old.get(k)
                            .and_then(|le| le.get(i))
                            .copied()
                            .filter(|e| joins(*e, a, b))
                            .or_else(|| named.get(&(a.min(b), a.max(b))).copied())
                            .or_else(|| self.line_between(a, b))
                            .or_else(|| self.edge_between(a, b))
                            .unwrap_or(NO_EDGE)
                    })
                    .collect()
            })
            .collect();
    }

    /// Link every face that names no edges (one saved before loops named
    /// them). Refused if a loop needs an edge the model does not have.
    fn link_unlinked(&mut self) -> Result<(), String> {
        let ids: Vec<FaceId> = self
            .faces
            .iter()
            .filter(|(_, f)| f.edges.is_empty())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.relink_face(id);
            if self.faces[&id]
                .edges
                .iter()
                .flatten()
                .any(|e| *e == NO_EDGE)
            {
                return Err(format!("face {id:?} uses a missing edge"));
            }
        }
        Ok(())
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

    /// A face's area: its outer loop minus its holes, exact for arcs.
    pub fn face_area(&self, id: FaceId) -> f64 {
        let Some(f) = self.face(id) else { return 0.0 };
        let Some(n) = f.plane().map(|p| p.normal) else {
            // A curved face is flat when unrolled.
            let area = |k: usize| {
                let p: Vec<DVec2> = self.unrolled(f, k).into_iter().map(|x| x.1).collect();
                (0..p.len())
                    .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
                    .sum::<f64>()
                    .abs()
                    * 0.5
            };
            return area(0) - (1..=f.holes.len()).map(area).sum::<f64>();
        };
        let area = |k: usize, l: &[VertexId]| {
            let poly = newell(&self.points(l)).dot(n) * 0.5;
            let bulges: f64 = f
                .loop_edges(k)
                .filter_map(|s| {
                    let e = self.edge(s.edge)?;
                    let (p, q) = (self.vertex(s.from), self.vertex(s.to));
                    let c = e.curve_from(s.from);
                    Some(c.bulge_area(p, q, n))
                })
                .sum();
            (poly + bulges).abs()
        };
        area(0, &f.outer)
            - f.holes
                .iter()
                .enumerate()
                .map(|(i, h)| area(i + 1, h))
                .sum::<f64>()
    }

    /// Loop `k` of a cylinder face unrolled flat: each drawn point (arcs
    /// give their inner points) with its place as (distance round the
    /// cylinder, height along its axis), walking the loop's sides so the
    /// angle never jumps. The sides of a cylinder face are lines along the
    /// axis and arcs round it, which unroll straight. Empty for a flat face.
    pub fn unrolled(&self, f: &Face, k: usize) -> Vec<(DVec3, DVec2)> {
        let Surface::Cylinder(c) = f.surface else {
            return Vec::new();
        };
        let Some(l) = f.loops().nth(k) else {
            return Vec::new();
        };
        let ids = f.loop_edge_ids(k);
        let height = |p: DVec3| (p - c.origin).dot(c.axis);
        let mut turned = 0.0;
        let mut out = Vec::with_capacity(l.len());
        for (i, v) in l.iter().enumerate() {
            let p = self.vertex(*v);
            out.push((p, DVec2::new(turned * c.radius, height(p))));
            let Some(e) = ids.get(i).and_then(|id| self.edges.get(id)) else {
                continue;
            };
            if let Curve::Arc { axis, .. } = e.curve {
                let q = self.vertex(l[(i + 1) % l.len()]);
                let (curve, dir) = if e.a == *v {
                    (e.curve.clone(), axis)
                } else {
                    (e.curve.reversed(), -axis)
                };
                let sweep = curve.sweep(p, q);
                let sign = dir.dot(c.axis).signum();
                let inner = curve.inner_points(p, q);
                let n = inner.len() + 1;
                for (j, x) in inner.into_iter().enumerate() {
                    let t = turned + sign * sweep * (j + 1) as f64 / n as f64;
                    out.push((x, DVec2::new(t * c.radius, height(x))));
                }
                turned += sign * sweep;
            } else {
                // A straight edge along the cylinder turns nothing; an
                // approximate segment (an ellipse's) turns by the angle
                // between its ends round the axis.
                let q = self.vertex(l[(i + 1) % l.len()]);
                let flat = |x: DVec3| {
                    let w = x - c.origin;
                    w - c.axis * w.dot(c.axis)
                };
                let (u, w) = (flat(p), flat(q));
                turned += u.cross(w).dot(c.axis).atan2(u.dot(w));
            }
        }
        out
    }

    /// Loop `k` of a curved face as places on its turned surface: each drawn
    /// point with (θ round the axis, s along the profile), unwrapped so
    /// neither jumps by a turn. A point on the axis (a sphere's pole, a
    /// cone's apex) has no angle of its own: it comes twice, at the angle
    /// of the side before and of the side after, crossing the way a face
    /// pointing out of its surface goes (counter-clockwise in (θ, s)).
    /// Empty for a flat face.
    pub fn param_loop(&self, f: &Face, k: usize) -> Vec<(DVec3, DVec2)> {
        let Some(rev) = f.surface.revolution() else {
            return Vec::new();
        };
        let pts = self.loop_points(f, k);
        let eps = self.tol().dist;
        let n = pts.len();
        // Start at a point off the axis.
        let Some(first) = (0..n).find(|i| rev.axis_distance(pts[*i]) > eps) else {
            return Vec::new();
        };
        let turn = std::f64::consts::TAU;
        let near = |x: f64, to: f64| x + ((to - x) / turn).round() * turn;
        let circle = matches!(rev.profile, prism_geom::Profile::Circle { .. });
        let mut out: Vec<(DVec3, DVec2)> = Vec::with_capacity(n + 4);
        let (mut t0, mut s0) = rev.param(pts[first]);
        out.push((pts[first], DVec2::new(t0, s0)));
        let mut j = 1;
        while j < n {
            let p = pts[(first + j) % n];
            let (t, s) = rev.param(p);
            let s = if circle { near(s, s0) } else { s };
            if rev.axis_distance(p) > eps {
                t0 = near(t, t0);
                s0 = s;
                out.push((p, DVec2::new(t0, s0)));
                j += 1;
                continue;
            }
            // On the axis: the angle of the next point off it.
            let next = (1..n)
                .map(|d| pts[(first + j + d) % n])
                .find(|q| rev.axis_distance(*q) > eps);
            let Some(q) = next else { break };
            let tq = rev.param(q).0;
            // Top of the region (s at its highest here): counter-clockwise
            // goes towards smaller θ; at the bottom towards larger.
            let top = s > s0;
            let back = top == f.surface.outward();
            let mut t1 = near(tq, t0);
            if back && t1 > t0 {
                t1 -= turn;
            } else if !back && t1 < t0 {
                t1 += turn;
            }
            out.push((p, DVec2::new(t0, s)));
            out.push((p, DVec2::new(t1, s)));
            t0 = t1;
            s0 = s;
            j += 1;
        }
        out
    }

    /// The points of loop `k` of a face, walking its edges (arcs give their
    /// drawn points): the polygon to test points and triangulate with.
    pub fn loop_points(&self, f: &Face, k: usize) -> Vec<DVec3> {
        let Some(l) = f.loops().nth(k) else {
            return Vec::new();
        };
        let ids = f.loop_edge_ids(k);
        let mut out = Vec::with_capacity(l.len());
        for (i, v) in l.iter().enumerate() {
            out.push(self.vertex(*v));
            let Some(e) = ids.get(i).and_then(|id| self.edges.get(id)) else {
                continue;
            };
            if e.curve.is_curved() {
                let (pa, pb) = (self.vertex(e.a), self.vertex(e.b));
                let mut inner = e.curve.inner_points(pa, pb);
                if *v != e.a {
                    inner.reverse();
                }
                out.extend(inner);
            }
        }
        out
    }

    /// Each loop's points (`loop_points`), the outer loop first.
    pub fn face_loop_points(&self, f: &Face) -> Vec<Vec<DVec3>> {
        (0..=f.holes.len())
            .map(|k| self.loop_points(f, k))
            .collect()
    }

    /// For every edge, the faces whose loops use it.
    pub fn edge_faces(&self) -> BTreeMap<EdgeId, Vec<FaceId>> {
        let mut map: BTreeMap<EdgeId, Vec<FaceId>> = BTreeMap::new();
        for (fid, f) in &self.faces {
            for e in f.edge_ids() {
                map.entry(e).or_default().push(*fid);
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
            curve: self.next_curve,
        }
    }

    /// Never hand out an id below `c` again (after an undo).
    pub fn keep_counters_at_least(&mut self, c: Counters) {
        self.next_vertex = self.next_vertex.max(c.vertex);
        self.next_edge = self.next_edge.max(c.edge);
        self.next_face = self.next_face.max(c.face);
        self.next_guide = self.next_guide.max(c.guide);
        self.next_curve = self.next_curve.max(c.curve);
    }

    /// Add everything in `other` beside what is here, with new ids (an
    /// import). Nothing is joined to what was here. Returns the new faces.
    pub fn append(&mut self, other: &Model) -> Vec<FaceId> {
        let mut vmap: BTreeMap<VertexId, VertexId> = BTreeMap::new();
        for (id, p) in other.vertices() {
            vmap.insert(id, self.add_vertex(p));
        }
        let mut emap: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
        let mut cmap: BTreeMap<CurveId, CurveId> = BTreeMap::new();
        for (old, e) in other.edges() {
            // The vertices are all new, so the edges cannot be here yet.
            if let (Some(a), Some(b)) = (vmap.get(&e.a), vmap.get(&e.b))
                && a != b
            {
                let cid = e
                    .curve_id
                    .map(|c| *cmap.entry(c).or_insert_with(|| self.new_curve_id()));
                let id = self.push_edge(*a, *b, e.curve.clone(), cid);
                self.set_soft(id, e.soft, e.smooth);
                emap.insert(old, id);
            }
        }
        let map = |l: &[VertexId]| l.iter().filter_map(|v| vmap.get(v).copied()).collect();
        let mut faces = Vec::new();
        for (_, f) in other.faces() {
            let edges = f
                .edges
                .iter()
                .map(|l| l.iter().map(|e| *emap.get(e).unwrap_or(&NO_EDGE)).collect())
                .collect();
            let mut nf = Face::with_edges(
                map(&f.outer),
                f.holes.iter().map(|h| map(h)).collect(),
                f.surface,
                edges,
            );
            // Its paint and texture placement come too.
            nf.paint = f.paint;
            faces.push(self.add_face(nf));
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
            edges: self.edges.iter().map(|(k, v)| (*k, v.clone())).collect(),
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
        if let Some((id, _)) = p.faces.iter().find(|(_, f)| !f.surface.is_valid()) {
            return Err(format!("face {id:?} has no finite surface"));
        }
        let mut m = Model {
            vertices,
            edges: ids(p.edges, "edge", |k: EdgeId| k.0 < c.edge)?,
            faces: ids(p.faces, "face", |k: FaceId| k.0 < c.face)?,
            guides: ids(p.guides, "guide", |k: GuideId| k.0 < c.guide)?,
            next_vertex: c.vertex,
            next_edge: c.edge,
            next_face: c.face,
            next_guide: c.guide,
            next_curve: c.curve,
        };
        m.link_unlinked()?;
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
        copy.next_curve = 0;
        copy.to_json()
    }

    /// Bookkeeping problems (empty if none): face loops use existing edges
    /// and vertices and name those edges in order, edges join existing
    /// vertices, no edge is repeated.
    pub fn bookkeeping_problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut pairs = std::collections::BTreeSet::new();
        let mut lines = std::collections::BTreeSet::new();
        let tol = self.tol();
        for (id, e) in &self.edges {
            if let Curve::Arc {
                centre,
                axis,
                radius,
            } = e.curve
            {
                if radius.is_nan() || radius <= tol.dist || (axis.length() - 1.0).abs() > 1e-9 {
                    out.push(format!("arc {id:?} has no proper radius or axis"));
                }
                for v in [e.a, e.b] {
                    let d = self.vertex(v) - centre;
                    if (d.length() - radius).abs() > tol.dist || d.dot(axis).abs() > tol.dist {
                        out.push(format!("arc {id:?}: point {v:?} is off its circle"));
                    }
                }
                if let Some(cid) = e.curve_id
                    && cid.0 >= self.next_curve
                {
                    out.push(format!("edge {id:?} has curve {cid:?} above its counter"));
                }
            }
            if let Curve::Spline(s) = &e.curve {
                for (v, t) in [(e.a, 0.0), (e.b, 1.0)] {
                    if self.has_vertex(v) && s.point(t).distance(self.vertex(v)) > tol.dist {
                        out.push(format!("curve {id:?}: point {v:?} is off it"));
                    }
                }
            }
            if !self.has_vertex(e.a) || !self.has_vertex(e.b) {
                out.push(format!("edge {id:?} uses a missing vertex"));
            }
            if e.a >= e.b {
                out.push(format!(
                    "edge {id:?} is not ordered or joins a vertex to itself"
                ));
            }
            pairs.insert((e.a, e.b));
            // A line and an arc, or two arcs, may join the same two vertices.
            if !e.curve.is_curved() && !lines.insert((e.a, e.b)) {
                out.push(format!("edge {id:?} repeats another edge"));
            }
        }
        for (id, f) in &self.faces {
            if f.edges.len() != 1 + f.holes.len() {
                out.push(format!(
                    "face {id:?} names edges for {} loops",
                    f.edges.len()
                ));
            }
            for (k, l) in f.loops().enumerate() {
                // Two sides make a loop only when one is an arc (half a
                // circle closed by its diameter, or a whole circle).
                let curved = f
                    .loop_edge_ids(k)
                    .iter()
                    .any(|e| self.edges.get(e).is_some_and(|x| x.curve.is_curved()));
                if l.len() < 3 && !(l.len() == 2 && curved) {
                    out.push(format!("face {id:?} has a loop of {} vertices", l.len()));
                }
                let named = f.loop_edge_ids(k);
                if named.len() != l.len() && f.edges.len() == 1 + f.holes.len() {
                    out.push(format!(
                        "face {id:?} loop {k} has {} vertices but {} edges",
                        l.len(),
                        named.len()
                    ));
                }
                for (i, (a, b)) in loop_pairs(l).enumerate() {
                    if !pairs.contains(&(a.min(b), a.max(b))) {
                        out.push(format!("face {id:?} uses a missing edge {a:?}-{b:?}"));
                        continue;
                    }
                    let Some(e) = named.get(i) else { continue };
                    match self.edges.get(e) {
                        Some(x) if x.has(a) && x.has(b) => match f.surface {
                            Surface::Plane(pl) => {
                                if let Curve::Arc { axis, .. } = x.curve
                                    && (axis.dot(pl.normal).abs() - 1.0).abs() > 1e-9
                                {
                                    out.push(format!(
                                        "face {id:?} uses arc {e:?}, which is not in its plane"
                                    ));
                                }
                                if x.curve.is_spline()
                                    && !f.surface.holds(
                                        &x.curve,
                                        self.vertex(x.a),
                                        self.vertex(x.b),
                                        self.tol().dist,
                                    )
                                {
                                    out.push(format!(
                                        "face {id:?} uses curve {e:?}, which is not in its plane"
                                    ));
                                }
                            }
                            curved => {
                                let (p, q) = (self.vertex(x.a), self.vertex(x.b));
                                // An approximate segment's chord cuts inside
                                // by design: its ends must be on the surface.
                                let eps = self.tol().dist;
                                let on = if x.approx {
                                    curved.distance(p).abs() <= eps
                                        && curved.distance(q).abs() <= eps
                                } else {
                                    curved.holds(&x.curve, p, q, eps)
                                };
                                if !on {
                                    out.push(format!(
                                        "face {id:?} uses edge {e:?}, which is not on its surface"
                                    ));
                                }
                            }
                        },
                        _ => out.push(format!(
                            "face {id:?} loop {k} side {i} names edge {e:?}, which does not join {a:?}-{b:?}"
                        )),
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
        m.add_face(Face::new(
            v.to_vec(),
            vec![],
            Surface::Plane(Plane::new(DVec3::ZERO, DVec3::Z)),
        ));
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

    fn sides_of(f: &Face) -> Vec<(VertexId, VertexId, EdgeId)> {
        f.sides().map(|s| (s.from, s.to, s.edge)).collect()
    }

    #[test]
    fn a_face_finds_its_edges() {
        let mut m = Model::new();
        let v = square(&mut m);
        let f = m.face(FaceId(0)).expect("face");
        let want: Vec<_> = (0..4)
            .map(|i| {
                let (a, b) = (v[i], v[(i + 1) % 4]);
                (a, b, m.edge_between(a, b).expect("edge"))
            })
            .collect();
        assert_eq!(sides_of(f), want);
        assert_eq!(m.edge_faces().len(), 4);
    }

    #[test]
    fn splitting_an_edge_relinks_every_face_using_it() {
        let mut m = Model::new();
        let v = square(&mut m);
        // A second face on the same first edge, walked the other way.
        let w = m.add_vertex(DVec3::new(2.0, -3.0, 0.0));
        m.add_edge(v[0], w);
        m.add_edge(w, v[1]);
        m.add_face(Face::new(
            vec![v[1], v[0], w],
            vec![],
            Surface::Plane(Plane::new(DVec3::ZERO, DVec3::Z)),
        ));
        let e = m.edge_between(v[0], v[1]).expect("edge");
        let mid = m.add_vertex(DVec3::new(2.0, 0.0, 0.0));
        let (first, second) = m.split_edge(e, mid).expect("splits");
        assert!(m.bookkeeping_problems().is_empty());
        let back = m.face(FaceId(1)).expect("face");
        assert_eq!(back.outer, vec![v[1], mid, v[0], w]);
        assert_eq!(&back.loop_edge_ids(0)[..2], &[second, first]);
        assert!(!m.faces().any(|(_, f)| f.uses_edge(e)));
    }

    #[test]
    fn the_check_finds_loop_edges_out_of_step() {
        let mut m = Model::new();
        let v = square(&mut m);
        // Change the loop behind the model's back: its edges are now wrong.
        let mut f = m.face(FaceId(0)).expect("face").clone();
        f.outer.rotate_left(1);
        m.faces.insert(FaceId(0), f);
        let p = m.bookkeeping_problems();
        assert!(p.iter().any(|x| x.contains("does not join")), "{p:?}");
        // Editing through the model finds them again.
        m.relink_face(FaceId(0));
        assert!(m.bookkeeping_problems().is_empty());
        // A loop whose edge is gone.
        m.remove_edge(m.edge_between(v[0], v[1]).expect("edge"));
        assert!(!m.bookkeeping_problems().is_empty());
    }

    #[test]
    fn faces_saved_without_edges_find_them() {
        let mut m = Model::new();
        square(&mut m);
        let mut parts = m.to_parts();
        let (id, f) = &parts.faces[0];
        parts.faces[0] = (*id, Face::new(f.outer.clone(), f.holes.clone(), f.surface));
        assert_eq!(Model::from_parts(parts).expect("reads"), m);
        let old = m.to_json().replace(r#","edges":[[0,1,2,3]]"#, "");
        assert!(!old.contains(r#""edges":[["#), "{old}");
        assert_eq!(Model::from_json(&old).expect("reads"), m);
    }

    #[test]
    fn appending_keeps_the_loop_edges() {
        let mut m = Model::new();
        square(&mut m);
        let mut both = m.clone();
        let faces = both.append(&m);
        assert!(both.bookkeeping_problems().is_empty());
        assert_eq!(both.face(faces[0]).expect("face").sides().count(), 4);
    }
}
