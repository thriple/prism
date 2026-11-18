//! Construction planes (Annotation & Sections part 4): the Construction
//! Plane tool and the Edit > Construction Plane commands.
//!
//! - **Tool:** hover a point (it snaps like any drawing tool): a ghost
//!   plane shows where it would go, square to the face under the cursor, or
//!   to the ground, or to the axis the arrows lock. Click places it; it
//!   becomes the working plane. Esc leaves the tool.
//! - **Working plane:** while one is in use (and shown), the cursor lands
//!   on it where it is drawn, in front of the ground and anything behind
//!   it, and every drawing tool draws on it. A face in front of it still
//!   wins. Hiding the planes (View > Construction Planes) stops using them.
//! - Planes are in the world and change no geometry.
//! - Drawing: a square outline; the plane in use is solid, the others
//!   dashed.

use prism_doc::{Command, WorkPlaneId};
use prism_infer::WorkPlane;
use prism_math::DVec3;
use prism_view::overlay::{Line, Overlay};

use super::Session;

pub const CPLANE_TOOL: &str = "tool.cplane";

impl Session {
    /// The working plane the inference can land on, if one is in use.
    pub(super) fn work_plane_ctx(&self) -> Option<WorkPlane> {
        if !self.view.work_planes {
            return None;
        }
        let (_, p) = self.doc.assembly().active_work_plane()?;
        Some(WorkPlane {
            origin: p.origin,
            normal: p.normal,
            half: self.plane_half(),
        })
    }

    fn work_plane_corners(&self, origin: DVec3, normal: DVec3) -> [DVec3; 4] {
        let (u, v) = prism_infer::plane_frame(normal);
        let h = self.plane_half();
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .map(|(a, b)| origin + u * (h * a) + v * (h * b))
    }

    /// The plane the Edit > Construction Plane commands act on: the one in
    /// use, else the newest.
    fn work_plane_target(&self) -> Option<WorkPlaneId> {
        let asm = self.doc.assembly();
        asm.active_work_plane()
            .map(|(id, _)| id)
            .or_else(|| asm.work_planes().0.keys().next_back().copied())
    }

    /// Whether the document has any construction plane (for menus).
    pub fn has_work_planes(&self) -> bool {
        !self.doc.assembly().work_planes().0.is_empty()
    }

    /// Add a plane and use it (for the assistant): its id.
    pub fn add_work_plane(&mut self, origin: DVec3, normal: DVec3) -> Option<u32> {
        self.cancel();
        if !self.work_plane_run(Command::AddWorkPlane { origin, normal }) {
            return None;
        }
        let (id, _) = self.doc.assembly().active_work_plane()?;
        Some(id.0)
    }

    /// Use a plane as the working plane, or stop (for the assistant).
    /// False if there is no such plane.
    pub fn set_work_plane_active(&mut self, id: u32, active: bool) -> bool {
        let id = WorkPlaneId(id);
        if self.doc.assembly().work_plane(id).is_none() {
            return false;
        }
        self.cancel();
        self.work_plane_run(Command::SetWorkPlaneActive { id, active });
        true
    }

    /// Run a plane command: planes are in the world, whichever group is
    /// open.
    fn work_plane_run(&mut self, c: Command) -> bool {
        if self.doc.run(c) {
            self.model_changed();
            true
        } else {
            false
        }
    }

    /// Where a plane would go: the snapped point, square to the face, the
    /// plane or the axis the cursor is on.
    pub(super) fn work_plane_hover(&mut self) {
        if self.tool != CPLANE_TOOL {
            return;
        }
        self.cplane_ghost = self
            .inference
            .clone()
            .map(|i| (i.point, self.plane_normal(&i)));
    }

    pub(super) fn work_plane_press(&mut self, s: prism_math::DVec2) {
        self.update_hover(s);
        self.work_plane_hover();
        if let Some((origin, normal)) = self.cplane_ghost
            && self.work_plane_run(Command::AddWorkPlane { origin, normal })
        {
            self.overlay_dirty = true;
            self.dirty = true;
            self.refresh();
        }
    }

    pub(super) fn work_plane_hint(&self) -> Option<&'static str> {
        (self.tool == CPLANE_TOOL).then_some(
            "Click to place a construction plane: square to the face or ground under the cursor, or to an axis the arrows lock",
        )
    }

    /// Edit > Construction Plane > Use / Delete.
    pub(super) fn work_plane_action(&mut self, id: &str) {
        let Some(plane) = self.work_plane_target() else {
            return;
        };
        self.cancel();
        let active = self.doc.assembly().active_work_plane().map(|(a, _)| a);
        match id {
            "cplane.active" => {
                self.work_plane_run(Command::SetWorkPlaneActive {
                    id: plane,
                    active: active != Some(plane),
                });
            }
            "cplane.delete" => {
                self.work_plane_run(Command::DeleteWorkPlane { id: plane });
            }
            _ => {}
        }
    }

    /// The planes, and the ghost while the tool waits.
    pub(super) fn work_plane_overlay(&self, o: &mut Overlay) {
        if !self.view.work_planes {
            return;
        }
        let colour = self.style.feedback.work_plane;
        let active = self.doc.assembly().active_work_plane().map(|(a, _)| a);
        let mut outline = |origin: DVec3, normal: DVec3, solid: bool| {
            let c = self.work_plane_corners(origin, normal);
            let dash = if solid { [0.0, 0.0] } else { [6.0, 4.0] };
            for k in 0..4 {
                o.lines.push(Line {
                    a: c[k],
                    b: c[(k + 1) % 4],
                    color: colour,
                    width: 1.5,
                    dash,
                });
            }
            // A cross at the plane's origin.
            let (u, v) = prism_infer::plane_frame(normal);
            let h = self.plane_half() * 0.05;
            for d in [u, v] {
                o.lines.push(Line {
                    a: origin - d * h,
                    b: origin + d * h,
                    color: colour,
                    width: 1.5,
                    dash: [0.0, 0.0],
                });
            }
        };
        for (id, p) in self.doc.assembly().work_planes().0 {
            outline(p.origin, p.normal, active == Some(*id));
        }
        if self.tool == CPLANE_TOOL
            && let Some((at, n)) = self.cplane_ghost
        {
            outline(at, n, false);
        }
    }
}
