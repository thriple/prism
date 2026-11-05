//! Push/Pull of a face.
//!
//! - A face whose every outer edge meets a perpendicular side face (the top
//!   of a prism) stretches: its vertices move.
//! - Otherwise it extrudes: the face moves out, with a wall for every edge of
//!   every loop (holes too).
//! - A standalone face (sharing no edge with another face) also gets a base,
//!   so the result is closed.
//! - With `new_loop` (Alt) the original boundary stays as a face, and a new
//!   loop is extruded from it.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::Surface;
use prism_math::Plane;
use prism_topo::{Face, FaceId, Model, VertexId, loop_pairs};

use crate::transform;

/// What Push/Pull did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pulled {
    Nothing,
    Stretched,
    Extruded { walls: Vec<FaceId> },
}

pub fn push_pull(model: &mut Model, face: FaceId, distance: f64, new_loop: bool) -> Pulled {
    let tol = model.tol();
    let Some(f) = model.face(face).cloned() else {
        return Pulled::Nothing;
    };
    if tol.is_zero(distance) {
        return Pulled::Nothing;
    }
    let n = f.plane().normal;
    let edge_faces = model.edge_faces();
    let others = |a: VertexId, b: VertexId| -> Vec<FaceId> {
        model
            .edge_between(a, b)
            .and_then(|e| edge_faces.get(&e))
            .map(|v| v.iter().copied().filter(|x| *x != face).collect())
            .unwrap_or_default()
    };
    let mut standalone = true;
    let mut stretch = f.holes.is_empty();
    for (li, l) in f.loops().enumerate() {
        for (a, b) in loop_pairs(l) {
            let o = others(a, b);
            if !o.is_empty() {
                standalone = false;
            }
            if li == 0
                && !o.iter().any(|g| {
                    model
                        .face(*g)
                        .is_some_and(|g| tol.perpendicular(g.plane().normal, n))
                })
            {
                stretch = false;
            }
        }
    }

    if stretch && !new_loop {
        let vs: BTreeSet<VertexId> = f.outer.iter().copied().collect();
        transform::translate(model, &vs, n * distance);
        return Pulled::Stretched;
    }

    let shift = n * distance;
    let mut moved: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for v in f.loops().flatten() {
        moved
            .entry(*v)
            .or_insert_with(|| model.add_vertex(model.vertex(*v) + shift));
    }
    let mut walls = Vec::new();
    for l in f.loops() {
        for (u, w) in loop_pairs(l) {
            let (nu, nw) = (moved[&u], moved[&w]);
            model.add_edge(w, nw);
            model.add_edge(nw, nu);
            model.add_edge(nu, u);
            let quad = vec![u, w, nw, nu];
            if let Some(plane) = Plane::fit(&model.points(&quad), &tol) {
                walls.push(model.add_face(Face {
                    outer: quad,
                    holes: vec![],
                    surface: Surface::Plane(plane),
                }));
            }
        }
    }

    // The face moves out.
    let remap = |l: &[VertexId]| -> Vec<VertexId> { l.iter().map(|v| moved[v]).collect() };
    let old_plane = *f.plane();
    let moved_plane = Plane::new(old_plane.origin + shift, old_plane.normal);
    if let Some(g) = model.face_mut(face) {
        g.outer = remap(&f.outer);
        g.holes = f.holes.iter().map(|h| remap(h)).collect();
        g.surface = Surface::Plane(moved_plane);
    }

    let original = |flip: bool| -> Face {
        let mut outer = f.outer.clone();
        let mut holes = f.holes.clone();
        let mut plane = old_plane;
        if flip {
            outer.reverse();
            holes.iter_mut().for_each(|h| h.reverse());
            plane = plane.flipped();
        }
        Face {
            outer,
            holes,
            surface: Surface::Plane(plane),
        }
    };
    if new_loop && !standalone {
        // Alt: the original boundary stays as a face.
        model.add_face(original(false));
    } else if standalone {
        if distance > 0.0 {
            // A base facing the other way closes the solid.
            model.add_face(original(true));
        } else {
            // Pushed back: the moved face now faces the other way, and the
            // original stays as the cap.
            if let Some(g) = model.face_mut(face) {
                g.outer.reverse();
                g.holes.iter_mut().for_each(|h| h.reverse());
                g.surface = Surface::Plane(moved_plane.flipped());
            }
            model.add_face(original(false));
        }
    }
    Pulled::Extruded { walls }
}
