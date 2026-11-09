//! The solid rule (DECISIONS #31): where faces enclose space, the model
//! keeps the boundary of that space.
//!
//! After an edit, the faces it touched are split where they cross other
//! faces (as Intersect Faces does), and then every face piece near the
//! edit that belongs to a closed shell is tested: a point just in front of
//! it and a point just behind it. Only a piece with solid on exactly one
//! side is a boundary, and it faces away from the solid (one facing into
//! it is turned round); the rest go. That one rule removes walls left
//! standing inside a box, faces pulled into another block, faces swallowed
//! by a pull, and opens a tunnel where a face is pushed out through the back.
//!
//! Faces that enclose nothing (a rectangle on the ground, an open box) are
//! never removed by this rule.

use std::collections::{BTreeMap, BTreeSet};

use prism_math::predicates::{Inside, point_in_polygon};
use prism_math::{DVec2, DVec3, Tol};
use prism_topo::{EdgeId, FaceId, Model, VertexId};

use crate::intersect::{Flat, crossings};
use crate::regions::interior_points;

/// Ray directions for the inside test, tried in turn until one passes
/// cleanly (not through an edge or along a face). Fixed, so results are
/// the same every run.
const RAYS: [DVec3; 4] = [
    DVec3::new(0.284_7, 0.539_1, 0.792_6),
    DVec3::new(-0.613_2, 0.217_9, 0.759_3),
    DVec3::new(0.471_5, -0.802_3, 0.366_1),
    DVec3::new(-0.152_8, -0.433_6, -0.888_1),
];

/// The distance from a face at which its two sides are tested: just past
/// the tolerance, so the test points stay inside the thinnest wall the
/// model can keep (a 0.5 mm plate 300 m wide is about 2 tolerances thick).
/// Points within one tolerance of a face or an edge count as unsure, and a
/// face whose sides are unsure is kept.
pub(crate) fn step(tol: &Tol) -> f64 {
    (tol.dist * 1.2).max(f64::MIN_POSITIVE)
}

/// A Push/Pull on a face of a solid, as a boolean: pulling out adds the
/// block the face sweeps through, pushing in removes it. Knows exactly what
/// is inside the result, from the solids as they were before the edit.
pub struct Swept {
    before: Vec<Flat>,
    /// Faces of solids before the edit, and every face before the edit.
    solid_ids: BTreeSet<FaceId>,
    all_ids: BTreeSet<FaceId>,
    face: Flat,
    d: f64,
    tol: Tol,
}

