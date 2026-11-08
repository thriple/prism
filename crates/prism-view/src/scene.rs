//! The visible scene: what drawing and picking both work from.
//!
//! Built from the model when it changes (not every frame). It is the one
//! place that decides what is visible, so when hiding arrives (PLAN 1.14,
//! "hidden must be free") hidden objects are left out here and nothing else
//! has to know about them.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3, Tol};
use prism_topo::{EdgeId, FaceId, Guide, GuideId, Model, VertexId};

use crate::tess::triangulate;

/// One face, ready for picking and drawing.
#[derive(Clone, Debug)]
pub struct SceneFace {
    pub id: FaceId,
    /// The face's own outward normal (unit).
    pub normal: DVec3,
    /// A point on its plane.
    pub origin: DVec3,
    /// 2D frame on the plane.
    pub u: DVec3,
    pub v: DVec3,
    pub outer: Vec<DVec2>,
    pub holes: Vec<Vec<DVec2>>,
    pub lo: DVec3,
    pub hi: DVec3,
    /// Its triangles in the mesh, as a range of `Mesh::indices`.
    pub tris: Range<u32>,
    /// Drawn faded: outside the open group (PLAN 1.2).
    pub faded: bool,
}

impl SceneFace {
    /// The face for crossing tests (`prism_ops::intersect`).
    pub fn flat(&self) -> prism_ops::intersect::Flat {
        prism_ops::intersect::Flat {
            origin: self.origin,
            u: self.u,
            v: self.v,
            normal: self.normal,
            outer: self.outer.clone(),
            holes: self.holes.clone(),
            lo: self.lo,
            hi: self.hi,
        }
    }

    fn to_2d(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        DVec2::new(d.dot(self.u), d.dot(self.v))
    }

    /// Where the ray hits this face (distance along it), if it does.
    pub fn hit(&self, o: DVec3, d: DVec3) -> Option<f64> {
        let den = self.normal.dot(d);
        if den.abs() < 1e-12 || !ray_box(o, d, self.lo, self.hi) {
            return None;
        }
        let t = (self.origin - o).dot(self.normal) / den;
        if t <= 0.0 {
            return None;
        }
        let q = self.to_2d(o + d * t);
        let inside = point_in_polygon(q, &self.outer) != Inside::Outside
            && self
                .holes
                .iter()
                .all(|h| point_in_polygon(q, h) != Inside::Inside);
        inside.then_some(t)
    }
}

/// A slab test, slightly widened so faces lying in the box's own walls pass.
fn ray_box(o: DVec3, d: DVec3, lo: DVec3, hi: DVec3) -> bool {
    let pad = (hi - lo).max_element() * 1e-9 + 1e-300;
    let (lo, hi) = (lo - DVec3::splat(pad), hi + DVec3::splat(pad));
    let mut t0 = 0.0f64;
    let mut t1 = f64::INFINITY;
    for k in 0..3 {
        if d[k].abs() < 1e-300 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return false;
            }
            continue;
        }
        let a = (lo[k] - o[k]) / d[k];
        let b = (hi[k] - o[k]) / d[k];
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// One edge, with the normals of the faces that use it (for the profile pass).
#[derive(Clone, Debug)]
pub struct SceneEdge {
    pub id: EdgeId,
    pub va: VertexId,
    pub vb: VertexId,
    pub a: DVec3,
    pub b: DVec3,
    /// The first two faces' normals; `None` where there are fewer faces.
    pub normals: [Option<DVec3>; 2],
    /// The first two faces that use it.
    pub faces: [Option<FaceId>; 2],
    /// Drawn faded: outside the open group.
    pub faded: bool,
}

