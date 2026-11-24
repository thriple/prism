//! Hinge and Slide joints (Fit & Mechanisms part 1): the Hinge and Slide
//! tools.
//!
//! - Click the first part, click the second, then click to set the axis:
//!   an edge's own direction, a hole or cylinder's axis, or a flat face's
//!   normal, wherever the cursor is (the usual inference, not limited to
//!   the two parts just picked). Esc at any point starts over.
//! - A joint changes no geometry; it is a document object of its own
//!   (`prism_doc::Joint`), the same way a Section Plane or Dimension is.

use prism_doc::{Command, InstanceId, JointKind};
use prism_geom::{Curve, Surface};
use prism_math::{DVec2, DVec3};
use prism_view::View;
use prism_view::overlay::{Line, Overlay};
use prism_view::pick;

use super::{Owner, Session};

pub const HINGE_TOOL: &str = "tool.hinge";
pub const SLIDE_TOOL: &str = "tool.slide";

pub(super) fn is_joint_tool(t: &str) -> bool {
    t == HINGE_TOOL || t == SLIDE_TOOL
}

/// The Hinge/Slide tool's clicks so far.
#[derive(Clone, Copy, Debug)]
pub(super) struct JointOp {
    pub a: InstanceId,
    pub b: Option<InstanceId>,
}

impl Session {
    fn joint_kind(&self) -> Option<JointKind> {
        match self.tool.as_str() {
            HINGE_TOOL => Some(JointKind::Hinge),
            SLIDE_TOOL => Some(JointKind::Slide),
            _ => None,
        }
    }

    /// The part under the cursor, in the open group, if any.
    fn joint_part_at(&self, s: DVec2) -> Option<InstanceId> {
        let hit = pick::pick(&self.scene, &self.view(), s, self.settings.aperture)?.hit;
        match self.owner(hit)? {
            Owner::Child(id) => Some(id),
            Owner::Outside => None,
        }
    }

    /// Where the cursor's inference lands, and along what direction: an
    /// edge's own direction (a hole or cylinder's own axis for an arc), or
    /// a face's normal (its axis, for a cylinder or cone). Read from the
    /// scene, not the work model, since the hit can be merged in from a
    /// closed group (a different id space, PLAN 1.2).
    fn joint_axis_candidate(&self) -> Option<(DVec3, DVec3)> {
        let i = self.inference.as_ref()?;
        if let Some(e) = i.edge
            && let Some(edge) = self.scene.edge(e)
        {
            if let Curve::Arc { centre, axis, .. } = edge.curve {
                return Some((centre, axis));
            }
            if let Some(d) = i.edge_dir {
                return Some((i.point, d));
            }
        }
        if let Some(f) = i.face
            && let Some(face) = self.scene.face(f)
        {
            let dir = match &face.curved {
                None => Some(face.normal),
                Some(c) => match &c.surface {
                    Surface::Cylinder(cy) => Some(cy.axis),
                    Surface::Cone(co) => Some(co.axis),
                    _ => None,
                },
            };
            if let Some(d) = dir {
                return Some((i.point, d));
            }
        }
        None
    }

    pub(super) fn joint_press(&mut self, s: DVec2) {
        let Some(kind) = self.joint_kind() else {
            return;
        };
        self.update_hover(s);
        match self.joint_op {
            None => {
                let Some(a) = self.joint_part_at(s) else {
                    return;
                };
                self.set_selection(Default::default());
                self.set_groups([a].into());
                self.joint_op = Some(JointOp { a, b: None });
            }
            Some(JointOp { a, b: None }) => {
                let Some(b) = self.joint_part_at(s) else {
                    return;
                };
                if b == a {
                    return;
                }
                self.set_groups([a, b].into());
                self.joint_op = Some(JointOp { a, b: Some(b) });
            }
            Some(JointOp { a, b: Some(b) }) => {
                let Some((origin, dir)) = self.joint_axis_candidate() else {
                    return;
                };
                self.joint_op = None;
                self.set_selection(Default::default());
                self.set_groups(Default::default());
                self.run(Command::AddJoint {
                    kind,
                    a,
                    b,
                    origin,
                    dir,
                });
            }
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    pub(super) fn joint_hover(&mut self, s: DVec2, view: &View) {
        if !is_joint_tool(&self.tool) || self.joint_op.is_some_and(|op| op.b.is_some()) {
            return;
        }
        let h = pick::pick(&self.scene, view, s, self.settings.aperture)
            .filter(|p| matches!(self.owner(p.hit), Some(Owner::Child(_))));
        self.set_hover_pick(h);
    }

    pub(super) fn joint_hint(&self) -> Option<&'static str> {
        if !is_joint_tool(&self.tool) {
            return None;
        }
        let what = if self.tool == HINGE_TOOL {
            "Hinge"
        } else {
            "Slide"
        };
        Some(match self.joint_op {
            None => match what {
                "Hinge" => "Click the first part to hinge",
                _ => "Click the first part to slide",
            },
            Some(JointOp { b: None, .. }) => "Click the second part · Esc starts over",
            Some(_) => {
                "Click an edge, a hole or cylinder's axis, or a face to set the axis · Esc starts over"
            }
        })
    }

    /// The axis candidate, drawn through the cursor's inference while the
    /// third click is still to come.
    pub(super) fn joint_overlay(&self, o: &mut Overlay) {
        let Some(JointOp { b: Some(_), .. }) = self.joint_op else {
            return;
        };
        let Some((origin, dir)) = self.joint_axis_candidate() else {
            return;
        };
        let view = self.view();
        let Some((_, depth)) = view.to_screen(origin) else {
            return;
        };
        let len = view.units_per_px(depth) * 80.0;
        let fb = &self.style.feedback;
        o.lines.push(Line {
            a: origin - dir * len,
            b: origin + dir * len,
            color: fb.rubber_band,
            width: fb.rubber_band_width,
            dash: [0.0, 0.0],
        });
    }
}
