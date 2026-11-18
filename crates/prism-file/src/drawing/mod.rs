//! DXF and SVG (Solids & CAD part 6, brief 45): flat drawings for routers,
//! lasers and CNC programs.
//!
//! The model is turned into a [`Drawing`]: named parts, each a list of
//! paths of lines, conics (arcs, circles and ellipses) and splines, in the
//! file's unit on a flat sheet. Curves stay exact: every view is a flat
//! (affine) map, so a circle maps to a circle or an ellipse and a spline
//! to the spline of its mapped control points. The DXF and SVG writers
//! then only spell the drawing out.

pub mod dxf;
pub mod svg;

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{PI, TAU};

use prism_doc::{Assembly, DefId, InstanceId, ROOT};
use prism_geom::{Curve, Nurbs};
use prism_math::{DAffine2, DAffine3, DMat3, DVec2, DVec3, LengthUnit};
use prism_ops::validate::validate;
use prism_topo::{EdgeId, FaceId, Model, VertexId};

/// Said when nothing could go into the drawing.
pub const NOTHING: &str = "Nothing to export: no flat face or edge to draw.";
/// Said when the Section view has no plane to cut with.
pub const NO_SECTION: &str =
    "No section plane is cutting: place one with the Section Plane tool first.";
/// Said when the cutting plane meets nothing.
pub const SECTION_MISSES: &str = "The cutting section plane does not meet the model.";
/// Said for a curved face chosen to be laid flat.
pub const CURVED: &str = "a curved face cannot be laid flat";
/// Said for a piece of a closed spline the drawing cannot cut out.
pub const PERIODIC: &str = "a piece of a closed spline";

/// How the model is seen on the sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// Each part's flat face laid on the sheet, outside up, in a row.
    #[default]
    Flat,
    /// Seen from above (x right, y up the sheet).
    Top,
    /// Seen from the front (x right, z up the sheet).
    Front,
    /// Seen from the right side (y right, z up the sheet).
    Side,
    /// Where the active section plane cuts the model, as a viewer on the
    /// cut-away side sees it: the exact lines, arcs and curves (Annotation
    /// & Sections part 5).
    Section,
}

impl View {
    pub const ALL: [View; 5] = [
        View::Flat,
        View::Top,
        View::Front,
        View::Side,
        View::Section,
    ];

    pub fn name(self) -> &'static str {
        match self {
            View::Flat => "Laid flat",
            View::Top => "Top",
            View::Front => "Front",
            View::Side => "Side",
            View::Section => "Section",
        }
    }

    /// The view by its MCP name ("flat", "top", "front", "side").
    pub fn of(name: &str) -> Option<View> {
        View::ALL
            .into_iter()
            .find(|v| v.name().eq_ignore_ascii_case(name) || (*v == View::Flat && name == "flat"))
    }

    /// From the world to the sheet (x, y), depth in z, as rows.
    fn axes(self) -> DMat3 {
        match self {
            View::Flat | View::Top | View::Section => DMat3::IDENTITY,
            View::Front => DMat3::from_cols(DVec3::X, DVec3::Z, -DVec3::Y).transpose(),
            View::Side => DMat3::from_cols(DVec3::Y, DVec3::Z, DVec3::X).transpose(),
        }
    }
}

/// One piece of a path on the sheet.
#[derive(Clone, Debug, PartialEq)]
pub enum Seg {
    Line(DVec2, DVec2),
    /// The points `c + u cos t + v sin t` for t from 0 to `sweep` (above
    /// 0): an arc of a circle when `u` and `v` are square and as long, of
    /// an ellipse otherwise.
    Conic {
        c: DVec2,
        u: DVec2,
        v: DVec2,
        sweep: f64,
    },
    /// A NURBS curve on the sheet (z is 0), walked from its start.
    Spline(Nurbs),
}

/// Pieces joined end to end.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub segs: Vec<Seg>,
    pub closed: bool,
}

/// A named part of the drawing (a DXF layer, an SVG group).
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    pub paths: Vec<Path>,
}

/// A drawing on a flat sheet, in `unit`. `eps` is the model's tolerance
/// in that unit (what counts as the same point, or a circle).
#[derive(Clone, Debug, PartialEq)]
pub struct Drawing {
    pub parts: Vec<Part>,
    pub unit: LengthUnit,
    pub eps: f64,
}

/// What is chosen: faces, edges and groups inside the group at `path`
/// (the one open for editing; empty for the top). All empty: everything
/// shown.
#[derive(Clone, Copy, Debug)]
pub struct Scope<'a> {
    pub path: &'a [InstanceId],
    pub faces: &'a BTreeSet<FaceId>,
    pub edges: &'a BTreeSet<EdgeId>,
    pub groups: &'a BTreeSet<InstanceId>,
}

