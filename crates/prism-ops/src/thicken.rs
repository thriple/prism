//! Push/Pull of a freeform face (Solids & CAD part 3c): the face moves
//! along its normal by the distance, everywhere.
//!
//! The kernel cannot offset a freeform face (arris refuses one), so Prism
//! makes the offset itself: cubic Hermite pieces through the face's
//! points moved along its normal, between grid lines that include the
//! surface's knot lines (where its normal bends sharply), each piece split
//! until it is within a tenth of the tolerance of the true offset. The
//! face, its offset and ruled sides between their edges close a slab. A
//! lone face becomes the slab, as a lone flat face becomes a box; on a
//! solid the face gives way to the slab's sides and offset (out for a
//! pull, in for a push). Nothing is cut where the pull runs into other
//! geometry (Intersect Faces does that), and the faces beside it stay as
//! they are: where the face is not square to them, the pull leaves a
//! crease.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Curve, Nurbs, NurbsSurface, Patch, Spline, Surface};
use prism_math::{DVec2, DVec3};
use prism_topo::{EdgeId, Face, FaceId, Model, VertexId};

/// Said when the offset cannot be made within the tolerance.
pub const NO_OFFSET: &str =
    "Refused: Prism could not move this freeform face out that far within the tolerance.";
/// Said for a face that goes all the way round its surface.
pub const ROUND: &str =
    "Refused: a freeform face that goes all the way round cannot be pushed or pulled yet.";

/// How many times the pieces may be split.
const ROUNDS: usize = 8;

/// The slab a face sweeps: closed, facing out, and which of its faces is
/// the copy of the face.
pub struct Slab {
    pub model: Model,
    pub base: FaceId,
}

/// The slab the face sweeps when pulled by `d` along its normal: the face,
/// its offset and ruled sides, closed and facing out of the slab.
pub fn slab(model: &Model, face: FaceId, d: f64) -> Result<Slab, &'static str> {
    let f = model.face(face).ok_or(NO_OFFSET)?;
    let patch = f.surface.patch().ok_or(NO_OFFSET)?.clone();
    let laid = crate::crossing::Laid::new(model, face).ok_or(NO_OFFSET)?;
    if laid.wraps() {
        return Err(ROUND);
    }
    let eps = model.tol().dist * 0.1;
    let off = |p: DVec3| p + patch.normal_at(p) * d;
    let top = offset_surface(&patch, &laid.polys[0], d, eps).ok_or(NO_OFFSET)?;

    let mut s = Model::new();
    let mut vmap: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    let mut up: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for v in f.loops().flatten() {
        if !vmap.contains_key(v) {
            let p = model.vertex(*v);
            vmap.insert(*v, s.add_vertex(p));
            up.insert(*v, s.add_vertex(off(p)));
        }
    }
    // A line up from each corner.
    let mut rise: BTreeMap<VertexId, EdgeId> = BTreeMap::new();
    for (v, a) in &vmap {
        rise.insert(*v, s.add_edge(*a, up[v]).ok_or(NO_OFFSET)?);
    }
    // Each edge, its offset, and the ruled side between them.
    let mut bottom_of: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    let mut top_of: BTreeMap<EdgeId, EdgeId> = BTreeMap::new();
    for side in f.sides() {
        if bottom_of.contains_key(&side.edge) {
            continue;
        }
        let e = model.edge(side.edge).ok_or(NO_OFFSET)?;
        let (pa, pb) = (model.vertex(e.a), model.vertex(e.b));
        let along = |t: f64| e.curve.point(pa, pb, t);
        let breaks = kinks(&e.curve, &along, &patch);
        let (low, high) = edge_pair(&along, &off, &breaks, eps).ok_or(NO_OFFSET)?;
        let b = s
            .add_curve_edge(vmap[&e.a], vmap[&e.b], e.curve.clone(), None)
            .ok_or(NO_OFFSET)?;
        let t = s
            .add_curve_edge(
                up[&e.a],
                up[&e.b],
                Curve::Spline(Spline::whole(high.clone())),
                None,
            )
            .ok_or(NO_OFFSET)?;
        bottom_of.insert(side.edge, b);
        top_of.insert(side.edge, t);
        let ruled = Patch::whole(ruled(&low, &high).ok_or(NO_OFFSET)?);
        // Walked the way the face walks it, the face is on the left (about
        // its normal) and the side faces right, out of the slab. Along u
        // from a to b and along v up, u × v faces right of a to b when the
        // pull is out: the side keeps it when the face walks a to b.
        let flip = (side.from == e.a) != (d > 0.0);
        let (va, vb, ua, ub) = (vmap[&e.a], vmap[&e.b], up[&e.a], up[&e.b]);
        let (ra, rb) = (rise[&e.a], rise[&e.b]);
        let (loop_v, loop_e) = if flip {
            (vec![va, ua, ub, vb], vec![ra, t, rb, b])
        } else {
            (vec![va, vb, ub, ua], vec![b, rb, t, ra])
        };
        s.add_face(Face::with_edges(
            loop_v,
            vec![],
            Surface::Nurbs(Patch {
                flipped: flip,
                ..ruled
            }),
            vec![loop_e],
        ));
    }
    // The face itself and its offset, with the same loops: the one behind
    // the slab turned round.
    let copy =
        |map: &BTreeMap<VertexId, VertexId>, edges: &BTreeMap<EdgeId, EdgeId>, surface: Surface| {
            let mut vs: Vec<Vec<VertexId>> = f
                .loops()
                .map(|l| l.iter().map(|v| map[v]).collect())
                .collect();
            let es: Vec<Vec<EdgeId>> = (0..vs.len())
                .map(|k| f.loop_edges(k).map(|x| edges[&x.edge]).collect())
                .collect();
            let outer = vs.remove(0);
            Face::with_edges(outer, vs, surface, es)
        };
    let mut bottom = copy(&vmap, &bottom_of, f.surface.clone());
    let mut lid = copy(
        &up,
        &top_of,
        Surface::Nurbs(Patch {
            flipped: patch.flipped,
            ..Patch::whole(top)
        }),
    );
    if d > 0.0 {
        bottom.reverse();
    } else {
        lid.reverse();
    }
    let base = s.add_face(bottom);
    s.add_face(lid);
    if !crate::boolean::is_solid(&s) {
        return Err(NO_OFFSET);
    }
    Ok(Slab { model: s, base })
}

