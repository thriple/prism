//! Picking and editing section planes, dimensions and construction planes
//! with the Select tool (Annotation & Sections gaps, 2026-11-18).
//!
//! Plain clicks in Select never touch them, so they cannot clash with the
//! model's parts. Instead:
//! - **Alt + click** picks the plane or dimension under the cursor (its
//!   outline or line); Alt + click on the picked one picks it up, it then
//!   follows the cursor (a plane along its normal, a dimension's line to
//!   the cursor) and the next click sets it. Type a distance and Enter to
//!   set it exactly.
//! - **Delete** removes the picked one, when nothing else is selected.
//! - **Right-click** near one (and not on the model) picks it and opens its
//!   menu.
//! - Esc leaves a move where it was.

use prism_doc::{Command, DimId, SectionId, WorkPlaneId};
use prism_math::{DVec2, DVec3};
use prism_view::View;

use super::dimensions::{DimOp, offset_towards};
use super::sections::line_param;
use super::{SELECT, Selection, Session};

/// Which one is picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ann {
    Section(SectionId),
    Dim(DimId),
    CPlane(WorkPlaneId),
}

/// A construction plane being moved along its normal.
#[derive(Clone, Copy, Debug)]
pub(super) struct PlaneOp {
    pub id: WorkPlaneId,
    pub origin: DVec3,
    pub normal: DVec3,
    pub t0: f64,
    pub t: f64,
}

impl PlaneOp {
    pub fn moved_to(&self) -> DVec3 {
        self.origin + self.normal * (self.t - self.t0)
    }
}

/// How near the cursor must be to a construction plane's outline, pixels.
const BORDER_PX: f64 = 8.0;

impl Session {
    /// What is picked, if anything.
    pub(super) fn ann_picked(&self) -> Option<Ann> {
        let asm = self.doc.assembly();
        self.dim_sel
            .filter(|id| asm.dimension(*id).is_some())
            .map(Ann::Dim)
            .or_else(|| {
                self.section_sel
                    .filter(|id| asm.section(*id).is_some())
                    .map(Ann::Section)
            })
            .or_else(|| {
                self.cplane_sel
                    .filter(|id| asm.work_plane(*id).is_some())
                    .map(Ann::CPlane)
            })
    }