impl Scope<'_> {
    fn is_empty(&self) -> bool {
        self.faces.is_empty() && self.edges.is_empty() && self.groups.is_empty()
    }
}

/// The unit a drawing is written in (feet and inches write inches).
pub fn file_unit(u: LengthUnit) -> LengthUnit {
    match u {
        LengthUnit::FeetInches => LengthUnit::Inches,
        u => u,
    }
}

/// Metres in one `unit`.
pub fn metres(unit: LengthUnit) -> f64 {
    match unit {
        LengthUnit::Millimetres => 1e-3,
        LengthUnit::Centimetres => 1e-2,
        LengthUnit::Metres => 1.0,
        LengthUnit::Inches | LengthUnit::FeetInches => 0.0254,
    }
}

/// The parts laid out apart on the sheet, in metres.
const GAP: f64 = 0.01;

/// The drawing of what `scope` chooses in `asm`, seen as `view`, in
/// `unit`; and why anything was left out.
pub fn make(
    asm: &Assembly,
    scope: Scope,
    view: View,
    unit: LengthUnit,
) -> Result<(Drawing, Vec<&'static str>), &'static str> {
    let unit = file_unit(unit);
    let k = 1.0 / metres(unit);
    let tol = asm.def(ROOT).map_or(1e-6, |d| d.model.tol().dist);
    let mut w = Walk {
        asm,
        k,
        eps: tol * k,
        left_out: Vec::new(),
        loose: 0,
    };
    let parts = match view {
        View::Flat => w.flat(scope),
        View::Section => w.sectioned(scope).ok_or(NO_SECTION)?,
        _ => w.seen(scope, view),
    };
    let parts: Vec<Part> = parts.into_iter().filter(|p| !p.paths.is_empty()).collect();
    if parts.is_empty() {
        return Err(if view == View::Section {
            SECTION_MISSES
        } else {
            NOTHING
        });
    }
    Ok((
        Drawing {
            parts,
            unit,
            eps: tol * k,
        },
        w.left_out,
    ))
}

struct Walk<'a> {
    asm: &'a Assembly,
    /// File units in a metre.
    k: f64,
    eps: f64,
    left_out: Vec<&'static str>,
    /// Loose bodies named so far ("Body 1", "Body 2").
    loose: usize,
}

/// A model and where it stands in the world, with the name of the part
/// its drawing goes in.
struct Placed<'a> {
    model: &'a Model,
    world: DAffine3,
    name: String,
}

impl<'a> Walk<'a> {
    fn name(&self, id: InstanceId, def: DefId) -> String {
        self.asm
            .def(def)
            .and_then(|d| d.name.clone())
            .or_else(|| self.asm.name_of(id).map(str::to_owned))
            .unwrap_or_else(|| "Group".to_owned())
    }

    /// The group open for editing: its definition and where it stands.
    fn open(&self, path: &[InstanceId]) -> (DefId, DAffine3) {
        let mut def = ROOT;
        for id in path {
            match self.asm.def(def).and_then(|d| d.instances.get(id)) {
                Some(i) => def = i.def,
                None => return (ROOT, DAffine3::IDENTITY),
            }
        }
        (def, self.asm.world_of(path).unwrap_or(DAffine3::IDENTITY))
    }

    /// `def`'s own model and every shown group inside it, moved by `at`.
    fn placed(&self, def: DefId, at: DAffine3, name: String, out: &mut Vec<Placed<'a>>) {
        let Some(d) = self.asm.def(def) else { return };
        out.push(Placed {
            model: &d.model,
            world: at,
            name,
        });
        for (id, inst) in &d.instances {
            if self.asm.is_shown(*id) {
                self.placed(inst.def, at * inst.transform, self.name(*id, inst.def), out);
            }
        }
    }

