//! The Scale tool's box and grips (owner, 2026-11-09), as in SketchUp: a
//! box round the selection, standing off it by 5% as the open group's box
//! does, with grips at its corners (scale evenly), edge middles (two axes)
//! and face middles (one axis). Dragging a grip scales about the opposite
//! grip, or about the centre after Alt. Shift switches an edge or face
//! grip between its own axes and even scaling. The drag follows the line
//! from the anchor through the grip, so no inference is used.

use prism_math::{DVec2, DVec3};
use prism_view::View;
use prism_view::overlay::{Line, Mark, Overlay, Shape};

use super::Session;
use super::edit::{EditOp, along_line};

/// How far the box stands off its contents, as a share of their largest
/// size (as the open group's box).
const MARGIN: f64 = 0.05;
/// A grip is under the cursor within this many points.
const GRIP_REACH: f64 = 9.0;
/// Scale factors are clamped to this range.
const MIN: f64 = 0.01;
const MAX: f64 = 100.0;
/// A dragged factor this close to 1 is 1.
const SNAP: f64 = 0.02;

/// The contents' box, and the box drawn round it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Boxes {
    pub lo: DVec3,
    pub hi: DVec3,
    pub dlo: DVec3,
    pub dhi: DVec3,
}

impl Boxes {
    fn flat(&self, a: usize) -> bool {
        self.hi[a] - self.lo[a] <= 1e-12
    }

    /// A grip's place: per axis 0 (low), 1 (middle) or 2 (high).
    fn at(&self, g: [u8; 3], drawn: bool) -> DVec3 {
        let (lo, hi) = if drawn {
            (self.dlo, self.dhi)
        } else {
            (self.lo, self.hi)
        };
        DVec3::from_array(std::array::from_fn(|a| match g[a] {
            0 => lo[a],
            2 => hi[a],
            _ => 0.5 * (lo[a] + hi[a]),
        }))
    }

    /// Every grip: not the middle, and none along a flat axis.
    fn grips(&self) -> Vec<[u8; 3]> {
        let mut out = Vec::new();
        for x in 0..3u8 {
            for y in 0..3u8 {
                for z in 0..3u8 {
                    let g = [x, y, z];
                    let along_flat = (0..3).any(|a| self.flat(a) && g[a] != 1);
                    if g != [1, 1, 1] && !along_flat {
                        out.push(g);
                    }
                }
            }
        }
        out
    }
}

/// The opposite grip (or the middle along axes the grip does not move).
fn opposite(g: [u8; 3]) -> [u8; 3] {
    g.map(|i| 2 - i)
}

/// The axes a grip moves along, as 1 (moves) or 0.
fn mask(g: [u8; 3]) -> DVec3 {
    DVec3::from_array(g.map(|i| if i == 1 { 0.0 } else { 1.0 }))
}

