//! Booleans of two solids (Solids & CAD part 2, brief 39): Union,
//! Intersect and Subtract, exact for flat faces and the analytic curved
//! ones, with Prism's own crossings and inside test. Freeform faces cross
//! along cubics within the tolerance (part 3).
//!
//! Each solid is cut where the other crosses it (as Intersect Faces > With
//! does), each piece is placed against the other solid by testing a point
//! just in front of it and one just behind, and the pieces the operation
//! keeps are joined. Edges left between two faces on one plane are
//! erased, so the result has no seams.

use std::collections::{BTreeMap, BTreeSet};

use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

use crate::Selection;
use crate::shell::{self, ShellFace};

/// Which boolean.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Op {
    /// Everything in either.
    Union,
    /// What is in both.
    Intersect,
    /// The first less the second.
    Subtract,
}

/// Said when an input is not a closed solid.
pub const NOT_SOLID: &str =
    "Refused: one of these is not a solid. Solid > Validate and Heal says why.";
/// Said when a piece could not be placed (every test ray grazed an edge).
pub const UNSURE: &str = "Refused: Prism could not tell inside from outside here.";

/// Where a piece of one solid lies against the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Inside,
    Outside,
    /// On the other's boundary, facing the same way as its face there.
    Same,
    /// On it, facing the other way (the solids touch there).
    Opposite,
}

/// Whether every edge of `m` has exactly two faces (and there are faces).
pub fn is_solid(m: &Model) -> bool {
    let ef = m.edge_faces();
    m.faces().next().is_some()
        && m.edges()
            .all(|(id, _)| ef.get(&id).is_some_and(|f| f.len() == 2))
}

/// `a` op `b`, both solids in one frame. The result may be empty
/// (Intersect of solids that do not meet).
pub fn boolean(a: &Model, b: &Model, op: Op) -> Result<Model, &'static str> {
    for m in [a, b] {
        if !is_solid(m) {
            return Err(NOT_SOLID);
        }
    }
    let mut both = a.clone();
    both.append(b);
    let tol = both.tol();
    let mut ma = a.clone();
    crate::intersect::intersect_with(&mut ma, b);
    let mut mb = b.clone();
    crate::intersect::intersect_with(&mut mb, a);
    let pa = places(&ma, b, &tol).ok_or(UNSURE)?;
    let pb = places(&mb, a, &tol).ok_or(UNSURE)?;
    let (keep_a, keep_b): (&[Place], &[Place]) = match op {
        Op::Union => (&[Place::Outside, Place::Same], &[Place::Outside]),
        Op::Intersect => (&[Place::Inside, Place::Same], &[Place::Inside]),
        Op::Subtract => (&[Place::Outside, Place::Opposite], &[Place::Inside]),
    };
    keep_only(&mut ma, &pa, keep_a);
    keep_only(&mut mb, &pb, keep_b);
    if op == Op::Subtract {
        let all: BTreeSet<FaceId> = mb.faces().map(|(id, _)| id).collect();
        crate::health::reverse(&mut mb, &all);
    }
    let mut out = ma;
    join(&mut out, &mb, &tol);
    crate::health::remove_stray(&mut out);
    merge_seams(&mut out);
    Ok(out)
}

/// Each face of `m` placed against the solid `other`. None if one cannot
/// be told.
fn places(m: &Model, other: &Model, tol: &prism_math::Tol) -> Option<BTreeMap<FaceId, Place>> {
    let faces: Vec<ShellFace> = other
        .faces()
        .filter_map(|(id, _)| ShellFace::of(other, id))
        .collect();
    let step = crate::solid::step(tol);
    let inside = |p| shell::winding(&faces, p, tol).map(|w| w >= 1);
    m.faces()
        .map(|(id, _)| {
            let (front, back) = shell::interior(m, id)
                .into_iter()
                .find_map(|(c, n)| Some((inside(c + n * step)?, inside(c - n * step)?)))?;
            let place = match (front, back) {
                (true, true) => Place::Inside,
                (false, false) => Place::Outside,
                // The other solid is behind it, as this one is.
                (false, true) => Place::Same,
                (true, false) => Place::Opposite,
            };
            Some((id, place))
        })
        .collect()
}

/// Add `b`'s faces to `out`, sharing its points and edges where they
/// coincide. Edges are matched by their middle as well as their ends: the
/// two halves of a circle join the same two points.
fn join(out: &mut Model, b: &Model, tol: &prism_math::Tol) {
    let mut vmap: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for (id, p) in b.vertices() {
        let at = out
            .vertices()
            .find(|(_, q)| tol.same_point(p, *q))
            .map(|(v, _)| v);
        vmap.insert(id, at.unwrap_or_else(|| out.add_vertex(p)));
    }
    let mut emap: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut cmap: BTreeMap<prism_topo::CurveId, prism_topo::CurveId> = BTreeMap::new();
    for (id, e) in b.edges() {
        let (va, vb) = (vmap[&e.a], vmap[&e.b]);
        let mid = e.curve.point(b.vertex(e.a), b.vertex(e.b), 0.5);
        let found = out
            .edges()
            .find(|(_, x)| {
                ((x.a, x.b) == (va, vb) || (x.a, x.b) == (vb, va))
                    && tol.same_point(x.curve.point(out.vertex(x.a), out.vertex(x.b), 0.5), mid)
            })
            .map(|(x, _)| x);
        let made = found.or_else(|| {
            if e.curve.is_curved() {
                let cid = e
                    .curve_id
                    .map(|c| *cmap.entry(c).or_insert_with(|| out.new_curve_id()));
                out.add_curve_edge(va, vb, e.curve.clone(), cid)
            } else {
                out.add_edge(va, vb)
            }
        });
        if let Some(x) = made {
            if found.is_none() {
                out.set_soft(x, e.soft, e.smooth);
            }
            emap.insert(id, x);
        }
    }
    for (_, f) in b.faces() {
        let map = |l: &Vec<VertexId>| l.iter().map(|v| vmap[v]).collect::<Vec<_>>();
        let edges: Vec<Vec<EdgeId>> = (0..=f.holes.len())
            .map(|k| {
                f.loop_edges(k)
                    .filter_map(|s| emap.get(&s.edge).copied())
                    .collect()
            })
            .collect();
        let mut face = Face::with_edges(
            map(&f.outer),
            f.holes.iter().map(map).collect(),
            f.surface.clone(),
            edges,
        );
        face.paint = f.paint;
        out.add_face(face);
    }
}

/// Remove the faces whose place is not in `keep`, then what only they used.
fn keep_only(m: &mut Model, places: &BTreeMap<FaceId, Place>, keep: &[Place]) {
    for (id, p) in places {
        if !keep.contains(p) {
            m.remove_face(*id);
        }
    }
    crate::health::remove_stray(m);
}

/// Erase edges between two faces on one plane facing the same way: the
/// faces merge (the eraser's rule, DECISIONS #15).
fn merge_seams(m: &mut Model) {
    let tol = m.tol();
    let seams: BTreeSet<_> = m
        .edge_faces()
        .into_iter()
        .filter(|(_, fs)| {
            let [f, g] = fs.as_slice() else { return false };
            match (
                m.face(*f).and_then(|x| x.plane()),
                m.face(*g).and_then(|x| x.plane()),
            ) {
                (Some(p), Some(q)) => p.same_surface(q, &tol) && p.normal.dot(q.normal) > 0.0,
                _ => false,
            }
        })
        .map(|(e, _)| e)
        .collect();
    if !seams.is_empty() {
        crate::remove(
            m,
            &Selection {
                edges: seams,
                ..Default::default()
            },
        );
    }
}