    /// The chosen groups (or all shown ones) inside the open group.
    fn groups(&self, scope: Scope, def: DefId, at: DAffine3) -> Vec<Placed<'a>> {
        let mut out = Vec::new();
        let Some(d) = self.asm.def(def) else {
            return out;
        };
        for (id, inst) in &d.instances {
            let chosen = if scope.is_empty() {
                self.asm.is_shown(*id)
            } else {
                scope.groups.contains(id)
            };
            if chosen {
                self.placed(
                    inst.def,
                    at * inst.transform,
                    self.name(*id, inst.def),
                    &mut out,
                );
            }
        }
        out
    }

    // ---- Laid flat ----

    fn flat(&mut self, scope: Scope) -> Vec<Part> {
        let (def, at) = self.open(scope.path);
        let mut faces: Vec<(String, &Model, DAffine3, FaceId)> = Vec::new();
        if let Some(d) = self.asm.def(def) {
            let all = scope.is_empty();
            for c in validate(&d.model) {
                let chosen: Vec<FaceId> = if all {
                    c.faces.iter().copied().collect()
                } else {
                    c.faces.intersection(scope.faces).copied().collect()
                };
                if chosen.is_empty() {
                    continue;
                }
                let whole = chosen.len() == c.faces.len();
                let name = match scope.path.last() {
                    None => {
                        self.loose += 1;
                        format!("Body {}", self.loose)
                    }
                    Some(id) => self.name(*id, def),
                };
                self.chosen_faces(&d.model, &chosen, whole, &name, at, &mut faces);
            }
        }
        for p in self.groups(scope, def, at) {
            for c in validate(p.model) {
                let chosen: Vec<FaceId> = c.faces.iter().copied().collect();
                self.chosen_faces(p.model, &chosen, true, &p.name, p.world, &mut faces);
            }
        }
        // Each face laid on the sheet, then set out in a row.
        let mut parts: Vec<Part> = Vec::new();
        let mut x = 0.0;
        for (name, model, world, f) in faces {
            let Some(map) = lay_flat(model, f, &world) else {
                continue;
            };
            let map = DAffine3::from_scale(DVec3::splat(self.k)) * map;
            let mut paths = Vec::new();
            let Some(face) = model.face(f) else { continue };
            for l in 0..=face.holes.len() {
                let sides: Vec<(EdgeId, VertexId)> =
                    face.loop_edges(l).map(|s| (s.edge, s.from)).collect();
                if let Some(p) = self.path(model, &sides, true, &map) {
                    paths.push(p);
                }
            }
            let Some((lo, hi)) = bounds(&paths) else {
                continue;
            };
            let shift = DVec2::new(x - lo.x, -lo.y);
            for p in &mut paths {
                p.moved(shift);
            }
            x += hi.x - lo.x + GAP * self.k;
            match parts.iter_mut().find(|p| p.name == name) {
                Some(p) => p.paths.extend(paths),
                None => parts.push(Part { name, paths }),
            }
        }
        parts
    }

    /// The faces of one body to lay flat: chosen whole, its largest flat
    /// face (facing up most, on a tie); otherwise each chosen flat face.
    fn chosen_faces<'m>(
        &mut self,
        model: &'m Model,
        chosen: &[FaceId],
        whole: bool,
        name: &str,
        world: DAffine3,
        out: &mut Vec<(String, &'m Model, DAffine3, FaceId)>,
    ) {
        let flat: Vec<FaceId> = chosen
            .iter()
            .copied()
            .filter(|f| model.face(*f).is_some_and(|x| x.plane().is_some()))
            .collect();
        if whole {
            let up = |f: FaceId| {
                model.face(f).and_then(|x| x.plane()).map_or(0.0, |p| {
                    world.transform_vector3(p.normal).normalize_or_zero().z
                })
            };
            let area = |f: FaceId| model.face_area(f);
            let best = flat.iter().copied().reduce(|a, b| {
                let (aa, ab) = (area(a), area(b));
                let same = (aa - ab).abs() <= 1e-9 * aa.max(ab);
                if (same && up(b) > up(a) + 1e-9) || (!same && ab > aa) {
                    b
                } else {
                    a
                }
            });
            match best {
                Some(f) => out.push((name.to_owned(), model, world, f)),
                None => self.left_out.push(CURVED),
            }
        } else {
            for _ in flat.len()..chosen.len() {
                self.left_out.push(CURVED);
            }
            for f in flat {
                out.push((name.to_owned(), model, world, f));
            }
        }
    }

    // ---- Cut by the section plane ----

    /// The lines, arcs and curves where the active section plane cuts each
    /// shown model, as a part each. None if no plane cuts.
    fn sectioned(&mut self, scope: Scope) -> Option<Vec<Part>> {
        let (_, sec) = self.asm.active_section()?;
        let plane = prism_view::section::CutPlane {
            origin: sec.origin,
            normal: sec.normal,
        };
        let (u, v) = plane.frame();
        // World to the sheet: x along u, y along v, as seen from the side
        // that is cut away.
        let to_sheet = DAffine3::from_scale(DVec3::splat(self.k))
            * DAffine3::from_mat3(DMat3::from_cols(u, v, plane.normal).transpose())
            * DAffine3::from_translation(-plane.origin);
        let (def, at) = self.open(scope.path);
        let mut models: Vec<Placed> = Vec::new();
        if let Some(d) = self.asm.def(def) {
            models.push(Placed {
                model: &d.model,
                world: at,
                name: match scope.path.last() {
                    None => "Model".to_owned(),
                    Some(id) => self.name(*id, def),
                },
            });
        }
        models.extend(self.groups(scope, def, at));
        let mut parts: Vec<Part> = Vec::new();
        for p in models {
            // The plane in the model's own coordinates.
            let inv = p.world.inverse();
            let local = prism_math::Plane::new(
                inv.transform_point3(plane.origin),
                inv.matrix3
                    .transpose()
                    .mul_vec3(plane.normal)
                    .normalize_or_zero(),
            );
            if local.normal == DVec3::ZERO {
                continue;
            }
            let Some((lo, hi)) = p.model.bbox() else {
                continue;
            };
            let mut m = p.model.clone();
            let big =
                (hi - lo).length().max(1e-9) * 2.0 + (local.origin - (lo + hi) * 0.5).length();
            let (lu, lv) = prism_infer_frame(local.normal);
            let centre = local.origin;
            let mut sheet = Model::new();
            prism_ops::add_polygon(
                &mut sheet,
                &[(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                    .map(|(a, b)| centre + lu * (big * a) + lv * (big * b)),
            );
            prism_ops::intersect::intersect_with(&mut m, &sheet);
            let tol = m.tol().dist;
            let edge_faces = m.edge_faces();
            let in_plane = |q: DVec3| local.distance(q).abs() <= tol * 10.0;
            let edges: BTreeSet<EdgeId> = m
                .edges()
                .filter(|(id, e)| {
                    let (a, b) = (m.vertex(e.a), m.vertex(e.b));
                    in_plane(a)
                        && in_plane(b)
                        && in_plane(e.curve.point(a, b, 0.5))
                        // The outline of the cut: used by a face that is not
                        // in the plane itself.
                        && edge_faces.get(id).is_some_and(|fs| {
                            fs.iter().any(|f| {
                                m.face(*f)
                                    .and_then(|x| x.plane())
                                    .is_none_or(|q| q.normal.dot(local.normal).abs() < 0.999_999)
                            })
                        })
                })
                .map(|(id, _)| id)
                .collect();
            let paths = self.chains(&m, &edges, &(to_sheet * p.world));
            match parts.iter_mut().find(|x| x.name == p.name) {
                Some(x) => x.paths.extend(paths),
                None => parts.push(Part {
                    name: p.name,
                    paths,
                }),
            }
        }
        Some(parts)
    }

    // ---- Seen from a side ----

    fn seen(&mut self, scope: Scope, view: View) -> Vec<Part> {
        let (def, at) = self.open(scope.path);
        let to_sheet =
            DAffine3::from_scale(DVec3::splat(self.k)) * DAffine3::from_mat3(view.axes());
        let mut parts: Vec<Part> = Vec::new();
        let add = |parts: &mut Vec<Part>, name: String, paths: Vec<Path>| match parts
            .iter_mut()
            .find(|p| p.name == name)
        {
            Some(p) => p.paths.extend(paths),
            None => parts.push(Part { name, paths }),
        };
        if let Some(d) = self.asm.def(def) {
            let m = &d.model;
            let edges: BTreeSet<EdgeId> = if scope.is_empty() {
                m.edges()
                    .filter(|(_, e)| !e.soft)
                    .map(|(id, _)| id)
                    .collect()
            } else {
                let mut s: BTreeSet<EdgeId> = scope.edges.clone();
                for f in scope.faces {
                    if let Some(face) = m.face(*f) {
                        for l in 0..=face.holes.len() {
                            s.extend(face.loop_edges(l).map(|x| x.edge));
                        }
                    }
                }
                s.into_iter()
                    .filter(|e| m.edge(*e).is_some_and(|x| !x.soft))
                    .collect()
            };
            let name = match scope.path.last() {
                None => "Model".to_owned(),
                Some(id) => self.name(*id, def),
            };
            let paths = self.chains(m, &edges, &(to_sheet * at));
            add(&mut parts, name, paths);
        }
        for p in self.groups(scope, def, at) {
            let edges: BTreeSet<EdgeId> = p
                .model
                .edges()
                .filter(|(_, e)| !e.soft)
                .map(|(id, _)| id)
                .collect();
            let paths = self.chains(p.model, &edges, &(to_sheet * p.world));
            add(&mut parts, p.name, paths);
        }
        parts
    }

    /// `edges` joined into chains where exactly two of them meet.
    fn chains(&mut self, m: &Model, edges: &BTreeSet<EdgeId>, map: &DAffine3) -> Vec<Path> {
        let mut at: BTreeMap<VertexId, Vec<EdgeId>> = BTreeMap::new();
        for e in edges {
            if let Some(x) = m.edge(*e) {
                at.entry(x.a).or_default().push(*e);
                at.entry(x.b).or_default().push(*e);
            }
        }
        let next = |v: VertexId, from: EdgeId, used: &BTreeSet<EdgeId>| -> Option<EdgeId> {
            let here = at.get(&v)?;
            (here.len() == 2)
                .then(|| here.iter().copied().find(|e| *e != from))
                .flatten()
                .filter(|e| !used.contains(e))
        };
        let mut used = BTreeSet::new();
        let mut out = Vec::new();
        for e in edges {
            if used.contains(e) {
                continue;
            }
            let Some(x) = m.edge(*e) else { continue };
            used.insert(*e);
            // Back from its start, then on from its end.
            let mut back = Vec::new();
            let (mut v, mut from) = (x.a, *e);
            while let Some(n) = next(v, from, &used) {
                used.insert(n);
                let o = m.edge(n).map_or(v, |y| y.other(v));
                back.push((n, o));
                v = o;
                from = n;
            }
            let mut sides: Vec<(EdgeId, VertexId)> = back.into_iter().rev().collect();
            sides.push((*e, x.a));
            let (mut v, mut from) = (x.b, *e);
            while let Some(n) = next(v, from, &used) {
                used.insert(n);
                sides.push((n, v));
                v = m.edge(n).map_or(v, |y| y.other(v));
                from = n;
            }
            let first = sides[0].1;
            let last = sides
                .last()
                .and_then(|(e, v)| m.edge(*e).map(|y| y.other(*v)));
            let closed = last == Some(first) && sides.len() > 1 || (sides.len() == 1 && x.a == x.b);
            if let Some(p) = self.path(m, &sides, closed, map) {
                out.push(p);
            }
        }
        out
    }

    /// Edges walked from the given vertices, mapped to the sheet.
    fn path(
        &mut self,
        m: &Model,
        sides: &[(EdgeId, VertexId)],
        closed: bool,
        map: &DAffine3,
    ) -> Option<Path> {
        let mut segs = Vec::new();
        for (e, from) in sides {
            let Some(x) = m.edge(*e) else { continue };
            let (a, b) = (m.vertex(*from), m.vertex(x.other(*from)));
            let flat = |p: DVec3| map.transform_point3(p).truncate();
            match x.curve_from(*from) {
                Curve::Line => {
                    if flat(a).distance(flat(b)) > self.eps {
                        segs.push(Seg::Line(flat(a), flat(b)));
                    }
                }
                c @ Curve::Arc { centre, axis, .. } => {
                    let u = a - centre;
                    let v = axis.cross(u);
                    let to = |d: DVec3| map.transform_vector3(d).truncate();
                    segs.extend(conic(flat(centre), to(u), to(v), c.sweep(a, b), self.eps));
                }
                Curve::Spline(_) => match prism_ops::sides::piece(m, *e, *from) {
                    Ok(n) => {
                        let mut n = n.transformed(map);
                        for i in 0..n.points().len() {
                            let p = n.points()[i];
                            n.set_control(i, p.with_z(0.0));
                        }
                        segs.push(Seg::Spline(n));
                    }
                    Err(_) => self.left_out.push(PERIODIC),
                },
            }
        }
        (!segs.is_empty()).then_some(Path { segs, closed })
    }
}

/// Two unit vectors that, with `n`, make a frame.
fn prism_infer_frame(n: DVec3) -> (DVec3, DVec3) {
    let u = n.any_orthonormal_vector();
    (u, n.cross(u))
}

/// A conic on the sheet; seen edge on (flat), the lines it folds into.
fn conic(c: DVec2, u: DVec2, v: DVec2, sweep: f64, eps: f64) -> Vec<Seg> {
    if u.perp_dot(v).abs() > eps * u.length().max(v.length()) {
        return vec![Seg::Conic { c, u, v, sweep }];
    }
    let at = |t: f64| c + u * t.cos() + v * t.sin();
    // Where it turns back: the ends of its (now flat) major axis.
    let t0 = 0.5 * (2.0 * u.dot(v)).atan2(u.dot(u) - v.dot(v));
    let mut ts = vec![0.0, sweep];
    for n in -2..=4 {
        let t = t0 + f64::from(n) * PI;
        if t > 0.0 && t < sweep {
            ts.push(t);
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.windows(2)
        .map(|w| (at(w[0]), at(w[1])))
        .filter(|(a, b)| a.distance(*b) > eps)
        .map(|(a, b)| Seg::Line(a, b))
        .collect()
}

/// From the world to a sheet lying on face `f`'s plane, its front up,
/// x along its longest straight outer edge.
fn lay_flat(model: &Model, f: FaceId, world: &DAffine3) -> Option<DAffine3> {
    let face = model.face(f)?;
    let plane = face.plane()?;
    let n = world
        .matrix3
        .inverse()
        .transpose()
        .mul_vec3(plane.normal)
        .try_normalize()?;
    let o = world.transform_point3(plane.origin);
    let longest = face
        .loop_edges(0)
        .filter(|s| {
            model
                .edge(s.edge)
                .is_some_and(|e| matches!(e.curve, Curve::Line))
        })
        .map(|s| {
            world.transform_point3(model.vertex(s.to))
                - world.transform_point3(model.vertex(s.from))
        })
        .reduce(|a, b| {
            if b.length() > a.length() + 1e-12 {
                b
            } else {
                a
            }
        });
    let x = longest
        .map(|d| d - n * d.dot(n))
        .and_then(DVec3::try_normalize)
        .unwrap_or_else(|| n.any_orthonormal_vector());
    let y = n.cross(x);
    let rows = DMat3::from_cols(x, y, n).transpose();
    Some(DAffine3::from_mat3(rows) * DAffine3::from_translation(-o) * *world)
}

/// The smallest box round the paths' points.
fn bounds(paths: &[Path]) -> Option<(DVec2, DVec2)> {
    let mut lo = DVec2::splat(f64::MAX);
    let mut hi = DVec2::splat(f64::MIN);
    for p in paths {
        for s in &p.segs {
            for q in s.samples() {
                lo = lo.min(q);
                hi = hi.max(q);
            }
        }
    }
    (lo.x <= hi.x).then_some((lo, hi))
}

impl Seg {
    pub fn start(&self) -> DVec2 {
        match self {
            Seg::Line(a, _) => *a,
            Seg::Conic { c, u, .. } => *c + *u,
            Seg::Spline(n) => n.eval(n.domain().0).truncate(),
        }
    }

    pub fn end(&self) -> DVec2 {
        match self {
            Seg::Line(_, b) => *b,
            Seg::Conic { c, u, v, sweep } => *c + *u * sweep.cos() + *v * sweep.sin(),
            Seg::Spline(n) => n.eval(n.domain().1).truncate(),
        }
    }

    /// Points along it, for its bounds.
    fn samples(&self) -> Vec<DVec2> {
        match self {
            Seg::Line(a, b) => vec![*a, *b],
            Seg::Conic { c, u, v, sweep } => (0..=64)
                .map(|i| {
                    let t = sweep * f64::from(i) / 64.0;
                    *c + *u * t.cos() + *v * t.sin()
                })
                .collect(),
            Seg::Spline(n) => {
                let (t0, t1) = n.domain();
                (0..=64)
                    .map(|i| n.eval(t0 + (t1 - t0) * f64::from(i) / 64.0).truncate())
                    .collect()
            }
        }
    }

    /// The same piece sent through `m`.
    pub fn mapped(&self, m: &DAffine2) -> Seg {
        match self {
            Seg::Line(a, b) => Seg::Line(m.transform_point2(*a), m.transform_point2(*b)),
            Seg::Conic { c, u, v, sweep } => Seg::Conic {
                c: m.transform_point2(*c),
                u: m.transform_vector2(*u),
                v: m.transform_vector2(*v),
                sweep: *sweep,
            },
            Seg::Spline(n) => {
                let a = m.matrix2;
                let t = m.translation;
                let m3 = DAffine3::from_cols(
                    a.x_axis.extend(0.0),
                    a.y_axis.extend(0.0),
                    DVec3::Z,
                    t.extend(0.0),
                );
                Seg::Spline(n.transformed(&m3))
            }
        }
    }

    fn moved(&mut self, d: DVec2) {
        match self {
            Seg::Line(a, b) => {
                *a += d;
                *b += d;
            }
            Seg::Conic { c, .. } => *c += d,
            Seg::Spline(n) => {
                *n = n.transformed(&DAffine3::from_translation(d.extend(0.0)));
            }
        }
    }

    /// As a circle or an arc of one, when it is: its centre, radius, start
    /// angle and signed sweep (above 0 counter-clockwise).
    pub fn circle(&self, eps: f64) -> Option<(DVec2, f64, f64, f64)> {
        let Seg::Conic { c, u, v, sweep } = self else {
            return None;
        };
        let r = u.length();
        if (v.length() - r).abs() > eps || u.dot(*v).abs() > eps * r {
            return None;
        }
        let turn = if u.perp_dot(*v) > 0.0 { *sweep } else { -sweep };
        Some((*c, r, u.y.atan2(u.x), turn))
    }

    /// As an ellipse going counter-clockwise: its centre, major axis (from
    /// the centre), the minor axis's length over the major's, and the start
    /// and end parameters (end above start).
    pub fn ellipse(&self) -> Option<(DVec2, DVec2, f64, f64, f64)> {
        let Seg::Conic { c, u, v, sweep } = self else {
            return None;
        };
        let (u, mut v) = (*u, *v);
        let (mut t0, mut t1) = (0.0, *sweep);
        if u.perp_dot(v) < 0.0 {
            // Walked the other way: the same points for t from -sweep to 0.
            v = -v;
            (t0, t1) = (-sweep, 0.0);
        }
        let mut a = 0.5 * (2.0 * u.dot(v)).atan2(u.dot(u) - v.dot(v));
        let mut major = u * a.cos() + v * a.sin();
        let minor = -u * a.sin() + v * a.cos();
        if minor.length() > major.length() {
            major = minor;
            a += PI / 2.0;
        }
        let ratio = (u.perp_dot(v) / major.length_squared()).clamp(0.0, 1.0);
        let start = (t0 - a).rem_euclid(TAU);
        Some((*c, major, ratio, start, start + (t1 - t0)))
    }
}

impl Path {
    fn moved(&mut self, d: DVec2) {
        for s in &mut self.segs {
            s.moved(d);
        }
    }
}

/// A file format for drawings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Dxf,
    Svg,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Dxf => "dxf",
            Format::Svg => "svg",
        }
    }
}

