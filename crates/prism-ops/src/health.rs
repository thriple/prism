//! Model Health (PLAN 1.10): what probably should not be in a model, and
//! fixes for the common problems. Pure functions over one model.

use std::collections::{BTreeMap, BTreeSet};

use prism_math::{DVec2, DVec3, Tol};
use prism_topo::{EdgeId, FaceId, Model, VertexId, loop_pairs};

use crate::intersect::{Flat, crossings};
use crate::regions::interior_points;
use crate::solid::{closed_faces, closed_shells, step, winding};

/// What one check found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub vertices: BTreeSet<VertexId>,
    pub edges: BTreeSet<EdgeId>,
    pub faces: BTreeSet<FaceId>,
}

impl Found {
    /// How many things were found.
    pub fn count(&self) -> usize {
        self.vertices.len() + self.edges.len() + self.faces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }
}

/// Every check's findings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Coincident points, edges with the same ends, faces with the same loops.
    pub duplicates: Found,
    /// Points closer than ten tolerances but not welded.
    pub near_misses: Found,
    /// Faces that disagree with their neighbours or face into a solid.
    pub reversed: Found,
    /// Edges with one face (holes in a shell).
    pub open_edges: Found,
    /// Edges with more than two faces.
    pub non_manifold: Found,
    /// Closed shells, and shells with holes.
    pub watertight: usize,
    pub leaky: usize,
    /// Faces of closed shells with the solid on both sides.
    pub interior: Found,
    /// Faces of almost no area or slivers, and edges shorter than the tolerance.
    pub degenerate: Found,
    /// Edges that belong to no face.
    pub stray: Found,
    /// Faces that pass through each other with no edge along the crossing.
    pub crossing: Found,
    /// Geometry far from the origin compared with its size.
    pub far_from_origin: bool,
    /// Pieces smaller than ten tolerances.
    pub tiny: Found,
}

/// Run every check.
pub fn check(model: &Model) -> Report {
    let tol = model.tol();
    let edge_faces = model.edge_faces();
    let mut r = Report {
        duplicates: duplicates(model, &tol),
        near_misses: near_misses(model, &tol),
        ..Report::default()
    };
    for (e, fs) in &edge_faces {
        match fs.len() {
            0 => {
                r.stray.edges.insert(*e);
            }
            1 => {
                r.open_edges.edges.insert(*e);
            }
            2 => {}
            _ => {
                r.non_manifold.edges.insert(*e);
            }
        }
    }
    // Edges no face map lists are stray too.
    for (id, _) in model.edges() {
        if !edge_faces.contains_key(&id) {
            r.stray.edges.insert(id);
        }
    }
    let (closed, open) = shells(model, &edge_faces);
    r.watertight = closed;
    r.leaky = open;
    r.reversed.faces = reversed(model);
    r.interior.faces = interior(model, &tol);
    r.degenerate = degenerate(model, &tol);
    r.crossing.faces = crossing_faces(model, &tol);
    if let Some((lo, hi)) = model.bbox() {
        let size = (hi - lo).length().max(tol.dist);
        r.far_from_origin = ((lo + hi) * 0.5).length() > 1e4 * size;
    }
    r.tiny = tiny(model, &tol);
    r
}

fn pair(a: VertexId, b: VertexId) -> (VertexId, VertexId) {
    (a.min(b), a.max(b))
}

/// Points closer than `d`, found by sweeping along x.
fn close_points(model: &Model, d: f64) -> Vec<(VertexId, VertexId, f64)> {
    let mut pts: Vec<(VertexId, DVec3)> = model.vertices().collect();
    pts.sort_by(|a, b| a.1.x.total_cmp(&b.1.x).then(a.0.cmp(&b.0)));
    let mut out = Vec::new();
    for i in 0..pts.len() {
        for j in i + 1..pts.len() {
            if pts[j].1.x - pts[i].1.x >= d {
                break;
            }
            let dist = pts[i].1.distance(pts[j].1);
            if dist < d {
                out.push((pts[i].0, pts[j].0, dist));
            }
        }
    }
    out
}

