//! Section planes (Annotation & Sections part 1): the Section Plane tool
//! and the Edit > Section commands.
//!
//! - **Tool:** hover a flat face: a ghost plane lies on it. Click places a
//!   plane there; it then follows the mouse along its normal, and the next
//!   click sets it. Click a plane's border to pick it up the same way.
//!   Esc leaves it where it is.
//! - A plane cuts away the side its normal points to (the side the viewer
//!   was on when it was placed), in the viewport only: the model is not
//!   changed. Only one plane cuts at a time (the newest, or the one made
//!   active).
//! - Drawing: each plane is a square outline with a short arrow toward the
//!   side that is cut away. The one that cuts is solid; the others dashed.

use prism_doc::{Command, SectionId};
use prism_math::{DVec2, DVec3};
use prism_view::View;
use prism_view::overlay::{Line, Overlay};

use super::Session;

pub const SECTION_TOOL: &str = "tool.section";

/// How near a click must be to a plane's outline to pick it up, in pixels.
const BORDER_PX: f64 = 8.0;

/// A plane being moved along its normal.
#[derive(Clone, Copy, Debug)]
pub(super) struct SectionOp {
    pub id: SectionId,
    pub origin: DVec3,
    pub normal: DVec3,
    /// Where on the normal's line the cursor was when it began, and is now.
    pub t0: f64,
    pub t: f64,
}

impl SectionOp {
    pub(super) fn moved_to(&self) -> DVec3 {
        self.origin + self.normal * (self.t - self.t0)
    }
}

/// Where along the line `p + n·t` the ray `o + d·s` passes closest.
/// None if the ray runs along the line.
pub fn line_param(o: DVec3, d: DVec3, p: DVec3, n: DVec3) -> Option<f64> {
    let w = p - o;
    let b = n.dot(d);
    let den = 1.0 - b * b;
    if den < 1e-9 {
        return None;
    }
    Some((b * d.dot(w) - n.dot(w)) / den)
}

impl Session {
    /// The planes' drawing size: a little more than the model.
    pub(super) fn plane_half(&self) -> f64 {
        self.scene
            .bounds
            .map_or(2.0, |(lo, hi)| ((hi - lo).length() * 0.6).max(1.0))
    }

    fn plane_corners(&self, origin: DVec3, normal: DVec3) -> [DVec3; 4] {
        let (u, v) = prism_view::section::CutPlane { origin, normal }.frame();
        let h = self.plane_half();
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .map(|(a, b)| origin + u * (h * a) + v * (h * b))
    }

    /// Cut a finished scene by the active section plane, if Section
    /// Display is on.
    pub(super) fn section_cut(&self, asm: &prism_doc::Assembly, scene: &mut prism_view::Scene) {
        if !self.view.section {
            return;
        }
        if let Some((_, sec)) = asm.active_section() {
            scene.cut_by(
                prism_view::section::CutPlane {
                    origin: sec.origin,
                    normal: sec.normal,
                },
                self.style.feedback.section_cap,
            );
        }
    }

    /// The plane the Edit > Section commands act on: the picked one, else
    /// the one that cuts, else the newest.
    fn section_target(&self) -> Option<SectionId> {
        let (all, _) = self.doc.assembly().sections();
        self.section_sel
            .filter(|id| all.contains_key(id))
            .or_else(|| self.doc.assembly().active_section().map(|(id, _)| id))
            .or_else(|| all.keys().next_back().copied())
    }

    /// Whether the document has any section plane (for menus).
    pub fn has_sections(&self) -> bool {
        !self.doc.assembly().sections().0.is_empty()
    }

    /// Add a plane (for the assistant): its id.
    pub fn add_section_plane(&mut self, origin: DVec3, normal: DVec3) -> Option<u32> {
        self.cancel();
        if !self.section_run(Command::AddSection { origin, normal }) {
            return None;
        }
        let (id, _) = self.doc.assembly().active_section()?;
        self.section_sel = Some(id);
        Some(id.0)
    }