/// A drawing written out, and what went into it.
#[derive(Clone, Debug, PartialEq)]
pub struct Exported {
    pub text: String,
    pub parts: usize,
    pub paths: usize,
    /// Why each thing left out was left out.
    pub left_out: Vec<&'static str>,
    /// What was written only closely, not exactly.
    pub close: Vec<&'static str>,
}

impl Exported {
    /// What was exported and what was left out, in a sentence or two.
    pub fn summary(&self) -> String {
        let count = |n: usize, one: &str, many: &str| match n {
            1 => format!("1 {one}"),
            n => format!("{n} {many}"),
        };
        let mut s = format!(
            "{} in {}",
            count(self.paths, "path", "paths"),
            count(self.parts, "part", "parts")
        );
        for (head, list) in [
            ("Left out", &self.left_out),
            ("Close, not exact", &self.close),
        ] {
            if list.is_empty() {
                continue;
            }
            let mut why: BTreeMap<&str, usize> = BTreeMap::new();
            for w in list {
                *why.entry(w).or_default() += 1;
            }
            let reasons: Vec<String> = why
                .into_iter()
                .map(|(w, n)| {
                    if n == 1 {
                        w.to_owned()
                    } else {
                        format!("{w} ({n})")
                    }
                })
                .collect();
            s.push_str(&format!(". {head} {}: {}", list.len(), reasons.join("; ")));
        }
        s
    }
}