fn duplicates(model: &Model, tol: &Tol) -> Found {
    let mut f = Found::default();
    for (a, b, _) in close_points(model, tol.dist) {
        f.vertices.insert(a);
        f.vertices.insert(b);
    }
    let mut ends: BTreeMap<(VertexId, VertexId), EdgeId> = BTreeMap::new();
    for (id, e) in model.edges() {
        if let Some(first) = ends.insert(pair(e.a, e.b), id) {
            f.edges.insert(first);
            f.edges.insert(id);
        }
    }
    let mut loops: BTreeMap<Vec<VertexId>, FaceId> = BTreeMap::new();
    for (id, face) in model.faces() {
        let mut key = face.outer.clone();
        key.sort();
        if let Some(first) = loops.insert(key, id) {
            f.faces.insert(first);
            f.faces.insert(id);
        }
    }
    f
}

fn near_misses(model: &Model, tol: &Tol) -> Found {
    let mut f = Found::default();
    for (a, b, d) in close_points(model, tol.dist * 10.0) {
        if d >= tol.dist {
            f.vertices.insert(a);
            f.vertices.insert(b);
        }
    }
    f
}

/// Connected pieces of faces (joined by edges), and which have no open edge.
fn shells(model: &Model, edge_faces: &BTreeMap<EdgeId, Vec<FaceId>>) -> (usize, usize) {
    let closed = closed_shells(model).len();
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
    for fs in edge_faces.values() {
        if let Some(first) = fs.first() {
            for f in &fs[1..] {
                let (a, b) = (find(&mut parent, index[first]), find(&mut parent, index[f]));
                parent[a] = b;
            }
        }
    }
    let all: BTreeSet<usize> = (0..ids.len()).map(|i| find(&mut parent, i)).collect();
    (closed, all.len().saturating_sub(closed))
}

/// Which way each face runs along each of its edges.
fn runs(model: &Model) -> BTreeMap<(VertexId, VertexId), Vec<(FaceId, bool)>> {
    let mut out: BTreeMap<(VertexId, VertexId), Vec<(FaceId, bool)>> = BTreeMap::new();
    for (id, f) in model.faces() {
        for (a, b) in f.loops().flat_map(|l| loop_pairs(l)) {
            out.entry(pair(a, b)).or_default().push((id, a < b));
        }
    }
    out
}

/// Faces that should be turned round: in each piece, the faces that
/// disagree with the most of their neighbours; then, for closed shells, all
/// of them if the piece faces into its solid.
fn reversed(model: &Model) -> BTreeSet<FaceId> {
    let runs = runs(model);
    // Neighbours across edges with exactly two faces: same direction means
    // they disagree.
    let mut next: BTreeMap<FaceId, Vec<(FaceId, bool)>> = BTreeMap::new();
    for fs in runs.values() {
        if let [(a, da), (b, db)] = fs[..] {
            next.entry(a).or_default().push((b, da == db));
            next.entry(b).or_default().push((a, da == db));
        }
    }
    let mut flip: BTreeMap<FaceId, bool> = BTreeMap::new();
    let mut out = BTreeSet::new();
    let tol = model.tol();
    let solid_faces: BTreeSet<FaceId> = closed_faces(model).into_iter().collect();
    for (start, _) in model.faces() {
        if flip.contains_key(&start) {
            continue;
        }
        // Spread one orientation over the piece.
        let mut piece = vec![start];
        flip.insert(start, false);
        let mut stack = vec![start];
        while let Some(f) = stack.pop() {
            let mine = flip[&f];
            for (g, disagree) in next.get(&f).into_iter().flatten() {
                if !flip.contains_key(g) {
                    flip.insert(*g, mine ^ disagree);
                    piece.push(*g);
                    stack.push(*g);
                }
            }
        }
        let flipped = piece.iter().filter(|f| flip[*f]).count();
        // The smaller side disagrees with the rest.
        let minority_flipped = flipped * 2 <= piece.len();
        let mut wrong: BTreeSet<FaceId> = piece
            .iter()
            .copied()
            .filter(|f| flip[f] == minority_flipped)
            .collect();
        // A closed shell whose right side faces into its own solid: the
        // rest are wrong instead.
        if solid_faces.contains(&start) {
            let flats: Vec<Flat> = piece.iter().filter_map(|f| Flat::of(model, *f)).collect();
            let right = piece.iter().find(|f| !wrong.contains(*f)).copied();
            if let Some(right) = right
                && faces_inward(model, right, &flats, &tol) == Some(true)
            {
                wrong = piece
                    .iter()
                    .copied()
                    .filter(|f| !wrong.contains(f))
                    .collect();
            }
        }
        out.extend(wrong);
    }
    out
}

