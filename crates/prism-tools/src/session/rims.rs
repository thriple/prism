//! Rim styles of a live shell (brief 49 part 3): the handles on its open
//! boundaries, and the ghost of how each wall ends.
//!
//! With the Shell tool and a live shell selected, every open boundary shows
//! a handle in the middle of it, and a boundary that ends flush in a plane
//! is drawn along its length (dashed where the program chose that, solid
//! where a person did).
//!
//! - **Click a handle:** switch that boundary between Square and Flush (the
//!   plane it lies in, or one through a straight boundary that a face, an
//!   axis, a guide point or the working plane gives).
//! - **Drag a handle on a straight boundary:** tilt the plane the wall ends
//!   in about the boundary, with the shell shown as it goes. It snaps to
//!   planes through the boundary (a face of the model, an axis plane, the
//!   working plane, a guide point) when close. Letting go makes it the
//!   style, in one undo step.
//! - A boundary that is not straight can only be Square or Flush to the
//!   plane it lies in, which a click chooses.

use std::cell::RefCell;
use std::rc::Rc;

use prism_doc::{Command, DefId, EdgeRef, Feature, RimBoundary, RimReport};
use prism_math::{DVec2, DVec3, Plane};
use prism_ops::shelling::RimStyle;
use prism_view::View;
use prism_view::overlay::{Line, Mark, Overlay, Shape};

use super::Session;
use super::features::GRAB;
use super::inspector::{Field, FieldKind, field};

/// How far the pointer must move before a press on a handle is a drag, in
/// pixels.
const DRAG_PX: f64 = 4.0;
/// How close (in degrees) the tilt must be to a plane through the boundary
/// to snap to it.
const SNAP_DEGREES: f64 = 4.0;

/// The rims of the selected live shell, kept until the document changes.
pub(super) struct RimCache {
    revision: u64,
    def: DefId,
    index: usize,
    report: Rc<RimReport>,
}

pub(super) type RimCell = RefCell<Option<RimCache>>;

/// A press on a rim handle, until it is let go.
#[derive(Clone, Debug)]
pub(super) struct RimDrag {
    def: DefId,
    index: usize,
    edge: EdgeRef,
    line: Option<(DVec3, DVec3)>,
    plane: Option<Plane>,
    now: RimStyle,
    from: DVec2,
    moved: bool,
    /// The style the pointer is at while dragging (None: not buildable).
    proposed: Option<RimStyle>,
    snapped: Option<&'static str>,
    refused: Option<&'static str>,
}