/// What `scope` chooses in `asm`, seen as `view`, as a `format` file in
/// `unit`.
pub fn export(
    asm: &Assembly,
    scope: Scope,
    view: View,
    unit: LengthUnit,
    format: Format,
) -> Result<Exported, &'static str> {
    let (d, left_out) = make(asm, scope, view, unit)?;
    let (text, close) = match format {
        Format::Dxf => (dxf::write(&d), Vec::new()),
        Format::Svg => svg::write(&d),
    };
    Ok(Exported {
        text,
        parts: d.parts.len(),
        paths: d.parts.iter().map(|p| p.paths.len()).sum(),
        left_out,
        close,
    })
}

// ---------- Import (part 6b) ----------

/// A drawing read from a file, in the file's own unit, before it comes in.
#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    pub paths: Vec<Path>,
    /// Points (DXF POINT): guide points.
    pub points: Vec<DVec2>,
    /// Metres in the file's unit, when the file says.
    pub stated: Option<f64>,
    /// What was left out, by kind ("text", "dimensions").
    pub skipped: Vec<String>,
}

impl Read {
    /// The width and height of what it holds, in the file's unit.
    pub fn size(&self) -> DVec2 {
        let pts: Vec<DVec2> = self
            .paths
            .iter()
            .flat_map(|p| p.segs.iter().flat_map(Seg::samples))
            .chain(self.points.iter().copied())
            .collect();
        let lo = pts.iter().fold(DVec2::splat(f64::MAX), |a, p| a.min(*p));
        let hi = pts.iter().fold(DVec2::splat(f64::MIN), |a, p| a.max(*p));
        if lo.x <= hi.x { hi - lo } else { DVec2::ZERO }
    }
}

