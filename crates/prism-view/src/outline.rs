//! Outline lines (Exact Curves 6d): where a smooth curved face turns away
//! from the eye, a line is drawn along its outline, as SketchUp's profile
//! lines on smooth surfaces. On screen only: they are not edges and cannot
//! be picked, and they move as the view turns.
//!
//! Each drawn triangle of a curved face knows the surface's normal at its
//! corners. The outline runs where the normal is square to the line of
//! sight: `n · (p - eye)` changes sign across the triangle, and the line
//! joins the two places on its sides where it is zero.

use prism_math::DVec3;
use prism_topo::FaceId;

use crate::scene::Scene;

/// One drawn triangle of a curved face.
#[derive(Clone, Debug)]
struct Tri {
    p: [DVec3; 3],
    n: [DVec3; 3],
    face: FaceId,
    index: usize,
    faded: bool,
}

/// One piece of an outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutlineLine {
    pub a: DVec3,
    pub b: DVec3,
    /// The face it runs across (it never hides its own line).
    pub face: FaceId,
    /// The face's place in `Scene::faces`.
    pub index: usize,
    /// Outside the open group: drawn faint, as its edges are.
    pub faded: bool,
}

/// The curved faces' triangles, kept to find outlines from any eye.
#[derive(Clone, Debug, Default)]
pub struct Outlines {
    tris: Vec<Tri>,
}

impl Outlines {
    pub fn of(scene: &Scene) -> Outlines {
        let mut tris = Vec::new();
        let m = &scene.mesh;
        for (index, f) in scene.faces.iter().enumerate() {
            let Some(c) = &f.curved else { continue };
            let idx = &m.indices[f.tris.start as usize..f.tris.end as usize];
            for t in idx.as_chunks::<3>().0 {
                let p = [0, 1, 2].map(|k| m.positions[t[k] as usize]);
                let n = p.map(|q| c.surface.normal_at(q));
                if n.iter().all(|x| x.is_finite()) {
                    tris.push(Tri {
                        p,
                        n,
                        face: f.id,
                        index,
                        faded: f.faded,
                    });
                }
            }
        }
        Outlines { tris }
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// The outline seen from `eye`.
    pub fn lines(&self, eye: DVec3) -> Vec<OutlineLine> {
        let mut out = Vec::new();
        for t in &self.tris {
            let g = [0, 1, 2].map(|k| t.n[k].dot(t.p[k] - eye));
            let front = g.map(|x| x > 0.0);
            if front[0] == front[1] && front[1] == front[2] {
                continue;
            }
            // The two sides whose ends are on opposite sides.
            let mut ends = [0, 1, 2].iter().filter_map(|&i| {
                let j = (i + 1) % 3;
                (front[i] != front[j]).then(|| {
                    let s = g[i] / (g[i] - g[j]);
                    t.p[i].lerp(t.p[j], s)
                })
            });
            if let (Some(a), Some(b)) = (ends.next(), ends.next()) {
                out.push(OutlineLine {
                    a,
                    b,
                    face: t.face,
                    index: t.index,
                    faded: t.faded,
                });
            }
        }
        out
    }
}