/// The surface `d` along the patch's normal from it over the box of
/// `outline` in (u, v), within `eps` of the true offset.
fn offset_surface(patch: &Patch, outline: &[DVec2], d: f64, eps: f64) -> Option<NurbsSurface> {
    let (lo, hi) = outline
        .iter()
        .fold((DVec2::INFINITY, DVec2::NEG_INFINITY), |(a, b), p| {
            (a.min(*p), b.max(*p))
        });
    let s = &patch.surface;
    let f = |u: f64, v: f64| {
        let q = DVec2::new(lo.x + (hi.x - lo.x) * u, lo.y + (hi.y - lo.y) * v);
        s.eval(q.x, q.y) + patch.normal(q.x, q.y) * d
    };
    // The box's ends and the knot lines inside it, as fractions across it.
    let dom = s.domain();
    let breaks = |k: usize| -> Vec<f64> {
        let (a, b) = (lo[k], hi[k]);
        let period = dom[k].1 - dom[k].0;
        let turns: &[f64] = if s.periodic()[k] {
            &[-1.0, 0.0, 1.0]
        } else {
            &[0.0]
        };
        let mut out: Vec<f64> = vec![0.0, 1.0];
        out.extend(
            s.knots()[k]
                .iter()
                .flat_map(|x| turns.iter().map(move |t| x + t * period))
                .filter(|x| *x > a && *x < b)
                .map(|x| (x - a) / (b - a)),
        );
        out.sort_by(f64::total_cmp);
        out.dedup_by(|x, y| (*x - *y).abs() < 1e-12);
        out
    };
    let (mut us, mut vs) = (breaks(0), breaks(1));
    for _ in 0..ROUNDS {
        let grid = NurbsSurface::hermite(&f, &us, &vs)?;
        let off = |u: f64, v: f64| grid.eval(u, v).distance(f(u, v)) > eps;
        // An interval is split where the surface is off half way along it
        // (on a grid line, or in the middle of a cell).
        let mut split_u = vec![false; us.len() - 1];
        let mut split_v = vec![false; vs.len() - 1];
        for (i, a) in us.windows(2).enumerate() {
            let mu = (a[0] + a[1]) / 2.0;
            for (j, b) in vs.windows(2).enumerate() {
                let mv = (b[0] + b[1]) / 2.0;
                if off(mu, b[0]) || off(mu, b[1]) {
                    split_u[i] = true;
                }
                if off(a[0], mv) || off(a[1], mv) {
                    split_v[j] = true;
                }
                if !split_u[i] && !split_v[j] && off(mu, mv) {
                    split_u[i] = true;
                    split_v[j] = true;
                }
            }
        }
        if !split_u.contains(&true) && !split_v.contains(&true) {
            return Some(grid);
        }
        us = halved(&us, &split_u);
        vs = halved(&vs, &split_v);
    }
    None
}

/// The nodes with the marked intervals cut in two.
fn halved(nodes: &[f64], split: &[bool]) -> Vec<f64> {
    let mut out = vec![nodes[0]];
    for (w, s) in nodes.windows(2).zip(split) {
        if *s {
            out.push((w[0] + w[1]) / 2.0);
        }
        out.push(w[1]);
    }
    out
}

