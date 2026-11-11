//! The segment fallback (DECISIONS #32, Exact Curves 6a): where an edit
//! would make a curve or surface Prism cannot store exactly, it is replaced
//! by fine straight segments and flat facets marked approximate. Their
//! points are exact; the chords and facets between them are not. Model
//! Health lists them, and NURBS can replace them later.
//!
//! How fine: a chord never strays from its curve by more than 1/1000 of
//! the curve's radius. A curve has never fewer segments than an arc of the
//! same turn is drawn with; a surface's facets follow the chord rule alone.

use std::collections::{BTreeMap, BTreeSet};

use prism_geom::{Curve, Surface, segments};
use prism_math::{DAffine3, DVec2, DVec3, Plane, newell};
use prism_topo::{CurveId, EdgeId, Face, FaceId, Model, VertexId};

/// The largest turn one segment may cover: its chord then strays by at
/// most 1/1000 of the radius (1 - cos(φ/2) = 1/1000).
fn max_step() -> f64 {
    2.0 * (1.0 - 1e-3f64).acos()
}

/// How many segments a curve turning `sweep` radians takes: fine enough
/// for the chord rule, and never fewer than an arc is drawn with.
pub fn fine_count(sweep: f64) -> usize {
    facet_count(sweep).max(segments(sweep.abs()))
}

/// How many facets a surface turning `sweep` radians takes one way: the
/// chord rule alone (in two directions the drawing count would make a
/// sphere thousands of facets).
fn facet_count(sweep: f64) -> usize {
    ((sweep.abs() / max_step()).ceil() as usize).max(1)
}

/// Approximate segments made from one curve share one new curve id.
type Ids = BTreeMap<Option<CurveId>, CurveId>;

fn new_id(model: &mut Model, ids: &mut Ids, old: Option<CurveId>, e: EdgeId) -> CurveId {
    // Edges with no curve id each get their own.
    let key = old.or(Some(CurveId(u32::MAX - e.0)));
    *ids.entry(key).or_insert_with(|| model.new_curve_id())
}

/// Replace arc `e` by fine straight segments along it, marked approximate.
/// Returns the points from the arc's `a` to its `b`.
pub fn segment_arc(model: &mut Model, e: EdgeId, ids: &mut Ids) -> Vec<VertexId> {
    let Some(x) = model.edge(e).copied() else {
        return Vec::new();
    };
    if !x.curve.is_arc() {
        return vec![x.a, x.b];
    }
    let (pa, pb) = (model.vertex(x.a), model.vertex(x.b));
    let n = fine_count(x.curve.sweep(pa, pb));
    let pts: Vec<DVec3> = (1..n)
        .map(|i| x.curve.point(pa, pb, i as f64 / n as f64))
        .collect();
    split_into(model, e, &pts, ids)
}

/// Split edge `e` at `pts` (strictly between its ends, in order from its
/// `a`), making every piece a straight approximate segment. Returns the
/// points from `a` to `b`.
fn split_into(model: &mut Model, e: EdgeId, pts: &[DVec3], ids: &mut Ids) -> Vec<VertexId> {
    let Some(x) = model.edge(e).copied() else {
        return Vec::new();
    };
    let cid = new_id(model, ids, x.curve_id, e);
    let mut chain = vec![x.a];
    let mut rest = e;
    for p in pts {
        let v = model.add_vertex(*p);
        let Some((first, second)) = model.split_edge(rest, v) else {
            break;
        };
        // The rest runs on from `v`, whichever way it was stored.
        let (done, more) = if model
            .edge(first)
            .is_some_and(|y| y.has(*chain.last().unwrap()))
        {
            (first, second)
        } else {
            (second, first)
        };
        mark(model, done, cid);
        chain.push(v);
        rest = more;
    }
    mark(model, rest, cid);
    chain.push(x.b);
    chain
}

fn mark(model: &mut Model, e: EdgeId, cid: CurveId) {
    model.set_curve(e, Curve::Line);
    model.set_approx(e, true);
    model.set_curve_id(e, Some(cid));
}