impl Session {
    /// The box round what Scale works on: the selection and the selected
    /// groups. None with nothing selected.
    pub(super) fn scale_boxes(&self) -> Option<Boxes> {
        let m = self.work_model();
        let pts: Vec<DVec3> = self
            .selection
            .all_vertices(m)
            .iter()
            .map(|v| m.vertex(*v))
            .chain(self.groups_corners())
            .collect();
        if pts.is_empty() {
            return None;
        }
        let (lo, hi) = pts.iter().fold(
            (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        let size = (hi - lo).max_element();
        if size <= self.scene.tol.dist {
            return None;
        }
        // Flat axes stay flat (a face on the ground keeps a flat box).
        let pad = DVec3::from_array(std::array::from_fn(|a| {
            if hi[a] - lo[a] <= 1e-12 {
                0.0
            } else {
                MARGIN * size
            }
        }));
        Some(Boxes {
            lo,
            hi,
            dlo: lo - pad,
            dhi: hi + pad,
        })
    }

    /// The grip under the cursor, if any.
    fn grip_at(&self, view: &View, s: DVec2) -> Option<[u8; 3]> {
        let b = self.scale_boxes()?;
        let reach = GRIP_REACH * self.settings.aperture;
        b.grips()
            .into_iter()
            .filter_map(|g| {
                let (q, depth) = view.to_screen(b.at(g, true))?;
                let d = q.distance(s);
                // Grips on one spot of the screen (above and below): the
                // nearer one, as it is the one seen.
                (d < reach).then_some((d.round(), depth, g))
            })
            .min_by(|x, y| {
                x.0.total_cmp(&y.0)
                    .then(x.1.total_cmp(&y.1))
                    .then(x.2.cmp(&y.2))
            })
            .map(|(_, _, g)| g)
    }

    pub(super) fn scale_hover(&mut self, s: DVec2, view: &View) {
        if self.edit.is_some() {
            self.scale_drag(s, view);
            return;
        }
        let g = self.grip_at(view, s);
        if g != self.scale_grip {
            self.scale_grip = g;
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    pub(super) fn scale_press(&mut self, s: DVec2) {
        if self.edit.is_some() {
            self.finish_edit();
            return;
        }
        let view = self.view();
        let Some(grip) = self.grip_at(&view, s) else {
            // Not on a grip: pick what is under the cursor (with what is
            // connected to it), so its box shows.
            if self.selection.is_empty() && self.groups.is_empty() {
                let sel = self.working_selection(s, true);
                if !sel.is_empty() {
                    self.set_selection(sel);
                    self.overlay_dirty = true;
                    self.refresh();
                }
            }
            return;
        };
        let Some(b) = self.scale_boxes() else { return };
        let verts = self.selection.all_vertices(self.work_model());
        self.edit = Some(EditOp::Scale {
            grip,
            anchor: self.scale_anchor(&b, grip),
            k: DVec3::ONE,
            verts,
        });
        self.after_start();
    }

    /// The point a grip scales about: the opposite grip on the contents,
    /// or their middle after Alt.
    fn scale_anchor(&self, b: &Boxes, grip: [u8; 3]) -> DVec3 {
        if self.scale_centre {
            b.at([1, 1, 1], false)
        } else {
            b.at(opposite(grip), false)
        }
    }

    /// Alt: scale about the middle instead of the opposite grip (a tap
    /// switches, as Push/Pull's new loop does).
    pub(super) fn toggle_scale_centre(&mut self) {
        self.scale_centre = !self.scale_centre;
        if let (Some(EditOp::Scale { grip, .. }), Some(b)) = (self.edit.clone(), self.scale_boxes())
        {
            let a = self.scale_anchor(&b, grip);
            if let Some(EditOp::Scale { anchor, .. }) = &mut self.edit {
                *anchor = a;
            }
            self.update_preview();
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    /// The factors that put grip `grip` (from `anchor`) under the cursor.
    fn scale_drag(&mut self, s: DVec2, view: &View) {
        let Some(EditOp::Scale { grip, .. }) = self.edit.clone() else {
            return;
        };
        let Some(b) = self.scale_boxes() else { return };
        let g = b.at(grip, true);
        // The anchor on the drawn box, so the grip starts at 1×.
        let a = if self.scale_centre {
            b.at([1, 1, 1], true)
        } else {
            b.at(opposite(grip), true)
        };
        let m = mask(grip);
        let moving = m.x + m.y + m.z;
        let (o, d) = view.ray(s);
        let span = g - a;
        let k = if moving == 2.0 && self.shift_held {
            // An edge grip with Shift, each axis on its own: the cursor on
            // the plane of the two axes it moves along.
            let n = DVec3::ONE - m;
            let den = d.dot(n);
            if den.abs() < 1e-12 {
                return;
            }
            let p = o + d * ((g - o).dot(n) / den);
            DVec3::from_array(std::array::from_fn(|i| {
                if m[i] == 0.0 || span[i].abs() <= 1e-12 {
                    1.0
                } else {
                    ((p[i] - a[i]) / span[i]).clamp(MIN, MAX)
                }
            }))
        } else {
            // Along the line from the anchor through the grip.
            let Some(t) = along_line(o, d, a, span) else {
                return;
            };
            let t = if (t - 1.0).abs() < SNAP { 1.0 } else { t };
            self.scale_factor(grip, t.clamp(MIN, MAX))
        };
        if let Some(EditOp::Scale { k: kk, .. }) = &mut self.edit {
            *kk = k;
        }
        self.overlay_dirty = true;
        self.update_preview();
    }

    /// Factor `f` on the axes the grip scales: a corner all three, an edge
    /// its two, a face its one (all three with Shift).
    pub(super) fn scale_factor(&self, grip: [u8; 3], f: f64) -> DVec3 {
        let m = mask(grip);
        if self.shift_held && m.x + m.y + m.z == 1.0 {
            DVec3::splat(f)
        } else {
            m * f + (DVec3::ONE - m)
        }
    }

    /// The box, its grips (the hovered one larger) and, while scaling, the
    /// anchor.
    pub(super) fn scale_overlay(&self, o: &mut Overlay) {
        let Some(b) = self.scale_boxes() else { return };
        let fb = &self.style.feedback;
        let c = |i: u8, j: u8, k: u8| b.at([i, j, k], true);
        for (p, q) in [
            ((0, 0, 0), (2, 0, 0)),
            ((0, 2, 0), (2, 2, 0)),
            ((0, 0, 2), (2, 0, 2)),
            ((0, 2, 2), (2, 2, 2)),
            ((0, 0, 0), (0, 2, 0)),
            ((2, 0, 0), (2, 2, 0)),
            ((0, 0, 2), (0, 2, 2)),
            ((2, 0, 2), (2, 2, 2)),
            ((0, 0, 0), (0, 0, 2)),
            ((2, 0, 0), (2, 0, 2)),
            ((0, 2, 0), (0, 2, 2)),
            ((2, 2, 0), (2, 2, 2)),
        ] {
            let (a, bb) = (c(p.0, p.1, p.2), c(q.0, q.1, q.2));
            if a.distance(bb) > 1e-12 {
                o.lines.push(Line {
                    a,
                    b: bb,
                    color: self.style.selected,
                    width: 1.0,
                    dash: [5.0, 4.0],
                });
            }
        }
        let active = match &self.edit {
            Some(EditOp::Scale { grip, .. }) => Some(*grip),
            _ => self.scale_grip,
        };
        for g in b.grips() {
            let on = active == Some(g);
            o.marks.push(Mark {
                at: b.at(g, true),
                shape: Shape::Square,
                color: if on { fb.pill } else { self.style.selected },
                size: if on { 6.0 } else { 4.0 },
            });
        }
        if let Some(EditOp::Scale { anchor, .. }) = &self.edit {
            o.marks.push(Mark {
                at: *anchor,
                shape: Shape::Circle,
                color: fb.pill,
                size: 4.5,
            });
        }
    }
}
