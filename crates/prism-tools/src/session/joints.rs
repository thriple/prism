//! Hinge and Slide joints (Fit & Mechanisms part 1): the Hinge and Slide
//! tools.
//!
//! - Click the first part, click the second, then click to set the axis:
//!   an edge's own direction, a hole or cylinder's axis, or a flat face's
//!   normal, wherever the cursor is (the usual inference, not limited to
//!   the two parts just picked). Esc at any point starts over.
//! - A joint changes no geometry; it is a document object of its own
//!   (`prism_doc::Joint`), the same way a Section Plane or Dimension is.

use std::f64::consts::{FRAC_PI_6, TAU};

use prism_doc::{Command, InstanceId, Joint, JointId, JointKind};
use prism_geom::{Curve, Surface};
use prism_math::{DVec2, DVec3};
use prism_ops::Selection;
use prism_view::View;
use prism_view::overlay::{Line, Overlay};
use prism_view::pick;

use super::edit::plane_point;
use super::sections::line_param;
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

/// A joint's bound being dragged (Alt + click on its handle): an angle
/// for a Hinge, a distance for a Slide, along its own axis or round it.
#[derive(Clone, Copy, Debug)]
pub(super) struct JointDrag {
    pub id: JointId,
    pub max: bool,
    pub t: f64,
}

/// What the bottom-centre island shows for the joint in context: each
/// bound that is set, as text to correct exactly. An unset bound has no
/// field here — drag its handle out on the gizmo to give it one.
#[derive(Clone, Debug, PartialEq)]
pub struct JointIsland {
    pub id: JointId,
    pub kind: JointKind,
    pub min: Option<String>,
    pub max: Option<String>,
}

/// A value typed into the island.
#[derive(Clone, Debug, PartialEq)]
pub enum JointIslandSet {
    Min(String),
    Max(String),
}

/// How near the cursor must be to a joint's gizmo or handle, in pixels.
const BORDER_PX: f64 = 8.0;
const HANDLE_PX: f64 = 10.0;
/// A Hinge's handle sits this far out when its bound is not set yet, so
/// there is something to drag; a Slide's, this share of the gizmo's reach.
const DEFAULT_ANGLE: f64 = FRAC_PI_6;
const DEFAULT_SLIDE: f64 = 0.4;

