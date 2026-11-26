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

use prism_math::DVec3;
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
    let mut mb = b.clone();
    crate::intersect::intersect_each(&mut ma, &mut mb);
    let mut pa = places(&ma, b, &tol).ok_or(UNSURE)?;
    let mut pb = places(&mb, a, &tol).ok_or(UNSURE)?;
    drop_membranes(&mut ma, &mut pa, a, &tol).ok_or(UNSURE)?;
    drop_membranes(&mut mb, &mut pb, b, &tol).ok_or(UNSURE)?;
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
    let faces = shell_faces(other);
    m.faces()
        .map(|(id, _)| Some((id, place_of(m, id, &faces, tol)?)))
        .collect()
}

fn shell_faces(m: &Model) -> Vec<ShellFace> {
    m.faces()
        .filter_map(|(id, _)| ShellFace::of(m, id))
        .collect()
}

/// Face `id` of `m` against the solid made of `faces`.
fn place_of(m: &Model, id: FaceId, faces: &[ShellFace], tol: &prism_math::Tol) -> Option<Place> {
    let step = crate::solid::step(tol);
    let inside = |p| shell::winding(faces, p, tol).map(|w| w >= 1);
    let (front, back) = shell::interior(m, id)
        .into_iter()
        .find_map(|(c, n)| Some((inside(c + n * step)?, inside(c - n * step)?)))?;
    Some(match (front, back) {
        (true, true) => Place::Inside,
        (false, false) => Place::Outside,
        // The other solid is behind it, as this one is.
        (false, true) => Place::Same,
        (true, false) => Place::Opposite,
    })
}

/// Cutting a solid by a face that runs right across it leaves that face
/// inside it, with the solid on both sides. If the other solid's face lies
/// on it, it is placed as Same or Opposite and would be kept. Cutting an
/// open-ended solid (a tray, its walls ending in one plane) by a face
/// lying across its open end likewise fills the end with a face that has
/// the solid on neither side. A face that is not on its own solid's
/// boundary is no part of the result: remove it. `m` is `own` cut by the
/// other; `None` if a face could not be told.
fn drop_membranes(
    m: &mut Model,
    places: &mut BTreeMap<FaceId, Place>,
    own: &Model,
    tol: &prism_math::Tol,
) -> Option<()> {
    let faces = shell_faces(own);
    let on_other: Vec<FaceId> = places
        .iter()
        .filter(|(_, p)| matches!(p, Place::Same | Place::Opposite))
        .map(|(id, _)| *id)
        .collect();
    for id in on_other {
        if matches!(
            place_of(m, id, &faces, tol)?,
            Place::Inside | Place::Outside
        ) {
            m.remove_face(id);
            places.remove(&id);
        }
    }
    Some(())
}

/// Add `b`'s faces to `out`, sharing its points and edges where they
/// coincide. Edges are matched by their middle as well as their ends: the
/// two halves of a circle join the same two points.
pub(crate) fn join(out: &mut Model, b: &Model, tol: &prism_math::Tol) {
    join_mapped(out, b, tol);
}

/// Where each point, edge and face of `b` went in `out`.
pub(crate) type Joined = (
    BTreeMap<VertexId, VertexId>,
    BTreeMap<EdgeId, EdgeId>,
    BTreeMap<FaceId, FaceId>,
);

/// [`join`], and where each of `b`'s points, edges and faces went.
pub(crate) fn join_mapped(out: &mut Model, b: &Model, tol: &prism_math::Tol) -> Joined {
    let mut fmap: BTreeMap<FaceId, FaceId> = BTreeMap::new();
    let mut vmap: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    // `out`'s points by grid cell (a cell wider than the tolerance, so a
    // point's match is in its cell or a neighbour), and its edges by their
    // ends: each lookup is then local, not a scan of the whole model (the
    // scans made joining a 50-face shell take seconds). The match found is
    // the same one as before: the first, by id, within the tolerance.
    let size = (tol.dist * 4.0).max(f64::MIN_POSITIVE);
    let cell = |p: DVec3| {
        let q = (p / size).floor();
        (q.x as i64, q.y as i64, q.z as i64)
    };
    let mut grid: BTreeMap<(i64, i64, i64), Vec<VertexId>> = BTreeMap::new();
    for (id, p) in out.vertices() {
        grid.entry(cell(p)).or_default().push(id);
    }
    for (id, p) in b.vertices() {
        let (cx, cy, cz) = cell(p);
        let mut at: Option<VertexId> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for v in grid.get(&(cx + dx, cy + dy, cz + dz)).into_iter().flatten() {
                        if tol.same_point(p, out.vertex(*v)) && at.is_none_or(|a| *v < a) {
                            at = Some(*v);
                        }
                    }
                }
            }
        }
        let v = at.unwrap_or_else(|| {
            let v = out.add_vertex(p);
            grid.entry(cell(p)).or_default().push(v);
            v
        });
        vmap.insert(id, v);
    }
    let ends = |a: VertexId, b: VertexId| if a < b { (a, b) } else { (b, a) };
    let mut by_ends: BTreeMap<(VertexId, VertexId), Vec<EdgeId>> = BTreeMap::new();
    for (id, x) in out.edges() {
        by_ends.entry(ends(x.a, x.b)).or_default().push(id);
    }
    let mut emap: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut cmap: BTreeMap<prism_topo::CurveId, prism_topo::CurveId> = BTreeMap::new();
    for (id, e) in b.edges() {
        let (va, vb) = (vmap[&e.a], vmap[&e.b]);
        let mid = e.curve.point(b.vertex(e.a), b.vertex(e.b), 0.5);
        let found = by_ends
            .get(&ends(va, vb))
            .into_iter()
            .flatten()
            .copied()
            .filter(|x| {
                out.edge(*x).is_some_and(|x| {
                    tol.same_point(x.curve.point(out.vertex(x.a), out.vertex(x.b), 0.5), mid)
                })
            })
            .min();
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
                by_ends.entry(ends(va, vb)).or_default().push(x);
            }
            emap.insert(id, x);
        }
    }
    for (id, f) in b.faces() {
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
        fmap.insert(id, out.add_face(face));
    }
    (vmap, emap, fmap)
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
            let (Some(ff), Some(gf)) = (m.face(*f), m.face(*g)) else {
                return false;
            };
            match (ff.plane(), gf.plane()) {
                (Some(p), Some(q)) => {
                    p.same_surface(q, &tol, m.face_extent(gf)) && p.normal.dot(q.normal) > 0.0
                }
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
