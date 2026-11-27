//! Round many edges of a solid at once through the kernel (Fillet Joins
//! step B, brief 32): fillets or chamfers of one size, with clean corners
//! where they meet (mitres, ball corners). The solid goes to the kernel by
//! [`crate::bridge`] and comes back with its untouched faces, edges and
//! corners keeping their ids and paint; new faces are Default. The lines
//! where a round meets a face it is tangent to are soft and smooth.

use std::collections::{BTreeMap, BTreeSet};

use prism_kernel::solid::Solid;
use prism_math::DVec3;
use prism_topo::{EdgeId, FaceId, Model, VertexId};

use crate::bridge::{put_back, region, solid_of};

pub use prism_kernel::solid::{NOT_ROUNDED, TOO_LARGE};

/// Said when there is nothing to round.
pub const NOTHING: &str = "Refused: pick the edges of a solid to round.";
/// Said for a size that is not more than zero.
pub const NO_SIZE: &str = "Refused: the size must be more than zero.";
/// Said when the edges are not all on one closed solid.
pub const TWO_SOLIDS: &str = "Refused: the edges are not all on one closed solid.";
/// Said for an edge without a face on each side.
pub const ONE_FACE: &str = "Refused: an edge needs one face on each side to round.";
/// Said when edges that are not on a closed solid cannot be rounded by
/// Prism's own round either.
pub const OPEN: &str = "Refused: off a closed solid, only straight edges between flat faces round so far, and not two that meet.";
/// Said for a line between two faces of one plane: drawn on a flat face,
/// with no corner to round.
pub const FLAT_LINE: &str =
    "Refused: this line is drawn on a flat face, so there is no corner to round.";

/// Round `edges` (all of one closed solid) by `size`: fillets, or chamfers
/// when `fillet` is false. Returns the solid's faces afterwards. A refusal
/// leaves the model as it was.
pub fn round_edges(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Result<Vec<FaceId>, &'static str> {
    let edge_faces = model.edge_faces();
    if edges
        .iter()
        .any(|e| edge_faces.get(e).is_none_or(|f| f.len() != 2))
    {
        return Err(ONE_FACE);
    }
    if edges.iter().any(|e| drawn_on_flat(model, *e)) {
        return Err(FLAT_LINE);
    }
    // On a copy the kernel can take, first; if that cannot be rounded, as
    // the solid is, and the copy's reason if the solid's is only that the
    // kernel could not take it.
    let prepared = match round_past_slits(model, edges, size, fillet) {
        Some(Ok(done)) => return Ok(done),
        Some(Err(why)) => Some(why),
        None => None,
    };
    let as_is = round_as_is(model, edges, size, fillet);
    match (as_is, prepared) {
        (Err(why), Some(better)) if why == crate::bridge::APPROXIMATE || why == OPEN => Err(better),
        (r, _) => r,
    }
}

/// [`round_edges`] on the solid as it is.
fn round_as_is(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Result<Vec<FaceId>, &'static str> {
    match round_once(model, edges, size, fillet) {
        Err(why) if why == prism_kernel::solid::NOT_ROUNDED || why == OPEN => {
            round_in_turn(model, edges, size, fillet)
                .or_else(|| round_with_caps(model, edges, size, fillet))
                .or_else(|| round_split_ends(model, edges, size, fillet))
                .or_else(|| round_swept(model, edges, size, fillet))
                .or_else(|| round_local(model, edges, size, fillet))
                .ok_or(why)
        }
        // Too large for the kernel's round: a swept one may still fit.
        Err(why) if why == TOO_LARGE => round_swept(model, edges, size, fillet).ok_or(why),
        // An approximate edge somewhere: round where the edges are.
        Err(why) if why == crate::bridge::APPROXIMATE => {
            round_local(model, edges, size, fillet).ok_or(why)
        }
        done => done,
    }
}

