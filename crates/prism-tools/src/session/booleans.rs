//! The Solid menu's booleans as a tool (Solids & CAD part 2, brief 39).
//! With two or more groups selected, Union, Intersect and Split (exactly
//! two) run at once on them (Union of three or more keeps their outer
//! shell). Otherwise, as in SketchUp:
//! click the first solid (for Subtract and Trim, the cutter), then the
//! second (the solid to cut). Esc starts over.

use prism_doc::{Command, InstanceId, SolidOp};
use prism_math::DVec2;
use prism_view::{Hit, View};

use super::{Owner, Session};

pub const UNION: &str = "solid.union";
pub const SUBTRACT: &str = "solid.sub";
pub const INTERSECT: &str = "solid.isect";
pub const TRIM: &str = "solid.trim";
pub const SPLIT: &str = "solid.split";

/// Said when Intersect is asked of more than two groups.
pub const TWO_AT_A_TIME: &str = "Intersect takes two solids at a time.";

/// The six, in menu order.
pub const SOLID_TOOLS: [&str; 5] = [UNION, SUBTRACT, INTERSECT, TRIM, SPLIT];

pub(super) fn is_solid_tool(t: &str) -> bool {
    SOLID_TOOLS.contains(&t)
}

/// The boolean a tool runs.
fn op_of(t: &str) -> Option<SolidOp> {
    Some(match t {
        UNION => SolidOp::Union,
        SUBTRACT => SolidOp::Subtract,
        INTERSECT => SolidOp::Intersect,
        TRIM => SolidOp::Trim,
        SPLIT => SolidOp::Split,
        _ => return None,
    })
}

impl Session {
    /// The tool was picked: run on the selected groups when that is what
    /// they ask for.
    pub(super) fn solid_tool_picked(&mut self) {
        self.solid_first = None;
        let groups = self.groups.clone();
        let n = groups.len();
        let ready = match self.tool.as_str() {
            UNION | INTERSECT => n >= 2,
            SPLIT => n == 2,
            _ => false,
        };
        if !ready {
            return;
        }
        // Union of more than two is their outer shell (the old Outer Shell
        // command, removed 2026-11-16: Union does it); Intersect takes two.
        let c = match op_of(&self.tool) {
            Some(SolidOp::Intersect) if n > 2 => {
                self.notice = Some(TWO_AT_A_TIME);
                self.overlay_dirty = true;
                return;
            }
            Some(op) if n == 2 => {
                let mut it = groups.iter().copied();
                let (Some(first), Some(second)) = (it.next(), it.next()) else {
                    return;
                };
                Command::Boolean { op, first, second }
            }
            _ => Command::OuterShell { instances: groups },
        };
        self.solid_run(c);
    }

    /// The group under the cursor in the open one, if any.
    fn solid_under(&self, s: DVec2, view: &View) -> Option<(InstanceId, prism_view::Picked)> {
        let (o, d) = view.ray(s);
        let (f, t) = self.scene.ray_face(o, d)?;
        match self.owner(Hit::Face(f)) {
            Some(Owner::Child(i)) => Some((
                i,
                prism_view::Picked {
                    hit: Hit::Face(f),
                    point: o + d * t,
                },
            )),
            _ => None,
        }
    }

    pub(super) fn solid_hint(&self) -> Option<&'static str> {
        if !is_solid_tool(&self.tool) {
            return None;
        }
        Some(match (self.tool.as_str(), self.solid_first.is_some()) {
            (SUBTRACT | TRIM, false) => "Click the cutter: the solid that cuts",
            (SUBTRACT | TRIM, true) => "Click the solid to cut · Esc starts over",
            (_, false) => "Click the first solid group or component",
            (_, true) => "Click the second solid · Esc starts over",
        })
    }

    pub(super) fn solid_hover(&mut self, s: DVec2, view: &View) {
        let h = self.solid_under(s, view).map(|(_, p)| p);
        self.set_hover_pick(h);
    }

    pub(super) fn solid_press(&mut self, s: DVec2) {
        let view = self.view();
        let Some((g, _)) = self.solid_under(s, &view) else {
            return;
        };
        match self.solid_first {
            None => {
                self.solid_first = Some(g);
                self.set_selection(Default::default());
                self.set_groups([g].into());
            }
            Some(first) if first == g => {}
            Some(first) => {
                self.solid_first = None;
                let c = match op_of(&self.tool) {
                    Some(op) => Command::Boolean {
                        op,
                        first,
                        second: g,
                    },
                    None => Command::OuterShell {
                        instances: [first, g].into(),
                    },
                };
                self.solid_run(c);
            }
        }
    }

    /// Run a boolean, or say why not.
    fn solid_run(&mut self, c: Command) {
        let asm = self.doc.assembly();
        let ctx = asm.context_def(&self.ctx).unwrap_or(prism_doc::ROOT);
        let why = match &c {
            Command::Boolean { op, first, second } => {
                asm.boolean_results(ctx, *op, *first, *second).err()
            }
            Command::OuterShell { instances } => asm.outer_shell_result(ctx, instances).err(),
            _ => None,
        };
        self.cancel();
        self.set_selection(Default::default());
        self.set_groups(Default::default());
        if let Some(why) = why {
            self.notice = Some(why);
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        self.run(c);
    }
}