/// A plane through a boundary the tilt can snap to, and what it is.
type Candidate = (Plane, &'static str);

impl Session {
    /// The rims of the selected live shell: (definition, index, report).
    fn rim_report(&self) -> Option<(DefId, usize, Rc<RimReport>)> {
        if self.tool != super::shelling::SHELL || self.shell_op.is_some() {
            return None;
        }
        let index = self.selected_feature()?;
        let def = self.doc.assembly().context_def(&self.ctx)?;
        Some((def, index, self.rim_report_of(def, index)?))
    }

    /// The rims of the live shell at `index` of `def` (the Inspector's too,
    /// which needs no tool).
    pub(super) fn rim_report_of(&self, def: DefId, index: usize) -> Option<Rc<RimReport>> {
        let revision = self.doc.revision();
        if let Some(c) = self.rim_cache.borrow().as_ref()
            && c.revision == revision
            && c.def == def
            && c.index == index
        {
            return Some(c.report.clone());
        }
        let stack = self.doc.assembly().def(def)?.stack.as_ref()?;
        let report = Rc::new(stack.rim_report(index)?);
        *self.rim_cache.borrow_mut() = Some(RimCache {
            revision,
            def,
            index,
            report: report.clone(),
        });
        Some(report)
    }

    /// Where a boundary's handle is, in the world.
    fn rim_handle(&self, b: &RimBoundary) -> Option<DVec3> {
        let p = *b.points.get(b.points.len() / 2)?;
        Some(self.to_world.transform_point3(p))
    }

    /// A press on a rim handle starts a drag. True if it did.
    pub(super) fn rim_press(&mut self, s: DVec2) -> bool {
        let Some((def, index, report)) = self.rim_report() else {
            return false;
        };
        let view = self.view();
        let reach = GRAB * self.settings.aperture;
        let mut best: Option<(f64, &RimBoundary)> = None;
        for b in &report.boundaries {
            let Some((q, _)) = self.rim_handle(b).and_then(|p| view.to_screen(p)) else {
                continue;
            };
            let d = q.distance(s);
            if d < reach && best.is_none_or(|x| d < x.0) {
                best = Some((d, b));
            }
        }
        let Some((_, b)) = best else {
            return false;
        };
        self.rim_drag = Some(RimDrag {
            def,
            index,
            edge: b.edge,
            line: b.line,
            plane: b.plane,
            now: b.style,
            from: s,
            moved: false,
            proposed: None,
            snapped: None,
            refused: None,
        });
        true
    }

    /// Planes through the straight boundary `a`..`b` that a tilt can snap
    /// to, in the shell's own coordinates.
    fn rim_candidates(&self, def: DefId, report: &RimReport, a: DVec3, b: DVec3) -> Vec<Candidate> {
        let tol = report.before.tol();
        let u = (b - a).normalize_or_zero();
        let holds = |p: &Plane| {
            tol.is_zero(p.distance(a)) && tol.is_zero(p.distance(b)) && p.normal.is_finite()
        };
        let mut out: Vec<Candidate> = Vec::new();
        for (_, f) in report.before.faces() {
            if let Some(p) = f.plane()
                && holds(p)
            {
                out.push((Plane::new(a, p.normal), "a face"));
            }
        }
        for axis in [DVec3::X, DVec3::Y, DVec3::Z] {
            let p = Plane::new(a, axis);
            if holds(&p) {
                out.push((p, "an axis plane"));
            }
        }
        if let Some((_, wp)) = self.doc.assembly().active_work_plane() {
            // The working plane is in the world.
            let local = self.to_world.inverse();
            let p = Plane::new(
                local.transform_point3(wp.origin),
                local
                    .matrix3
                    .transpose()
                    .mul_vec3(wp.normal)
                    .normalize_or_zero(),
            );
            if holds(&p) {
                out.push((Plane::new(a, p.normal), "the working plane"));
            }
        }
        for (_, g) in report.before.guides() {
            match g {
                prism_topo::Guide::Point { at } => {
                    let n = u.cross(*at - a).normalize_or_zero();
                    if n != DVec3::ZERO {
                        out.push((Plane::new(a, n), "a guide point"));
                    }
                }
                // A guide line makes a plane with the boundary where the
                // two lie in one plane (cross, or run side by side).
                prism_topo::Guide::Line { origin, dir } => {
                    let mut n = u.cross(*dir).normalize_or_zero();
                    if n == DVec3::ZERO {
                        n = u.cross(*origin - a).normalize_or_zero();
                    }
                    if n != DVec3::ZERO && tol.is_zero(Plane::new(a, n).distance(*origin)) {
                        out.push((Plane::new(a, n), "a guide line"));
                    }
                }
            }
        }
        // The flat faces of the other parts placed beside it in the same
        // group: a part that meets the boundary there.
        if let Some(d) = self.doc.assembly().def(def) {
            for inst in d.instances.values() {
                let Some(other) = self.doc.assembly().def(inst.def) else {
                    continue;
                };
                let m = inst.transform;
                for (_, f) in other.model.faces() {
                    let Some(p) = f.plane() else {
                        continue;
                    };
                    let q = Plane::new(
                        m.transform_point3(p.origin),
                        m.matrix3.inverse().transpose().mul_vec3(p.normal),
                    );
                    if holds(&q) {
                        out.push((Plane::new(a, q.normal), "another part"));
                    }
                }
            }
        }
        out
    }

    /// The plane through the line `a`..`b` that the pointer's ray is on the
    /// side of: where the ray meets the plane square to the line through
    /// its middle, and the plane through the line and that point.
    fn rim_tilt(&self, s: DVec2, view: &View, a: DVec3, b: DVec3) -> Option<Plane> {
        let local = self.to_world.inverse();
        let (o, d) = view.ray(s);
        let (o, d) = (
            local.transform_point3(o),
            local.transform_vector3(d).normalize_or_zero(),
        );
        let u = (b - a).normalize_or_zero();
        let mid = (a + b) * 0.5;
        let den = u.dot(d);
        if den.abs() < prism_math::tol::UNIT_GUARD {
            return None;
        }
        let q = o + d * (u.dot(mid - o) / den);
        let n = u.cross(q - mid);
        (n.length() > self.scene.tol.dist).then(|| Plane::new(a, n))
    }

    /// While a rim handle is held on a straight boundary, the plane the
    /// pointer tilts to (snapped) is shown on the body. True while held.
    pub(super) fn rim_hover(&mut self, s: DVec2, view: &View) -> bool {
        let Some(mut drag) = self.rim_drag.take() else {
            return false;
        };
        if !drag.moved && s.distance(drag.from) >= DRAG_PX {
            drag.moved = true;
        }
        if drag.moved
            && let Some((a, b)) = drag.line
            && let Some((_, _, report)) = self.rim_report()
            && let Some(tilt) = self.rim_tilt(s, view, a, b)
        {
            let tol = report.before.tol();
            let mut plane = tilt;
            drag.snapped = None;
            let cos = SNAP_DEGREES.to_radians().cos();
            if let Some((p, what)) = self
                .rim_candidates(drag.def, &report, a, b)
                .into_iter()
                .filter(|(p, _)| p.normal.dot(tilt.normal).abs() > cos)
                .max_by(|x, y| {
                    x.0.normal
                        .dot(tilt.normal)
                        .abs()
                        .total_cmp(&y.0.normal.dot(tilt.normal).abs())
                })
            {
                plane = p;
                drag.snapped = Some(what);
            }
            let _ = tol;
            let style = RimStyle::Flush(plane);
            drag.refused = None;
            match self.rim_at(&drag, style) {
                Ok(m) => {
                    drag.proposed = Some(style);
                    self.set_preview(Some(m));
                }
                Err(why) => {
                    drag.proposed = None;
                    drag.refused = Some(why);
                    self.set_preview(None);
                }
            }
        }
        self.rim_drag = Some(drag);
        self.overlay_dirty = true;
        self.dirty = true;
        true
    }

    /// The body with the shell's rim `drag.edge` ending as `style`, or why
    /// the kernel would not.
    fn rim_at(&self, drag: &RimDrag, style: RimStyle) -> Result<prism_topo::Model, &'static str> {
        self.rim_with(drag.def, drag.index, &drag.edge, style)
    }

    /// The body with the shell `index` of `def` ending its rim `edge` as
    /// `style`, or why the kernel would not.
    pub(super) fn rim_with(
        &self,
        def: DefId,
        index: usize,
        edge: &EdgeRef,
        style: RimStyle,
    ) -> Result<prism_topo::Model, &'static str> {
        let stack = self
            .doc
            .assembly()
            .def(def)
            .and_then(|d| d.stack.clone())
            .ok_or("Refused: this is no live shell.")?;
        let mut next = stack;
        if !next.set_rim(index, edge, Some(style)) {
            return Err("Refused: that is not a rim of this shell.");
        }
        let m = next.evaluate();
        // The shell is built anyway (a rim it cannot reach is cut square,
        // DECISIONS #41), but the handle offers only a style it builds as
        // asked, and says why not: the tilt stays where it last worked.
        let strict = self.rim_refusal(def, index, edge, style);
        match next.features.get(index) {
            Some(f) if !f.is_broken() && strict.is_none() => Ok(m),
            _ => Err(strict.unwrap_or("Refused: the shell cannot end that way.")),
        }
    }

    /// Why the kernel refuses a rim style, in its own words.
    fn rim_refusal(
        &self,
        def: DefId,
        index: usize,
        edge: &EdgeRef,
        style: RimStyle,
    ) -> Option<&'static str> {
        let report = self.rim_report_of(def, index)?;
        let stack = self.doc.assembly().def(def)?.stack.as_ref()?;
        let Feature::Shell {
            faces,
            open,
            thickness,
            ..
        } = stack.features.get(index)?
        else {
            return None;
        };
        let (mut faces, mut open) = (faces.clone(), open.clone());
        let on_base = index == 0;
        let (found, open) = (
            prism_doc::resolve_faces(&report.before, &mut faces, on_base)?,
            prism_doc::resolve_faces(&report.before, &mut open, on_base)?,
        );
        let edge = prism_doc::edge_at(&report.before, edge)?;
        let styles = prism_ops::shelling::styles_for_boundaries(
            &report.before,
            &found,
            &open,
            &[(edge, style)],
        );
        prism_ops::shelling::refusal_with(&report.before, &found, &open, *thickness, &styles)
    }

    /// The button let go: a click switches the boundary between Square and
    /// Flush, a drag sets the plane it was tilted to. True if a handle was
    /// held.
    pub(super) fn rim_release(&mut self) -> bool {
        let Some(drag) = self.rim_drag.take() else {
            return false;
        };
        self.set_preview(None);
        self.notice = None;
        let target = if drag.moved && drag.line.is_some() {
            drag.proposed.or_else(|| {
                self.notice = drag.refused;
                None
            })
        } else {
            self.rim_toggle(&drag)
        };
        if let Some(style) = target {
            self.run(Command::SetRimStyle {
                def: drag.def,
                index: drag.index,
                edge: drag.edge,
                style: Some(style),
            });
            // The feature's faces are new ones: keep it selected.
            self.select_feature(drag.index);
        }
        self.overlay_dirty = true;
        self.refresh();
        true
    }

    /// What a click on the handle does: Flush becomes Square; Square
    /// becomes Flush in the plane the boundary lies in, or the first plane
    /// through a straight one that the shell can end in.
    fn rim_toggle(&mut self, drag: &RimDrag) -> Option<RimStyle> {
        if matches!(drag.now, RimStyle::Flush(_)) {
            return Some(RimStyle::Square);
        }
        let (_, _, report) = self.rim_report()?;
        match self.rim_flush(
            drag.def, drag.index, &report, drag.plane, drag.line, &drag.edge,
        ) {
            Ok(style) => Some(style),
            Err(why) => {
                self.notice = Some(why);
                None
            }
        }
    }

    /// The first plane a boundary can end flush in: the one it lies in, or
    /// one through a straight one that a face, an axis, the working plane
    /// or a guide point gives, that the shell can be built with.
    pub(super) fn rim_flush(
        &self,
        def: DefId,
        index: usize,
        report: &RimReport,
        plane: Option<Plane>,
        line: Option<(DVec3, DVec3)>,
        edge: &EdgeRef,
    ) -> Result<RimStyle, &'static str> {
        let mut planes: Vec<Plane> = plane.into_iter().collect();
        if let Some((a, b)) = line {
            planes.extend(
                self.rim_candidates(def, report, a, b)
                    .into_iter()
                    .map(|c| c.0),
            );
        }
        let mut why = None;
        for p in planes {
            let style = RimStyle::Flush(p);
            match self.rim_with(def, index, edge, style) {
                Ok(_) => return Ok(style),
                Err(r) => why = Some(r),
            }
        }
        Err(why.unwrap_or("Refused: no plane to end flush in: tilt the handle to pick one."))
    }

    /// Whether a rim handle is held (so Esc lets it go first).
    pub(super) fn rim_drag_held(&self) -> bool {
        self.rim_drag.is_some()
    }

    pub(super) fn rim_clear(&mut self) {
        if self.rim_drag.take().is_some() {
            self.set_preview(None);
        }
    }

    pub(super) fn rim_hint(&self) -> Option<&'static str> {
        if let Some(d) = &self.rim_drag {
            if let Some(r) = d.refused {
                return Some(r);
            }
            if d.moved && d.line.is_some() {
                return Some(match d.snapped {
                    Some("a face") => "Flush to a face's plane · let go to set it · Esc cancels",
                    Some("an axis plane") => {
                        "Flush to an axis plane · let go to set it · Esc cancels"
                    }
                    Some("the working plane") => {
                        "Flush to the working plane · let go to set it · Esc cancels"
                    }
                    Some("another part") => {
                        "Flush to the plane of another part · let go to set it · Esc cancels"
                    }
                    Some("a guide line") => {
                        "Flush to a plane through a guide line · let go to set it · Esc cancels"
                    }
                    Some(_) => {
                        "Flush to a plane through a guide point · let go to set it · Esc cancels"
                    }
                    None => {
                        "Tilting the cut about the edge · it snaps to nearby planes · let go to set it · Esc cancels"
                    }
                });
            }
        }
        None
    }

    /// The rims: a line along each boundary that ends flush (dashed where
    /// the program chose that), a handle on each, and while one is tilted
    /// the plane it ends in.
    pub(super) fn rim_overlay(&self, o: &mut Overlay) {
        let Some((_, _, report)) = self.rim_report() else {
            return;
        };
        let fb = &self.style.feedback;
        let world = |p: DVec3| self.to_world.transform_point3(p);
        for b in &report.boundaries {
            let held = self
                .rim_drag
                .as_ref()
                .is_some_and(|d| d.edge.id == b.edge.id);
            let flush = matches!(b.style, RimStyle::Flush(_));
            if flush {
                for w in b.points.windows(2) {
                    o.lines.push(Line {
                        a: world(w[0]),
                        b: world(w[1]),
                        color: fb.rim_flush,
                        width: fb.target_width * 1.5,
                        dash: if b.chosen { [0.0, 0.0] } else { [6.0, 4.0] },
                    });
                }
            }
            if let Some(at) = self.rim_handle(b) {
                o.marks.push(Mark {
                    at,
                    shape: if flush {
                        Shape::Diamond
                    } else {
                        Shape::DiamondOutline
                    },
                    color: if held { self.style.hover } else { fb.rim_flush },
                    size: fb.glyph_size,
                });
            }
        }
        // The plane being tilted to, as a sheet along the boundary.
        if let Some(d) = &self.rim_drag
            && d.moved
            && let (Some((a, b)), Some(RimStyle::Flush(p))) = (d.line, d.proposed)
        {
            let u = (b - a).normalize_or_zero();
            let v = p.normal.cross(u).normalize_or_zero();
            let width = (b - a).length() * 0.25;
            let sheet = [a, b, b + v * width, a + v * width];
            for k in 0..4 {
                o.lines.push(Line {
                    a: world(sheet[k]),
                    b: world(sheet[(k + 1) % 4]),
                    color: if d.snapped.is_some() {
                        self.style.hover
                    } else {
                        fb.rim_flush
                    },
                    width: fb.target_width,
                    dash: [0.0, 0.0],
                });
            }
        }
    }
}