/// Whether a face points into the solid its shell encloses.
fn faces_inward(model: &Model, face: FaceId, shell: &[Flat], tol: &Tol) -> Option<bool> {
    let f = Flat::of(model, face)?;
    let s = step(tol);
    interior_points(&loops(&f)).into_iter().find_map(|q| {
        let c = f.origin + f.u * q.x + f.v * q.y;
        let front = winding(shell, c + f.normal * s, tol)?;
        let back = winding(shell, c - f.normal * s, tol)?;
        // Outward faces have the solid behind. Inward faces count the solid
        // the other way round, as -1 in front.
        Some(front != 0 && back == 0)
    })
}

fn loops(f: &Flat) -> Vec<Vec<DVec2>> {
    std::iter::once(f.outer.clone())
        .chain(f.holes.iter().cloned())
        .collect()
}

/// Faces inside a solid: a point on the face is inside the closed shells
/// around it, with the face itself left out, along every ray. A face of
/// the outside has rays that get away (or count it out), so it is never
/// taken. Only shells whose box holds the point are counted: a ray passes
/// in and out of any other.
fn interior(model: &Model, tol: &Tol) -> BTreeSet<FaceId> {
    let shells: Vec<Shell> = closed_shells(model)
        .into_iter()
        .map(|faces| {
            let flats: Vec<(FaceId, Flat)> = faces
                .into_iter()
                .filter_map(|f| Flat::of(model, f).map(|x| (f, x)))
                .collect();
            let lo = flats.iter().fold(DVec3::INFINITY, |a, (_, f)| a.min(f.lo));
            let hi = flats
                .iter()
                .fold(DVec3::NEG_INFINITY, |a, (_, f)| a.max(f.hi));
            (flats, lo, hi)
        })
        .collect();
    let pad = DVec3::splat(tol.dist * 10.0);
    let mut out = BTreeSet::new();
    for (flats, _, _) in &shells {
        for (id, f) in flats {
            let Some(q) = interior_points(&loops(f)).into_iter().next() else {
                continue;
            };
            let c = f.origin + f.u * q.x + f.v * q.y;
            let around: Vec<&Flat> = shells
                .iter()
                .filter(|(_, lo, hi)| c.cmpge(*lo - pad).all() && c.cmple(*hi + pad).all())
                .flat_map(|(fl, _, _)| fl.iter().filter(|(g, _)| g != id).map(|(_, g)| g))
                .collect();
            if inside_every_way(&around, c, f.normal, tol) {
                out.insert(*id);
            }
        }
    }
    out
}

/// A closed shell's faces and its box.
type Shell = (Vec<(FaceId, Flat)>, DVec3, DVec3);

