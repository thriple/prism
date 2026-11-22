//! Align, Distribute and Drop to Face (brief 51 part 3): line the selected
//! groups and components up by their boxes, space them evenly, or let them
//! fall until they rest on what is below. Each is one undo step (a `Place`
//! for every group that has to move, in one batch).
//!
//! A group's box here is the smallest box along the red, green and blue
//! lines that holds its blue box, so a turned group is lined up by its
//! real extent.

use prism_doc::{Change, Command, InstanceId};
use prism_math::DVec3;

use super::Session;

pub const ALIGN_NEED_TWO: &str = "Select two or more groups or components to line up";
pub const DISTRIBUTE_NEED_THREE: &str = "Select three or more groups or components to space out";
pub const DROP_NOTHING: &str = "Select a group or component to drop";
pub const CENTRE_NOTHING: &str = "Select a group or component to centre";
pub const ALREADY: &str = "Already there: nothing moved";

/// Which side of the boxes a line-up goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Low,
    Middle,
    High,
}

type Bounds = (DVec3, DVec3);

fn bounds_of(corners: &[DVec3; 8]) -> Bounds {
    corners
        .iter()
        .fold((corners[0], corners[0]), |(l, h), p| (l.min(*p), h.max(*p)))
}

impl Session {
    /// The world box of each selected group.
    fn selected_boxes(&self) -> Vec<(InstanceId, Bounds)> {
        self.groups
            .iter()
            .filter_map(|g| self.group_box(*g).map(|c| (*g, bounds_of(&c))))
            .collect()
    }