    /// Make a plane the one that cuts, or stop it (for the assistant).
    /// False if there is no such plane.
    pub fn set_section_plane_active(&mut self, id: u32, active: bool) -> bool {
        let id = SectionId(id);
        if self.doc.assembly().section(id).is_none() {
            return false;
        }
        self.cancel();
        self.section_run(Command::SetSectionActive { id, active });
        true
    }

    /// Run a section command: planes are in the world, whichever group is
    /// open.
    pub(super) fn section_run(&mut self, c: Command) -> bool {
        let placed = match c {
            Command::AddSection { origin, normal } => Some((origin, normal)),
            _ => None,
        };
        if self.doc.run(c) {
            self.model_changed();
            if let Some((o, n)) = placed {
                let c = self.plane_corners(o, n);
                self.effect_outline(c);
            }
            true
        } else {
            false
        }
    }

    /// The plane outline under the cursor, if any.
    pub(super) fn section_border_at(&self, s: DVec2, view: &View) -> Option<SectionId> {
        let mut best: Option<(f64, SectionId)> = None;
        for (id, sec) in self.doc.assembly().sections().0 {
            let c = self.plane_corners(sec.origin, sec.normal);
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

    /// Where a new plane goes: with an axis locked, through the snapped
    /// point, square to that axis; otherwise on the flat face under the
    /// cursor. Its normal is turned to face the viewer.
    fn section_place(&self, s: DVec2, view: &View) -> Option<(DVec3, DVec3)> {
        if let prism_infer::Lock::Axis(a) = self.lock
            && let Some(i) = &self.inference
        {
            let (_, d) = view.ray(s);
            let n = prism_infer::AXES[a as usize];
            return Some((i.point, if n.dot(d) > 0.0 { -n } else { n }));
        }
        self.section_face_at(s, view)
    }

    /// Where a plane moved along its normal goes, as `t` on the normal's
    /// line (see [`SectionOp`]): level with a snapped point (a corner, a
    /// midpoint, an edge, a guide), else where the cursor's ray passes
    /// closest to the line.
    pub(super) fn plane_t(
        &self,
        s: DVec2,
        view: &View,
        origin: DVec3,
        normal: DVec3,
        t0: f64,
    ) -> Option<f64> {
        use prism_infer::Kind;
        if let Some(i) = &self.inference
            && !matches!(
                i.kind,
                Kind::OnFace | Kind::OnPlane | Kind::OnGround | Kind::Free
            )
        {
            return Some(t0 + (i.point - origin).dot(normal));
        }
        let (o, d) = view.ray(s);
        line_param(o, d, origin, normal)
    }

    /// The flat face under the cursor: where, and its normal turned to face
    /// the viewer.
    fn section_face_at(&self, s: DVec2, view: &View) -> Option<(DVec3, DVec3)> {
        let (o, d) = view.ray(s);
        let (id, t) = self.scene.ray_face(o, d)?;
        let f = self.scene.face(id)?;
        let n = f.flat_normal()?;
        let at = o + d * t;
        Some((at, if n.dot(d) > 0.0 { -n } else { n }))
    }

    pub(super) fn section_press(&mut self, s: DVec2) {
        self.update_hover(s);
        let view = self.view();
        let (o, d) = view.ray(s);
        match self.section_op.take() {
            // The second click sets the plane where it is.
            Some(op) => {
                if (op.t - op.t0).abs() > self.scene.tol.dist {
                    let origin = op.moved_to();
                    self.section_run(Command::MoveSection { id: op.id, origin });
                }
            }
            None => {
                if let Some(id) = self.section_border_at(s, &view) {
                    self.section_sel = Some(id);
                    if let Some(sec) = self.doc.assembly().section(id)
                        && let Some(t) = line_param(o, d, sec.origin, sec.normal)
                    {
                        self.section_op = Some(SectionOp {
                            id,
                            origin: sec.origin,
                            normal: sec.normal,
                            t0: t,
                            t,
                        });
                    }
                } else if let Some((at, normal)) = self.section_place(s, &view)
                    && self.section_run(Command::AddSection { origin: at, normal })
                    && let Some((id, _)) = self.doc.assembly().active_section()
                {
                    self.section_sel = Some(id);
                    if let Some(t) = line_param(o, d, at, normal) {
                        self.section_op = Some(SectionOp {
                            id,
                            origin: at,
                            normal,
                            t0: t,
                            t,
                        });
                    }
                }
            }
        }
        self.section_ghost = None;
        self.overlay_dirty = true;
        self.dirty = true;
        self.refresh();
    }

    pub(super) fn section_hover(&mut self, s: DVec2, view: &View) {
        if self.tool != SECTION_TOOL && self.section_op.is_none() {
            return;
        }
        if let Some(op) = self.section_op {
            if let Some(t) = self.plane_t(s, view, op.origin, op.normal, op.t0)
                && let Some(op) = &mut self.section_op
            {
                op.t = t;
            }
            return;
        }
        let ghost = self.section_place(s, view);
        if ghost != self.section_ghost {
            self.section_ghost = ghost;
        }
    }

    pub(super) fn section_hint(&self) -> Option<&'static str> {
        if self.tool != SECTION_TOOL {
            return None;
        }
        Some(if self.section_op.is_some() {
            "Move the plane along its normal · click to set it · Esc leaves it"
        } else {
            "Click a face to place a section plane (an arrow key locks its direction) · click a plane's outline to move it"
        })
    }

    /// Edit > Section > Flip, Activate, Delete.
    pub(super) fn section_action(&mut self, id: &str) {
        let Some(sec) = self.section_target() else {
            return;
        };
        self.cancel();
        let active = self.doc.assembly().active_section().map(|(a, _)| a);
        match id {
            "section.flip" => {
                self.section_run(Command::FlipSection { id: sec });
            }
            "section.active" => {
                self.section_run(Command::SetSectionActive {
                    id: sec,
                    active: active != Some(sec),
                });
            }
            "section.delete" => {
                self.section_run(Command::DeleteSection { id: sec });
                self.section_sel = None;
            }
            _ => {}
        }
    }

    /// The planes, the ghost while the tool waits, and the plane being moved.
    pub(super) fn section_overlay(&self, o: &mut Overlay) {
        let colour = self.style.feedback.section;
        let active = self.doc.assembly().active_section().map(|(a, _)| a);
        let picked = self.section_sel;
        let mut outline = |origin: DVec3, normal: DVec3, solid: bool, hot: bool| {
            let c = self.plane_corners(origin, normal);
            let dash = if solid { [0.0, 0.0] } else { [6.0, 4.0] };
            let width = if hot { 2.5 } else { 1.5 };
            let color = if hot { self.style.selected } else { colour };
            for k in 0..4 {
                o.lines.push(Line {
                    a: c[k],
                    b: c[(k + 1) % 4],
                    color,
                    width,
                    dash,
                });
            }
            // The arrow toward the side that is cut away.
            let h = self.plane_half() * 0.3;
            o.lines.push(Line {
                a: origin,
                b: origin + normal * h,
                color,
                width,
                dash: [0.0, 0.0],
            });
        };
        if self.view.section_planes {
            for (id, sec) in self.doc.assembly().sections().0 {
                if self.section_op.is_some_and(|op| op.id == *id) {
                    continue;
                }
                outline(
                    sec.origin,
                    sec.normal,
                    active == Some(*id),
                    picked == Some(*id),
                );
            }
        }
        if let Some(op) = &self.section_op {
            outline(op.moved_to(), op.normal, true, true);
        } else if self.tool == SECTION_TOOL
            && let Some((at, n)) = self.section_ghost
        {
            outline(at, n, false, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ray_finds_its_place_along_a_normal() {
        // A ray from above looking down at the point (2, 0, 0): along the x
        // normal through the origin it passes at x = 2.
        let t = line_param(DVec3::new(2.0, 0.0, 5.0), -DVec3::Z, DVec3::ZERO, DVec3::X)
            .expect("a place");
        assert!((t - 2.0).abs() < 1e-12);
        // Along the line itself: none.
        assert!(line_param(DVec3::new(0.0, 0.0, 5.0), -DVec3::Z, DVec3::ZERO, DVec3::Z).is_none());
    }
}