/// The display triangles of all faces: one vertex per face corner, so every
/// face shades flat with its own normal.
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub positions: Vec<DVec3>,
    pub normals: Vec<DVec3>,
    /// Which face (index into `Scene::faces`) each vertex belongs to.
    pub faces: Vec<u32>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub faces: Vec<SceneFace>,
    pub face_index: BTreeMap<FaceId, usize>,
    pub edges: Vec<SceneEdge>,
    pub edge_index: BTreeMap<EdgeId, usize>,
    pub vertices: Vec<(VertexId, DVec3)>,
    pub mesh: Mesh,
    pub bounds: Option<(DVec3, DVec3)>,
    pub tol: Tol,
    /// The guides, if they are shown (hidden guides are left out, so they
    /// cost nothing: PLAN 1.14).
    pub guides: Vec<(GuideId, Guide)>,
    /// The model vertex of each mesh corner (for cheap rigid previews).
    pub mesh_vertex_ids: Vec<VertexId>,
    /// Who owns the faces, edges and points merged in from other models
    /// (groups): their ids are at or above [`FOREIGN`], and the owner is a
    /// number the caller chose. The model's own ids are not listed.
    pub face_owner: BTreeMap<FaceId, u32>,
    pub edge_owner: BTreeMap<EdgeId, u32>,
    pub vertex_owner: BTreeMap<VertexId, u32>,
}

/// Ids merged in from other models start here, clear of a model's own.
pub const FOREIGN: u32 = 1 << 31;

impl Scene {
    pub fn build(model: &Model) -> Scene {
        Self::build_with(model, true)
    }