/// Brief 56 step 2: a straight edge whose end is a busier corner than
/// three edges (owner's session 29: a top edge running into a wall that
/// sits on the block and overhangs its side), which the kernel will not
/// round. The solid is cut by a plane square to the edge at each such end;
/// the piece with the edge then ends in a plain corner, is rounded, and
/// the pieces are joined again. Where the round took material away, the
/// other piece's cut face is left: a flat end cap square to the edge.
fn round_split_ends(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<Vec<FaceId>> {
    let tol = model.tol();
    // One straight edge (or a straight run of pieces): its two ends.
    let run: Vec<&prism_topo::Edge> = edges.iter().filter_map(|e| model.edge(*e)).collect();
    if run.len() != edges.len() || run.iter().any(|e| e.curve.is_curved()) {
        return None;
    }
    let mut count: BTreeMap<VertexId, usize> = BTreeMap::new();
    for e in &run {
        *count.entry(e.a).or_default() += 1;
        *count.entry(e.b).or_default() += 1;
    }
    let ends: Vec<VertexId> = count
        .iter()
        .filter(|(_, n)| **n == 1)
        .map(|(v, _)| *v)
        .collect();
    let [a, b] = ends[..] else { return None };
    let (pa, pb) = (model.vertex(a), model.vertex(b));
    let dir = (pb - pa).normalize_or_zero();
    if dir == DVec3::ZERO {
        return None;
    }
    let degree = |v: VertexId| model.edges().filter(|(_, x)| x.has(v)).count();
    // Each busy end, with the way out of the edge there.
    let busy: Vec<(DVec3, DVec3)> = [(a, pa, -dir), (b, pb, dir)]
        .into_iter()
        .filter(|(v, ..)| degree(*v) > 3)
        .map(|(_, p, out)| (p, out))
        .collect();
    if busy.is_empty() {
        return None;
    }
    // The solid on its own.
    let faces = solid_faces(model, edges).ok()?;
    let solid = alone(model, &faces);
    let (lo, hi) = solid.bbox()?;
    let big = (hi - lo).length() * 2.0 + 1.0;
    // A block beyond each busy end: the half of space past the plane
    // square to the edge there, as far as the solid goes.
    let block = |p: DVec3, out: DVec3| -> Option<Model> {
        let plane = prism_math::Plane::new(p, out);
        let fr = plane.frame();
        let mut m = Model::new();
        m.set_tol(tol);
        let c = |x: f64, y: f64| p + fr.u * x + fr.v * y;
        crate::add_polygon(
            &mut m,
            &[c(-big, -big), c(big, -big), c(big, big), c(-big, big)],
        );
        let f = m.faces().next()?.0;
        let n = m.face(f)?.plane()?.normal;
        crate::push_pull(&mut m, f, if n.dot(out) > 0.0 { big } else { -big }, false);
        crate::boolean::is_solid(&m).then_some(m)
    };
    let mut inner = solid.clone();
    let mut outer: Vec<Model> = Vec::new();
    for (p, out) in &busy {
        let cut = block(*p, *out)?;
        let past = crate::boolean::boolean(&solid, &cut, crate::boolean::Op::Intersect).ok()?;
        inner = crate::boolean::boolean(&inner, &cut, crate::boolean::Op::Subtract).ok()?;
        if past.faces().next().is_some() {
            outer.push(past);
        }
    }
    // The edge in the inner piece: what lies along the original line.
    let on_line = |q: DVec3| {
        let t = (q - pa).dot(dir);
        (pa + dir * t).distance(q) <= tol.dist
            && t >= -tol.dist
            && t <= (pb - pa).length() + tol.dist
    };
    let mine: BTreeSet<EdgeId> = inner
        .edges()
        .filter(|(_, e)| {
            !e.curve.is_curved() && on_line(inner.vertex(e.a)) && on_line(inner.vertex(e.b))
        })
        .map(|(id, _)| id)
        .collect();
    if mine.is_empty() {
        return None;
    }
    round_once(&mut inner, &mine, size, fillet).ok()?;
    let mut joined = inner;
    for piece in &outer {
        joined = crate::boolean::boolean(&joined, piece, crate::boolean::Op::Union).ok()?;
    }
    put_in(model, &faces, &joined)
}

/// Brief 56 step 4: a chain of lines and arcs along the edge of one flat
/// face (the floor), each with a wall standing square to it on the other
/// side (a flat or curved face, as Push/Pull makes), which the kernel will
/// not round (a round along the inside of a curved wall). The round's
/// cross-section (the corner less a quarter circle, or a triangle for a
/// chamfer) is swept along the chain with Follow Me, and added to the
/// solid at an inside corner or taken away at an outside one. The
/// cross-section reaches a little past the corner, into the solid or out
/// of it, so the booleans never meet faces lying on each other. The chain
/// must run smoothly (its pieces tangent where they meet); corners and
/// slight creases are left for the corner step.
fn round_swept(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<Vec<FaceId>> {
    let tol = model.tol();
    let (points, pieces, closed) = crate::follow::chain(model, edges)?;
    if closed {
        return None;
    }
    let ef = model.edge_faces();
    // The floor: the flat face on every edge of the chain.
    let floor = {
        let first = ef.get(pieces.first()?)?;
        *first.iter().find(|f| {
            model.face(**f).is_some_and(|x| x.plane().is_some())
                && pieces
                    .iter()
                    .all(|e| ef.get(e).is_some_and(|fs| fs.contains(f)))
        })?
    };
    let plane = *model.face(floor)?.plane()?;
    let up = plane.normal;
    // Each piece in chain order: its ends and its direction of travel at
    // each end.
    let mut runs_of: Vec<Piece> = Vec::new();
    for (k, e) in pieces.iter().enumerate() {
        let x = model.edge(*e)?;
        if x.curve.is_spline() {
            return None;
        }
        let (from, to) = (points[k], points[k + 1]);
        let forward = x.a == from;
        let (pa, pb) = (model.vertex(x.a), model.vertex(x.b));
        let dir = |p: DVec3| {
            let t = x.curve.tangent_at(pa, pb, p).normalize_or_zero();
            if forward { t } else { -t }
        };
        let (p, q, m) = (model.vertex(from), model.vertex(to), middle(model, x));
        // The chain lies in the floor's plane, and the wall stands square
        // to it.
        if [p, q, m]
            .iter()
            .any(|r| plane.distance(*r).abs() > tol.dist)
        {
            return None;
        }
        let wall = *ef.get(e)?.iter().find(|f| **f != floor)?;
        let n = model.face(wall)?.surface.normal_at(m).normalize_or_zero();
        if !tol.perpendicular_over(n, up, size.max(tol.dist)) {
            return None;
        }
        runs_of.push(Piece {
            edge: *e,
            from,
            to,
            start: dir(p),
            end: dir(q),
            middle: m,
            across: up.cross(dir(m)).normalize_or_zero(),
        });
    }
    if runs_of
        .iter()
        .any(|p| p.start == DVec3::ZERO || p.end == DVec3::ZERO)
    {
        return None;
    }
    let faces = solid_faces(model, edges).ok()?;
    let solid = alone(model, &faces);
    let shell: Vec<crate::shell::ShellFace> = solid
        .faces()
        .filter_map(|(id, _)| crate::shell::ShellFace::of(&solid, id))
        .collect();
    // Which side of the chain the floor is on, and whether each corner is
    // an inside one (the solid behind both faces): the same all along.
    let probe = (size * 0.5).max(tol.dist * 10.0);
    let on_floor = |q: DVec3| crate::face_at(model, q) == Some(floor);
    let first = &runs_of[0];
    let sign = if on_floor(first.middle + first.across * probe) {
        1.0
    } else if on_floor(first.middle - first.across * probe) {
        -1.0
    } else {
        return None;
    };
    let mut inside = None;
    for p in &runs_of {
        let behind = p.middle - p.across * sign * probe - up * probe;
        let here = crate::shell::winding(&shell, behind, &tol)? >= 1;
        if inside.is_some_and(|i| i != here) {
            return None;
        }
        inside = Some(here);
    }
    let inside = inside?;
    let wall_dir = if inside { up } else { -up };
    // The round must fit: its line on the floor lies on the floor, and its
    // line on each wall is no higher than the wall reaches.
    let reach = size - tol.dist;
    for p in &runs_of {
        if !on_floor(p.middle + p.across * sign * reach) {
            return None;
        }
        let wall = *ef.get(&p.edge)?.iter().find(|f| **f != floor)?;
        let top = model
            .face(wall)?
            .loops()
            .flatten()
            .map(|v| (model.vertex(*v) - p.middle).dot(wall_dir))
            .fold(f64::NEG_INFINITY, f64::max);
        if top < reach {
            return None;
        }
    }
    let shape = Shape {
        up,
        sign,
        wall: wall_dir,
        size,
        past: size * 0.1,
        fillet,
    };
    // One smooth run: a chain that turns a corner (or a slight crease) is
    // left to the corner step.
    if runs_of
        .windows(2)
        .any(|w| !(tol.parallel(w[0].end, w[1].start) && w[0].end.dot(w[1].start) > 0.0))
    {
        return None;
    }
    let part: Vec<&Piece> = runs_of.iter().collect();
    let tool = strip_along(model, &part, &shape)?;
    let op = if inside {
        crate::boolean::Op::Union
    } else {
        crate::boolean::Op::Subtract
    };
    let mut joined = crate::boolean::boolean(&solid, &tool, op).ok()?;
    keep_looks(&solid, &mut joined);
    let mut work = model.clone();
    let out = put_in(&mut work, &faces, &joined)?;
    *model = work;
    Some(out)
}

/// One piece of a chain being rounded by [`round_swept`].
struct Piece {
    edge: EdgeId,
    from: VertexId,
    to: VertexId,
    /// The direction of travel at `from` and at `to`.
    start: DVec3,
    end: DVec3,
    middle: DVec3,
    /// Across the piece in the floor, at its middle (up × travel).
    across: DVec3,
}

/// The round's cross-section for [`round_swept`].
struct Shape {
    /// The floor's normal.
    up: DVec3,
    /// Which side of the chain the floor is on: into the floor is up ×
    /// travel × sign.
    sign: f64,
    /// Up the wall from the corner.
    wall: DVec3,
    size: f64,
    /// How far the cross-section reaches past the corner.
    past: f64,
    fillet: bool,
}

/// The round's cross-section swept along `part` (pieces of `model`, in
/// order): a solid of its own.
fn strip_along(model: &Model, part: &[&Piece], shape: &Shape) -> Option<Model> {
    let tol = model.tol();
    let (head, tail) = (part.first()?, part.last()?);
    let mut sweep = Model::new();
    sweep.set_tol(tol);
    let mut ids: BTreeMap<VertexId, VertexId> = BTreeMap::new();
    for p in part {
        for v in [p.from, p.to] {
            ids.entry(v)
                .or_insert_with(|| sweep.add_vertex(model.vertex(v)));
        }
    }
    let mut path = BTreeSet::new();
    for p in part {
        let x = model.edge(p.edge)?;
        let (va, vb) = (*ids.get(&x.a)?, *ids.get(&x.b)?);
        let new = sweep.add_edge(va, vb)?;
        if let prism_geom::Curve::Arc {
            centre,
            axis,
            radius,
        } = &x.curve
        {
            // An arc runs counter-clockwise from its first point: the other
            // way round if the new edge has its ends swapped.
            let flip = sweep.edge(new)?.a != va;
            sweep.set_curve(
                new,
                prism_geom::Curve::Arc {
                    centre: *centre,
                    axis: if flip { -*axis } else { *axis },
                    radius: *radius,
                },
            );
        }
        path.insert(new);
    }
    let (start, end) = (model.vertex(head.from), model.vertex(tail.to));
    // Follow Me starts at one end of the path: the cross-section goes
    // there, square to it.
    let (order, ..) = crate::follow::chain(&sweep, &path)?;
    let at_start = sweep.vertex(*order.first()?).distance(start) <= tol.dist;
    let (p, travel) = if at_start {
        (start, head.start)
    } else {
        (end, tail.end)
    };
    let u = shape.up.cross(travel).normalize_or_zero() * shape.sign;
    let (v, s, d) = (shape.wall, shape.size, shape.past);
    let corner = [
        p - u * d - v * d,
        p + u * s - v * d,
        p + u * s,
        p + v * s,
        p + v * s - u * d,
    ];
    crate::add_polygon(&mut sweep, &corner);
    if shape.fillet {
        let (va, vb) = (
            crate::vertex_at(&sweep, corner[2])?,
            crate::vertex_at(&sweep, corner[3])?,
        );
        let arc = sweep.edge_between(va, vb)?;
        let centre = p + (u + v) * s;
        let x = sweep.edge(arc)?;
        let (xa, xb) = (sweep.vertex(x.a) - centre, sweep.vertex(x.b) - centre);
        sweep.set_curve(
            arc,
            prism_geom::Curve::Arc {
                centre,
                axis: xa.cross(xb).normalize_or_zero(),
                radius: s,
            },
        );
    }
    let profile = sweep.faces().map(|(id, _)| id).find(|id| {
        sweep
            .face(*id)
            .and_then(|f| f.plane())
            .is_some_and(|pl| pl.normal.dot(travel).abs() > 0.5)
    })?;
    if !crate::follow::follow(&mut sweep, &path, profile) {
        return None;
    }
    let used = sweep.edge_faces();
    let loose: Vec<EdgeId> = sweep
        .edges()
        .map(|(id, _)| id)
        .filter(|id| used.get(id).is_none_or(|f| f.is_empty()))
        .collect();
    for x in loose {
        sweep.remove_edge(x);
    }
    crate::boolean::is_solid(&sweep).then_some(sweep)
}

/// Brief 56 step 3: round just the neighbourhood of the edges, where the
/// kernel will not round them on the whole body (something elsewhere on it
/// is in the way). The solid is cut by a box around the edges; the piece
/// inside is rounded, and joined again with the rest.
fn round_local(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<Vec<FaceId>> {
    let tol = model.tol();
    let faces = solid_faces(model, edges).ok()?;
    let solid = alone(model, &faces);
    // Each edge, as its two ends and its middle, to find it again.
    let marks: Vec<[DVec3; 3]> = edges
        .iter()
        .filter_map(|e| model.edge(*e))
        .map(|e| [model.vertex(e.a), model.vertex(e.b), middle(model, e)])
        .collect();
    let points: Vec<DVec3> = marks.iter().flatten().copied().collect();
    let (first, last) = (*points.first()?, marks.last()?[1]);
    // The box's frame: along the edges, and halfway between the normals
    // of the flat faces beside them.
    let along = (last - first).normalize_or_zero();
    let along = if along == DVec3::ZERO {
        (marks[0][2] - marks[0][0]).normalize_or_zero()
    } else {
        along
    };
    if along == DVec3::ZERO {
        return None;
    }
    let ef = model.edge_faces();
    let normal: DVec3 = ef
        .get(edges.first()?)?
        .iter()
        .filter_map(|f| model.face(*f)?.plane())
        .map(|p| p.normal)
        .sum();
    let up = (normal - along * normal.dot(along)).normalize_or_zero();
    let up = if up == DVec3::ZERO {
        prism_math::Plane::new(first, along).frame().u
    } else {
        up
    };
    let side = along.cross(up);
    let span = |d: DVec3| {
        let t = points.iter().map(|p| p.dot(d));
        (
            t.clone().fold(f64::INFINITY, f64::min),
            t.fold(f64::NEG_INFINITY, f64::max),
        )
    };
    let (sa, ss, su) = (span(along), span(side), span(up));
    for margin in [2.0 * size, 4.0 * size, 8.0 * size] {
        let corner = |a: f64, s: f64, u: f64| along * a + side * s + up * u;
        let (a0, a1) = (sa.0 - margin, sa.1 + margin);
        let (s0, s1) = (ss.0 - margin, ss.1 + margin);
        let mut cut = Model::new();
        cut.set_tol(tol);
        crate::add_polygon(
            &mut cut,
            &[
                corner(a0, s0, su.0 - margin),
                corner(a1, s0, su.0 - margin),
                corner(a1, s1, su.0 - margin),
                corner(a0, s1, su.0 - margin),
            ],
        );
        let Some(f) = cut.faces().next().map(|(id, _)| id) else {
            continue;
        };
        let Some(n) = cut.face(f).and_then(|x| x.plane()).map(|p| p.normal) else {
            continue;
        };
        let height = su.1 - su.0 + 2.0 * margin;
        crate::push_pull(
            &mut cut,
            f,
            if n.dot(up) > 0.0 { height } else { -height },
            false,
        );
        if !crate::boolean::is_solid(&cut) {
            continue;
        }
        // The box's six planes; no face of the solid may lie in one.
        let walls: Vec<prism_math::Plane> = cut
            .faces()
            .filter_map(|(_, f)| f.plane().copied())
            .collect();
        if solid.faces().any(|(id, _)| on_walls(&solid, id, &walls)) {
            continue;
        }
        let Ok(mut piece) = crate::boolean::boolean(&solid, &cut, crate::boolean::Op::Intersect)
        else {
            continue;
        };
        let Ok(mut rest) = crate::boolean::boolean(&solid, &cut, crate::boolean::Op::Subtract)
        else {
            continue;
        };
        for part in [&mut piece, &mut rest] {
            if part.edges().any(|(_, e)| e.approx) {
                exact_cuts(part);
            }
        }
        let near = |a: DVec3, b: DVec3| a.distance(b) <= tol.dist;
        let mine: BTreeSet<EdgeId> = piece
            .edges()
            .filter(|(_, e)| {
                let (a, b, m) = (piece.vertex(e.a), piece.vertex(e.b), middle(&piece, e));
                marks.iter().any(|k| {
                    near(m, k[2])
                        && ((near(a, k[0]) && near(b, k[1])) || (near(a, k[1]) && near(b, k[0])))
                })
            })
            .map(|(id, _)| id)
            .collect();
        if mine.len() != marks.len() {
            continue;
        }
        let on_box = |m: &Model| -> Vec<f64> {
            let mut a: Vec<f64> = m
                .faces()
                .filter(|(id, _)| on_walls(m, *id, &walls))
                .map(|(id, _)| m.face_area(id))
                .collect();
            a.sort_by(f64::total_cmp);
            a
        };
        let walled = on_box(&piece);
        if round_once(&mut piece, &mine, size, fillet).is_err() {
            continue;
        }
        // The round must not reach the box: else a bigger box.
        let after = on_box(&piece);
        if after.len() != walled.len()
            || after
                .iter()
                .zip(&walled)
                .any(|(x, y)| (x - y).abs() > tol.dist * y.sqrt().max(tol.dist))
        {
            continue;
        }
        let mut joined = glue(&rest, &piece, &walls);
        keep_looks(&solid, &mut joined);
        let mut work = model.clone();
        if let Some(out) = put_in(&mut work, &faces, &joined) {
            *model = work;
            return Some(out);
        }
    }
    None
}

/// Edges of `out` that are edges of `was` (the same ends and middle) get
/// back its look and kind: soft, smooth and approximate.
/// The booleans that cut and join a solid make its edges anew.
fn keep_looks(was: &Model, out: &mut Model) {
    let tol = was.tol();
    let near = |a: DVec3, b: DVec3| a.distance(b) <= tol.dist;
    let old: Vec<([DVec3; 3], prism_topo::Edge)> = was
        .edges()
        .map(|(_, e)| {
            (
                [was.vertex(e.a), was.vertex(e.b), middle(was, e)],
                e.clone(),
            )
        })
        .collect();
    let ids: Vec<EdgeId> = out.edges().map(|(id, _)| id).collect();
    for id in ids {
        let Some(e) = out.edge(id) else { continue };
        let (a, b, m) = (out.vertex(e.a), out.vertex(e.b), middle(out, e));
        let Some((_, o)) = old.iter().find(|(k, _)| {
            near(m, k[2]) && ((near(a, k[0]) && near(b, k[1])) || (near(a, k[1]) && near(b, k[0])))
        }) else {
            continue;
        };
        let o = o.clone();
        if o.soft || o.smooth {
            out.set_soft(id, o.soft, o.smooth);
        }
        if o.approx {
            out.set_approx(id, true);
        }
    }
}

/// Whether face `id` of `m` lies in one of `walls`.
fn on_walls(m: &Model, id: FaceId, walls: &[prism_math::Plane]) -> bool {
    let tol = m.tol();
    let Some(f) = m.face(id) else { return false };
    f.plane().is_some()
        && walls.iter().any(|w| {
            f.loops()
                .flatten()
                .all(|v| w.distance(m.vertex(*v)).abs() <= tol.dist)
        })
}

/// The two parts of a solid cut by a box, `rest` outside it and `piece`
/// inside, joined again along the box's `walls`: the faces on the walls go
/// from both, and a face the box had cut in two is one again (flat ones
/// merge; curved ones keep a soft, smooth seam).
fn glue(rest: &Model, piece: &Model, walls: &[prism_math::Plane]) -> Model {
    let tol = rest.tol();
    let strip = |m: &Model| {
        let mut m = m.clone();
        let gone: Vec<FaceId> = m
            .faces()
            .map(|(id, _)| id)
            .filter(|id| on_walls(&m, *id, walls))
            .collect();
        for f in gone {
            m.remove_face(f);
        }
        m
    };
    let mut out = strip(rest);
    crate::boolean::join(&mut out, &strip(piece), &tol);
    // The seams the box left: edges lying in a wall.
    let in_wall = |m: &Model, e: &prism_topo::Edge| {
        let pts = [m.vertex(e.a), m.vertex(e.b), middle(m, e)];
        walls
            .iter()
            .any(|w| pts.iter().all(|p| w.distance(*p).abs() <= tol.dist))
    };
    // One side may have a point partway along a straight seam where the
    // other has none: split the seam there, so the two sides meet edge to
    // edge.
    loop {
        let split = out.edges().find_map(|(id, e)| {
            if e.curve.is_curved() || !in_wall(&out, e) {
                return None;
            }
            let (a, b) = (out.vertex(e.a), out.vertex(e.b));
            out.vertices()
                .find(|(v, p)| {
                    *v != e.a
                        && *v != e.b
                        && p.distance(a) > tol.dist
                        && p.distance(b) > tol.dist
                        && distance_to_segment(*p, a, b) <= tol.dist
                })
                .map(|(v, _)| (id, v))
        });
        let Some((e, v)) = split else { break };
        if out.split_edge(e, v).is_none() {
            break;
        }
    }
    crate::settle::merge_duplicate_edges(&mut out, &tol);
    crate::health::remove_stray(&mut out);
    let ef = out.edge_faces();
    let (mut flat, mut curved) = (BTreeSet::new(), Vec::new());
    for (id, e) in out.edges() {
        if !in_wall(&out, e) {
            continue;
        }
        let Some([f, g]) = ef
            .get(&id)
            .and_then(|v| <[FaceId; 2]>::try_from(v.as_slice()).ok())
        else {
            continue;
        };
        let (Some(ff), Some(gf)) = (out.face(f), out.face(g)) else {
            continue;
        };
        match (ff.plane(), gf.plane()) {
            (Some(p), Some(q)) => {
                if p.same_surface(q, &tol, out.face_extent(gf)) && p.normal.dot(q.normal) > 0.0 {
                    flat.insert(id);
                }
            }
            (None, None) if ff.surface == gf.surface => curved.push(id),
            _ => {}
        }
    }
    for e in curved {
        out.set_soft(e, true, true);
    }
    if !flat.is_empty() {
        crate::remove(
            &mut out,
            &crate::Selection {
                edges: flat,
                ..Default::default()
            },
        );
    }
    out
}

/// The solid of `faces` on its own: the other faces of the model, and
/// edges left with no face, taken away.
fn alone(model: &Model, faces: &BTreeSet<FaceId>) -> Model {
    let mut solid = model.clone();
    let others: Vec<FaceId> = solid
        .faces()
        .map(|(id, _)| id)
        .filter(|id| !faces.contains(id))
        .collect();
    for f in others {
        solid.remove_face(f);
    }
    let used = solid.edge_faces();
    let loose: Vec<EdgeId> = solid
        .edges()
        .map(|(id, _)| id)
        .filter(|id| used.get(id).is_none_or(|f| f.is_empty()))
        .collect();
    for e in loose {
        solid.remove_edge(e);
    }
    solid
}

/// `joined` put into `model` in place of the solid of `faces`, if the
/// model then checks clean and its health is no worse. Returns the new
/// faces.
fn put_in(model: &mut Model, faces: &BTreeSet<FaceId>, joined: &Model) -> Option<Vec<FaceId>> {
    let mut work = model.clone();
    for f in faces {
        work.remove_face(*f);
    }
    let (used, was) = (work.edge_faces(), model.edge_faces());
    // The solid's own edges, now with no face (loose lines stay).
    let loose: Vec<EdgeId> = work
        .edges()
        .map(|(id, _)| id)
        .filter(|id| {
            used.get(id).is_none_or(|f| f.is_empty()) && was.get(id).is_some_and(|f| !f.is_empty())
        })
        .collect();
    for e in loose {
        work.remove_edge(e);
    }
    let points: BTreeSet<VertexId> = faces
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.loops().flatten().copied().collect::<Vec<_>>())
        .collect();
    for v in points {
        if !work.edges().any(|(_, x)| x.has(v)) {
            work.remove_vertex(v);
        }
    }
    let had: BTreeSet<FaceId> = work.faces().map(|(id, _)| id).collect();
    work.append(joined);
    if !crate::check::problems(&work).is_empty() || worse_health(model, &work) {
        return None;
    }
    let out: Vec<FaceId> = work
        .faces()
        .map(|(id, _)| id)
        .filter(|id| !had.contains(id))
        .collect();
    *model = work;
    Some(out)
}

/// [`round_any`], then past lines drawn on the faces where it cannot.
fn round_once(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Result<Vec<FaceId>, &'static str> {
    match round_any(model, edges, size, fillet) {
        Err(why)
            if why == prism_kernel::solid::TOO_LARGE
                || why == prism_kernel::solid::NOT_ROUNDED
                || why == TWO_SOLIDS =>
        {
            let why = if why == TWO_SOLIDS { OPEN } else { why };
            round_through_lines(model, edges, size, fillet).ok_or(why)
        }
        done => done,
    }
}

/// Where the kernel cannot round `edges` together (a corner it cannot
/// close: an inside round meeting outside ones, as at the inner corner of
/// an L-shaped block), round them in turn: the inside (concave) edges,
/// then the outside ones, or the other way round, or else one at a time.
/// Each later round runs onto the faces the earlier ones made, so the
/// corner is closed by the rounds themselves (DECISIONS #46).
fn round_in_turn(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<Vec<FaceId>> {
    if edges.len() < 2 {
        return None;
    }
    let (inside, outside): (BTreeSet<EdgeId>, BTreeSet<EdgeId>) =
        edges.iter().partition(|e| concave(model, **e));
    let mut orders: Vec<Vec<BTreeSet<EdgeId>>> = Vec::new();
    if !inside.is_empty() && !outside.is_empty() {
        orders.push(vec![inside.clone(), outside.clone()]);
        orders.push(vec![outside, inside]);
    }
    // One at a time: in every order when there are few (an inside corner
    // where three rounds meet can close in one order and not another).
    let one: Vec<EdgeId> = edges.iter().copied().collect();
    if one.len() <= 4 {
        for p in orderings(&one) {
            orders.push(p.into_iter().map(|e| BTreeSet::from([e])).collect());
        }
    } else {
        orders.push(one.iter().map(|e| BTreeSet::from([*e])).collect());
    }
    for groups in orders {
        let mut work = model.clone();
        let mut last = None;
        let mut ok = true;
        for g in &groups {
            let now = follow(model, &work, g);
            if now.is_empty() {
                ok = false;
                break;
            }
            match round_once(&mut work, &now, size, fillet) {
                Ok(f) => last = Some(f),
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }
        if ok && crate::check::problems(&work).is_empty() && !worse_health(model, &work) {
            *model = work;
            return last;
        }
    }
    None
}

/// Every order of `items`.
fn orderings<T: Copy>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut out = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let first = rest.remove(i);
        for mut tail in orderings(&rest) {
            tail.insert(0, first);
            out.push(tail);
        }
    }
    out
}

/// Where rounded edges meet at a corner of four or more edges that the
/// kernel cannot close (a pyramid's apex), even in turn: the corner is cut
/// off first by a small flat face, past where the rounds reach, square to
/// the corner's average direction (each edge is cut where it meets that
/// plane, so the face is flat); every round then ends at a plain corner
/// of three edges, which the kernel closes. A step toward DECISIONS #46's
/// smooth corner patch, which would replace that small face.
fn round_with_caps(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<Vec<FaceId>> {
    let mut at: BTreeMap<VertexId, usize> = BTreeMap::new();
    for e in edges {
        let x = model.edge(*e)?;
        *at.entry(x.a).or_default() += 1;
        *at.entry(x.b).or_default() += 1;
    }
    let degree = |m: &Model, v: VertexId| m.edges().filter(|(_, x)| x.has(v)).count();
    let corners: Vec<VertexId> = at
        .iter()
        .filter(|(v, n)| **n >= 2 && degree(model, **v) >= 4)
        .map(|(v, _)| *v)
        .collect();
    if corners.is_empty() {
        return None;
    }
    let tol = model.tol();
    let back = reach(model, edges, size, fillet) * 1.2;
    let mut work = model.clone();
    for v in corners {
        let p = work.vertex(v);
        let spokes: Vec<(EdgeId, DVec3, f64)> = work
            .edges()
            .filter(|(_, x)| x.has(v))
            .map(|(id, x)| {
                let q = work.vertex(x.other(v));
                (id, (q - p).normalize_or_zero(), q.distance(p))
            })
            .collect();
        if spokes
            .iter()
            .any(|(id, ..)| work.edge(*id).is_some_and(|x| x.curve.is_curved()))
        {
            return None;
        }
        let n = -spokes
            .iter()
            .map(|(_, d, _)| *d)
            .sum::<DVec3>()
            .normalize_or_zero();
        if n == DVec3::ZERO || spokes.iter().any(|(_, d, _)| n.dot(*d) >= -1e-6) {
            return None;
        }
        let h = back
            * spokes
                .iter()
                .map(|(_, d, _)| -n.dot(*d))
                .fold(0.0, f64::max);
        let cuts: Vec<DVec3> = spokes
            .iter()
            .map(|(_, d, len)| {
                let t = h / -n.dot(*d);
                (t < len * 0.5).then_some(p + *d * t)
            })
            .collect::<Option<_>>()?;
        for c in &cuts {
            crate::sticky::insert_point(&mut work, *c, &tol);
        }
        // Each face round the corner gets a line between its two cuts.
        let around: Vec<FaceId> = work
            .faces()
            .filter(|(_, f)| f.loops().flatten().any(|x| *x == v))
            .map(|(id, _)| id)
            .collect();
        for f in around {
            let on: Vec<DVec3> = cuts
                .iter()
                .copied()
                .filter(|c| {
                    work.face(f).is_some_and(|face| {
                        face.loops()
                            .flatten()
                            .any(|x| tol.same_point(work.vertex(*x), *c))
                    })
                })
                .collect();
            let [a, b] = on[..] else { return None };
            crate::sticky::add_segment(&mut work, a, b);
        }
        // Off with the corner: the small faces at it, its edges, itself.
        let small: Vec<FaceId> = work
            .faces()
            .filter(|(_, f)| f.loops().flatten().any(|x| *x == v))
            .map(|(id, _)| id)
            .collect();
        for f in small {
            work.remove_face(f);
        }
        let spokes: Vec<EdgeId> = work
            .edges()
            .filter(|(_, x)| x.has(v))
            .map(|(id, _)| id)
            .collect();
        for e in spokes {
            work.remove_edge(e);
        }
        work.remove_vertex(v);
        // The flat face across it, its points in turn round the corner.
        let (u, w) = (
            n.any_orthonormal_vector(),
            n.cross(n.any_orthonormal_vector()),
        );
        let mut ring = cuts.clone();
        ring.sort_by(|a, b| {
            let ang = |q: &DVec3| (*q - p).dot(w).atan2((*q - p).dot(u));
            ang(a).total_cmp(&ang(b))
        });
        crate::sticky::add_polygon(&mut work, &ring);
    }
    crate::health::orient(&mut work, None);
    if !crate::check::problems(&work).is_empty() {
        return None;
    }
    let now = follow(model, &work, edges);
    let capped = work.clone();
    let out = match round_once(&mut work, &now, size, fillet) {
        Ok(out) => out,
        Err(_) => {
            work = capped;
            round_in_turn(&mut work, &now, size, fillet)?
        }
    };
    if worse_health(model, &work) {
        return None;
    }
    *model = work;
    Some(out)
}

/// Whether the solid turns inward at `e` (between two flat faces).
fn concave(model: &Model, e: EdgeId) -> bool {
    crate::round::sides(model, e).is_some_and(|[(_, _, u1), (_, n2, _)]| n2.dot(u1) > 0.0)
}

/// The edges of `now` that are what is left of `edges` of `before` (an
/// edge an earlier round shortened at its end is found along its line or
/// arc).
fn follow(before: &Model, now: &Model, edges: &BTreeSet<EdgeId>) -> BTreeSet<EdgeId> {
    let tol = before.tol();
    let mut out = BTreeSet::new();
    for e in edges {
        let Some(x) = before.edge(*e) else { continue };
        let (a, b) = (before.vertex(x.a), before.vertex(x.b));
        if let Some(y) = now.edge(*e)
            && tol.same_point(now.vertex(y.a), a)
            && tol.same_point(now.vertex(y.b), b)
        {
            out.insert(*e);
            continue;
        }
        let on = |p: DVec3| {
            tol.same_point(p, a)
                || tol.same_point(p, b)
                || crate::sticky::on_curve_inside(&x.curve, a, b, p, &tol)
        };
        out.extend(
            now.edges()
                .filter(|(_, y)| y.curve.is_curved() == x.curve.is_curved())
                .filter(|(_, y)| on(now.vertex(y.a)) && on(now.vertex(y.b)))
                .map(|(id, _)| id),
        );
    }
    out
}

/// Whether `e` is a straight line between two flat faces of one plane
/// that face the same way.
fn drawn_on_flat(model: &Model, e: EdgeId) -> bool {
    let Some(x) = model.edge(e) else { return false };
    if x.curve.is_curved() {
        return false;
    }
    let edge_faces = model.edge_faces();
    let Some([f, g]) = edge_faces.get(&e).map(Vec::as_slice) else {
        return false;
    };
    let tol = model.tol();
    match (
        model.face(*f).and_then(|x| x.plane()),
        model.face(*g).and_then(|x| x.plane()),
    ) {
        (Some(p), Some(q)) => {
            p.normal.dot(q.normal) > 0.0
                && p.same_surface(q, &tol, model.face_extent(model.face(*g).expect("a face")))
        }
        _ => false,
    }
}

/// [`round_plain`], and where the kernel will not, the edges one by one
/// by Prism's own round ([`crate::round`]), when they are straight edges
/// between flat faces that do not touch: the kernel refuses a small round
/// on a shallow edge as too large (a plain box with its top edge bevelled
/// at 12 degrees: every size; at 45 degrees, 5 mm), which a cylinder
/// tangent to both faces makes exactly (owner's Xby4, 2026-11-26).
fn round_any(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Result<Vec<FaceId>, &'static str> {
    let first = round_plain(model, edges, size, fillet);
    match first {
        Err(why) if why == TWO_SOLIDS => round_capped(model, edges, size, fillet)
            .or_else(|| round_apart(model, edges, size, fillet))
            .ok_or(why),
        Err(why)
            if why == prism_kernel::solid::TOO_LARGE || why == prism_kernel::solid::NOT_ROUNDED =>
        {
            round_apart(model, edges, size, fillet).ok_or(why)
        }
        done => done,
    }
}

/// Edges not on a closed solid rounded by the kernel all the same: each
/// opening of the faces they are on (a loop of edges with a face on one
/// side) that lies in one plane is closed by a flat face for the kernel,
/// and the faces made of it taken away after (owner, 2026-11-26: "fillet /
/// chamfer should work along that whole edge, no complaints"; the owner's
/// session 29: a body closed but for a hole 0.24 mm across, where a short
/// line and an arc met, so no edge on it could be rounded together with
/// another, or beside a cylinder).
fn round_capped(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<Vec<FaceId>> {
    let tol = model.tol();
    let edge_faces = model.edge_faces();
    let seed = *edge_faces.get(edges.first()?)?.first()?;
    let piece = piece_of(model, seed);
    let open: Vec<EdgeId> = model
        .edges()
        .map(|(id, _)| id)
        .filter(|id| {
            edge_faces
                .get(id)
                .is_some_and(|fs| fs.len() == 1 && piece.contains(&fs[0]))
        })
        .collect();
    if open.is_empty() {
        return None;
    }
    // The openings: loops of open edges, each point on two of them.
    let mut at: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
    for e in &open {
        let x = model.edge(*e)?;
        at.entry(x.a).or_default().push(*e);
        at.entry(x.b).or_default().push(*e);
    }
    if at.values().any(|es| es.len() != 2) {
        return None;
    }
    let mut left: BTreeSet<EdgeId> = open.iter().copied().collect();
    let mut work = model.clone();
    let mut caps: Vec<prism_math::Plane> = Vec::new();
    while let Some(first) = left.pop_first() {
        let x = model.edge(first)?;
        let (start, mut v) = (x.a, x.b);
        let (mut loop_v, mut loop_e) = (vec![x.a], vec![first]);
        while v != start {
            loop_v.push(v);
            let next = *at.get(&v)?.iter().find(|e| left.contains(e))?;
            left.remove(&next);
            loop_e.push(next);
            v = model.edge(next)?.other(v);
        }
        // A sliver: two short edges between the same two points (a line
        // and an arc 0.24 mm long, a notch in one face, in the owner's
        // session 29): the two points become one (no further apart than
        // a hundred times the precision), which closes it for good; the
        // shape moves no further than the sliver is long.
        if let [a, b] = loop_e[..] {
            let _ = (a, b);
            let [p, q] = loop_v[..] else { return None };
            if work.vertex(p).distance(work.vertex(q)) > tol.dist * 100.0 {
                return None;
            }
            crate::transform::apply_weld(&mut work, &BTreeMap::from([(p.max(q), p.min(q))]));
            continue;
        }
        // The opening's points in the order it is walked.
        let mut pts: Vec<DVec3> = Vec::new();
        for (k, e) in loop_e.iter().enumerate() {
            let mut along = model.edge_points(*e);
            if model.edge(*e)?.a != loop_v[k] {
                along.reverse();
            }
            pts.extend(along);
        }
        let n = prism_math::newell(&pts).normalize_or_zero();
        if n == DVec3::ZERO {
            return None;
        }
        let plane = prism_math::Plane::new(pts[0], n);
        if !pts.iter().all(|p| plane.contains(*p, &tol)) {
            return None;
        }
        work.add_face(prism_topo::Face::with_edges(
            loop_v,
            Vec::new(),
            prism_geom::Surface::Plane(plane),
            vec![loop_e],
        ));
        caps.push(plane);
    }
    crate::health::orient(&mut work, None);
    let had: BTreeSet<FaceId> = model.faces().map(|(id, _)| id).collect();
    let r = round_once(&mut work, edges, size, fillet);
    r.ok()?;
    // The caps go again, and whatever the round made of them.
    let gone: Vec<FaceId> = work
        .faces()
        .filter(|(id, f)| {
            !had.contains(id)
                && f.plane().is_some_and(|p| {
                    caps.iter().any(|c| {
                        p.normal.dot(c.normal).abs() > 1.0 - 1e-9 && c.contains(p.origin, &tol)
                    })
                })
                && f.loops()
                    .flatten()
                    .all(|v| caps.iter().any(|c| c.contains(work.vertex(*v), &tol)))
        })
        .map(|(id, _)| id)
        .collect();
    if gone.is_empty() && !caps.is_empty() {
        return None;
    }
    for f in &gone {
        work.remove_face(*f);
    }
    let used = work.edge_faces();
    let stray: Vec<EdgeId> = work
        .edges()
        .map(|(id, _)| id)
        .filter(|id| used.get(id).is_none_or(|f| f.is_empty()))
        .collect();
    for e in stray {
        work.remove_edge(e);
    }
    if !crate::check::problems(&work).is_empty() || worse_health(model, &work) {
        return None;
    }
    let new = work
        .faces()
        .map(|(id, _)| id)
        .find(|id| !had.contains(id))?;
    let out = piece_of(&work, new).into_iter().collect();
    *model = work;
    Some(out)
}

/// `edges` rounded one by one by [`crate::round::round`], if none of them
/// share a point and each can be. The solid's faces afterwards.
fn round_apart(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<Vec<FaceId>> {
    let mut ends = BTreeSet::new();
    for e in edges {
        let x = model.edge(*e)?;
        if !ends.insert(x.a) || !ends.insert(x.b) {
            return None;
        }
    }
    let mut work = model.clone();
    let before: BTreeSet<EdgeId> = work.edges().map(|(id, _)| id).collect();
    for e in edges {
        if !crate::round::round(&mut work, *e, size, fillet) {
            return None;
        }
    }
    soften_tangent(&mut work, &before);
    if !crate::check::problems(&work).is_empty() {
        return None;
    }
    let new = work
        .faces()
        .map(|(id, _)| id)
        .find(|id| model.face(*id).is_none())?;
    let out = piece_of(&work, new).into_iter().collect();
    *model = work;
    Some(out)
}

/// The faces joined to `seed` through edges: its closed solid, or the
/// sheet it is part of.
fn piece_of(model: &Model, seed: FaceId) -> BTreeSet<FaceId> {
    if let Ok(s) = solid_of(model, seed) {
        return s;
    }
    let edge_faces = model.edge_faces();
    let mut out = BTreeSet::from([seed]);
    let mut todo = vec![seed];
    while let Some(f) = todo.pop() {
        let Some(face) = model.face(f) else { continue };
        for e in face.edge_ids() {
            for g in edge_faces.get(&e).into_iter().flatten() {
                if out.insert(*g) {
                    todo.push(*g);
                }
            }
        }
    }
    out
}

/// The faces of the closed solid all of `edges` are on, or else the
/// faces joined to theirs.
fn piece_faces(model: &Model, edges: &BTreeSet<EdgeId>) -> Option<BTreeSet<FaceId>> {
    if let Ok(f) = solid_faces(model, edges) {
        return Some(f);
    }
    let edge_faces = model.edge_faces();
    let seed = *edge_faces.get(edges.first()?)?.first()?;
    let piece = piece_of(model, seed);
    edges
        .iter()
        .all(|e| {
            edge_faces
                .get(e)
                .is_some_and(|fs| fs.iter().all(|f| piece.contains(f)))
        })
        .then_some(piece)
}

/// [`round_edges`] again where the kernel found the size too large, past
/// lines that are not part of the solid's shape, only drawn on it: a line
/// between two faces of one plane (drawn on a face beside the round's
/// end), and a point where an edge was split and nothing else meets. To
/// the kernel each is a corner a round may not run past (owner's session
/// 22: a round stopped at 0.028 m, then at 0.109 m, where 0.2 m fits).
/// Those near the ends of `edges` are taken off, the edges rounded, and
/// the lines drawn back on. None if it still cannot be done, or the lines
/// would not go back.
fn round_through_lines(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<Vec<FaceId>> {
    let mut work = model.clone();
    let faces = piece_faces(&work, edges)?;
    let reach = reach(&work, edges, size, fillet);
    let seams = drawn_lines(&work, &faces, edges, reach);
    let lines: Vec<(DVec3, DVec3)> = seams
        .iter()
        .filter_map(|e| work.edge(*e))
        .map(|e| (work.vertex(e.a), work.vertex(e.b)))
        .collect();
    if !seams.is_empty() {
        crate::remove(
            &mut work,
            &crate::Selection {
                edges: seams,
                ..Default::default()
            },
        );
    }
    let faces = piece_faces(&work, edges)?;
    let mut edges = edges.clone();
    let joined = join_straight(&mut work, &faces, &mut edges);
    if lines.is_empty() && joined == 0 {
        return None;
    }
    let along: Vec<(DVec3, DVec3)> = edges
        .iter()
        .filter_map(|e| work.edge(*e))
        .filter(|e| !e.curve.is_curved())
        .map(|e| (work.vertex(e.a), work.vertex(e.b)))
        .collect();
    let old_faces: BTreeSet<FaceId> = work.faces().map(|(id, _)| id).collect();
    round_any(&mut work, &edges, size, fillet).ok()?;
    // The lines go back as far as they still lie on the solid: a round
    // that cut into the face one was on leaves its end in the air.
    draw_back(&mut work, &lines, false);
    // One that met a rounded edge carries on across the round as a seam,
    // as on the kernel's round (owner, 2026-11-27: "B for both").
    let before_seams = work.clone();
    carry_across(&mut work, &lines, &along, &old_faces);
    if !crate::check::problems(&work).is_empty() || worse_health(model, &work) {
        work = before_seams;
    }
    if !crate::check::problems(&work).is_empty() || worse_health(model, &work) {
        return None;
    }
    let seed = edges
        .iter()
        .find_map(|e| work.edge_faces().get(e).and_then(|f| f.first().copied()));
    let out = match seed {
        Some(f) => piece_of(&work, f).into_iter().collect(),
        None => {
            // The rounded edges are gone: the solid is the faces that were
            // not there before, with the rest it joins.
            let new = work
                .faces()
                .map(|(id, _)| id)
                .find(|id| model.face(*id).is_none())?;
            piece_of(&work, new).into_iter().collect()
        }
    };
    *model = work;
    Some(out)
}

/// For each of `lines` with an end on one of the straight edges `along`
/// (not at its ends), the cross-section of the round made there, from one
/// of its long edges to the other: an arc on a fillet's cylinder, a line
/// on a chamfer. Only faces not in `old` (the round's own) are looked at.
fn carry_across(
    work: &mut Model,
    lines: &[(DVec3, DVec3)],
    along: &[(DVec3, DVec3)],
    old: &BTreeSet<FaceId>,
) {
    let tol = work.tol();
    // Each cut's two ends, and for a fillet its centre, axis and radius.
    type Cut = (DVec3, DVec3, Option<(DVec3, DVec3, f64)>);
    let mut cuts: Vec<Cut> = Vec::new();
    for p in lines.iter().flat_map(|(a, b)| [*a, *b]) {
        for (ea, eb) in along {
            let d = *eb - *ea;
            let len = d.length();
            if len <= tol.dist {
                continue;
            }
            let dir = d / len;
            let t = (p - *ea).dot(dir);
            if t <= tol.dist || t >= len - tol.dist || (*ea + dir * t).distance(p) > tol.dist {
                continue;
            }
            for (id, f) in work.faces() {
                if old.contains(&id) {
                    continue;
                }
                let round = match &f.surface {
                    prism_geom::Surface::Cylinder(c) if c.axis.dot(dir).abs() > 1.0 - 1e-9 => {
                        Some(*c)
                    }
                    prism_geom::Surface::Plane(pl) if pl.normal.dot(dir).abs() < 1e-9 => None,
                    _ => continue,
                };
                // Where the face's long edges (along the round) cross the
                // plane square to it at p.
                let ends: Vec<DVec3> = f
                    .edge_ids()
                    .filter_map(|e| work.edge(e))
                    .filter(|e| !e.curve.is_curved())
                    .filter_map(|e| {
                        let (a, b) = (work.vertex(e.a), work.vertex(e.b));
                        let s = b - a;
                        if s.length() <= tol.dist || s.normalize().dot(dir).abs() < 1.0 - 1e-9 {
                            return None;
                        }
                        let k = (p - a).dot(dir) / s.dot(dir);
                        (k > 0.0 && k < 1.0).then(|| a + s * k)
                    })
                    .collect();
                let [q1, q2] = ends[..] else { continue };
                cuts.push((q1, q2, round.map(|c| (c.axis_point(q1), dir, c.radius))));
            }
        }
    }
    for (q1, q2, round) in cuts {
        match round {
            None => {
                crate::add_segment(work, q1, q2);
            }
            Some((centre, dir, r)) => {
                let (u, w) = (q1 - centre, q2 - centre);
                let turn = dir.dot(u.cross(w)).atan2(u.dot(w));
                let axis = if turn > 0.0 { dir } else { -dir };
                crate::add_arc(work, centre, axis, r, q1, turn.abs());
            }
        }
    }
}

/// Draw `lines` back on `work` after a round, as far as they still lie on
/// its faces: a piece left in the air (where the round cut into the face
/// it was on) is not kept, nor a piece left loose (unless `keep_loose`:
/// a slit, which ended inside its face to begin with).
fn draw_back(work: &mut Model, lines: &[(DVec3, DVec3)], keep_loose: bool) {
    let had: BTreeSet<EdgeId> = work.edges().map(|(id, _)| id).collect();
    for (a, b) in lines {
        crate::add_segment(work, *a, *b);
    }
    let used = work.edge_faces();
    let loose: Vec<EdgeId> = work
        .edges()
        .map(|(id, _)| id)
        .filter(|id| !had.contains(id) && used.get(id).is_none_or(|f| f.is_empty()))
        .filter(|id| {
            // A slit drawn back is loose on its face (it ends inside it):
            // kept while its middle is still on a flat face.
            !keep_loose
                || work.edge(*id).is_none_or(|e| {
                    let mid = work.vertex(e.a).lerp(work.vertex(e.b), 0.5);
                    e.curve.is_curved() || crate::face_at(work, mid).is_none()
                })
        })
        .collect();
    for id in loose {
        let Some(e) = work.edge(id).cloned() else {
            continue;
        };
        work.remove_edge(id);
        for v in [e.a, e.b] {
            if !work.edges().any(|(_, x)| x.a == v || x.b == v) {
                work.remove_vertex(v);
            }
        }
    }
}

/// The slits of the piece `edges` are on: lines drawn into a face from
/// its edge without dividing it, so the face walks them out and back and
/// has them on both sides.
fn slits(model: &Model, edges: &BTreeSet<EdgeId>) -> BTreeSet<EdgeId> {
    let edge_faces = model.edge_faces();
    let Some(seed) = edges
        .first()
        .and_then(|e| edge_faces.get(e))
        .and_then(|f| f.first())
    else {
        return BTreeSet::new();
    };
    let piece = piece_of(model, *seed);
    edge_faces
        .iter()
        .filter(|(_, fs)| matches!(fs[..], [f, g] if f == g && piece.contains(&f)))
        .map(|(e, _)| *e)
        .collect()
}

/// Notches: two edges with a face on one side only, between the same two
/// points no further apart than a hundred times the precision (owner's
/// session 29: a line and an arc 0.24 mm long, nothing between them).
fn notches(model: &Model) -> Vec<(VertexId, VertexId)> {
    let tol = model.tol();
    let edge_faces = model.edge_faces();
    let mut seen: BTreeMap<(VertexId, VertexId), usize> = BTreeMap::new();
    for (id, e) in model.edges() {
        if edge_faces.get(&id).is_some_and(|f| f.len() == 1)
            && model.vertex(e.a).distance(model.vertex(e.b)) <= tol.dist * 100.0
        {
            *seen.entry((e.a.min(e.b), e.a.max(e.b))).or_default() += 1;
        }
    }
    seen.into_iter()
        .filter(|(_, n)| *n == 2)
        .map(|(k, _)| k)
        .collect()
}

/// Each notch closed for good: its two points become one. The shape moves
/// no further than the notch is long (owner, 2026-11-26: kept in the
/// model).
fn weld_notches(model: &mut Model) {
    for (p, q) in notches(model) {
        crate::transform::apply_weld(model, &BTreeMap::from([(q, p)]));
    }
}

/// Runs of approximate straight pieces where a plane cuts a cylinder (the
/// owner's corner test: a sloping wall across a curved one, 16 pieces),
/// each made one exact edge: the cut is an ellipse, stored exactly as a
/// rational curve (DECISIONS #38), so the kernel can take the solid. Each
/// piece's points must lie on that ellipse. Returns how many runs.
pub fn exact_cuts(model: &mut Model) -> usize {
    let tol = model.tol();
    let edge_faces = model.edge_faces();
    let pair = |e: EdgeId| -> Option<[FaceId; 2]> {
        let fs = edge_faces.get(&e)?;
        let mut fs: [FaceId; 2] = fs[..].try_into().ok()?;
        fs.sort();
        Some(fs)
    };
    let approx: BTreeSet<EdgeId> = model
        .edges()
        .filter(|(id, e)| e.approx && !e.curve.is_curved() && pair(*id).is_some())
        .map(|(id, _)| id)
        .collect();
    let degree = |v: VertexId| model.edges().filter(|(_, e)| e.has(v)).count();
    let mut seen: BTreeSet<EdgeId> = BTreeSet::new();
    let mut runs: Vec<(Vec<VertexId>, Vec<EdgeId>, [FaceId; 2])> = Vec::new();
    for &e in &approx {
        if seen.contains(&e) {
            continue;
        }
        let fs = pair(e).expect("checked");
        // Walk both ways along pieces between the same two faces, through
        // points no other edge meets.
        let x = model.edge(e).expect("an edge");
        let mut verts = std::collections::VecDeque::from([x.a, x.b]);
        let mut run = std::collections::VecDeque::from([e]);
        seen.insert(e);
        for forward in [true, false] {
            loop {
                let end = if forward {
                    *verts.back().expect("")
                } else {
                    verts[0]
                };
                if degree(end) != 2 {
                    break;
                }
                let Some((next, y)) = model.edges().find(|(id, y)| {
                    y.has(end) && approx.contains(id) && !seen.contains(id) && pair(*id) == Some(fs)
                }) else {
                    break;
                };
                seen.insert(next);
                let other = y.other(end);
                if forward {
                    verts.push_back(other);
                    run.push_back(next);
                } else {
                    verts.push_front(other);
                    run.push_front(next);
                }
            }
        }
        runs.push((verts.into(), run.into(), fs));
    }
    let mut done = 0;
    for (verts, run, fs) in runs {
        let (Some(f), Some(g)) = (model.face(fs[0]), model.face(fs[1])) else {
            continue;
        };
        let (cyl, plane) = match (&f.surface, &g.surface) {
            (prism_geom::Surface::Cylinder(c), prism_geom::Surface::Plane(p))
            | (prism_geom::Surface::Plane(p), prism_geom::Surface::Cylinder(c)) => (*c, *p),
            _ => continue,
        };
        let (u, n) = (cyl.axis, plane.normal);
        let un = u.dot(n);
        if un.abs() < 1e-6 {
            continue;
        }
        // The axis meets the plane at the ellipse's centre; the circle of
        // the cylinder there, slid along the axis onto the plane, is it.
        let c = cyl.origin + u * ((plane.origin - cyl.origin).dot(n) / un);
        let on_circle = |p: DVec3| p - u * (p - c).dot(u);
        let pts: Vec<DVec3> = verts.iter().map(|v| model.vertex(*v)).collect();
        let (qa, qb) = (on_circle(pts[0]), on_circle(pts[pts.len() - 1]));
        let angle = |q: DVec3, axis: DVec3| {
            let (a, b) = (qa - c, q - c);
            let t = axis.dot(a.cross(b)).atan2(a.dot(b));
            if t < 0.0 {
                t + std::f64::consts::TAU
            } else {
                t
            }
        };
        let mid = on_circle(pts[pts.len() / 2]);
        // Turn the way that passes the run's middle.
        let axis = if angle(mid, u) < angle(qb, u) { u } else { -u };
        let sweep = angle(qb, axis);
        let Some(arc) = prism_geom::Nurbs::arc(c, axis, qa, sweep) else {
            continue;
        };
        // Slide along the axis onto the plane: an affine map, which a
        // rational curve follows exactly.
        let lin = prism_math::DMat3::from_cols(
            prism_math::DVec3::X - u * (n.x / un),
            prism_math::DVec3::Y - u * (n.y / un),
            prism_math::DVec3::Z - u * (n.z / un),
        );
        let slide =
            prism_math::DAffine3::from_mat3_translation(lin, u * (plane.origin.dot(n) / un));
        let ellipse = arc.transformed(&slide);
        let (t0, t1) = ellipse.domain();
        let near = |p: DVec3| {
            let t = ellipse.closest(p, t0, t1);
            ellipse.eval(t).distance(p) <= tol.dist
        };
        if !tol.same_point(ellipse.eval(t0), pts[0])
            || !tol.same_point(ellipse.eval(t1), pts[pts.len() - 1])
            || !pts.iter().all(|p| near(*p))
        {
            continue;
        }
        let (a, b) = (verts[0], verts[verts.len() - 1]);
        let soft = model.edge(run[0]).map(|x| (x.soft, x.smooth));
        for e in &run {
            model.remove_edge(*e);
        }
        let curve = prism_geom::Curve::Spline(prism_geom::Spline::whole(ellipse));
        let Some(new) = model.add_curve_edge(a, b, curve, None) else {
            continue;
        };
        if let Some((s, m)) = soft {
            model.set_soft(new, s, m);
        }
        let inner: BTreeSet<VertexId> = verts[1..verts.len() - 1].iter().copied().collect();
        for face in fs {
            model.edit_face(face, |x| {
                x.outer.retain(|v| !inner.contains(v));
                for h in &mut x.holes {
                    h.retain(|v| !inner.contains(v));
                }
            });
        }
        for v in inner {
            model.remove_vertex(v);
        }
        done += 1;
    }
    done
}

/// Brief 56 step 1: where the solid has slits or notches (owner's session
/// 29: a line 0.37 m long drawn into an end face, and a notch 0.24 mm
/// long), or approximate edges (owner's corner test: a plane's cut across
/// a cylinder in 16 pieces), the kernel cannot take it. The slits are taken
/// off a copy, the notches welded and the approximate cuts made exact
/// ([`exact_cuts`]), the edges rounded, and the slits drawn back as far as
/// they still lie on a face.
fn round_past_slits(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<Result<Vec<FaceId>, &'static str>> {
    let cut = slits(model, edges);
    let approx = model.edges().any(|(_, e)| e.approx);
    if edges.iter().any(|e| cut.contains(e))
        || (cut.is_empty() && notches(model).is_empty() && !approx)
    {
        return None;
    }
    let lines: Vec<(DVec3, DVec3)> = cut
        .iter()
        .filter_map(|e| model.edge(*e))
        .map(|e| (model.vertex(e.a), model.vertex(e.b)))
        .collect();
    let mut work = model.clone();
    let mut changed = !cut.is_empty() || !notches(model).is_empty();
    crate::remove(
        &mut work,
        &crate::Selection {
            edges: cut,
            ..Default::default()
        },
    );
    if !slits(&work, edges).is_empty() {
        return None;
    }
    weld_notches(&mut work);
    // Approximate cuts made exact; rounded pieces of one follow it.
    let before: BTreeSet<EdgeId> = work.edges().map(|(id, _)| id).collect();
    let mut edges = edges.clone();
    if approx && exact_cuts(&mut work) > 0 {
        changed = true;
        let gone = edges.iter().any(|e| work.edge(*e).is_none());
        edges.retain(|e| work.edge(*e).is_some());
        if gone {
            let ef = work.edge_faces();
            edges.extend(
                work.edges()
                    .map(|(id, _)| id)
                    .filter(|id| !before.contains(id))
                    .filter(|id| ef.get(id).is_some_and(|f| f.len() == 2)),
            );
        }
    }
    edges.retain(|e| work.edge(*e).is_some());
    if !changed || edges.is_empty() {
        return None;
    }
    let got = round_edges(&mut work, &edges, size, fillet);
    let faces = match got {
        Ok(f) => f,
        Err(why) => return Some(Err(why)),
    };
    draw_back(&mut work, &lines, true);
    if !crate::check::problems(&work).is_empty() || worse_health(model, &work) {
        return None;
    }
    let seed = faces.iter().find(|f| work.face(**f).is_some())?;
    let out = piece_of(&work, *seed).into_iter().collect();
    *model = work;
    Some(Ok(out))
}

/// Whether `after` has faults Model Health finds that `before` did not:
/// edges with more than two faces, faces inside a solid or turned round.
fn worse_health(before: &Model, after: &Model) -> bool {
    let (a, b) = (crate::health::check(before), crate::health::check(after));
    b.non_manifold.edges.len() > a.non_manifold.edges.len()
        || b.interior.faces.len() > a.interior.faces.len()
        || b.reversed.faces.len() > a.reversed.faces.len()
        || b.leaky > a.leaky
}

/// The faces of the one closed solid all of `edges` are on.
fn solid_faces(model: &Model, edges: &BTreeSet<EdgeId>) -> Result<BTreeSet<FaceId>, &'static str> {
    let first = *edges.first().ok_or(NOTHING)?;
    let edge_faces = model.edge_faces();
    let seed = edge_faces
        .get(&first)
        .and_then(|v| v.first())
        .copied()
        .ok_or(TWO_SOLIDS)?;
    let faces = solid_of(model, seed).map_err(|_| TWO_SOLIDS)?;
    for e in edges {
        let on = edge_faces
            .get(e)
            .is_some_and(|fs| fs.iter().filter(|f| faces.contains(f)).count() == 2);
        if !on {
            return Err(TWO_SOLIDS);
        }
    }
    Ok(faces.into_iter().collect())
}

/// The faces of `faces` with a point at an end of `edges`.
fn end_faces(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    edges: &BTreeSet<EdgeId>,
) -> BTreeSet<FaceId> {
    let ends: BTreeSet<_> = edges
        .iter()
        .filter_map(|e| model.edge(*e))
        .flat_map(|e| [e.a, e.b])
        .collect();
    faces
        .iter()
        .copied()
        .filter(|f| {
            model
                .face(*f)
                .is_some_and(|x| x.loops().flatten().any(|v| ends.contains(v)))
        })
        .collect()
}

/// Straight lines between two flat faces of one plane, facing the same
/// way, one of them at an end of `edges`: drawn on the solid, not part of
/// its shape.
fn drawn_lines(
    model: &Model,
    faces: &BTreeSet<FaceId>,
    edges: &BTreeSet<EdgeId>,
    reach: f64,
) -> BTreeSet<EdgeId> {
    let near = end_faces(model, faces, edges);
    let tol = model.tol();
    // The rounded edges as segments: a line within `reach` of one is in
    // the round's way.
    let spans: Vec<(DVec3, DVec3)> = edges
        .iter()
        .flat_map(|e| {
            let pts = model.edge_points(*e);
            pts.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>()
        })
        .collect();
    let in_way = |a: DVec3, b: DVec3| {
        (0..=16).any(|k| {
            let p = a.lerp(b, f64::from(k) / 16.0);
            spans
                .iter()
                .any(|(c, d)| distance_to_segment(p, *c, *d) <= reach + tol.dist)
        })
    };
    model
        .edge_faces()
        .into_iter()
        .filter(|(e, fs)| {
            let [f, g] = fs.as_slice() else { return false };
            if edges.contains(e) || !(near.contains(f) || near.contains(g)) {
                return false;
            }
            let straight = model.edge(*e).is_some_and(|x| {
                !x.curve.is_curved() && in_way(model.vertex(x.a), model.vertex(x.b))
            });
            match (
                model.face(*f).and_then(|x| x.plane()),
                model.face(*g).and_then(|x| x.plane()),
            ) {
                (Some(p), Some(q)) => {
                    straight
                        && faces.contains(f)
                        && faces.contains(g)
                        && p.normal.dot(q.normal) > 0.0
                        && p.same_surface(q, &tol, model.face_extent(model.face(*g).unwrap()))
                }
                _ => false,
            }
        })
        .map(|(e, _)| e)
        .collect()
}

/// How far from the rounded edges a round reaches into the faces beside
/// them, with room to spare: a fillet's setback is its radius over the
/// tangent of half the turn between the faces, a chamfer's its size.
fn reach(model: &Model, edges: &BTreeSet<EdgeId>, size: f64, fillet: bool) -> f64 {
    let edge_faces = model.edge_faces();
    let mut most = size;
    for e in edges {
        let Some(x) = model.edge(*e) else { continue };
        let mid = middle(model, x);
        let Some([f, g]) = edge_faces.get(e).map(Vec::as_slice) else {
            continue;
        };
        let (Some(a), Some(b)) = (model.face(*f), model.face(*g)) else {
            continue;
        };
        let turn = a
            .surface
            .normal_at(mid)
            .dot(b.surface.normal_at(mid))
            .clamp(-1.0, 1.0)
            .acos();
        let back = if fillet && turn > 1e-6 {
            size / (turn / 2.0).tan().max(1e-6)
        } else {
            size
        };
        most = most.max(back);
    }
    most * 1.5
}

fn distance_to_segment(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// [`round_edges`] as the kernel takes it, with nothing done first.
fn round_plain(
    model: &mut Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Result<Vec<FaceId>, &'static str> {
    let first = *edges.first().ok_or(NOTHING)?;
    if !(size.is_finite() && size > model.tol().dist) {
        return Err(NO_SIZE);
    }
    let edge_faces = model.edge_faces();
    let seed = edge_faces
        .get(&first)
        .and_then(|v| v.first())
        .copied()
        .ok_or(TWO_SOLIDS)?;
    let faces = solid_of(model, seed).map_err(|_| TWO_SOLIDS)?;
    for e in edges {
        let on = edge_faces
            .get(e)
            .is_some_and(|fs| fs.iter().filter(|f| faces.contains(f)).count() == 2);
        if !on {
            return Err(TWO_SOLIDS);
        }
    }
    let r = region(model, &faces)?;
    let mut s = Solid::build(&r, model.tol().dist)?;
    let keys: Vec<u64> = edges.iter().map(|e| u64::from(e.0)).collect();
    s.round(&keys, size, fillet)?;
    let back = s.region()?;
    let mut work = model.clone();
    let before: BTreeSet<EdgeId> = work.edges().map(|(id, _)| id).collect();
    let out = put_back(&mut work, &faces, &back)?;
    soften_tangent(&mut work, &before);
    if !crate::check::problems(&work).is_empty() {
        return Err(prism_kernel::solid::CANNOT_STORE);
    }
    *model = work;
    Ok(out)
}

/// Join, on the faces at the ends of `edges` (of the solid `faces`), each
/// pair of straight edges that meet in line at a point nothing else meets
/// and lie between the same two faces: the point is only where a line was
/// once split, but to the kernel it is a corner, and a round may not run
/// past a corner (owner's session 22: a round stopped at 0.028 m, the
/// height of such a point below the edge, where 0.2 m fits). Edges being
/// rounded are left. Returns how many points went.
fn join_straight(
    model: &mut Model,
    faces: &BTreeSet<FaceId>,
    edges: &mut BTreeSet<EdgeId>,
) -> usize {
    let ends: BTreeSet<_> = edges
        .iter()
        .filter_map(|e| model.edge(*e))
        .flat_map(|e| [e.a, e.b])
        .collect();
    // The points on the faces at those ends.
    let near = end_faces(model, faces, edges);
    let points: BTreeSet<_> = near
        .iter()
        .filter_map(|f| model.face(*f))
        .flat_map(|f| f.loops().flatten().copied().collect::<Vec<_>>())
        .collect();
    let tol = model.tol();
    let mut done = 0;
    for v in points {
        let at: Vec<EdgeId> = model
            .edges()
            .filter(|(_, e)| e.a == v || e.b == v)
            .map(|(id, _)| id)
            .collect();
        let [e1, e2] = at[..] else { continue };
        // Two pieces of one rounded line join into one rounded edge; a
        // rounded edge and one that is not stay apart (the round ends
        // there).
        let rounded = (edges.contains(&e1), edges.contains(&e2));
        if rounded.0 != rounded.1 || (!rounded.0 && ends.contains(&v)) {
            continue;
        }
        let ef = model.edge_faces();
        let pair = |e: EdgeId| {
            let mut fs = ef.get(&e).cloned().unwrap_or_default();
            fs.sort();
            fs
        };
        let fs = pair(e1);
        if fs.len() != 2 || fs != pair(e2) || !fs.iter().all(|f| faces.contains(f)) {
            continue;
        }
        let (Some(x1), Some(x2)) = (model.edge(e1).cloned(), model.edge(e2).cloned()) else {
            continue;
        };
        if x1.curve.is_curved() || x2.curve.is_curved() {
            continue;
        }
        let a = if x1.a == v { x1.b } else { x1.a };
        let b = if x2.a == v { x2.b } else { x2.a };
        let (pa, pb, pv) = (model.vertex(a), model.vertex(b), model.vertex(v));
        // In line: the point is on the line from a to b, between them.
        let along = pb - pa;
        let t = (pv - pa).dot(along) / along.length_squared().max(f64::MIN_POSITIVE);
        if !(0.0..=1.0).contains(&t) || !tol.same_point(pa + along * t, pv) {
            continue;
        }
        let Some(joined) = model.add_edge(a, b) else {
            continue;
        };
        model.set_soft(joined, x1.soft, x1.smooth);
        for f in &fs {
            model.edit_face(*f, |face| {
                face.outer.retain(|x| *x != v);
                for h in &mut face.holes {
                    h.retain(|x| *x != v);
                }
            });
        }
        model.remove_edge(e1);
        model.remove_edge(e2);
        model.remove_vertex(v);
        if rounded.0 {
            edges.remove(&e1);
            edges.remove(&e2);
            edges.insert(joined);
        }
        done += 1;
    }
    done
}

/// Why rounding `edges` would be refused, if it would.
pub fn refusal(
    model: &Model,
    edges: &BTreeSet<EdgeId>,
    size: f64,
    fillet: bool,
) -> Option<&'static str> {
    let mut m = model.clone();
    round_edges(&mut m, edges, size, fillet).err()
}

/// The new edges (not in `before`) between two faces that are tangent
/// there are soft and smooth: the line between a round and its faces.
fn soften_tangent(model: &mut Model, before: &BTreeSet<EdgeId>) {
    let edge_faces = model.edge_faces();
    let mut soft = Vec::new();
    for (id, e) in model.edges() {
        if before.contains(&id) {
            continue;
        }
        let Some([f, g]) = edge_faces.get(&id).map(Vec::as_slice) else {
            continue;
        };
        let mid = middle(model, e);
        let (Some(a), Some(b)) = (model.face(*f), model.face(*g)) else {
            continue;
        };
        let (na, nb) = (a.surface.normal_at(mid), b.surface.normal_at(mid));
        if na.dot(nb) > 1.0 - 1e-6 {
            soft.push(id);
        }
    }
    for id in soft {
        model.set_soft(id, true, true);
    }
}

/// A point halfway along an edge.
fn middle(model: &Model, e: &prism_topo::Edge) -> DVec3 {
    let (a, b) = (model.vertex(e.a), model.vertex(e.b));
    match &e.curve {
        prism_geom::Curve::Line => (a + b) * 0.5,
        prism_geom::Curve::Arc { centre, axis, .. } => {
            let (xa, xb) = (a - *centre, b - *centre);
            let mut sweep = axis.dot(xa.cross(xb)).atan2(xa.dot(xb));
            if sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
            *centre + prism_math::DQuat::from_axis_angle(*axis, sweep / 2.0) * xa
        }
        prism_geom::Curve::Spline(sp) => sp.point(sp.halfway()),
    }
}

/// The straight run `edge` is part of: the segments end to end with it,
/// along the same line, each between faces on the same two planes as its
/// own (a line drawn in pieces, rounded as a whole). Just `edge` when it
/// is curved or not between two flat faces.
pub fn straight_run(model: &Model, edge: EdgeId) -> BTreeSet<EdgeId> {
    let mut out = BTreeSet::from([edge]);
    let edge_faces = model.edge_faces();
    let planes = |e: EdgeId| -> Option<[prism_math::Plane; 2]> {
        let fs = edge_faces.get(&e)?;
        let [f, g] = fs.as_slice() else { return None };
        Some([*model.face(*f)?.plane()?, *model.face(*g)?.plane()?])
    };
    let tol = model.tol();
    let same_plane = |p: &prism_math::Plane, q: &prism_math::Plane| {
        p.normal.dot(q.normal) > 1.0 - prism_math::tol::ANGLE
            && tol.is_zero((q.origin - p.origin).dot(p.normal))
    };
    let same_pair = |a: &[prism_math::Plane; 2], b: &[prism_math::Plane; 2]| {
        (same_plane(&a[0], &b[0]) && same_plane(&a[1], &b[1]))
            || (same_plane(&a[0], &b[1]) && same_plane(&a[1], &b[0]))
    };
    let Some(e0) = model.edge(edge) else {
        return out;
    };
    let Some(pair) = planes(edge) else {
        return out;
    };
    if !matches!(e0.curve, prism_geom::Curve::Line) {
        return out;
    }
    let (a0, b0) = (model.vertex(e0.a), model.vertex(e0.b));
    let dir = (b0 - a0).normalize_or_zero();
    let on_line = |p: DVec3| {
        let w = p - a0;
        tol.is_zero((w - dir * w.dot(dir)).length())
    };
    for start in [e0.a, e0.b] {
        let (mut v, mut from) = (start, edge);
        loop {
            let next: Vec<EdgeId> = model
                .edges()
                .filter(|(id, x)| {
                    *id != from
                        && !out.contains(id)
                        && x.has(v)
                        && matches!(x.curve, prism_geom::Curve::Line)
                        && on_line(model.vertex(x.a))
                        && on_line(model.vertex(x.b))
                        && planes(*id).is_some_and(|p| same_pair(&p, &pair))
                })
                .map(|(id, _)| id)
                .collect();
            let [n] = next.as_slice() else { break };
            let x = model.edge(*n).expect("an edge");
            out.insert(*n);
            v = if x.a == v { x.b } else { x.a };
            from = *n;
        }
    }
    out
}

/// A size that fits every edge of `edges`: the smallest of the straight
/// edges' own fits (curved edges have none of their own and take it too).
pub fn fit(model: &Model, edges: &BTreeSet<EdgeId>) -> Option<f64> {
    edges
        .iter()
        .filter_map(|e| crate::round::default_size(model, *e))
        .min_by(f64::total_cmp)
}
