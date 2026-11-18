//! Section planes (Annotation & Sections part 1): cutting the visible
//! scene with a plane.
//!
//! A display effect only: the model is untouched. [`Scene::cut_by`] drops
//! what lies on the side the plane's normal points to, clips the triangles
//! and edges that cross it, and works out where the plane meets the
//! surfaces: the cut lines, and, where they close into loops, flat caps so
//! a cut solid does not look hollow. Picking and snapping then see only
//! what is left.

use std::collections::BTreeMap;

use prism_math::predicates::{Inside, point_in_polygon, signed_area2};
use prism_math::{DVec2, DVec3};
use prism_topo::{FaceId, MaterialId, VertexId};

use crate::scene::{Scene, SceneFace};
use crate::style::Rgba;
use crate::tess::triangulate;

/// Cap faces take ids from here up, and this material's colour.
pub const CAP_FACE: u32 = 0xFFFF_0000;
pub const CAP_MATERIAL: MaterialId = MaterialId(u32::MAX);

/// A plane that cuts away everything on the side its normal points to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CutPlane {
    pub origin: DVec3,
    /// Unit length.
    pub normal: DVec3,
}

impl CutPlane {
    /// Positive on the side that is cut away.
    pub fn dist(&self, p: DVec3) -> f64 {
        (p - self.origin).dot(self.normal)
    }

    /// Two axes in the plane, with `u × v` along the normal.
    pub fn frame(&self) -> (DVec3, DVec3) {
        let u = self.normal.any_orthonormal_vector();
        (u, self.normal.cross(u))
    }
}

/// What a cut left: where the plane meets the surfaces.
#[derive(Clone, Debug, PartialEq)]
pub struct SectionCut {
    pub plane: CutPlane,
    /// The cut lines, each a straight piece in the plane.
    pub lines: Vec<[DVec3; 2]>,
    /// How many caps were made, and their total area (holes taken off).
    pub caps: usize,
    pub cap_area: f64,
}

impl Scene {
    /// Cut the scene by `plane`; the caps are drawn in `cap`. A scene is
    /// cut once (build it again to cut it differently).
    pub fn cut_by(&mut self, plane: CutPlane, cap: Rgba) {
        let eps = self.tol.dist;
        let mut segments: Vec<[DVec3; 2]> = Vec::new();

        // Triangles: keep what is on the kept side, clip what crosses.
        let old = std::mem::take(&mut self.mesh.indices);
        for f in &mut self.faces {
            let start = self.mesh.indices.len() as u32;
            for t in old[f.tris.start as usize..f.tris.end as usize].chunks_exact(3) {
                clip_triangle(&mut self.mesh, [t[0], t[1], t[2]], &plane, eps, &mut segments);
            }
            f.tris = start..self.mesh.indices.len() as u32;
        }

        // Edges: the kept part of each.
        let mut edges = std::mem::take(&mut self.edges);
        edges.retain_mut(|e| {
            let pts: Vec<DVec3> = std::iter::once(e.a)
                .chain(e.mid.iter().copied())
                .chain(std::iter::once(e.b))
                .collect();
            match kept_run(&pts, &plane, eps) {
                None => false,
                Some(run) => {
                    if run.len() != pts.len() || run != pts {
                        e.a = run[0];
                        e.b = *run.last().expect("a run has points");
                        e.mid = run[1..run.len() - 1].to_vec();
                    }
                    true
                }
            }
        });
        self.edge_index = edges.iter().enumerate().map(|(i, e)| (e.id, i)).collect();
        self.edges = edges;
        self.vertices.retain(|(_, p)| plane.dist(*p) <= eps);

        // The cut lines: one of each, whichever triangle made it.
        let q = (eps * 10.0).max(self.tol.dist);
        let mut seen = std::collections::BTreeSet::new();
        segments.retain(|[a, b]| {
            let (ka, kb) = (key(*a, q), key(*b, q));
            ka != kb && seen.insert(if ka < kb { (ka, kb) } else { (kb, ka) })
        });

        let (caps, cap_area) = self.add_caps(&plane, &segments, cap, q);
        self.cut = Some(SectionCut {
            plane,
            lines: segments,
            caps,
            cap_area,
        });
    }