/// The drawing in `bytes`, read as `format`.
pub fn read(bytes: &[u8], format: Format) -> Result<Read, String> {
    let text = String::from_utf8_lossy(bytes);
    match format {
        Format::Dxf => dxf::read(&text),
        Format::Svg => svg::read(&text),
    }
}

/// A drawing made into an assembly to bring in, and what it held.
#[derive(Clone, Debug)]
pub struct Imported {
    pub assembly: Assembly,
    pub edges: usize,
    pub faces: usize,
    pub skipped: Vec<String>,
}

impl Imported {
    /// What came in and what was left out, in a sentence or two.
    pub fn summary(&self) -> String {
        let count = |n: usize, one: &str, many: &str| match n {
            1 => format!("1 {one}"),
            n => format!("{n} {many}"),
        };
        let mut s = format!(
            "{} and {}",
            count(self.edges, "edge", "edges"),
            count(self.faces, "face", "faces")
        );
        if !self.skipped.is_empty() {
            let mut why: BTreeMap<&str, usize> = BTreeMap::new();
            for w in &self.skipped {
                *why.entry(w.as_str()).or_default() += 1;
            }
            let reasons: Vec<String> = why.into_iter().map(|(w, n)| format!("{w} ({n})")).collect();
            s.push_str(&format!(
                ". Left out {}: {} (not imported yet)",
                self.skipped.len(),
                reasons.join(", ")
            ));
        }
        s
    }
}

