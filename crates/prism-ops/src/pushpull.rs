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
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId, loop_pairs};

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

/// Said when Push/Pull is asked to pull a curved face, or a face whose
/// pull would bend a curved face beside it.
pub const CURVED_PULL: &str =
    "Curved faces cannot be pulled yet, nor faces that would bend one beside them.";

/// Why `face` cannot be pulled yet, if it cannot. A face beside a curved
/// face can be pulled when the curved face runs along the pull (the cap of
/// a cylinder), so it only stretches.
pub fn refusal(model: &Model, face: FaceId) -> Option<&'static str> {
    let f = model.face(face)?;
    let Some(plane) = f.plane() else {
        // A new radius: Exact Curves 4d.
        return Some(CURVED_PULL);
    };
    let tol = model.tol();
    let bends = model.faces().any(|(g, x)| {
        g != face
            && match x.surface {
                Surface::Cylinder(c) => {
                    !tol.parallel(c.axis, plane.normal)
                        && x.loops().flatten().any(|v| f.uses_vertex(*v))
                }
                Surface::Plane(_) => false,
            }
    });
    bends.then_some(CURVED_PULL)
}

pub fn push_pull(model: &mut Model, face: FaceId, distance: f64, new_loop: bool) -> Pulled {
    let tol = model.tol();
    let Some(f) = model.face(face).cloned() else {
        return Pulled::Nothing;
    };
    if refusal(model, face).is_some() {
        return Pulled::Nothing;
    }
    let Some(&plane) = f.plane() else {
        return Pulled::Nothing;
    };
    // It stops at the first face in its way (Alt's new loop is not
    // limited: the original loop stays behind).
    let distance = if new_loop {
        distance
    } else {
        crate::reach::limit(model, face, distance)
    };
    if tol.is_zero(distance) {
        return Pulled::Nothing;
    }
    let n = plane.normal;
    let shift = n * distance;
    // On a solid, Push/Pull is a boolean with the block the face sweeps.
    // Not yet with arcs or curved faces (the block's test goes by flat
    // faces' corners).
    let curved = model.face_has_arcs(face) || model.faces().any(|(_, g)| g.is_curved());
    let swept = if new_loop || curved {
        None
    } else {
        crate::solid::Swept::of(model, face, distance)
    };
    let face_edges_before = edges_with_faces(model);
    // The planes around the face, before the edit: a face there may shrink
    // to nothing, and its plane may still need a face (a post left on top).
    let around_planes: Vec<Plane> = {
        let vs: BTreeSet<VertexId> = f.loops().flatten().copied().collect();
        model
            .faces()
            .filter(|(_, g)| g.loops().flatten().any(|v| vs.contains(v)))
            .filter_map(|(_, g)| g.plane().copied())
            .collect()
    };
    let before_ids: BTreeSet<prism_topo::EdgeId> = model.edges().map(|(id, _)| id).collect();
    let edge_faces = model.edge_faces();
    let others = |e: prism_topo::EdgeId| -> Vec<FaceId> {
        edge_faces
            .get(&e)
            .map(|v| v.iter().copied().filter(|x| *x != face).collect())
            .unwrap_or_default()
    };
    // A face that runs along the push and lies behind the face (the walls
    // under a box's top), so it can stretch.
    let origin = plane.origin;
    let along = |g: FaceId| {
        model.face(g).is_some_and(|g| {
            let pts = model.points(&g.outer);
            let centre = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
            let runs_along = match g.surface {
                Surface::Plane(p) => tol.perpendicular(p.normal, n),
                Surface::Cylinder(c) => tol.parallel(c.axis, n),
            };
            runs_along && (centre - origin).dot(n) < 0.0
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
    for side in f.sides() {
        let (a, b) = (side.from, side.to);
        let o = others(side.edge);
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
    // An arc with one end staying and one moving would bend; one that
    // slides would need a stretched arc in its neighbour. Not yet.
    let arc_sides: Vec<(VertexId, VertexId)> = f
        .sides()
        .filter(|s| model.edge(s.edge).is_some_and(|e| e.curve.is_arc()))
        .map(|s| (s.from, s.to))
        .collect();
    let bends = arc_sides.iter().any(|(a, b)| {
        staying.contains(a) != staying.contains(b)
            || slides
                .iter()
                .any(|(k, _)| *k == key(*a, *b) && (staying.contains(a) || staying.contains(b)))
    });
    if bends {
        return Pulled::Nothing;
    }

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
            let chain: Vec<VertexId> = [*a, to[a], to[b], *b].into();
            for w in chain.windows(2) {
                model.add_edge(w[0], w[1]);
            }
            model.edit_face(*g, |gf| {
                for l in std::iter::once(&mut gf.outer).chain(gf.holes.iter_mut()) {
                    let len = l.len();
                    let Some(i) = (0..len).find(|&i| key(l[i], l[(i + 1) % len]) == key(*a, *b))
                    else {
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
            });
        }
    }

    // Where each side of the face goes: the same edge when both its ends
    // move in place, else a copy at the new points (an arc moved, with a
    // new curve id per curve).
    // Made as each wall needs it, then for the other sides (the order
    // edges were made in before arcs, so recorded sessions still match).
    let step = prism_math::DAffine3::from_translation(shift);
    let mut moved_edge: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut new_curve: BTreeMap<prism_topo::CurveId, prism_topo::CurveId> = BTreeMap::new();
    let mut move_side = |model: &mut Model, edge: EdgeId| -> Option<EdgeId> {
        if let Some(c) = moved_edge.get(&edge) {
            return Some(*c);
        }
        let e = model.edge(edge).cloned()?;
        let (na, nb) = (to[&e.a], to[&e.b]);
        let copy = if na == e.a && nb == e.b {
            Some(edge)
        } else if let Some(curve) = e.curve.transformed(&step).filter(|c| c.is_arc()) {
            let id = e
                .curve_id
                .map(|c| *new_curve.entry(c).or_insert_with(|| model.new_curve_id()));
            model.add_curve_edge(na, nb, curve, id)
        } else {
            model.add_edge(na, nb)
        }?;
        moved_edge.insert(edge, copy);
        Some(copy)
    };

    // Walls: flat for a line, part of a cylinder for an arc.
    let mut walls = Vec::new();
    for side in f.sides() {
        let (u, w) = (side.from, side.to);
        if !walls_at.contains(&key(u, w)) {
            continue;
        }
        let (nu, nw) = (to[&u], to[&w]);
        let quad = vec![u, w, nw, nu];
        let arc = model.edge(side.edge).and_then(|e| match e.curve {
            prism_geom::Curve::Arc { centre, radius, .. } => Some((centre, radius)),
            prism_geom::Curve::Line => None,
        });
        let up = model.add_edge(w, nw);
        let top = move_side(model, side.edge);
        let down = model.add_edge(nu, u);
        let Some((centre, radius)) = arc else {
            if let Some(plane) = Plane::fit(&model.points(&quad), &tol) {
                walls.push(model.add_face(Face::new(quad, vec![], Surface::Plane(plane))));
            }
            continue;
        };
        let (Some(up), Some(top), Some(down)) = (up, top, down) else {
            continue;
        };
        let cylinder = prism_geom::Cylinder {
            origin: centre,
            axis: n,
            radius,
            outward: true,
        };
        let wall = model.add_face(Face::with_edges(
            quad,
            vec![],
            Surface::Cylinder(cylinder),
            vec![vec![side.edge, up, top, down]],
        ));
        // It faces out where its loop winds counter-clockwise unrolled, as
        // a flat wall faces the way its loop winds.
        if let Some(g) = model.face(wall) {
            let p = model.unrolled(g, 0);
            let area: f64 = (0..p.len())
                .map(|i| p[i].1.perp_dot(p[(i + 1) % p.len()].1))
                .sum();
            if area < 0.0 {
                model.edit_face(wall, |g| g.surface = g.surface.flipped());
            }
        }
        walls.push(wall);
    }

    for side in f.sides() {
        move_side(model, side.edge);
    }
    let moved_edge = moved_edge;

    // The face moves out.
    let remap = |l: &[VertexId]| -> Vec<VertexId> { l.iter().map(|v| to[v]).collect() };
    let loop_edges = |k: usize, map: &dyn Fn(EdgeId) -> EdgeId| -> Vec<EdgeId> {
        f.loop_edge_ids(k).iter().map(|e| map(*e)).collect()
    };
    let n_loops = 1 + f.holes.len();
    let moved_edges: Vec<Vec<EdgeId>> = (0..n_loops)
        .map(|k| loop_edges(k, &|e| moved_edge.get(&e).copied().unwrap_or(e)))
        .collect();
    let old_plane = plane;
    let moved_plane = Plane::new(old_plane.origin + shift, old_plane.normal);
    let moved_face = Face::with_edges(
        remap(&f.outer),
        f.holes.iter().map(|h| remap(h)).collect(),
        Surface::Plane(moved_plane),
        moved_edges,
    );
    model.edit_face(face, |g| *g = moved_face);

    let original = |flip: bool| -> Face {
        let edges: Vec<Vec<EdgeId>> = (0..n_loops).map(|k| loop_edges(k, &|e| e)).collect();
        let mut o = Face::with_edges(
            f.outer.clone(),
            f.holes.clone(),
            Surface::Plane(old_plane),
            edges,
        );
        if flip {
            o.reverse();
        }
        o
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
            // (Its surface is the moved plane, which turns round too.)
            // The walls were wound for a pull out: they turn round too, so
            // the solid faces out.
            model.edit_face(face, |g| g.reverse());
            model.add_face(original(false));
            for w in &walls {
                model.edit_face(*w, |g| g.reverse());
            }
        }
    }

    // Edges of the old boundary that no face uses any more.
    for e in f.edge_ids() {
        if model.edge(e).is_some() && !model.faces().any(|(_, g)| g.uses_edge(e)) {
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
    let test = swept.as_ref().map(|s| move |x: DVec3| s.inside(x));
    let test: Option<&dyn Fn(DVec3) -> Option<bool>> =
        test.as_ref().map(|f| f as &dyn Fn(DVec3) -> Option<bool>);
    crate::settle::settle_with(model, &touched, &around_planes, test);
    // Alt keeps the original face on purpose, inside or not. Its inside
    // test goes by flat faces, so not yet with curved ones.
    if !new_loop && !curved {
        crate::solid::keep_boundary(model, &touched, swept.as_ref());
    }
    crate::settle::dedupe_faces(model);
    // Edges the edit left without a face (a box pushed flat, faces
    // removed as inside) go, with their points; loose lines drawn before
    // stay.
    let now = edges_with_faces(model);
    let left: Vec<prism_topo::EdgeId> = model
        .edges()
        .filter(|(id, _)| {
            !now.contains(id) && (face_edges_before.contains(id) || !before_ids.contains(id))
        })
        .map(|(id, _)| id)
        .collect();
    let mut ends = BTreeSet::new();
    for e in left {
        if let Some(ed) = model.edge(e) {
            ends.insert(ed.a);
            ends.insert(ed.b);
        }
        model.remove_edge(e);
    }
    for v in ends {
        if !model.edges().any(|(_, e)| e.has(v)) && !model.faces().any(|(_, f)| f.uses_vertex(v)) {
            model.remove_vertex(v);
        }
    }
    walls.retain(|w| model.face(*w).is_some());

    if new.is_empty() {
        Pulled::Stretched
    } else {
        Pulled::Extruded { walls }
    }
}

/// Edges that at least one face runs along.
fn edges_with_faces(model: &Model) -> BTreeSet<prism_topo::EdgeId> {
    model
        .edge_faces()
        .into_iter()
        .filter(|(_, f)| !f.is_empty())
        .map(|(e, _)| e)
        .collect()
}