/// A direction square to the line `u` to measure a tilt from: the part of
/// world Z square to it, or X where the line runs along Z.
fn tilt_reference(u: DVec3) -> DVec3 {
    let z = DVec3::Z - u * u.dot(DVec3::Z);
    if z.length() > 1e-6 {
        z.normalize()
    } else {
        (DVec3::X - u * u.dot(DVec3::X)).normalize()
    }
}

/// How far, in degrees (-90 to 90), the plane `p` through the line
/// `a`..`b` is turned about it from the vertical plane through it (the
/// one that holds world Z); a line along Z is measured from the one that
/// holds X.
pub(super) fn tilt_degrees(a: DVec3, b: DVec3, p: &Plane) -> f64 {
    let u = (b - a).normalize_or_zero();
    let r = tilt_reference(u);
    let w = p.normal.cross(u).normalize_or_zero();
    let mut deg = u.dot(r.cross(w)).atan2(r.dot(w)).to_degrees();
    if deg > 90.0 {
        deg -= 180.0;
    } else if deg <= -90.0 {
        deg += 180.0;
    }
    deg
}

/// The plane through the line `a`..`b` turned `degrees` about it from the
/// vertical plane (the inverse of `tilt_degrees`).
pub(super) fn plane_at_tilt(a: DVec3, b: DVec3, degrees: f64) -> Plane {
    let u = (b - a).normalize_or_zero();
    let r = tilt_reference(u);
    let t = degrees.to_radians();
    let w = r * t.cos() + u.cross(r) * t.sin();
    Plane::new(a, u.cross(w))
}