/// The grid a curved face is faceted on: its (θ, s) rectangle and the
/// steps each way.
struct Grid {
    t: Vec<f64>,
    s: Vec<f64>,
}

/// Replace each arc of `edges` by fine approximate segments, the pieces of
/// one curve sharing one new curve id.
pub fn segment_arcs(model: &mut Model, edges: &[EdgeId]) {
    let mut ids = Ids::new();
    for e in edges {
        segment_arc(model, *e, &mut ids);
    }
}

/// What faceting a face needs, found without changing anything: None if
/// the face is not a rectangle in (θ, s) with grid points on its border.
struct Plan {
    face: Face,
    rev: prism_geom::Revolution,
    grid: Grid,
    sides: Vec<(EdgeId, DVec2, DVec2, bool)>,
    nt: usize,
    ns: usize,
}

fn plan(model: &Model, id: FaceId) -> Option<Plan> {
    let f = model.face(id).cloned()?;
    let rev = f.surface.revolution()?;
    if !f.holes.is_empty() {
        return None;
    }
    let pl = model.param_loop(&f, 0);
    if pl.len() < 2 {
        return None;
    }
    let ang = 1e-9;
    let (mut t0, mut t1, mut s0, mut s1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for (_, q) in &pl {
        t0 = t0.min(q.x);
        t1 = t1.max(q.x);
        s0 = s0.min(q.y);
        s1 = s1.max(q.y);
    }
    if t1 - t0 <= ang || s1 - s0 <= ang * (1.0 + s1.abs().max(s0.abs())) {
        return None;
    }
    let near = |a: f64, b: f64| (a - b).abs() <= ang * (1.0 + a.abs().max(b.abs()));
    // Every side runs along the rectangle's border, square to an axis.
    let n = pl.len();
    for i in 0..n {
        let (a, b) = (pl[i].1, pl[(i + 1) % n].1);
        let along_s = near(a.y, b.y) && (near(a.y, s0) || near(a.y, s1));
        let along_t = near(a.x, b.x) && (near(a.x, t0) || near(a.x, t1));
        if !along_s && !along_t {
            return None;
        }
    }
    let circle = matches!(rev.profile, prism_geom::Profile::Circle { .. });
    let on_grid = |x: f64, g: &[f64]| g.iter().any(|y| near(x, *y));
    let nt = facet_count(t1 - t0);
    let ns = if circle { facet_count(s1 - s0) } else { 1 };
    let grid = Grid {
        t: (0..=nt)
            .map(|i| t0 + (t1 - t0) * i as f64 / nt as f64)
            .collect(),
        s: (0..=ns)
            .map(|j| s0 + (s1 - s0) * j as f64 / ns as f64)
            .collect(),
    };
    // Each vertex's place in the rectangle (angles brought into it); a
    // point on the axis takes the angle of the side it ends.
    let tol = model.tol();
    let turn = std::f64::consts::TAU;
    let into = |x: f64, lo: f64, hi: f64| {
        let mid = (lo + hi) / 2.0;
        x + ((mid - x) / turn).round() * turn
    };
    // A point on the axis takes the angle of the side's middle (a side
    // touching the axis runs along a meridian).
    let place = |model: &Model, v: VertexId, mid: DVec3| -> DVec2 {
        let p = model.vertex(v);
        let (t, s) = rev.param(p);
        let s = if circle { into(s, s0, s1) } else { s };
        let t = if rev.axis_distance(p) <= tol.dist {
            into(rev.param(mid).0, t0, t1)
        } else {
            into(t, t0, t1)
        };
        DVec2::new(t, s)
    };
    let m = f.outer.len();
    let mut sides: Vec<(EdgeId, DVec2, DVec2, bool)> = Vec::with_capacity(m);
    for i in 0..m {
        let (va, vb) = (f.outer[i], f.outer[(i + 1) % m]);
        let e = f.loop_edge_ids(0).get(i).copied()?;
        let x = model.edge(e).copied()?;
        let mid = x.curve.point(model.vertex(x.a), model.vertex(x.b), 0.5);
        let (qa, qb) = (place(model, va, mid), place(model, vb, mid));
        // Points already on the border must be grid points.
        for q in [qa, qb] {
            let ok = ((near(q.y, s0) || near(q.y, s1)) && on_grid(q.x, &grid.t))
                || ((near(q.x, t0) || near(q.x, t1)) && on_grid(q.y, &grid.s));
            if !ok {
                return None;
            }
        }
        sides.push((e, qa, qb, x.a == va));
    }
    Some(Plan {
        face: f,
        rev,
        grid,
        sides,
        nt,
        ns,
    })
}

/// Whether [`facet`] can facet curved face `id` (a face cut by Intersect
/// Faces cannot yet: step 6b-2).
pub fn can_facet(model: &Model, id: FaceId) -> bool {
    plan(model, id).is_some()
}

/// Replace curved face `id` by flat facets on a grid in (θ, s), if its
/// loop is a rectangle there (as every face Push/Pull, Fillet and Follow
/// Me make is). Facets between grid points are exactly flat on every
/// turned surface (two points on each of two circles about the axis, at
/// the same two angles, make an isosceles trapezoid), so they stay flat
/// under any affine map. The lines between facets are soft, smooth and
/// approximate; the face's own edges are split into approximate segments.
/// False if the face is not such a rectangle (it is left as it is).
pub fn facet(model: &mut Model, id: FaceId, ids: &mut Ids) -> bool {
    let Some(Plan {
        face: f,
        rev,
        grid,
        sides,
        nt,
        ns,
    }) = plan(model, id)
    else {
        return false;
    };
    let ang = 1e-9;
    let near = |a: f64, b: f64| (a - b).abs() <= ang * (1.0 + a.abs().max(b.abs()));
    let tol = model.tol();
    // Split each side at the grid points inside it.
    for (e, qa, qb, forward) in sides {
        let Some(x) = model.edge(e).copied() else {
            continue;
        };
        let flat_s = near(qa.y, qb.y);
        let (along, from, to) = if flat_s {
            (&grid.t, qa.x, qb.x)
        } else {
            (&grid.s, qa.y, qb.y)
        };
        let (lo, hi) = (from.min(to), from.max(to));
        let mut inside: Vec<f64> = along
            .iter()
            .copied()
            .filter(|g| *g > lo + ang && *g < hi - ang)
            .collect();
        // In the edge's stored direction.
        if (from > to) == forward {
            inside.reverse();
        }
        let pts: Vec<DVec3> = inside
            .iter()
            .map(|g| {
                if flat_s {
                    rev.point(*g, qa.y)
                } else {
                    rev.point(qa.x, *g)
                }
            })
            .collect();
        if pts.is_empty() && !x.curve.is_arc() {
            continue;
        }
        split_into(model, e, &pts, ids);
    }
    // Grid vertices: the border's by place (a pole is one vertex), the
    // inside new.
    let border: Vec<VertexId> = {
        let Some(g) = model.face(id) else {
            return false;
        };
        g.outer.clone()
    };
    let find = |model: &Model, p: DVec3| -> Option<VertexId> {
        border
            .iter()
            .copied()
            .find(|v| tol.same_point(model.vertex(*v), p))
    };
    let mut gv: Vec<Vec<VertexId>> = Vec::with_capacity(nt + 1);
    for i in 0..=nt {
        let mut col = Vec::with_capacity(ns + 1);
        for j in 0..=ns {
            let p = rev.point(grid.t[i], grid.s[j]);
            let edge = i == 0 || i == nt || j == 0 || j == ns;
            let v = if edge || rev.axis_distance(p) <= tol.dist {
                match find(model, p) {
                    Some(v) => v,
                    None if edge => return false,
                    None => model.add_vertex(p),
                }
            } else {
                model.add_vertex(p)
            };
            col.push(v);
        }
        gv.push(col);
    }
    // Lines between facets: soft, smooth, approximate. The border's are
    // there already; the rest are new (kept in a table: looking each up
    // in the model is slow).
    let mut made: BTreeMap<(VertexId, VertexId), EdgeId> = BTreeMap::new();
    if let Some(g) = model.face(id) {
        let n = g.outer.len();
        for (k, e) in g.loop_edge_ids(0).iter().enumerate() {
            let (a, b) = (g.outer[k], g.outer[(k + 1) % n]);
            made.insert((a.min(b), a.max(b)), *e);
        }
    }
    let mut line = |model: &mut Model, a: VertexId, b: VertexId| -> Option<EdgeId> {
        if a == b {
            return None;
        }
        let key = (a.min(b), a.max(b));
        if let Some(e) = made.get(&key) {
            return Some(*e);
        }
        let e = model.add_new_edge(a, b);
        model.set_soft(e, true, true);
        model.set_approx(e, true);
        made.insert(key, e);
        Some(e)
    };
    let surface = f.surface;
    let out = if surface.outward() { 1.0 } else { -1.0 };
    let mut facets = Vec::new();
    for i in 0..nt {
        for j in 0..ns {
            // Counter-clockwise in (θ, s): outward on the surface.
            let mut l = vec![gv[i][j], gv[i + 1][j], gv[i + 1][j + 1], gv[i][j + 1]];
            l.dedup();
            if l.first() == l.last() && l.len() > 1 {
                l.pop();
            }
            if l.len() < 3 {
                continue;
            }
            let mut es = Vec::with_capacity(l.len());
            for k in 0..l.len() {
                let Some(e) = line(model, l[k], l[(k + 1) % l.len()]) else {
                    return false;
                };
                es.push(e);
            }
            let pts: Vec<DVec3> = l.iter().map(|v| model.vertex(*v)).collect();
            let want = rev.normal(
                (grid.t[i] + grid.t[i + 1]) / 2.0,
                (grid.s[j] + grid.s[j + 1]) / 2.0,
            ) * out;
            let nn = newell(&pts).normalize_or_zero();
            let c = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
            let mut face = Face::with_edges(l, vec![], Surface::Plane(Plane::new(c, nn)), vec![es]);
            if nn.dot(want) < 0.0 {
                face.reverse();
            }
            facets.push(face);
        }
    }
    model.remove_face(id);
    for face in facets {
        model.add_face(face);
    }
    true
}

/// Before `m` (an uneven scale) moves `vs`: facet every curved face it
/// moves whole and segment every arc it moves whole, since neither would
/// stay a cylinder, cone, sphere, torus or circle. Returns `vs` with the
/// new points.
pub fn flatten_for(model: &mut Model, vs: &BTreeSet<VertexId>, m: &DAffine3) -> BTreeSet<VertexId> {
    let before: BTreeSet<VertexId> = model.vertices().map(|(id, _)| id).collect();
    let mut ids = Ids::new();
    let faces: Vec<FaceId> = model
        .faces()
        .filter(|(_, f)| {
            f.is_curved()
                && f.surface.transformed(m).is_none()
                && f.loops().flatten().all(|v| vs.contains(v))
        })
        .map(|(id, _)| id)
        .collect();
    for id in faces {
        facet(model, id, &mut ids);
    }
    let arcs: Vec<EdgeId> = model
        .edges()
        .filter(|(_, e)| {
            e.curve.is_arc()
                && e.curve.transformed(m).is_none()
                && vs.contains(&e.a)
                && vs.contains(&e.b)
        })
        .map(|(id, _)| id)
        .collect();
    for e in arcs {
        segment_arc(model, e, &mut ids);
    }
    let mut out = vs.clone();
    out.extend(
        model
            .vertices()
            .map(|(id, _)| id)
            .filter(|v| !before.contains(v)),
    );
    out
}