    fn placement_notice(&mut self, text: &'static str) {
        self.notice = Some(text);
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Move groups by their own steps, as one undo step.
    fn move_each(&mut self, steps: Vec<(InstanceId, DVec3)>) {
        let tol = self.scene.tol.dist;
        let commands: Vec<Command> = steps
            .into_iter()
            .filter(|(_, by)| by.length() > tol)
            .map(|(g, by)| Command::Place {
                instances: [g].into(),
                change: Change::Translate { by },
            })
            .collect();
        match commands.len() {
            0 => self.placement_notice(ALREADY),
            1 => {
                self.cancel();
                self.run(commands.into_iter().next().expect("one"));
            }
            _ => {
                self.cancel();
                self.run(Command::Batch { commands });
            }
        }
    }

    /// Edit > Align: `align.x.low`, `align.y.middle`, `align.z.high`...
    pub(super) fn align_action(&mut self, id: &str) {
        let mut parts = id.split('.').skip(1);
        let axis = match parts.next() {
            Some("x") => 0,
            Some("y") => 1,
            Some("z") => 2,
            _ => return,
        };
        let side = match parts.next() {
            Some("low") => Side::Low,
            Some("middle") => Side::Middle,
            Some("high") => Side::High,
            _ => return,
        };
        let boxes = self.selected_boxes();
        if boxes.len() < 2 {
            return self.placement_notice(ALIGN_NEED_TWO);
        }
        if self.locked_selected() {
            return;
        }
        let lo = boxes
            .iter()
            .map(|(_, b)| b.0[axis])
            .fold(f64::MAX, f64::min);
        let hi = boxes
            .iter()
            .map(|(_, b)| b.1[axis])
            .fold(f64::MIN, f64::max);
        let steps = boxes
            .iter()
            .map(|(g, b)| {
                let by = match side {
                    Side::Low => lo - b.0[axis],
                    Side::High => hi - b.1[axis],
                    Side::Middle => (lo + hi) / 2.0 - (b.0[axis] + b.1[axis]) / 2.0,
                };
                let mut v = DVec3::ZERO;
                v[axis] = by;
                (*g, v)
            })
            .collect();
        self.move_each(steps);
    }

    /// Edit > Distribute: equal gaps between the boxes along an axis; the
    /// two outermost stay where they are.
    pub(super) fn distribute(&mut self, axis: usize) {
        let mut boxes = self.selected_boxes();
        if boxes.len() < 3 {
            return self.placement_notice(DISTRIBUTE_NEED_THREE);
        }
        if self.locked_selected() {
            return;
        }
        boxes.sort_by(|a, b| {
            let (ca, cb) = (a.1.0[axis] + a.1.1[axis], b.1.0[axis] + b.1.1[axis]);
            ca.total_cmp(&cb)
        });
        let first = boxes[0].1.0[axis];
        let last = boxes[boxes.len() - 1].1.1[axis];
        let sizes: f64 = boxes.iter().map(|(_, b)| b.1[axis] - b.0[axis]).sum();
        let gap = (last - first - sizes) / (boxes.len() - 1) as f64;
        let mut at = first;
        let mut steps = Vec::new();
        for (g, b) in &boxes {
            let mut v = DVec3::ZERO;
            v[axis] = at - b.0[axis];
            steps.push((*g, v));
            at += b.1[axis] - b.0[axis] + gap;
        }
        self.move_each(steps);
    }

    /// Edit > Center on Origin: the selected groups together, their box
    /// middle on the red and green zero lines (their height stays).
    pub(super) fn centre_on_origin(&mut self) {
        let boxes = self.selected_boxes();
        if boxes.is_empty() {
            return self.placement_notice(CENTRE_NOTHING);
        }
        if self.locked_selected() {
            return;
        }
        let lo = boxes.iter().fold(DVec3::MAX, |m, (_, b)| m.min(b.0));
        let hi = boxes.iter().fold(DVec3::MIN, |m, (_, b)| m.max(b.1));
        let by = DVec3::new(-(lo.x + hi.x) / 2.0, -(lo.y + hi.y) / 2.0, 0.0);
        self.move_each(boxes.iter().map(|(g, _)| (*g, by)).collect());
    }

    /// The boxes (low and high corners) of what could hold a dropped group
    /// up: the other groups and components in this context, and the flat
    /// faces that look up in it.
    fn supports(&self) -> Vec<Bounds> {
        let asm = self.doc.assembly();
        let mut out = Vec::new();
        if let Some(def) = asm.context_def(&self.ctx).and_then(|d| asm.def(d)) {
            for i in def.instances.keys() {
                if self.groups.contains(i) || !asm.is_shown(*i) {
                    continue;
                }
                if let Some(c) = self.group_box(*i) {
                    out.push(bounds_of(&c));
                }
            }
        }
        // Loose faces count at the top level only (inside an open group the
        // model is in the group's own coordinates).
        let m = self.work_model();
        for (_, f) in m.faces().filter(|_| self.ctx.is_empty()) {
            let Some(p) = f.plane() else { continue };
            if p.normal.z < 0.5 {
                continue;
            }
            let pts: Vec<DVec3> = f.outer.iter().map(|v| m.vertex(*v)).collect();
            if let Some(first) = pts.first() {
                let (lo, hi) = pts
                    .iter()
                    .fold((*first, *first), |(l, h), p| (l.min(*p), h.max(*p)));
                out.push((lo, hi));
            }
        }
        out
    }

    /// Edit > Drop to Face: each selected group falls straight down until
    /// it rests on the highest thing under it (another group's top, a flat
    /// face, or the ground). The lowest falls first, so a selected stack
    /// stays a stack.
    pub(super) fn drop_to_face(&mut self) {
        let mut boxes = self.selected_boxes();
        if boxes.is_empty() {
            return self.placement_notice(DROP_NOTHING);
        }
        if self.locked_selected() {
            return;
        }
        let tol = self.scene.tol.dist;
        let mut held = self.supports();
        boxes.sort_by(|a, b| a.1.0.z.total_cmp(&b.1.0.z));
        let mut steps = Vec::new();
        for (g, b) in boxes {
            // Under it: boxes that overlap it from above, and the ground
            // (only if it is not already below the ground).
            let mut top = if b.0.z >= -tol { Some(0.0) } else { None };
            for (lo, hi) in &held {
                let across = lo.x < b.1.x - tol
                    && hi.x > b.0.x + tol
                    && lo.y < b.1.y - tol
                    && hi.y > b.0.y + tol;
                if across && hi.z <= b.0.z + tol {
                    top = Some(top.map_or(hi.z, |t: f64| t.max(hi.z)));
                }
            }
            let by = top.map_or(0.0, |t| t - b.0.z);
            steps.push((g, DVec3::new(0.0, 0.0, by)));
            // What it lands on holds the next one.
            held.push((
                b.0 + DVec3::new(0.0, 0.0, by),
                b.1 + DVec3::new(0.0, 0.0, by),
            ));
        }
        self.move_each(steps);
    }
}