    /// The scene, with or without the guides.
    pub fn build_with(model: &Model, show_guides: bool) -> Scene {
        let mut s = Scene {
            faces: Vec::new(),
            face_index: BTreeMap::new(),
            edges: Vec::new(),
            edge_index: BTreeMap::new(),
            vertices: Vec::new(),
            mesh: Mesh::default(),
            bounds: model.bbox(),
            tol: model.tol(),
            mesh_vertex_ids: Vec::new(),
            face_owner: BTreeMap::new(),
            edge_owner: BTreeMap::new(),
            vertex_owner: BTreeMap::new(),
            guides: if show_guides {
                model.guides().map(|(id, g)| (id, *g)).collect()
            } else {
                Vec::new()
            },
        };
        let edge_faces = model.edge_faces();
        for (id, f) in model.faces() {
            let plane = *f.plane();
            let frame = plane.frame();
            let (u, v) = if frame.normal.dot(plane.normal) >= 0.0 {
                (frame.u, frame.v)
            } else {
                // Keep u × v along the face's own normal.
                (frame.v, frame.u)
            };
            let pts: Vec<DVec3> = f.loops().flatten().map(|v| model.vertex(*v)).collect();
            let (mut lo, mut hi) = (pts[0], pts[0]);
            for p in &pts {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
            let mut face = SceneFace {
                id,
                normal: plane.normal,
                origin: plane.origin,
                u,
                v,
                outer: Vec::new(),
                holes: Vec::new(),
                lo,
                hi,
                tris: 0..0,
                faded: false,
            };
            face.outer = f
                .outer
                .iter()
                .map(|v| face.to_2d(model.vertex(*v)))
                .collect();
            face.holes = f
                .holes
                .iter()
                .map(|h| h.iter().map(|v| face.to_2d(model.vertex(*v))).collect())
                .collect();
            let start = s.mesh.indices.len() as u32;
            let base = s.mesh.positions.len() as u32;
            s.mesh.positions.extend_from_slice(&pts);
            s.mesh_vertex_ids.extend(f.loops().flatten().copied());
            s.mesh
                .normals
                .extend(std::iter::repeat_n(plane.normal, pts.len()));
            s.mesh
                .faces
                .extend(std::iter::repeat_n(s.faces.len() as u32, pts.len()));
            for t in triangulate(&face.outer, &face.holes) {
                s.mesh.indices.extend(t.iter().map(|&i| base + i as u32));
            }
            face.tris = start..s.mesh.indices.len() as u32;
            s.face_index.insert(id, s.faces.len());
            s.faces.push(face);
        }
        for (id, e) in model.edges() {
            let fs = edge_faces.get(&id);
            let fid = |k: usize| fs.and_then(|v| v.get(k)).copied();
            let n = |k: usize| fid(k).and_then(|f| model.face(f)).map(|f| f.plane().normal);
            s.edge_index.insert(id, s.edges.len());
            s.edges.push(SceneEdge {
                id,
                va: e.a,
                vb: e.b,
                a: model.vertex(e.a),
                b: model.vertex(e.b),
                normals: [n(0), n(1)],
                faces: [fid(0), fid(1)],
                faded: false,
            });
        }
        s.vertices = model.vertices().collect();
        s
    }

    /// The scene with the vertices `moving` sent through a rigid motion
    /// (`f` for points, `nf` for directions), without rebuilding or
    /// triangulating anything. None unless every face and edge moves whole
    /// or not at all; then the caller rebuilds. This makes Move and Rotate
    /// previews of whole bodies cheap on big models.
    pub fn rigidly_moved(
        &self,
        moving: &BTreeSet<VertexId>,
        f: impl Fn(DVec3) -> DVec3,
        nf: impl Fn(DVec3) -> DVec3,
    ) -> Option<Scene> {
        let mut s = self.clone();
        // Edges: whole or not at all.
        for e in &mut s.edges {
            match (moving.contains(&e.va), moving.contains(&e.vb)) {
                (true, true) => {
                    e.a = f(e.a);
                    e.b = f(e.b);
                    for n in e.normals.iter_mut().flatten() {
                        *n = nf(*n);
                    }
                }
                (false, false) => {}
                _ => return None,
            }
        }
        // Faces: a face moves if its edges did; check its vertices.
        let mut moved_face = vec![false; s.faces.len()];
        for (k, face) in s.faces.iter_mut().enumerate() {
            let r = face.tris.clone();
            let first = s.mesh.indices.get(r.start as usize).copied();
            let Some(first) = first else { continue };
            let on = moving.contains(&self.mesh_vertex_id(first)?);
            // Every corner of the face must agree.
            for &i in &s.mesh.indices[r.start as usize..r.end as usize] {
                if moving.contains(&self.mesh_vertex_id(i)?) != on {
                    return None;
                }
            }
            if on {
                moved_face[k] = true;
                face.origin = f(face.origin);
                face.u = nf(face.u);
                face.v = nf(face.v);
                face.normal = nf(face.normal);
            }
        }
        for i in 0..s.mesh.positions.len() {
            if moved_face[s.mesh.faces[i] as usize] {
                s.mesh.positions[i] = f(s.mesh.positions[i]);
                s.mesh.normals[i] = nf(s.mesh.normals[i]);
            }
        }
        // Each moved face's box, from its corners.
        let mut boxes: Vec<Option<(DVec3, DVec3)>> = vec![None; s.faces.len()];
        for (i, p) in s.mesh.positions.iter().enumerate() {
            let k = s.mesh.faces[i] as usize;
            if moved_face[k] {
                let b = boxes[k].get_or_insert((*p, *p));
                *b = (b.0.min(*p), b.1.max(*p));
            }
        }
        for (k, b) in boxes.into_iter().enumerate() {
            if let Some((lo, hi)) = b {
                s.faces[k].lo = lo;
                s.faces[k].hi = hi;
            }
        }
        for (id, p) in &mut s.vertices {
            if moving.contains(id) {
                *p = f(*p);
            }
        }
        let mut it = s.vertices.iter().map(|(_, p)| *p);
        s.bounds = it
            .next()
            .map(|first| it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))));
        Some(s)
    }

    /// Merge another model's scene in (a group's geometry, already in
    /// world coordinates), with new ids at or above [`FOREIGN`] and owned by
    /// `owner`. Its guides are left out: guides belong to the open group.
    pub fn merge(&mut self, other: Scene, owner: u32, faded: bool) {
        let f0 = self
            .face_owner
            .keys()
            .next_back()
            .map_or(FOREIGN, |k| k.0 + 1);
        let e0 = self
            .edge_owner
            .keys()
            .next_back()
            .map_or(FOREIGN, |k| k.0 + 1);
        let v0 = self
            .vertex_owner
            .keys()
            .next_back()
            .map_or(FOREIGN, |k| k.0 + 1);
        let fmap: BTreeMap<FaceId, FaceId> = other
            .faces
            .iter()
            .enumerate()
            .map(|(i, f)| (f.id, FaceId(f0 + i as u32)))
            .collect();
        let vmap: BTreeMap<VertexId, VertexId> = other
            .vertices
            .iter()
            .enumerate()
            .map(|(i, (v, _))| (*v, VertexId(v0 + i as u32)))
            .collect();
        let face_base = self.faces.len() as u32;
        let index_base = self.mesh.indices.len() as u32;
        let corner_base = self.mesh.positions.len() as u32;
        for mut f in other.faces {
            f.id = fmap[&f.id];
            f.tris = f.tris.start + index_base..f.tris.end + index_base;
            f.faded = faded;
            self.face_owner.insert(f.id, owner);
            self.face_index.insert(f.id, self.faces.len());
            self.faces.push(f);
        }
        for (i, mut e) in other.edges.into_iter().enumerate() {
            e.id = EdgeId(e0 + i as u32);
            e.va = vmap.get(&e.va).copied().unwrap_or(e.va);
            e.vb = vmap.get(&e.vb).copied().unwrap_or(e.vb);
            e.faces = e.faces.map(|f| f.and_then(|f| fmap.get(&f).copied()));
            e.faded = faded;
            self.edge_owner.insert(e.id, owner);
            self.edge_index.insert(e.id, self.edges.len());
            self.edges.push(e);
        }
        for (v, p) in other.vertices {
            let id = vmap[&v];
            self.vertex_owner.insert(id, owner);
            self.vertices.push((id, p));
        }
        self.mesh.positions.extend(other.mesh.positions);
        self.mesh.normals.extend(other.mesh.normals);
        self.mesh
            .faces
            .extend(other.mesh.faces.into_iter().map(|k| k + face_base));
        self.mesh
            .indices
            .extend(other.mesh.indices.into_iter().map(|i| i + corner_base));
        self.mesh_vertex_ids.extend(
            other
                .mesh_vertex_ids
                .into_iter()
                .map(|v| vmap.get(&v).copied().unwrap_or(v)),
        );
        self.bounds = match (self.bounds, other.bounds) {
            (Some(a), Some(b)) => Some((a.0.min(b.0), a.1.max(b.1))),
            (a, b) => a.or(b),
        };
        if let Some((lo, hi)) = self.bounds {
            self.tol = Tol::for_points(&[lo, hi]);
        }
    }

    /// Who owns what a pick hit, if it came from another model ([`Scene::merge`]).
    pub fn owner_of(&self, hit: crate::Hit) -> Option<u32> {
        match hit {
            crate::Hit::Vertex(v) => self.vertex_owner.get(&v).copied(),
            crate::Hit::Edge(e) => self.edge_owner.get(&e).copied(),
            crate::Hit::Face(f) => self.face_owner.get(&f).copied(),
        }
    }

    /// The model vertex a mesh corner came from.
    fn mesh_vertex_id(&self, i: u32) -> Option<VertexId> {
        self.mesh_vertex_ids.get(i as usize).copied()
    }

    pub fn face(&self, id: FaceId) -> Option<&SceneFace> {
        self.face_index.get(&id).map(|&i| &self.faces[i])
    }

    pub fn edge(&self, id: EdgeId) -> Option<&SceneEdge> {
        self.edge_index.get(&id).map(|&i| &self.edges[i])
    }

    /// The nearest face the ray hits, and how far along.
    pub fn ray_face(&self, o: DVec3, d: DVec3) -> Option<(FaceId, f64)> {
        self.faces
            .iter()
            .filter_map(|f| f.hit(o, d).map(|t| (f.id, t)))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
    }

    /// Whether something hides `p` from `eye`. Faces through `p` itself do not.
    pub fn occluded(&self, eye: DVec3, p: DVec3) -> bool {
        let dv = p - eye;
        let len = dv.length();
        if len <= 0.0 {
            return false;
        }
        let d = dv / len;
        let slack = self.tol.dist + len * 1e-9;
        self.faces
            .iter()
            .any(|f| f.hit(eye, d).is_some_and(|t| t < len - slack))
    }
}