/// The ends of an edge (as fractions along it) and where its offset bends
/// sharply between: its own curve's knots, and where it crosses the
/// surface's knot lines.
fn kinks(curve: &Curve, along: &dyn Fn(f64) -> DVec3, patch: &Patch) -> Vec<f64> {
    let mut out = Vec::new();
    if let Curve::Spline(sp) = curve {
        let (a, b) = (sp.t0, sp.t1);
        out.extend(sp.curve.knots().iter().map(|k| (k - a) / (b - a)));
    }
    const N: usize = 64;
    let uv: Vec<(f64, f64)> = (0..=N)
        .map(|i| patch.param(along(i as f64 / N as f64)))
        .collect();
    for k in 0..2 {
        let at = |x: (f64, f64)| if k == 0 { x.0 } else { x.1 };
        for knot in patch.surface.knots()[k] {
            for i in 0..N {
                let (x, y) = (at(uv[i]) - knot, at(uv[i + 1]) - knot);
                if x == 0.0 || (x < 0.0) == (y < 0.0) {
                    continue;
                }
                let (mut a, mut b) = (i as f64 / N as f64, (i + 1) as f64 / N as f64);
                for _ in 0..50 {
                    let c = (a + b) / 2.0;
                    if (at(patch.param(along(c))) - knot < 0.0) == (x < 0.0) {
                        a = c;
                    } else {
                        b = c;
                    }
                }
                out.push((a + b) / 2.0);
            }
        }
    }
    out.retain(|s| *s > 1e-9 && *s < 1.0 - 1e-9);
    out.extend([0.0, 1.0]);
    out.sort_by(f64::total_cmp);
    out.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    out
}

/// An edge as a cubic, and its offset with the same knots, both within
/// `eps` of the edge and of its offset: Hermite pieces between `nodes`,
/// split until close.
fn edge_pair(
    along: &dyn Fn(f64) -> DVec3,
    off: &dyn Fn(DVec3) -> DVec3,
    nodes: &[f64],
    eps: f64,
) -> Option<(Nurbs, Nurbs)> {
    let high_at = |t: f64| off(along(t));
    let mut ts = nodes.to_vec();
    for _ in 0..ROUNDS {
        let low = Nurbs::hermite(along, &ts)?;
        let high = Nurbs::hermite(&high_at, &ts)?;
        let split: Vec<bool> = ts
            .windows(2)
            .map(|w| {
                let t = (w[0] + w[1]) / 2.0;
                low.eval(t).distance(along(t)) > eps || high.eval(t).distance(high_at(t)) > eps
            })
            .collect();
        if !split.contains(&true) {
            return Some((low, high));
        }
        ts = halved(&ts, &split);
    }
    None
}

/// The ruled surface from `low` (at v = 0) to `high` (at v = 1), two
/// curves with the same knots.
fn ruled(low: &Nurbs, high: &Nurbs) -> Option<NurbsSurface> {
    let n = low.points().len();
    if high.points().len() != n || low.knots() != high.knots() {
        return None;
    }
    let mut points = Vec::with_capacity(2 * n);
    let mut weights = Vec::with_capacity(2 * n);
    for i in 0..n {
        points.extend([low.points()[i], high.points()[i]]);
        weights.extend([low.weights()[i], high.weights()[i]]);
    }
    NurbsSurface::new(
        [low.degree(), 1],
        [low.knots().to_vec(), vec![0.0, 0.0, 1.0, 1.0]],
        [n, 2],
        points,
        weights,
        [false, false],
    )
}

/// Push/Pull a freeform face by `d` along its normal. A lone face becomes
/// the slab; on a solid, the face gives way to the slab's sides and
/// offset, facing out of the solid.
pub fn pull(model: &mut Model, face: FaceId, d: f64) -> Result<(), &'static str> {
    let Slab {
        model: mut slab,
        base,
    } = slab(model, face, d)?;
    let on_solid = crate::solid::closed_shells(model)
        .iter()
        .any(|s| s.contains(&face));
    if on_solid {
        // The slab's copy of the face goes with it; pushed in, the rest
        // faces into the slab, which is now outside.
        slab.remove_face(base);
        if d < 0.0 {
            let all: BTreeSet<FaceId> = slab.faces().map(|(id, _)| id).collect();
            crate::health::reverse(&mut slab, &all);
        }
    }
    let tol = model.tol();
    remove_faces(model, &[face].into());
    crate::boolean::join(model, &slab, &tol);
    Ok(())
}

/// Remove `faces`, and the edges and points only they used.
fn remove_faces(model: &mut Model, faces: &BTreeSet<FaceId>) {
    let points: BTreeSet<VertexId> = faces
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.loops().flatten().copied().collect::<Vec<_>>())
        .collect();
    let used: BTreeSet<EdgeId> = faces
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.edge_ids().collect::<Vec<_>>())
        .collect();
    for f in faces {
        model.remove_face(*f);
    }
    let ef = model.edge_faces();
    for e in used {
        if ef.get(&e).is_none_or(|x| x.is_empty()) {
            model.remove_edge(e);
        }
    }
    let lone: Vec<VertexId> = points
        .into_iter()
        .filter(|v| !model.edges().any(|(_, e)| e.has(*v)))
        .collect();
    for v in lone {
        model.remove_vertex(v);
    }
}
