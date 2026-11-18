//! Group Axes (brief 36): the Axes tool, and Change Axes, Reset Axes and
//! Axes to Centre for the selected groups.
//!
//! - **Axes tool:** inside an open group, click the origin, a point on the
//!   red axis, then a point that sets green (made square to red), all with
//!   inference. The group's contents keep their place in the world; only
//!   its axes change. At the top level the same three clicks set the
//!   model's own axes (Annotation & Sections part 4): everything moves so
//!   that the new axes are the world's, and the view moves with it.
//! - The axes are worked out in the group's own coordinates and kept rigid,
//!   so curved geometry stays exact even in a group scaled unevenly.

use std::collections::BTreeSet;

use prism_doc::{Command, DefId};
use prism_math::{DAffine3, DVec2, DVec3};
use prism_view::overlay::{Line, Overlay};

use super::Session;

pub const AXES_TOOL: &str = "tool.axes";

/// The axes drawn by the tool, in pixels on screen.
const AXES_PX: f64 = 80.0;

/// The Axes tool's clicks so far.
#[derive(Clone, Copy, Debug)]
pub(super) struct AxesOp {
    pub origin: DVec3,
    /// The point on red, once clicked.
    pub red: Option<DVec3>,
    /// The cursor's point.
    pub at: DVec3,
}

/// The frame a Change Model Axes command sets, if `c` is one.
pub(super) fn model_axes_of(c: &Command) -> Option<DAffine3> {
    match c {
        Command::ChangeModelAxes { axes } => Some(*axes),
        _ => None,
    }
}

/// Rigid axes at `origin` with red towards `red` and green towards `green`
/// (made square to red). None if the points are too close or in a line.
pub fn axes_from(origin: DVec3, red: DVec3, green: DVec3, tiny: f64) -> Option<DAffine3> {
    let x = red - origin;
    if x.length() <= tiny {
        return None;
    }
    let x = x.normalize();
    let g = green - origin;
    let y = g - x * g.dot(x);
    if y.length() <= tiny {
        return None;
    }
    let y = y.normalize();
    Some(DAffine3::from_cols(x, y, x.cross(y), origin))
}

impl Session {
    fn axes_tiny(&self, p: DVec3) -> f64 {
        self.scene.tol.dist.max(p.length() * 1e-12)
    }