    /// What is picked, for the right-click menu: "section", "dimension" or
    /// "cplane".
    pub fn picked_annotation(&self) -> Option<&'static str> {
        self.ann_picked().map(|a| match a {
            Ann::Section(_) => "section",
            Ann::Dim(_) => "dimension",
            Ann::CPlane(_) => "cplane",
        })
    }

    pub(super) fn ann_clear(&mut self) {
        if self.section_sel.is_some() || self.dim_sel.is_some() || self.cplane_sel.is_some() {
            self.section_sel = None;
            self.dim_sel = None;
            self.cplane_sel = None;
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    fn ann_select(&mut self, a: Ann) {
        self.ann_clear();
        match a {
            Ann::Section(id) => self.section_sel = Some(id),
            Ann::Dim(id) => self.dim_sel = Some(id),
            Ann::CPlane(id) => self.cplane_sel = Some(id),
        }
        // One thing is picked at a time: Delete and the menus know which.
        self.set_selection(Selection::default());
        self.set_groups(Default::default());
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The construction plane outline under the cursor.
    fn cplane_border_at(&self, s: DVec2, view: &View) -> Option<WorkPlaneId> {
        let mut best: Option<(f64, WorkPlaneId)> = None;
        for (id, p) in self.doc.assembly().work_planes().0 {
            let c = self.work_plane_corners(p.origin, p.normal);
            for k in 0..4 {
                let (Some((a, _)), Some((b, _))) =
                    (view.to_screen(c[k]), view.to_screen(c[(k + 1) % 4]))
                else {
                    continue;
                };
                let ab = b - a;
                let len2 = ab.length_squared();
                let t = if len2 > 0.0 {
                    ((s - a).dot(ab) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let d = s.distance(a + ab * t);
                if d <= BORDER_PX && best.is_none_or(|b| d < b.0) {
                    best = Some((d, *id));
                }
            }
        }
        best.map(|b| b.1)
    }

    /// What is under the cursor: a dimension's line first, then a section
    /// plane's outline, then a construction plane's (the ones that are
    /// drawn).
    pub(super) fn ann_pick(&self, s: DVec2, view: &View) -> Option<Ann> {
        if self.view.dimensions
            && let Some(id) = self.dim_at(s, view)
        {
            return Some(Ann::Dim(id));
        }
        if self.view.section_planes
            && let Some(id) = self.section_border_at(s, view)
        {
            return Some(Ann::Section(id));
        }
        if self.view.work_planes
            && let Some(id) = self.cplane_border_at(s, view)
        {
            return Some(Ann::CPlane(id));
        }
        None
    }

    /// Whether a move picked up in Select is under way.
    pub(super) fn ann_moving(&self) -> bool {
        self.tool == SELECT
            && (self.section_op.is_some() || self.cplane_op.is_some() || self.dim_op.is_some())
    }

    /// Alt + click in Select.
    pub(super) fn ann_press(&mut self, s: DVec2) {
        let view = self.view();
        match self.ann_pick(s, &view) {
            None => self.ann_clear(),
            Some(a) if self.ann_picked() == Some(a) => self.ann_pick_up(a, s, &view),
            Some(a) => self.ann_select(a),
        }
        self.overlay_dirty = true;
        self.dirty = true;
        self.refresh();
    }

    /// The picked one follows the cursor until the next click.
    fn ann_pick_up(&mut self, a: Ann, s: DVec2, view: &View) {
        let (o, d) = view.ray(s);
        let asm = self.doc.assembly();
        match a {
            Ann::Section(id) => {
                if let Some(p) = asm.section(id)
                    && let Some(t) = line_param(o, d, p.origin, p.normal)
                {
                    self.section_op = Some(super::sections::SectionOp {
                        id,
                        origin: p.origin,
                        normal: p.normal,
                        t0: t,
                        t,
                    });
                }
            }
            Ann::CPlane(id) => {
                if let Some(p) = asm.work_plane(id)
                    && let Some(t) = line_param(o, d, p.origin, p.normal)
                {
                    self.cplane_op = Some(PlaneOp {
                        id,
                        origin: p.origin,
                        normal: p.normal,
                        t0: t,
                        t,
                    });
                }
            }
            Ann::Dim(id) => {
                if let Some(dm) = asm.dimension(id).copied() {
                    self.dim_op = Some(DimOp {
                        a: dm.a,
                        b: Some(dm.b),
                        at: dm.a + dm.offset,
                        moving: Some(id),
                    });
                }
            }
        }
    }

    /// Where the cursor points on the plane a dimension lies in.
    fn dim_cursor(&self, s: DVec2, view: &View, a: DVec3, b: DVec3) -> Option<DVec3> {
        let (o, d) = view.ray(s);
        let line = (b - a).normalize_or_zero();
        // The plane holding the dimension: through its line, as square to
        // the view as it can be.
        let n = line.cross(d).cross(line).try_normalize()?;
        let den = n.dot(d);
        if den.abs() < 1e-9 {
            return None;
        }
        Some(o + d * ((a - o).dot(n) / den))
    }

    /// The cursor moved while something is picked up in Select.
    pub(super) fn ann_hover(&mut self, s: DVec2, view: &View) {
        if self.tool != SELECT {
            return;
        }
        // Planes and dimension lines go level with a snapped point.
        if let Some(op) = self.section_op {
            if let Some(t) = self.plane_t(s, view, op.origin, op.normal, op.t0)
                && let Some(op) = &mut self.section_op
            {
                op.t = t;
            }
            self.overlay_dirty = true;
        }
        if let Some(op) = self.cplane_op {
            if let Some(t) = self.plane_t(s, view, op.origin, op.normal, op.t0)
                && let Some(op) = &mut self.cplane_op
            {
                op.t = t;
            }
            self.overlay_dirty = true;
        }
        let snapped = self
            .inference
            .as_ref()
            .filter(|i| {
                !matches!(
                    i.kind,
                    prism_infer::Kind::OnFace
                        | prism_infer::Kind::OnPlane
                        | prism_infer::Kind::OnGround
                        | prism_infer::Kind::Free
                )
            })
            .map(|i| i.point);
        if let Some(op) = self.dim_op
            && let Some(b) = op.b
            && let Some(p) = snapped.or_else(|| self.dim_cursor(s, view, op.a, b))
        {
            self.dim_op = Some(DimOp { at: p, ..op });
            self.overlay_dirty = true;
        }
    }

    /// The click that sets a picked-up one down.
    pub(super) fn ann_set(&mut self, s: DVec2) {
        self.update_hover(s);
        if let Some(op) = self.section_op.take()
            && (op.t - op.t0).abs() > self.scene.tol.dist
        {
            self.section_run(Command::MoveSection {
                id: op.id,
                origin: op.moved_to(),
            });
        }
        if let Some(op) = self.cplane_op.take()
            && (op.t - op.t0).abs() > self.scene.tol.dist
        {
            self.work_plane_run(Command::MoveWorkPlane {
                id: op.id,
                origin: op.moved_to(),
            });
        }
        if let Some(op) = self.dim_op.take()
            && let (Some(b), Some(id)) = (op.b, op.moving)
        {
            let offset = offset_towards(op.a, b, op.at);
            if offset.length() > self.dim_tiny(op.at)
                && self.doc.run(Command::MoveDimension { id, offset })
            {
                self.model_changed();
            }
        }
        self.overlay_dirty = true;
        self.dirty = true;
        self.refresh();
    }

    /// Whether the measurement box takes a distance now.
    pub(super) fn ann_typing(&self) -> bool {
        self.section_op.is_some()
            || self.cplane_op.is_some()
            || self.dim_op.is_some_and(|o| o.b.is_some())
            || !self.cplane_pts.is_empty()
    }

    /// Enter with a typed distance while a plane or dimension line is
    /// being placed: a plane moves that far along its normal, a dimension
    /// line stands that far from what it measures. True if it was used.
    pub(super) fn ann_enter(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        if self.section_op.is_none()
            && self.cplane_op.is_none()
            && !self.dim_op.is_some_and(|o| o.b.is_some())
        {
            return false;
        }
        let Some(dist) = self.number.length(text.trim()) else {
            return false;
        };
        if let Some(op) = self.section_op.take() {
            self.section_run(Command::MoveSection {
                id: op.id,
                origin: op.origin + op.normal * dist,
            });
        } else if let Some(op) = self.cplane_op.take() {
            self.work_plane_run(Command::MoveWorkPlane {
                id: op.id,
                origin: op.origin + op.normal * dist,
            });
        } else if let Some(op) = self.dim_op.take()
            && let Some(b) = op.b
        {
            // Along the side the cursor is on, as far as typed.
            let side = offset_towards(op.a, b, op.at).normalize_or_zero();
            let side = if side == DVec3::ZERO {
                (b - op.a).any_orthonormal_vector()
            } else {
                side
            };
            let offset = side * dist.abs();
            if offset.length() > self.dim_tiny(op.a) {
                let c = match op.moving {
                    Some(id) => Command::MoveDimension { id, offset },
                    None => self.dim_add(op.a, b, offset),
                };
                if self.doc.run(c) {
                    self.model_changed();
                    if op.moving.is_none() {
                        self.effect_dimension(op.a, b, offset);
                    }
                }
            }
        }
        self.overlay_dirty = true;
        self.dirty = true;
        self.refresh();
        true
    }

    /// Delete removes the picked one. False if none is picked.
    pub(super) fn ann_delete(&mut self) -> bool {
        let Some(a) = self.ann_picked() else {
            return false;
        };
        self.cancel();
        match a {
            Ann::Section(id) => {
                self.section_run(Command::DeleteSection { id });
            }
            Ann::CPlane(id) => {
                self.work_plane_run(Command::DeleteWorkPlane { id });
            }
            Ann::Dim(id) => {
                if self.doc.run(Command::DeleteDimension { id }) {
                    self.model_changed();
                }
            }
        }
        self.ann_clear();
        true
    }

    /// A right-click in Select: what the menu is for. Not on the model, a
    /// nearby plane or dimension is picked; on the model, none is.
    pub(super) fn ann_right_click(&mut self, s: DVec2, on_model: bool) {
        if self.tool != SELECT {
            return;
        }
        if on_model {
            self.ann_clear();
        } else if let Some(a) = self.ann_pick(s, &self.view()) {
            self.ann_select(a);
        }
    }
}
