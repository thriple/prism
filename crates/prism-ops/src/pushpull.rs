//! Push/Pull of a face, decided edge by edge.
//!
//! - An edge whose other faces all run along the push direction (the walls
//!   of a box, under its top) slides with the face: those faces stretch.
//!   An edge with no other face slides when lines run back along the push
//!   from both its ends (an open box).
//! - Every other edge gets a new wall between where it was and where it
//!   goes (a face pulled out of a bigger face, or the hole of a ring whose
//!   middle stays behind).
//! - A face with no sliding edge extrudes whole; a standalone face (sharing
//!   no edge) also gets a base, so the result is closed.
//! - With `new_loop` (Alt) the original boundary stays as a face, and a new
//!   loop is extruded from it.
//!
//! Afterwards the coplanar rule runs on every plane the pull touched
//! ([`crate::settle`]).

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::Surface;
use prism_math::{DVec3, Plane};
use prism_topo::{Face, FaceId, Model, VertexId, loop_pairs};

use crate::transform;

/// What Push/Pull did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pulled {
    Nothing,
    Stretched,
    Extruded { walls: Vec<FaceId> },
}

fn key(a: VertexId, b: VertexId) -> (VertexId, VertexId) {
    (a.min(b), a.max(b))
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
    let shift = n * distance;
    let edge_faces = model.edge_faces();
    let others = |a: VertexId, b: VertexId| -> Vec<FaceId> {
        model
            .edge_between(a, b)
            .and_then(|e| edge_faces.get(&e))
            .map(|v| v.iter().copied().filter(|x| *x != face).collect())
            .unwrap_or_default()
    };
    // A face that runs along the push and lies behind the face (the walls
    // under a box's top), so it can stretch.
    let origin = f.plane().origin;
    let along = |g: FaceId| {
        model.face(g).is_some_and(|g| {
            let pts = model.points(&g.outer);
            let centre = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
            tol.perpendicular(g.plane().normal, n) && (centre - origin).dot(n) < 0.0
        })
    };
    // A point with a line running back from it along the push (an open box).
    let line_behind = |v: VertexId| {
        let at = model.vertex(v);
        model.edges().filter(|(_, e)| e.has(v)).any(|(_, e)| {
            let d = model.vertex(e.other(v)) - at;
            d.length() > tol.dist && tol.parallel(d.normalize(), n) && d.dot(n) < 0.0
        })
    };

    // Which edges slide and which get a wall.
    let pairs: Vec<(VertexId, VertexId)> = f.loops().flat_map(|l| loop_pairs(l)).collect();
    let mut standalone = true;
    let mut any_slide = false;
    let mut walls_at: BTreeSet<(VertexId, VertexId)> = BTreeSet::new();
    let mut slides: Vec<((VertexId, VertexId), Vec<FaceId>)> = Vec::new();
    for &(a, b) in &pairs {
        let o = others(a, b);
        if o.is_empty() {
            if line_behind(a) && line_behind(b) {
                slides.push(((a, b), o));
            } else {
                walls_at.insert(key(a, b));
            }
            continue;
        }
        standalone = false;
        if o.iter().all(|g| along(*g)) {
            any_slide = true;
            slides.push(((a, b), o));
        } else {
            walls_at.insert(key(a, b));
        }
    }
    if new_loop || !any_slide {
        walls_at = pairs.iter().map(|&(a, b)| key(a, b)).collect();
        slides.clear();
    }

    // A vertex moves in place when nothing that stays behind uses it: none
    // of its edges on the face gets a wall, every other face at it runs
    // along the push, and every other edge at it points along the push.
    let on_face: BTreeSet<(VertexId, VertexId)> = pairs.iter().map(|&(a, b)| key(a, b)).collect();
    let verts: BTreeSet<VertexId> = f.loops().flatten().copied().collect();
    let in_place = |v: VertexId| -> bool {
        if on_face
            .iter()
            .any(|&(a, b)| (a == v || b == v) && walls_at.contains(&(a, b)))
        {
            return false;
        }
        let faces_ok = model
            .faces()
            .filter(|(id, g)| *id != face && g.uses_vertex(v))
            .all(|(id, _)| along(id));
        let at = model.vertex(v);
        let edges_ok = model.edges().filter(|(_, e)| e.has(v)).all(|(_, e)| {
            let w = e.other(v);
            if on_face.contains(&key(v, w)) {
                return true;
            }
            let d = model.vertex(w) - at;
            d.length() > tol.dist && tol.parallel(d.normalize(), n)
        });
        faces_ok && edges_ok
    };
    let staying: BTreeSet<VertexId> = verts.iter().copied().filter(|v| !in_place(*v)).collect();
    let moving: BTreeSet<VertexId> = verts.difference(&staying).copied().collect();

    // New points for the vertices that stay behind.
    let mut to: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for v in f.loops().flatten() {
        if to.contains_key(v) {
            continue;
        }
        let w = if staying.contains(v) {
            model.add_vertex(model.vertex(*v) + shift)
        } else {
            *v
        };
        to.insert(*v, w);
    }
    let new: BTreeSet<VertexId> = staying.iter().map(|v| to[v]).collect();

    // Sliding edges: a neighbour that ran along a-b now runs a, a', b', b,
    // leaving out the points that move in place.
    for ((a, b), neighbours) in &slides {
        if !staying.contains(a) && !staying.contains(b) {
            continue;
        }
        for g in neighbours {
            let Some(gf) = model.face_mut(*g) else {
                continue;
            };
            for l in std::iter::once(&mut gf.outer).chain(gf.holes.iter_mut()) {
                let len = l.len();
                let Some(i) = (0..len).find(|&i| key(l[i], l[(i + 1) % len]) == key(*a, *b)) else {
                    continue;
                };
                let (x, y) = (l[i], l[(i + 1) % len]);
                let mut mid = Vec::new();
                if staying.contains(&x) {
                    mid.push(to[&x]);
                }
                if staying.contains(&y) {
                    mid.push(to[&y]);
                }
                for (k, m) in mid.into_iter().enumerate() {
                    l.insert(i + 1 + k, m);
                }
                break;
            }
            let chain: Vec<VertexId> = [*a, to[a], to[b], *b].into();
            for w in chain.windows(2) {
                model.add_edge(w[0], w[1]);
            }
        }
    }

    // Walls.
    let mut walls = Vec::new();
    for l in f.loops() {
        for (u, w) in loop_pairs(l) {
            if !walls_at.contains(&key(u, w)) {
                continue;
            }
            let (nu, nw) = (to[&u], to[&w]);
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
    let remap = |l: &[VertexId]| -> Vec<VertexId> { l.iter().map(|v| to[v]).collect() };
    for (a, b) in loop_pairs(&remap(&f.outer)).chain(f.holes.iter().flat_map(|h| {
        let r = remap(h);
        loop_pairs(&r).collect::<Vec<_>>()
    })) {
        model.add_edge(a, b);
    }
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

    // Edges of the old boundary that no face uses any more.
    for (a, b) in &on_face {
        if let Some(e) = model.edge_between(*a, *b)
            && !model.faces().any(|(_, g)| {
                g.loops()
                    .any(|l| loop_pairs(l).any(|(x, y)| key(x, y) == (*a, *b)))
            })
        {
            model.remove_edge(e);
        }
    }

    // Move the points that go in place (this refits the faces around them
    // and welds), then let new points land on existing ones (a box pulled
    // up beside another shares its edges).
    let landing: Vec<DVec3> = moving
        .iter()
        .map(|v| model.vertex(*v) + shift)
        .chain(new.iter().map(|v| model.vertex(*v)))
        .collect();
    if !moving.is_empty() {
        transform::translate_few(model, &moving, shift);
    }
    transform::weld_new(model, &new, &tol);
    walls.retain(|w| model.face(*w).is_some());

    let touched: BTreeSet<VertexId> = model
        .vertices()
        .filter(|(_, p)| landing.iter().any(|q| tol.same_point(*p, *q)))
        .map(|(id, _)| id)
        .collect();
    crate::settle::settle(model, &touched);
    walls.retain(|w| model.face(*w).is_some());

    if new.is_empty() {
        Pulled::Stretched
    } else {
        Pulled::Extruded { walls }
    }
}