    /// Caps for the closed loops the cut lines make.
    fn add_caps(
        &mut self,
        plane: &CutPlane,
        segments: &[[DVec3; 2]],
        cap: Rgba,
        q: f64,
    ) -> (usize, f64) {
        let loops = closed_loops(segments, q);
        if loops.is_empty() {
            return (0, 0.0);
        }
        let (u, v) = plane.frame();
        let to2 = |p: DVec3| DVec2::new((p - plane.origin).dot(u), (p - plane.origin).dot(v));
        let flat: Vec<Vec<DVec2>> = loops
            .iter()
            .map(|l| l.iter().map(|p| to2(*p)).collect())
            .collect();
        // How many other loops each one is inside: even is a boundary,
        // odd a hole in the loop that holds it directly.
        let depth: Vec<usize> = (0..flat.len())
            .map(|i| {
                (0..flat.len())
                    .filter(|&j| j != i && inside(&flat[j], flat[i][0]))
                    .count()
            })
            .collect();
        self.palette.insert(CAP_MATERIAL, cap);
        let mut made = 0;
        let mut area = 0.0;
        for i in (0..flat.len()).filter(|&i| depth[i] % 2 == 0) {
            let holes: Vec<usize> = (0..flat.len())
                .filter(|&j| {
                    depth[j] == depth[i] + 1 && inside(&flat[i], flat[j][0]) && j != i
                })
                .collect();
            let outer = &flat[i];
            let hole_pts: Vec<Vec<DVec2>> = holes.iter().map(|&j| flat[j].clone()).collect();
            let tris = triangulate(outer, &hole_pts);
            if tris.is_empty() {
                continue;
            }
            let corners: Vec<DVec3> = loops[i]
                .iter()
                .chain(holes.iter().flat_map(|&j| loops[j].iter()))
                .copied()
                .collect();
            let base = self.mesh.positions.len() as u32;
            let face_index = self.faces.len() as u32;
            let start = self.mesh.indices.len() as u32;
            for p in &corners {
                self.mesh.positions.push(*p);
                self.mesh.normals.push(plane.normal);
                self.mesh.uvs.push(to2(*p));
                self.mesh.faces.push(face_index);
                self.mesh_vertex_ids.push(VertexId(u32::MAX));
            }
            for t in &tris {
                self.mesh.indices.extend(t.iter().map(|&k| base + k as u32));
            }
            let (mut lo, mut hi) = (corners[0], corners[0]);
            for p in &corners {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
            area += (signed_area2(outer).abs()
                - hole_pts.iter().map(|h| signed_area2(h).abs()).sum::<f64>())
                / 2.0;
            let id = FaceId(CAP_FACE + made as u32);
            self.face_index.insert(id, self.faces.len());
            self.faces.push(SceneFace {
                id,
                normal: plane.normal,
                origin: plane.origin,
                u,
                v,
                outer: outer.clone(),
                holes: hole_pts,
                lo,
                hi,
                tris: start..self.mesh.indices.len() as u32,
                faded: false,
                curved: None,
                paint: [Some(CAP_MATERIAL); 2],
                inherited: None,
                cap: true,
            });
            made += 1;
        }
        (made, area)
    }
}

/// `p` inside the loop (a point on its boundary does not count).
fn inside(poly: &[DVec2], p: DVec2) -> bool {
    point_in_polygon(p, poly) == Inside::Inside
}

fn key(p: DVec3, q: f64) -> [i64; 3] {
    p.to_array().map(|x| (x / q).round() as i64)
}

/// Clip one triangle (mesh corner indices) to the kept side: the kept
/// pieces go to the mesh's indices, and where the plane crosses it, the
/// cut segment goes to `segments`.
fn clip_triangle(
    mesh: &mut crate::scene::Mesh,
    t: [u32; 3],
    plane: &CutPlane,
    eps: f64,
    segments: &mut Vec<[DVec3; 2]>,
) {
    let p = t.map(|i| mesh.positions[i as usize]);
    let d = p.map(|x| plane.dist(x));
    let side = d.map(|x| {
        if x > eps {
            1
        } else if x < -eps {
            -1
        } else {
            0
        }
    });
    // The cut segment: where the plane meets the triangle.
    let mut hits: Vec<DVec3> = Vec::new();
    for k in 0..3 {
        let (a, b) = (k, (k + 1) % 3);
        if side[a] == 0 {
            hits.push(p[a]);
        } else if side[a] * side[b] < 0 {
            hits.push(p[a].lerp(p[b], d[a] / (d[a] - d[b])));
        }
    }
    hits.dedup_by(|a, b| a.distance(*b) <= eps);
    if hits.len() == 2 && !(side[0] == 0 && side[1] == 0 && side[2] == 0) {
        segments.push([hits[0], hits[1]]);
    }
    if side.iter().all(|s| *s <= 0) {
        mesh.indices.extend_from_slice(&t);
        return;
    }
    if side.iter().all(|s| *s >= 0) && side.iter().any(|s| *s > 0) {
        // Only on-plane corners or cut away: nothing left.
        return;
    }
    // Straddling: walk the corners, keeping those on the kept side and
    // the points where an edge crosses the plane.
    let mut poly: Vec<u32> = Vec::new();
    for k in 0..3 {
        let (a, b) = (k, (k + 1) % 3);
        if side[a] <= 0 {
            poly.push(t[a]);
        }
        if side[a] * side[b] < 0 {
            let s = d[a] / (d[a] - d[b]);
            let (ia, ib) = (t[a] as usize, t[b] as usize);
            let n = mesh.normals[ia].lerp(mesh.normals[ib], s).normalize_or_zero();
            let uv = mesh.uvs[ia].lerp(mesh.uvs[ib], s);
            let at = mesh.positions[ia].lerp(mesh.positions[ib], s);
            let face = mesh.faces[ia];
            mesh.positions.push(at);
            mesh.normals.push(if n == DVec3::ZERO { mesh.normals[ia] } else { n });
            mesh.uvs.push(uv);
            mesh.faces.push(face);
            poly.push(mesh.positions.len() as u32 - 1);
        }
    }
    for k in 1..poly.len().saturating_sub(1) {
        mesh.indices.extend_from_slice(&[poly[0], poly[k], poly[k + 1]]);
    }
}

/// The part of a polyline on the kept side (the longest piece, if a
/// curve leaves and comes back). None if none of it is kept.
fn kept_run(pts: &[DVec3], plane: &CutPlane, eps: f64) -> Option<Vec<DVec3>> {
    let d: Vec<f64> = pts.iter().map(|p| plane.dist(*p)).collect();
    if d.iter().all(|x| *x <= eps) {
        return Some(pts.to_vec());
    }
    if d.iter().all(|x| *x > eps) {
        return None;
    }
    let mut runs: Vec<Vec<DVec3>> = Vec::new();
    let mut cur: Vec<DVec3> = Vec::new();
    for k in 0..pts.len() {
        if d[k] <= eps {
            cur.push(pts[k]);
        }
        if let Some(&next) = d.get(k + 1) {
            if (d[k] <= eps) != (next <= eps) && (d[k] - next).abs() > 0.0 {
                let at = pts[k].lerp(pts[k + 1], d[k] / (d[k] - next));
                if d[k] <= eps {
                    cur.push(at);
                    runs.push(std::mem::take(&mut cur));
                } else {
                    cur = vec![at];
                }
            }
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
    }
    runs.into_iter()
        .filter(|r| r.len() >= 2)
        .max_by(|a, b| length(a).total_cmp(&length(b)))
}

fn length(r: &[DVec3]) -> f64 {
    r.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// Join segments end to end into closed loops (those that do not close
/// are left out: an open surface has no cap).
fn closed_loops(segments: &[[DVec3; 2]], q: f64) -> Vec<Vec<DVec3>> {
    let mut at: BTreeMap<[i64; 3], Vec<usize>> = BTreeMap::new();
    for (i, [a, b]) in segments.iter().enumerate() {
        at.entry(key(*a, q)).or_default().push(i);
        at.entry(key(*b, q)).or_default().push(i);
    }
    let mut used = vec![false; segments.len()];
    let mut loops = Vec::new();
    for s in 0..segments.len() {
        if used[s] {
            continue;
        }
        used[s] = true;
        let start = key(segments[s][0], q);
        let mut pts = vec![segments[s][0]];
        let mut here = segments[s][1];
        let mut closed = false;
        loop {
            let k = key(here, q);
            if k == start {
                closed = true;
                break;
            }
            pts.push(here);
            let Some(next) = at
                .get(&k)
                .and_then(|v| v.iter().copied().find(|&j| !used[j]))
            else {
                break;
            };
            used[next] = true;
            let [a, b] = segments[next];
            here = if key(a, q) == k { b } else { a };
        }
        if closed && pts.len() >= 3 {
            loops.push(pts);
        }
    }
    loops
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_topo::Model;

    fn v(x: f64, y: f64, z: f64) -> DVec3 {
        DVec3::new(x, y, z)
    }

    /// A box `w` wide with its base on the ground, `h` tall.
    fn boxed(m: &mut Model, x: f64, w: f64, h: f64) {
        prism_ops::add_rect(m, x, 0.0, x + w, w, 0.0);
        let f = prism_ops::face_at(m, v(x + w / 2.0, w / 2.0, 0.0)).expect("base");
        let up = m.face(f).unwrap().plane().unwrap().normal.z;
        prism_ops::push_pull(m, f, h * up.signum(), false);
    }

    fn cut(m: &Model, origin: DVec3, normal: DVec3) -> Scene {
        let mut s = Scene::build_with(m, false);
        s.cut_by(
            CutPlane {
                origin,
                normal: normal.normalize(),
            },
            Rgba::rgb(200, 60, 60),
        );
        s
    }

    fn corners(s: &Scene) -> impl Iterator<Item = DVec3> + '_ {
        s.mesh.indices.iter().map(|i| s.mesh.positions[*i as usize])
    }

    #[test]
    fn a_box_cut_flat_is_half_a_box_with_a_cap() {
        let mut m = Model::new();
        boxed(&mut m, 0.0, 2.0, 2.0);
        let s = cut(&m, v(0.0, 0.0, 1.0), DVec3::Z);
        let c = s.cut.as_ref().expect("cut");
        assert_eq!(c.caps, 1);
        assert!((c.cap_area - 4.0).abs() < 1e-9, "{}", c.cap_area);
        let len: f64 = c.lines.iter().map(|[a, b]| a.distance(*b)).sum();
        assert!((len - 8.0).abs() < 1e-9, "the four walls' cuts: {len}");
        // Nothing drawn above the plane; the cap is on it, facing up.
        let tol = s.tol.dist;
        assert!(corners(&s).all(|p| p.z <= 1.0 + tol));
        assert!(s.edges.iter().all(|e| e.a.z <= 1.0 + tol && e.b.z <= 1.0 + tol));
        assert!(s.vertices.iter().all(|(_, p)| p.z <= 1.0 + tol));
        let cap = s.faces.iter().find(|f| f.cap).expect("a cap");
        assert_eq!(cap.normal, DVec3::Z);
        assert!(cap.id.0 >= CAP_FACE);
        // The cap is area 4: its triangles add up.
        let area: f64 = s.mesh.indices[cap.tris.start as usize..cap.tris.end as usize]
            .chunks_exact(3)
            .map(|t| {
                let p = [0, 1, 2].map(|k| s.mesh.positions[t[k] as usize]);
                (p[1] - p[0]).cross(p[2] - p[0]).length() / 2.0
            })
            .sum();
        assert!((area - 4.0).abs() < 1e-9, "{area}");
    }

    #[test]
    fn what_is_cut_away_cannot_be_picked_or_hide_anything() {
        let mut m = Model::new();
        boxed(&mut m, 0.0, 2.0, 2.0);
        let s = cut(&m, v(0.0, 0.0, 1.0), DVec3::Z);
        // From above, the top is gone: the ray meets the floor inside.
        let (id, t) = s
            .ray_face(v(1.0, 1.0, 5.0), -DVec3::Z)
            .expect("the floor is hit");
        assert!((5.0 - t).abs() < 1e-9, "the floor at z = 0, not the top");
        assert_ne!(s.face(id).unwrap().cap, true, "caps are not picked");
        // The cap is solid: it hides the floor from a point above it.
        assert!(s.occluded(v(1.0, 1.0, 5.0), v(1.0, 1.0, 0.0)));
        // A sheet that is cut away hides nothing behind it.
        let mut m2 = Model::new();
        prism_ops::add_rect(&mut m2, 0.0, 0.0, 2.0, 2.0, 0.0);
        prism_ops::add_rect(&mut m2, 0.0, 0.0, 2.0, 2.0, 2.0);
        let s2 = cut(&m2, v(0.0, 0.0, 1.0), DVec3::Z);
        assert!(!s2.occluded(v(1.0, 1.0, 5.0), v(1.0, 1.0, 0.0)));
        assert!(s2.ray_face(v(1.0, 1.0, 5.0), -DVec3::Z).is_some());
        // The outside of a wall below the plane is still picked.
        assert!(s.ray_face(v(5.0, 1.0, 0.5), -DVec3::X).is_some());
    }

    #[test]
    fn an_oblique_cut_caps_the_slanted_section() {
        let mut m = Model::new();
        boxed(&mut m, 0.0, 2.0, 2.0);
        let s = cut(&m, v(1.0, 1.0, 1.0), v(1.0, 0.0, 1.0));
        let c = s.cut.as_ref().expect("cut");
        assert_eq!(c.caps, 1);
        // A 2 by 2·√2 rectangle.
        assert!((c.cap_area - 4.0 * 2f64.sqrt()).abs() < 1e-9, "{}", c.cap_area);
    }

    #[test]
    fn a_pocket_makes_a_cap_with_a_hole() {
        let mut m = Model::new();
        boxed(&mut m, 0.0, 4.0, 2.0);
        // A square hole 0.8 in from each side, down to z = 0.5.
        prism_ops::add_rect(&mut m, 0.8, 0.8, 3.2, 3.2, 2.0);
        let f = prism_ops::face_at(&m, v(2.0, 2.0, 2.0)).expect("the pocket's floor");
        prism_ops::push_pull(&mut m, f, -1.5, false);
        let s = cut(&m, v(0.0, 0.0, 1.0), DVec3::Z);
        let c = s.cut.as_ref().expect("cut");
        assert_eq!(c.caps, 1, "one cap: the ring");
        assert!((c.cap_area - (16.0 - 5.76)).abs() < 1e-9, "{}", c.cap_area);
        let cap = s.faces.iter().find(|f| f.cap).unwrap();
        assert_eq!(cap.holes.len(), 1);
    }

    #[test]
    fn an_open_surface_has_cut_lines_and_no_cap() {
        let mut m = Model::new();
        prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 2.0, 0.0);
        let s = cut(&m, v(1.0, 0.0, 0.0), DVec3::X);
        let c = s.cut.as_ref().expect("cut");
        assert_eq!(c.caps, 0);
        // The two triangles each cut a piece; together they run 2 long.
        let len: f64 = c.lines.iter().map(|[a, b]| a.distance(*b)).sum();
        assert!((len - 2.0).abs() < 1e-9, "{len}");
        assert!(c.lines.iter().flatten().all(|p| (p.x - 1.0).abs() < 1e-9));
        assert!(s.faces.iter().all(|f| !f.cap));
    }

    #[test]
    fn a_plane_that_misses_changes_nothing_and_one_beyond_hides_all() {
        let mut m = Model::new();
        boxed(&mut m, 0.0, 2.0, 2.0);
        let whole = Scene::build_with(&m, false);
        let s = cut(&m, v(0.0, 0.0, 5.0), DVec3::Z);
        assert_eq!(s.mesh.indices.len(), whole.mesh.indices.len());
        assert_eq!(s.edges.len(), whole.edges.len());
        assert_eq!(s.cut.as_ref().unwrap().caps, 0);
        let gone = cut(&m, v(0.0, 0.0, -1.0), DVec3::Z);
        assert!(gone.mesh.indices.is_empty());
        assert!(gone.edges.is_empty());
        assert!(gone.vertices.is_empty());
    }

    #[test]
    fn flipping_the_plane_keeps_the_other_half() {
        let mut m = Model::new();
        boxed(&mut m, 0.0, 2.0, 2.0);
        let s = cut(&m, v(0.0, 0.0, 1.0), -DVec3::Z);
        let tol = s.tol.dist;
        assert!(corners(&s).all(|p| p.z >= 1.0 - tol));
        let cap = s.faces.iter().find(|f| f.cap).expect("a cap");
        assert_eq!(cap.normal, -DVec3::Z, "the cap faces the cut-away side");
    }
}
