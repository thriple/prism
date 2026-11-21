//! The Mirror tool (Shapes & Placement part 1): select groups or
//! components, pick the Mirror tool, and click a flat face: a mirrored copy
//! of each selected group appears on the other side of that face's plane.
//! Alt in place of a copy turns the groups themselves over. With an arrow
//! key (an axis lock) the plane goes through the snapped point, square to
//! that axis. Edit > Mirror Copy has the three planes through the
//! selection's centre.

use prism_doc::Change;
use prism_math::{DVec2, DVec3};
use prism_view::View;
use prism_view::overlay::{Line, Overlay};

use super::Session;

pub const MIRROR_TOOL: &str = "tool.mirror";

/// Said when the tool is clicked with nothing selected to mirror.
pub const MIRROR_NOTHING: &str = "Select a group or component to mirror first";

impl Session {
    /// Where the plane would go for a click here.
    fn mirror_plane(&self, s: DVec2, view: &View) -> Option<(DVec3, DVec3)> {
        self.section_place(s, view)
    }

    pub(super) fn mirror_hover(&mut self, s: DVec2, view: &View) {
        if self.tool != MIRROR_TOOL {
            return;
        }
        let ghost = self.mirror_plane(s, view);
        if ghost != self.mirror_ghost {
            self.mirror_ghost = ghost;
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    /// Mirror the selected groups across a plane: a copy, or (`in_place`)
    /// the groups themselves. One undo step.
    pub(super) fn mirror_across(&mut self, origin: DVec3, normal: DVec3, in_place: bool) {
        if self.groups.is_empty() {
            self.notice = Some(MIRROR_NOTHING);
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        self.cancel();
        let change = Change::Mirror { origin, normal };
        if let Some(c) = self.with_groups(None, change, !in_place) {
            self.run(c);
        }
    }

    pub(super) fn mirror_press(&mut self, s: DVec2) {
        self.update_hover(s);
        let view = self.view();
        if let Some((at, n)) = self.mirror_plane(s, &view) {
            let in_place = self.press_alt;
            self.mirror_across(at, n, in_place);
        }
        self.mirror_ghost = None;
        self.refresh();
    }

    /// Edit > Mirror Copy > Red, Green or Blue plane: through the centre
    /// of the selected groups together, square to that axis.
    pub(super) fn mirror_axis(&mut self, axis: usize) {
        let corners = self.groups_corners();
        if corners.is_empty() {
            self.notice = Some(MIRROR_NOTHING);
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        let (lo, hi) = corners
            .iter()
            .fold((corners[0], corners[0]), |(l, h), p| (l.min(*p), h.max(*p)));
        let mut normal = DVec3::ZERO;
        normal[axis] = 1.0;
        self.mirror_across((lo + hi) * 0.5, normal, false);
    }

    pub(super) fn mirror_hint(&self) -> Option<&'static str> {
        if self.tool != MIRROR_TOOL {
            return None;
        }
        Some(if self.groups.is_empty() {
            "Select the groups or components to mirror, then click a face to mirror across"
        } else {
            "Click a face to put a mirrored copy across it (Alt turns the groups over instead; an arrow key locks the plane's direction)"
        })
    }

    /// The plane the click would mirror across.
    pub(super) fn mirror_overlay(&self, o: &mut Overlay) {
        if self.tool != MIRROR_TOOL {
            return;
        }
        let Some((at, n)) = self.mirror_ghost else {
            return;
        };
        let (u, v) = prism_view::section::CutPlane {
            origin: at,
            normal: n,
        }
        .frame();
        let h = self.plane_half();
        let c = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .map(|(a, b)| at + u * (h * a) + v * (h * b));
        for k in 0..4 {
            o.lines.push(Line {
                a: c[k],
                b: c[(k + 1) % 4],
                color: self.style.feedback.section,
                width: 1.5,
                dash: [6.0, 4.0],
            });
        }
    }
}