/// Said when a drawing holds nothing to bring in.
pub const EMPTY: &str = "Nothing to import: the drawing has no lines, arcs or curves.";

/// `read` on the ground, `metres` to its unit, as a group named `name`:
/// its lines, arcs and curves as edges, faces where they close (the
/// usual drawing rule), its points as guide points. One command, so one
/// undo step when it comes in.
pub fn import(read: &Read, metres: f64, name: &str) -> Result<Imported, String> {
    let to = DAffine2::from_scale(DVec2::splat(metres));
    let at = |p: DVec2| to.transform_point2(p).extend(0.0);
    let mut commands = Vec::new();
    for p in &read.paths {
        for s in &p.segs {
            match s.mapped(&to) {
                Seg::Line(a, b) => commands.push(prism_doc::Command::Segment {
                    p: a.extend(0.0),
                    q: b.extend(0.0),
                }),
                Seg::Conic { c, u, v, sweep } => {
                    let turn = u.perp_dot(v);
                    let r = u.length();
                    let round =
                        (v.length() - r).abs() <= 1e-9 * r && u.dot(v).abs() <= 1e-9 * r * r;
                    let full = sweep >= TAU - 1e-9;
                    if round && full {
                        commands.push(prism_doc::Command::Circle {
                            centre: c.extend(0.0),
                            normal: DVec3::Z,
                            radius: r,
                        });
                    } else if round {
                        commands.push(prism_doc::Command::Arc {
                            centre: c.extend(0.0),
                            normal: if turn > 0.0 { DVec3::Z } else { -DVec3::Z },
                            radius: r,
                            start: (c + u).extend(0.0),
                            sweep,
                            pie: false,
                        });
                    } else {
                        // An ellipse: the unit circle sent onto it, exactly,
                        // in halves at most (a spline is not a whole loop).
                        let m = DAffine3::from_cols(
                            u.extend(0.0),
                            v.extend(0.0),
                            DVec3::Z,
                            c.extend(0.0),
                        );
                        let n = if full { 2 } else { 1 };
                        for k in 0..n {
                            let t0 = sweep * f64::from(k) / f64::from(n);
                            let start = DVec3::new(t0.cos(), t0.sin(), 0.0);
                            if let Some(arc) =
                                Nurbs::arc(DVec3::ZERO, DVec3::Z, start, sweep / f64::from(n))
                            {
                                commands.push(prism_doc::Command::Spline {
                                    curve: arc.transformed(&m),
                                });
                            }
                        }
                    }
                }
                Seg::Spline(n) => commands.push(prism_doc::Command::Spline { curve: n }),
            }
        }
    }
    for p in &read.points {
        commands.push(prism_doc::Command::AddGuide {
            guide: prism_topo::Guide::Point { at: at(*p) },
        });
    }
    if commands.is_empty() {
        return Err(EMPTY.to_owned());
    }
    let mut d = prism_doc::Document::new();
    d.run(prism_doc::Command::Batch { commands });
    let m = d.model();
    let (edges, faces) = (m.edges().count(), m.faces().count());
    let selection = prism_ops::Selection {
        vertices: m.vertices().map(|(id, _)| id).collect(),
        edges: m.edges().map(|(id, _)| id).collect(),
        faces: m.faces().map(|(id, _)| id).collect(),
        guides: m.guides().map(|(id, _)| id).collect(),
    };
    let group = d.assembly().next_instance();
    d.run(prism_doc::Command::MakeGroup {
        selection,
        instances: Default::default(),
    });
    d.run(prism_doc::Command::NameInstance {
        instance: group,
        name: name.to_owned(),
    });
    Ok(Imported {
        assembly: d.assembly().clone(),
        edges,
        faces,
        skipped: read.skipped.clone(),
    })
}

/// Numbers as written: at most six places, no trailing zeros, no "-0".
pub(crate) fn num(x: f64) -> String {
    let s = format!("{x:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}