    pub(super) fn axes_press(&mut self, s: DVec2) {
        self.update_hover(s);
        let Some(p) = self.inference.as_ref().map(|i| i.point) else {
            return;
        };
        let tiny = self.axes_tiny(p);
        match self.axes_op {
            None => {
                self.axes_op = Some(AxesOp {
                    origin: p,
                    red: None,
                    at: p,
                });
            }
            Some(mut a) => match a.red {
                None if p.distance(a.origin) > tiny => {
                    a.red = Some(p);
                    a.at = p;
                    self.axes_op = Some(a);
                }
                Some(red) if self.commit_axes(a.origin, red, p) => self.axes_op = None,
                _ => {}
            },
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    pub(super) fn axes_hover(&mut self) {
        let Some(p) = self.inference.as_ref().map(|i| i.point) else {
            return;
        };
        if let Some(a) = &mut self.axes_op {
            a.at = p;
        }
    }

    /// The open group's new axes from three world points. True if they
    /// were used.
    fn commit_axes(&mut self, origin: DVec3, red: DVec3, green: DVec3) -> bool {
        let Some(&id) = self.ctx.last() else {
            // At the top level: the model's own axes, in world coordinates.
            let Some(axes) = axes_from(origin, red, green, self.axes_tiny(origin)) else {
                return false;
            };
            self.cancel();
            if !self.run(Command::ChangeModelAxes { axes }) {
                return false;
            }
            self.move_view(&axes.inverse());
            return true;
        };
        let local = self.to_world.inverse();
        let [o, r, g] = [origin, red, green].map(|p| local.transform_point3(p));
        let Some(axes) = axes_from(o, r, g, self.axes_tiny(o)) else {
            return false;
        };
        self.run(Command::ChangeAxes { instance: id, axes });
        true
    }

    /// Carry the camera along when the model's axes change (`m` is what
    /// the model itself was moved by), so the view looks the same.
    pub(super) fn move_view(&mut self, m: &DAffine3) {
        self.settle_camera();
        self.camera.eye = m.transform_point3(self.camera.eye);
        self.camera.target = m.transform_point3(self.camera.target);
        self.dirty = true;
    }

    /// The selected groups, one per definition (copies of a component
    /// share their axes).
    fn axes_targets(&self) -> Vec<(prism_doc::InstanceId, DefId)> {
        let asm = self.doc.assembly();
        let mut seen = BTreeSet::new();
        self.groups
            .iter()
            .filter_map(|i| {
                let (_, inst) = asm.find_instance(*i)?;
                seen.insert(inst.def).then_some((*i, inst.def))
            })
            .collect()
    }

    /// Edit > Axes > Change Axes: open the selected group and start the
    /// Axes tool in it.
    pub(super) fn change_axes(&mut self) {
        if self.groups.len() != 1 {
            return;
        }
        let g = *self.groups.iter().next().expect("one group");
        self.open_group(g);
        if self.ctx.last() == Some(&g) {
            self.set_tool(AXES_TOOL);
        }
    }

    /// Edit > Axes > Reset Axes, or Axes to Centre (`centre`).
    pub(super) fn reset_axes(&mut self, centre: bool) {
        let asm = self.doc.assembly();
        let commands: Vec<Command> = self
            .axes_targets()
            .into_iter()
            .filter_map(|(i, def)| {
                let axes = if centre {
                    asm.centre_axes(def)?
                } else {
                    asm.reset_axes(i)?
                };
                Some(Command::ChangeAxes { instance: i, axes })
            })
            .collect();
        if commands.is_empty() {
            return;
        }
        self.cancel();
        if commands.len() == 1 {
            self.run(commands.into_iter().next().expect("one"));
        } else {
            self.run(Command::Batch { commands });
        }
    }

    pub(super) fn axes_hint(&self) -> Option<&'static str> {
        if self.tool != AXES_TOOL {
            return None;
        }
        let what = if self.ctx.is_empty() {
            "model"
        } else {
            "group"
        };
        Some(match self.axes_op.map(|a| a.red.is_some()) {
            None if what == "model" => "Click the model's new origin",
            None => "Click the new origin",
            Some(false) => "Click a point on the red axis · Esc cancels",
            Some(true) => "Click a point that sets green · Esc cancels",
        })
    }

    /// The axes being set, or the open group's own while the tool waits.
    pub(super) fn axes_overlay(&self, o: &mut Overlay) {
        if self.tool != AXES_TOOL {
            return;
        }
        let (origin, dirs) = match self.axes_op {
            None => {
                let m = self.to_world;
                (
                    m.translation,
                    [m.matrix3.x_axis, m.matrix3.y_axis, m.matrix3.z_axis]
                        .map(DVec3::normalize_or_zero),
                )
            }
            Some(a) => {
                let (red, green) = match a.red {
                    None => (a.at, None),
                    Some(r) => (r, Some(a.at)),
                };
                let x = (red - a.origin).normalize_or_zero();
                let frame = green.and_then(|g| axes_from(a.origin, red, g, self.axes_tiny(g)));
                match frame {
                    Some(f) => (
                        a.origin,
                        [f.matrix3.x_axis, f.matrix3.y_axis, f.matrix3.z_axis],
                    ),
                    None => (a.origin, [x, DVec3::ZERO, DVec3::ZERO]),
                }
            }
        };
        let view = self.view();
        let Some((_, depth)) = view.to_screen(origin) else {
            return;
        };
        let len = view.units_per_px(depth) * AXES_PX;
        for (k, d) in dirs.iter().enumerate() {
            if *d == DVec3::ZERO {
                continue;
            }
            o.lines.push(Line {
                a: origin,
                b: origin + *d * len,
                color: self.style.axes[k],
                width: self.style.axis_width,
                dash: [0.0, 0.0],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn green_is_made_square_to_red() {
        let a = axes_from(
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(1.0, 3.0, 0.0),
            DVec3::new(-2.0, 2.0, 0.0),
            1e-9,
        )
        .expect("axes");
        assert!(a.matrix3.x_axis.abs_diff_eq(DVec3::Y, 1e-12));
        assert!(a.matrix3.y_axis.abs_diff_eq(-DVec3::X, 1e-12));
        assert!(a.matrix3.z_axis.abs_diff_eq(DVec3::Z, 1e-12));
        assert!(axes_from(DVec3::ZERO, DVec3::X, DVec3::X * 2.0, 1e-9).is_none());
    }
}