/// Whether `p` is inside the faces along every ray that passes cleanly
/// (at least three must): a signed count of crossings, nonzero.
fn inside_every_way(faces: &[&Flat], p: DVec3, normal: DVec3, tol: &Tol) -> bool {
    const DIRS: [DVec3; 6] = [
        DVec3::new(0.284_7, 0.539_1, 0.792_6),
        DVec3::new(-0.613_2, 0.217_9, 0.759_3),
        DVec3::new(0.471_5, -0.802_3, 0.366_1),
        DVec3::new(-0.152_8, -0.433_6, -0.888_1),
        DVec3::new(0.811_2, 0.322_5, -0.487_3),
        DVec3::new(-0.402_9, -0.716_4, 0.569_8),
    ];
    let mut clean = 0;
    'ray: for d in DIRS {
        let d = d.normalize();
        // A ray along the face's own plane says nothing.
        if d.dot(normal).abs() < 0.05 {
            continue;
        }
        let mut w = 0i32;
        for f in faces {
            let den = f.normal.dot(d);
            if den.abs() < 1e-9 {
                continue;
            }
            let t = (f.origin - p).dot(f.normal) / den;
            if t <= tol.dist {
                continue;
            }
            let hit = p + d * t;
            let q = DVec2::new((hit - f.origin).dot(f.u), (hit - f.origin).dot(f.v));
            use prism_math::predicates::{Inside, point_in_polygon};
            let outer = point_in_polygon(q, &f.outer);
            if outer == Inside::Boundary
                || f.holes
                    .iter()
                    .any(|h| point_in_polygon(q, h) == Inside::Boundary)
            {
                continue 'ray;
            }
            if outer == Inside::Inside
                && f.holes
                    .iter()
                    .all(|h| point_in_polygon(q, h) == Inside::Outside)
            {
                w += if den > 0.0 { 1 } else { -1 };
            }
        }
        if w == 0 {
            return false;
        }
        clean += 1;
    }
    clean >= 3
}

fn degenerate(model: &Model, tol: &Tol) -> Found {
    let mut f = Found::default();
    for (id, face) in model.faces() {
        let area = model.face_area(id);
        let pts = model.points(&face.outer);
        let perimeter: f64 = (0..pts.len())
            .map(|i| pts[i].distance(pts[(i + 1) % pts.len()]))
            .sum();
        // Almost no area, or a sliver: thinner than the tolerance on average.
        if area <= tol.area() * 10.0 || area <= perimeter * tol.dist * 0.5 {
            f.faces.insert(id);
        }
    }
    for (id, e) in model.edges() {
        if model.vertex(e.a).distance(model.vertex(e.b)) < tol.dist {
            f.edges.insert(id);
        }
    }
    f
}

/// Faces that cross another with no edge along the crossing. Faces are
/// swept along x, so only those whose boxes overlap are compared.
fn crossing_faces(model: &Model, tol: &Tol) -> BTreeSet<FaceId> {
    let mut flats: Vec<(FaceId, Flat)> = model
        .faces()
        .filter_map(|(id, _)| Flat::of(model, id).map(|f| (id, f)))
        .collect();
    flats.sort_by(|a, b| a.1.lo.x.total_cmp(&b.1.lo.x).then(a.0.cmp(&b.0)));
    let mut out = BTreeSet::new();
    for i in 0..flats.len() {
        for j in i + 1..flats.len() {
            if flats[j].1.lo.x > flats[i].1.hi.x + tol.dist {
                break;
            }
            if !crossings(&flats[i].1, &flats[j].1, tol).is_empty() {
                out.insert(flats[i].0);
                out.insert(flats[j].0);
            }
        }
    }
    out
}

/// Connected pieces (points joined by edges) smaller than ten
/// tolerances, found in one pass over the edges.
fn tiny(model: &Model, tol: &Tol) -> Found {
    let ids: Vec<VertexId> = model.vertices().map(|(v, _)| v).collect();
    let index: BTreeMap<VertexId, usize> = ids.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let mut parent: Vec<usize> = (0..ids.len()).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for (_, e) in model.edges() {
        let (a, b) = (
            find(&mut parent, index[&e.a]),
            find(&mut parent, index[&e.b]),
        );
        parent[a] = b;
    }
    let mut boxes: BTreeMap<usize, (DVec3, DVec3)> = BTreeMap::new();
    for (i, v) in ids.iter().enumerate() {
        let p = model.vertex(*v);
        let r = find(&mut parent, i);
        let b = boxes.entry(r).or_insert((p, p));
        *b = (b.0.min(p), b.1.max(p));
    }
    let small: BTreeSet<usize> = boxes
        .into_iter()
        .filter(|(_, (lo, hi))| (*hi - *lo).length() < tol.dist * 10.0)
        .map(|(r, _)| r)
        .collect();
    let mut f = Found::default();
    if small.is_empty() {
        return f;
    }
    for (i, v) in ids.iter().enumerate() {
        if small.contains(&find(&mut parent, i)) {
            f.vertices.insert(*v);
        }
    }
    for (id, e) in model.edges() {
        if f.vertices.contains(&e.a) {
            f.edges.insert(id);
        }
    }
    for (id, face) in model.faces() {
        if face.outer.iter().all(|v| f.vertices.contains(v)) {
            f.faces.insert(id);
        }
    }
    f
}