/// Where the screen point `s` is closest to the segment `a`-`b`, in
/// pixels; None if either end is behind the view.
fn screen_segment_dist(view: &View, s: DVec2, a: DVec3, b: DVec3) -> Option<f64> {
    let (pa, _) = view.to_screen(a)?;
    let (pb, _) = view.to_screen(b)?;
    let ab = pb - pa;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 {
        ((s - pa).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some(s.distance(pa + ab * t))
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

    /// The joint naming the one part this Rotate or Move would act on, if
    /// the selection is exactly that one whole part and nothing else: the
    /// axis it turns or slides about, and the id to clamp to.
    pub(super) fn joint_for_edit(&self, selection: &Selection) -> Option<(JointId, Joint)> {
        if self.groups.len() != 1 || !selection.is_empty() {
            return None;
        }
        let i = *self.groups.iter().next()?;
        self.doc.assembly().joint_of(i).map(|(id, j)| (id, *j))
    }

    /// `angle` brought inside `id`'s limit, unchanged if `id` is None or
    /// gone.
    pub(super) fn joint_clamp_angle(&self, id: Option<JointId>, angle: f64) -> f64 {
        id.and_then(|id| self.doc.assembly().joint(id))
            .map_or(angle, |j| j.clamp(angle))
    }

    /// `delta` kept along `id`'s own axis and brought inside its range,
    /// unchanged if `id` is None or gone.
    pub(super) fn joint_clamp_delta(&self, id: Option<JointId>, delta: DVec3) -> DVec3 {
        let Some(j) = id.and_then(|id| self.doc.assembly().joint(id)) else {
            return delta;
        };
        j.axis_dir * j.clamp(delta.dot(j.axis_dir))
    }

    /// How far a joint's gizmo reaches: a share of the model's size, the
    /// same idea as a section plane's own half-width.
    fn joint_reach(&self) -> f64 {
        self.scene
            .bounds
            .map_or(1.0, |(lo, hi)| ((hi - lo).length() * 0.2).max(0.3))
    }

    /// A bound's own value: what is set, or a default reach to drag out
    /// from if it is not.
    fn joint_bound_value(j: &Joint, max: bool, reach: f64) -> f64 {
        let set = if max { j.max } else { j.min };
        let default = match j.kind {
            JointKind::Hinge => DEFAULT_ANGLE,
            JointKind::Slide => reach * DEFAULT_SLIDE,
        };
        set.unwrap_or(if max { default } else { -default })
    }

    /// A point at value `v` (an angle for a Hinge, a distance for a
    /// Slide) along the gizmo.
    fn joint_point_at(j: &Joint, v: f64, reach: f64) -> DVec3 {
        match j.kind {
            JointKind::Slide => j.axis_origin + j.axis_dir * v,
            JointKind::Hinge => {
                let (u, w) = prism_infer::plane_frame(j.axis_dir);
                j.axis_origin + (u * v.cos() + w * v.sin()) * reach
            }
        }
    }

    /// Where a bound's handle sits in the world right now: wherever it is
    /// being dragged to, else what is set, else a default reach to drag
    /// out from.
    fn joint_handle_point(&self, id: JointId, j: &Joint, max: bool, reach: f64) -> DVec3 {
        let v = match self.joint_drag.filter(|d| d.id == id && d.max == max) {
            Some(d) => d.t,
            None => Self::joint_bound_value(j, max, reach),
        };
        Self::joint_point_at(j, v, reach)
    }

    /// The nearer handle under the cursor, of any joint, if within reach.
    pub(super) fn joint_handle_at(&self, s: DVec2, view: &View) -> Option<(JointId, bool)> {
        let reach = self.joint_reach();
        let mut best: Option<(f64, JointId, bool)> = None;
        for (id, j) in self.doc.assembly().joints().0 {
            for max in [true, false] {
                let p = self.joint_handle_point(*id, j, max, reach);
                let Some((q, _)) = view.to_screen(p) else {
                    continue;
                };
                let d = q.distance(s);
                if d <= HANDLE_PX && best.is_none_or(|b| d < b.0) {
                    best = Some((d, *id, max));
                }
            }
        }
        best.map(|(_, id, max)| (id, max))
    }

    /// The joint under the cursor's gizmo (its circle or its rail), not
    /// counting a handle (checked separately, and first: it wins where
    /// they overlap near an unset bound's default position).
    pub(super) fn joint_border_at(&self, s: DVec2, view: &View) -> Option<JointId> {
        let reach = self.joint_reach();
        let mut best: Option<(f64, JointId)> = None;
        for (id, j) in self.doc.assembly().joints().0 {
            let d = match j.kind {
                JointKind::Slide => screen_segment_dist(
                    view,
                    s,
                    j.axis_origin - j.axis_dir * reach,
                    j.axis_origin + j.axis_dir * reach,
                ),
                JointKind::Hinge => {
                    let (u, w) = prism_infer::plane_frame(j.axis_dir);
                    const N: usize = 48;
                    let at = |k: usize| {
                        let a = k as f64 / N as f64 * TAU;
                        j.axis_origin + (u * a.cos() + w * a.sin()) * reach
                    };
                    (0..N)
                        .filter_map(|k| screen_segment_dist(view, s, at(k), at(k + 1)))
                        .fold(None, |acc: Option<f64>, d| {
                            Some(acc.map_or(d, |a: f64| a.min(d)))
                        })
                }
            };
            if let Some(d) = d
                && d <= BORDER_PX
                && best.is_none_or(|b| d < b.0)
            {
                best = Some((d, *id));
            }
        }
        best.map(|b| b.1)
    }

    /// Alt + click on a handle: pick it up, to follow the cursor until the
    /// next click (or a typed value) sets it.
    pub(super) fn joint_pick_up_handle(&mut self, id: JointId, max: bool) {
        let reach = self.joint_reach();
        let Some(j) = self.doc.assembly().joint(id) else {
            return;
        };
        let t = Self::joint_bound_value(j, max, reach);
        self.joint_drag = Some(JointDrag { id, max, t });
    }

    /// The cursor moved while a handle is picked up.
    pub(super) fn joint_drag_hover(&mut self, s: DVec2, view: &View) {
        let Some(op) = self.joint_drag else {
            return;
        };
        let Some(j) = self.doc.assembly().joint(op.id) else {
            self.joint_drag = None;
            return;
        };
        let (o, d) = view.ray(s);
        let t = match j.kind {
            JointKind::Slide => line_param(o, d, j.axis_origin, j.axis_dir),
            JointKind::Hinge => plane_point(view, s, j.axis_origin, j.axis_dir).map(|p| {
                let (u, w) = prism_infer::plane_frame(j.axis_dir);
                let r = p - j.axis_origin;
                r.dot(w).atan2(r.dot(u))
            }),
        };
        if let Some(t) = t {
            self.joint_drag = Some(JointDrag { t, ..op });
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    /// The click that sets a picked-up handle down.
    pub(super) fn joint_drag_set(&mut self) {
        if let Some(op) = self.joint_drag.take() {
            self.run(Command::SetJointBound {
                id: op.id,
                max: op.max,
                value: Some(op.t),
            });
        }
    }

    /// Enter with a typed value while a handle is picked up: an angle in
    /// degrees for a Hinge, a distance for a Slide. True if it was used.
    pub(super) fn joint_drag_enter(&mut self, text: &str) -> bool {
        let Some(op) = self.joint_drag else {
            return false;
        };
        let Some(j) = self.doc.assembly().joint(op.id) else {
            return false;
        };
        let kind = j.kind;
        let value = match kind {
            JointKind::Hinge => text.trim().trim_end_matches('°').parse::<f64>().ok(),
            JointKind::Slide => self.number.length(text),
        };
        let Some(value) = value else {
            return false;
        };
        self.joint_drag = None;
        self.run(Command::SetJointBound {
            id: op.id,
            max: op.max,
            value: Some(if matches!(kind, JointKind::Hinge) {
                value.to_radians()
            } else {
                value
            }),
        });
        true
    }

    /// Remove the selected joint. False if none is selected.
    pub(super) fn joint_remove(&mut self, id: JointId) {
        self.run(Command::DeleteJoint { id });
    }

    pub(super) fn joint_set_stopped(&mut self, id: JointId, stopped: bool) {
        self.run(Command::SetJointStopped { id, stopped });
    }

    /// The joint Edit > Joint's actions and the right-click menu act on:
    /// the selected one, else the newest.
    fn joint_target(&self) -> Option<JointId> {
        let (all, _) = self.doc.assembly().joints();
        self.joint_sel
            .filter(|id| all.contains_key(id))
            .or_else(|| all.keys().next_back().copied())
    }

    /// Whether the document has any joint (for menus).
    pub fn has_joints(&self) -> bool {
        !self.doc.assembly().joints().0.is_empty()
    }

    /// Edit > Joint > Stop Joint, Unstop Joint or Remove Joint.
    pub(super) fn joint_action(&mut self, id: &str) {
        let Some(joint) = self.joint_target() else {
            return;
        };
        self.cancel();
        match id {
            "joint.stop" => self.joint_set_stopped(joint, true),
            "joint.unstop" => self.joint_set_stopped(joint, false),
            "joint.delete" => {
                self.joint_remove(joint);
                self.ann_clear();
            }
            _ => {}
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Remove Joint, from the bottom-centre island.
    pub fn joint_remove_selected(&mut self) {
        self.joint_action("joint.delete");
    }

    /// A bound's value as text: degrees for a Hinge, the document's own
    /// length units for a Slide.
    fn joint_bound_text(&self, kind: JointKind, v: f64) -> String {
        match kind {
            JointKind::Hinge => format!("{}", (v.to_degrees() * 1000.0).round() / 1000.0),
            JointKind::Slide => self.number.show(v),
        }
    }

    /// The bottom-centre island (brief 53's diegetic design): while a
    /// Hinge/Slide tool is active, or an existing joint is selected, the
    /// joint in context (`joint_target`), and each bound that is set, as
    /// a typed value to correct exactly. Nothing else lives here — no
    /// part picker, axis field or Stopped toggle; those are the gizmo's
    /// own job.
    pub fn joint_island(&self) -> Option<JointIsland> {
        if !is_joint_tool(&self.tool) && self.joint_sel.is_none() {
            return None;
        }
        let id = self.joint_target()?;
        let j = self.doc.assembly().joint(id)?;
        Some(JointIsland {
            id,
            kind: j.kind,
            min: j.min.map(|v| self.joint_bound_text(j.kind, v)),
            max: j.max.map(|v| self.joint_bound_text(j.kind, v)),
        })
    }

    /// The island's own typed field for a bound: parsed the same way the
    /// gizmo handle's typed value already is (degrees for a Hinge, a
    /// length for a Slide).
    pub fn joint_island_input(&mut self, op: JointIslandSet) {
        let Some(id) = self.joint_target() else {
            return;
        };
        let Some(j) = self.doc.assembly().joint(id) else {
            return;
        };
        let (max, text) = match op {
            JointIslandSet::Min(t) => (false, t),
            JointIslandSet::Max(t) => (true, t),
        };
        let value = match j.kind {
            JointKind::Hinge => text
                .trim()
                .trim_end_matches('°')
                .parse::<f64>()
                .ok()
                .map(f64::to_radians),
            JointKind::Slide => self.number.length(&text),
        };
        let Some(value) = value else {
            return;
        };
        self.run(Command::SetJointBound {
            id,
            max,
            value: Some(value),
        });
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Every joint's gizmo: a Hinge's circle (a filled arc between its
    /// bounds) or a Slide's rail (a bar between them), with a handle at
    /// each bound, solid where it is set.
    pub(super) fn joints_overlay(&self, o: &mut Overlay) {
        let reach = self.joint_reach();
        let fb = &self.style.feedback;
        for (id, j) in self.doc.assembly().joints().0 {
            let hot = self.joint_sel == Some(*id);
            let color = if hot { self.style.selected } else { fb.guide };
            match j.kind {
                JointKind::Slide => {
                    o.lines.push(Line {
                        a: j.axis_origin - j.axis_dir * reach,
                        b: j.axis_origin + j.axis_dir * reach,
                        color,
                        width: if hot { 2.0 } else { 1.0 },
                        dash: [0.0, 0.0],
                    });
                }
                JointKind::Hinge => {
                    let (u, w) = prism_infer::plane_frame(j.axis_dir);
                    const N: usize = 48;
                    for k in 0..N {
                        let a = |i: usize| i as f64 / N as f64 * TAU;
                        o.lines.push(Line {
                            a: j.axis_origin + (u * a(k).cos() + w * a(k).sin()) * reach,
                            b: j.axis_origin + (u * a(k + 1).cos() + w * a(k + 1).sin()) * reach,
                            color,
                            width: if hot { 2.0 } else { 1.0 },
                            dash: [0.0, 0.0],
                        });
                    }
                }
            }
            for max in [true, false] {
                let set = if max { j.max } else { j.min };
                let p = self.joint_handle_point(*id, j, max, reach);
                o.marks.push(prism_view::overlay::Mark {
                    at: p,
                    shape: prism_view::overlay::Shape::Diamond,
                    color: if set.is_some() { color } else { fb.woken },
                    size: fb.woken_size,
                });
            }
        }
    }
}