const SQUARE: &str = "Square";
const FLUSH: &str = "Flush";

impl Session {
    /// The Inspector rows for the rims of the live shell at `index` of the
    /// body being edited: for each open boundary how its wall ends
    /// (`feature.N.rim.B`), and, where it is flush and straight, the tilt
    /// of its plane in degrees (`feature.N.rim.B.tilt`).
    pub(super) fn rim_fields(&self, index: usize) -> Vec<Field> {
        let Some(def) = self.features_target() else {
            return Vec::new();
        };
        let Some(report) = self.rim_report_of(def, index) else {
            return Vec::new();
        };
        let mut fields = Vec::new();
        for (n, b) in report.boundaries.iter().enumerate() {
            let flush = matches!(b.style, RimStyle::Flush(_));
            let note = if flush && !b.chosen {
                " (the program's pick)"
            } else {
                ""
            };
            fields.push(field(
                &format!("feature.{index}.rim.{n}"),
                &format!("Rim {} ends{note}", n + 1),
                FieldKind::Choice(vec![SQUARE.into(), FLUSH.into()]),
                if flush { FLUSH } else { SQUARE }.into(),
            ));
            if let (RimStyle::Flush(p), Some((a, c))) = (b.style, b.line) {
                fields.push(field(
                    &format!("feature.{index}.rim.{n}.tilt"),
                    &format!("Rim {} tilt (degrees)", n + 1),
                    FieldKind::Text,
                    format!("{:.1}", tilt_degrees(a, c, &p)),
                ));
            }
        }
        fields
    }