// ---------- fixes ----------

/// Turn a face round.
fn flip(model: &mut Model, id: FaceId) {
    model.edit_face(id, |f| f.reverse());
}

/// Orient Faces: turn round every face the reversed check finds, or only
/// those in `only`. Returns how many turned.
pub fn orient(model: &mut Model, only: Option<&BTreeSet<FaceId>>) -> usize {
    let wrong: Vec<FaceId> = reversed(model)
        .into_iter()
        .filter(|f| only.is_none_or(|s| s.contains(f)))
        .collect();
    for f in &wrong {
        flip(model, *f);
    }
    wrong.len()
}

/// Reverse Faces: turn the given faces round.
pub fn reverse(model: &mut Model, faces: &BTreeSet<FaceId>) {
    for f in faces {
        flip(model, *f);
    }
}

/// Clean Envelope: remove interior faces (in `only`, or everywhere), and
/// the edges and points only they used. Returns how many faces went.
pub fn clean_envelope(model: &mut Model, only: Option<&BTreeSet<FaceId>>) -> usize {
    let tol = model.tol();
    let inside: Vec<FaceId> = interior(model, &tol)
        .into_iter()
        .filter(|f| only.is_none_or(|s| s.contains(f)))
        .collect();
    let mut sel = crate::Selection::default();
    sel.faces.extend(inside.iter().copied());
    if inside.is_empty() {
        return 0;
    }
    // Remove the faces, then edges no face uses any more.
    let mut edges_of: BTreeSet<EdgeId> = BTreeSet::new();
    for f in &inside {
        if let Some(face) = model.remove_face(*f) {
            edges_of.extend(face.edge_ids());
        }
    }
    let used: BTreeSet<EdgeId> = model.faces().flat_map(|(_, f)| f.edge_ids()).collect();
    for e in edges_of.difference(&used) {
        model.remove_edge(*e);
    }
    remove_lonely_points(model);
    inside.len()
}

/// Remove Stray Edges: edges in no face, and points left on nothing.
pub fn remove_stray(model: &mut Model) -> usize {
    let r = check(model);
    for e in &r.stray.edges {
        model.remove_edge(*e);
    }
    remove_lonely_points(model);
    r.stray.edges.len()
}

fn remove_lonely_points(model: &mut Model) {
    let on: BTreeSet<VertexId> = model
        .edges()
        .flat_map(|(_, e)| [e.a, e.b])
        .chain(
            model
                .faces()
                .flat_map(|(_, f)| f.loops().flatten().copied().collect::<Vec<_>>()),
        )
        .collect();
    let lonely: Vec<VertexId> = model
        .vertices()
        .map(|(v, _)| v)
        .filter(|v| !on.contains(v))
        .collect();
    for v in lonely {
        model.remove_vertex(v);
    }
}

/// What an export should say about the model it wrote, if anything: open
/// edges, reversed faces and crossing faces. `printing` (STL, 3MF) says
/// when the model is not watertight.
pub fn export_note(model: &Model, printing: bool) -> Option<String> {
    let r = check(model);
    let mut parts = Vec::new();
    let n = |k: usize, one: &str, many: &str| {
        if k == 1 {
            format!("1 {one}")
        } else {
            format!("{k} {many}")
        }
    };
    if !r.open_edges.is_empty() {
        let mut s = n(r.open_edges.count(), "open edge", "open edges");
        if printing {
            s.push_str(": the model is not watertight");
        }
        parts.push(s);
    }
    if !r.reversed.is_empty() {
        parts.push(n(r.reversed.count(), "reversed face", "reversed faces"));
    }
    if !r.crossing.is_empty() {
        parts.push(n(r.crossing.count(), "crossing face", "crossing faces"));
    }
    (!parts.is_empty()).then(|| format!("{}; see Model Health", parts.join(", ")))
}