impl Swept {
    /// None if the face is not on a closed solid (a lone face, an open box):
    /// then Push/Pull is not a boolean, and only the plain rules apply.
    pub fn of(model: &Model, face: FaceId, d: f64) -> Option<Swept> {
        let shells = closed_shells(model);
        if !shells.iter().any(|s| s.contains(&face)) {
            return None;
        }
        let flat = Flat::of(model, face)?;
        let tol = model.tol();
        // Only solids whose box overlaps the swept block can contain a point
        // near the edit; a ray passes in and out of any other, adding zero.
        let shift = flat.normal * d;
        let pad = DVec3::splat(tol.dist * 1000.0);
        let (lo, hi) = (
            flat.lo.min(flat.lo + shift) - pad,
            flat.hi.max(flat.hi + shift) + pad,
        );
        let mut before = Vec::new();
        let mut solid_ids = BTreeSet::new();
        for shell in shells {
            let flats: Vec<Flat> = shell.iter().filter_map(|f| Flat::of(model, *f)).collect();
            let (slo, shi) = flats
                .iter()
                .fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(l, h), f| {
                    (l.min(f.lo), h.max(f.hi))
                });
            if shi.cmplt(lo).any() || hi.cmplt(slo).any() {
                continue;
            }
            solid_ids.extend(shell.iter().copied());
            before.extend(flats);
        }
        Some(Swept {
            before,
            solid_ids,
            all_ids: model.faces().map(|(id, _)| id).collect(),
            face: flat,
            d,
            tol,
        })
    }

    /// Whether `x` is inside the result. None if the ray test was unsure.
    pub fn inside(&self, x: DVec3) -> Option<bool> {
        let was = winding(&self.before, x, &self.tol)? >= 1;
        let swept = self.in_block(x)?;
        Some(if self.d > 0.0 {
            was || swept
        } else {
            was && !swept
        })
    }

    /// The box around the block the face sweeps through.
    fn bounds(&self) -> (DVec3, DVec3) {
        let shift = self.face.normal * self.d;
        (
            self.face.lo.min(self.face.lo + shift),
            self.face.hi.max(self.face.hi + shift),
        )
    }

    /// Whether `x` is in the swept block; None when it is on its boundary.
    fn in_block(&self, x: DVec3) -> Option<bool> {
        let f = &self.face;
        let t = self.tol.dist;
        let h = (x - f.origin).dot(f.normal);
        let (lo, hi) = if self.d > 0.0 {
            (0.0, self.d)
        } else {
            (self.d, 0.0)
        };
        if h < lo - t || h > hi + t {
            return Some(false);
        }
        let q = DVec2::new((x - f.origin).dot(f.u), (x - f.origin).dot(f.v));
        if near_outline(f, q, t) {
            return None;
        }
        let over = point_in_polygon(q, &f.outer) == Inside::Inside
            && f.holes
                .iter()
                .all(|l| point_in_polygon(q, l) == Inside::Outside);
        if !over {
            return Some(false);
        }
        if (h - lo).abs() <= t || (h - hi).abs() <= t {
            return None;
        }
        Some(true)
    }
}