    /// The command a rim row's text asks for: `boundary` is the boundary's
    /// number and `what` is None (Square or Flush) or "tilt".
    pub(super) fn rim_command(
        &self,
        def: DefId,
        index: usize,
        boundary: Option<&str>,
        what: Option<&str>,
        text: &str,
    ) -> Result<Command, String> {
        let report = self
            .rim_report_of(def, index)
            .ok_or("This feature has no rims")?;
        let n: usize = boundary
            .and_then(|n| n.parse().ok())
            .ok_or("Name a rim: feature.N.rim.B")?;
        let b = report
            .boundaries
            .get(n)
            .ok_or(format!("The shell has no rim {n}"))?;
        let style = match what {
            None => match text.trim().to_ascii_lowercase().as_str() {
                "square" => RimStyle::Square,
                "flush" => {
                    if matches!(b.style, RimStyle::Flush(_)) {
                        b.style
                    } else {
                        self.rim_flush(def, index, &report, b.plane, b.line, &b.edge)
                            .map_err(str::to_owned)?
                    }
                }
                _ => return Err(format!("{text:?} is not Square or Flush")),
            },
            Some("tilt") => {
                let (a, c) = b.line.ok_or("Only a straight rim can be tilted")?;
                let degrees: f64 = text
                    .trim()
                    .trim_end_matches(['°'])
                    .trim()
                    .trim_end_matches("deg")
                    .trim()
                    .parse()
                    .map_err(|_| format!("{text:?} is not a number of degrees"))?;
                if !degrees.is_finite() {
                    return Err(format!("{text:?} is not a number of degrees"));
                }
                RimStyle::Flush(plane_at_tilt(a, c, degrees))
            }
            Some(other) => return Err(format!("{other:?} is not a rim field")),
        };
        // Say why in the kernel's words when it will not build.
        self.rim_with(def, index, &b.edge, style)
            .map_err(str::to_owned)?;
        Ok(Command::SetRimStyle {
            def,
            index,
            edge: b.edge,
            style: Some(style),
        })
    }
}