/// Apply the rule around the vertices an edit touched. With `swept` (a
/// Push/Pull on a solid) the inside is known exactly; without, it is
/// counted from the closed shells there are now.
pub fn keep_boundary(model: &mut Model, touched: &BTreeSet<VertexId>, swept: Option<&Swept>) {
    if touched.is_empty() && swept.is_none() {
        return;
    }
    let tol = model.tol();
    let near: BTreeSet<FaceId> = model
        .faces()
        .filter(|(_, f)| f.loops().flatten().any(|v| touched.contains(v)))
        .map(|(id, _)| id)
        .collect();
    if near.is_empty() && swept.is_none() {
        return;
    }

    // 1. Split where the touched faces cross others.
    let flats: Vec<(FaceId, Flat)> = model
        .faces()
        .filter_map(|(id, _)| Flat::of(model, id).map(|f| (id, f)))
        .collect();
    let mut segs = Vec::new();
    for (i, (a, fa)) in flats.iter().enumerate() {
        if !near.contains(a) {
            continue;
        }
        for (j, (b, fb)) in flats.iter().enumerate() {
            if i == j || (near.contains(b) && j < i) {
                continue;
            }
            segs.extend(crossings(fa, fb, &tol));
        }
    }
    let (mut lo, mut hi) = (DVec3::INFINITY, DVec3::NEG_INFINITY);
    for (id, f) in &flats {
        if near.contains(id) {
            lo = lo.min(f.lo);
            hi = hi.max(f.hi);
        }
    }
    if let Some(sw) = swept {
        let (a, b) = sw.bounds();
        lo = lo.min(a);
        hi = hi.max(b);
    }
    for (a, b) in &segs {
        crate::add_segment(model, *a, *b);
    }
    // Rebuild the planes around the edit, so every face is cut along every
    // edge in its plane (a crossing can run along an edge that was there
    // already, which adds nothing).
    if let Some(sw) = swept
        && !segs.is_empty()
    {
        // Every face that reaches into the swept block (where the solid
        // changed), cut along every edge in its plane.
        let (blo, bhi) = sw.bounds();
        let pad = DVec3::splat(tol.dist * 10.0);
        let mut points: BTreeSet<VertexId> = BTreeSet::new();
        let mut planes = Vec::new();
        for (_, f) in model.faces() {
            let pts = model.points(&f.outer);
            let (flo, fhi) = pts
                .iter()
                .fold((pts[0], pts[0]), |(l, h), p| (l.min(*p), h.max(*p)));
            if (fhi + pad).cmplt(blo).any() || (bhi + pad).cmplt(flo).any() {
                continue;
            }
            points.extend(f.loops().flatten().copied());
            planes.extend(f.plane().copied());
        }
        let test = |x: DVec3| sw.inside(x);
        crate::settle::settle_with(model, &points, &planes, Some(&test));
    }

    // 2. Which faces to test, and how to tell inside from outside. With a
    //    sweep: faces of the solids before, and faces the edit made (a lone
    //    rectangle nearby is never touched). Without: closed shells now.
    let candidates: Vec<FaceId>;
    let counted: Vec<Flat>;
    match swept {
        Some(sw) => {
            candidates = model
                .faces()
                .map(|(id, _)| id)
                .filter(|id| sw.solid_ids.contains(id) || !sw.all_ids.contains(id))
                .collect();
            counted = Vec::new();
        }
        None => {
            candidates = closed_faces(model);
            counted = candidates
                .iter()
                .filter_map(|id| Flat::of(model, *id))
                .collect();
        }
    }
    let inside = |x: DVec3| -> Option<bool> {
        match swept {
            Some(sw) => sw.inside(x),
            None => winding(&counted, x, &tol).map(|w| w >= 1),
        }
    };

    // 3. Test every candidate around the edit: a boundary has the solid on
    //    exactly one side. Solid on both sides is inside (a wall left in a
    //    box); on neither, it bounds nothing (a tube pushed out the back).
    let pad = DVec3::splat(tol.dist * 10.0);
    let step = step(&tol);
    let mut inside_faces = Vec::new();
    let mut backwards = Vec::new();
    for id in &candidates {
        let Some(f) = Flat::of(model, *id) else {
            continue;
        };
        if (f.hi + pad).cmplt(lo).any() || (hi + pad).cmplt(f.lo).any() {
            continue;
        }
        let loops: Vec<Vec<DVec2>> = std::iter::once(f.outer.clone())
            .chain(f.holes.iter().cloned())
            .collect();
        // The first point on the face where both sides give a clear answer.
        let sides = interior_points(&loops).into_iter().find_map(|q| {
            let c = f.origin + f.u * q.x + f.v * q.y;
            Some((inside(c + f.normal * step)?, inside(c - f.normal * step)?))
        });
        let Some((front, back)) = sides else {
            continue;
        };
        if front == back {
            inside_faces.push(*id);
        } else if front {
            // The solid is in front: the face looks into it. Turn it round.
            backwards.push(*id);
        }
    }
    for id in backwards {
        model.edit_face(id, |f| f.reverse());
    }

    // 4. Remove them, and the edges and points only they used.
    let mut maybe_loose: BTreeSet<EdgeId> = BTreeSet::new();
    for id in &inside_faces {
        if let Some(f) = model.remove_face(*id) {
            maybe_loose.extend(f.edge_ids());
        }
    }
    let used: BTreeSet<EdgeId> = model.faces().flat_map(|(_, f)| f.edge_ids()).collect();
    let mut points: BTreeSet<VertexId> = BTreeSet::new();
    for e in maybe_loose.difference(&used) {
        if let Some(ed) = model.edge(*e).copied() {
            model.remove_edge(*e);
            points.insert(ed.a);
            points.insert(ed.b);
        }
    }
    for v in points {
        if !model.edges().any(|(_, e)| e.has(v)) {
            model.remove_vertex(v);
        }
    }
}

/// Closed shells: faces joined by edges, with no open edge.
pub(crate) fn closed_shells(model: &Model) -> Vec<Vec<FaceId>> {
    let edge_faces: BTreeMap<EdgeId, Vec<FaceId>> = model.edge_faces();
    // Join faces that share an edge (union-find over face ids).
    let ids: Vec<FaceId> = model.faces().map(|(id, _)| id).collect();
    let index: BTreeMap<FaceId, usize> = ids.iter().enumerate().map(|(i, f)| (*f, i)).collect();
    let mut parent: Vec<usize> = (0..ids.len()).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut open: BTreeSet<usize> = BTreeSet::new();
    for faces in edge_faces.values() {
        let first = index[&faces[0]];
        for f in &faces[1..] {
            let (a, b) = (find(&mut parent, first), find(&mut parent, index[f]));
            parent[a] = b;
        }
        if faces.len() < 2 {
            open.insert(first);
        }
    }
    let open_roots: BTreeSet<usize> = open.iter().map(|i| find(&mut parent, *i)).collect();
    let mut shells: BTreeMap<usize, Vec<FaceId>> = BTreeMap::new();
    for (i, f) in ids.iter().enumerate() {
        let root = find(&mut parent, i);
        if !open_roots.contains(&root) {
            shells.entry(root).or_default().push(*f);
        }
    }
    shells.into_values().collect()
}

/// The faces of closed shells, all together.
pub(crate) fn closed_faces(model: &Model) -> Vec<FaceId> {
    closed_shells(model).into_iter().flatten().collect()
}

/// How many closed shells `p` is inside: count the faces a ray from `p`
/// crosses, +1 leaving through a face and -1 entering. None if every ray
/// grazes an edge or runs along a face.
pub(crate) fn winding(faces: &[Flat], p: DVec3, tol: &Tol) -> Option<i32> {
    'ray: for d in RAYS {
        let d = d.normalize();
        let mut w = 0;
        for f in faces {
            if !ray_box(p, d, f.lo, f.hi, tol.dist) {
                continue;
            }
            let den = f.normal.dot(d);
            let t = (f.origin - p).dot(f.normal);
            if den.abs() < 1e-9 {
                if t.abs() <= tol.dist {
                    continue 'ray;
                }
                continue;
            }
            let hit = p + d * (t / den);
            if t.abs() <= tol.dist {
                // The point is on this face's plane: on the face itself, no
                // ray can tell which side it is on.
                let q = DVec2::new((hit - f.origin).dot(f.u), (hit - f.origin).dot(f.v));
                if near_outline(f, q, tol.dist)
                    || point_in_polygon(q, &f.outer) != Inside::Outside
                        && f.holes
                            .iter()
                            .all(|h| point_in_polygon(q, h) != Inside::Inside)
                {
                    return None;
                }
                continue;
            }
            let t = t / den;
            if t <= 0.0 {
                continue;
            }
            let q = DVec2::new((hit - f.origin).dot(f.u), (hit - f.origin).dot(f.v));
            // Too close to an edge to count reliably: try another ray.
            if near_outline(f, q, tol.dist) {
                continue 'ray;
            }
            let mut inside = match point_in_polygon(q, &f.outer) {
                Inside::Inside => true,
                Inside::Outside => false,
                Inside::Boundary => continue 'ray,
            };
            for h in &f.holes {
                match point_in_polygon(q, h) {
                    Inside::Inside => inside = false,
                    Inside::Boundary => continue 'ray,
                    Inside::Outside => {}
                }
            }
            if inside {
                w += if den > 0.0 { 1 } else { -1 };
            }
        }
        return Some(w);
    }
    None
}

/// Whether the ray from `o` along `d` passes through the box (padded).
fn ray_box(o: DVec3, d: DVec3, lo: DVec3, hi: DVec3, pad: f64) -> bool {
    let (lo, hi) = (lo - DVec3::splat(pad), hi + DVec3::splat(pad));
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for k in 0..3 {
        if d[k].abs() < 1e-300 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return false;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// Whether `q` is within `d` of a face's outline or a hole's.
fn near_outline(f: &Flat, q: DVec2, d: f64) -> bool {
    std::iter::once(&f.outer).chain(f.holes.iter()).any(|l| {
        (0..l.len()).any(|i| {
            let (a, b) = (l[i], l[(i + 1) % l.len()]);
            let e = b - a;
            let t = ((q - a).dot(e) / e.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
            q.distance(a + e * t) < d
        })
    })
}
