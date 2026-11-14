//! The Fillet / Chamfer tool (brief 26, step 5a; its window, brief 29):
//! click a solid's edge to round it, or cut it off flat (chamfer), at the
//! window's size (empty fits each edge). Ctrl swaps the kind while held.
//! Hovering shows the result before the click. A new size or kind right
//! after a round changes it. A click near a flat face's corner rounds the
//! corner (brief 24's fillet).

use prism_doc::Command;
use prism_math::{DVec2, DVec3};
use prism_topo::EdgeId;
use prism_view::{Hit, View};
use serde::{Deserialize, Serialize};

use super::{Event, Input, Session};

pub const FILLET: &str = "tool.fillet";

const HOW: &str = "Click an edge to round it · Ctrl swaps fillet and chamfer · type a size + Enter · a click near a flat corner rounds the corner";

/// A change from the Fillet window (recorded as `Input::Round`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RoundOp {
    /// Round the edge (true) or cut it off flat.
    Fillet(bool),
    /// The size; `None` fits each edge.
    Size(Option<f64>),
}

/// What the Fillet window shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoundPanel {
    /// Chamfer chosen in the window (Ctrl swaps it while held).
    pub chamfer: bool,
    /// The size as text; empty fits each edge.
    pub size: String,
    /// The size that would fit the hovered edge, as text.
    pub fits: Option<String>,
    /// Whether a new size or kind changes the last round.
    pub changes_last: bool,
    /// Why the hovered edge cannot be rounded, if it cannot.
    pub refused: Option<&'static str>,
    pub how: &'static str,
}

/// What a click would round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Target {
    Corner(DVec3),
    Edge(EdgeId),
}

impl Session {
    /// The kind a click makes now: the window's, swapped while Ctrl is held.
    fn round_fillet(&self, ctrl: bool) -> bool {
        self.round_chamfer == ctrl
    }

    /// The corner or edge under the cursor at `s`.
    fn round_target(&self, s: DVec2) -> Option<Target> {
        let p = self.hover?;
        let Hit::Edge(e) = p.hit else { return None };
        let view = self.view();
        let m = self.work_model();
        // Near an end that is a flat corner (two edges): round the corner.
        let corner = m.edge(e).and_then(|x| {
            [x.a, x.b].into_iter().find(|v| {
                m.edges().filter(|(_, y)| y.has(*v)).count() == 2
                    && view.to_screen(m.vertex(*v)).is_some_and(|(q, _)| {
                        q.distance(s) < prism_view::pick::EDGE_RADIUS * self.settings.aperture
                    })
            })
        });
        Some(match corner {
            Some(v) => Target::Corner(m.vertex(v)),
            None => Target::Edge(e),
        })
    }

    /// The command a click on `t` would run, or why there is none.
    fn round_command(&self, t: Target, fillet: bool) -> Result<Command, &'static str> {
        let m = self.work_model();
        let c = match t {
            Target::Corner(corner) => {
                let r = self
                    .round_size
                    .or_else(|| prism_ops::fillet::default_radius(m, corner))
                    .ok_or(prism_ops::fillet::NOT_A_CORNER)?;
                Command::Fillet { corner, radius: r }
            }
            Target::Edge(edge) => {
                let size = self
                    .round_size
                    .or_else(|| prism_ops::round::default_size(m, edge))
                    .ok_or(prism_ops::round::NOT_AN_EDGE)?;
                // Through the kernel when the edge is on a closed solid
                // (DECISIONS #37: its one-edge result matches the old
                // round); the old round still serves an open shell.
                let kernel = Command::RoundEdges {
                    edges: std::collections::BTreeSet::from([edge]),
                    size,
                    fillet,
                };
                if kernel.refusal(m).is_none() {
                    return Ok(kernel);
                }
                Command::RoundEdge { edge, size, fillet }
            }
        };
        match c.refusal(m) {
            Some(why) => Err(why),
            None => Ok(c),
        }
    }

    pub(super) fn round_hover(&mut self, s: DVec2, view: &View) {
        let h = super::edit::edge_pick(&self.scene, view, s, self.settings.aperture);
        self.set_hover_pick(h);
        self.round_at = Some(s);
        self.round_preview();
    }

    /// Show the hovered edge rounded at the window's kind and size (only
    /// rebuilt when one of those, or the model, changes).
    pub(super) fn round_preview(&mut self) {
        if self.tool != FILLET {
            return;
        }
        let fillet = self.round_fillet(self.ctrl_held);
        let target = self.round_at.and_then(|s| self.round_target(s));
        let key = target.map(|t| (t, fillet, self.round_size));
        if key == self.round_shown {
            return;
        }
        self.round_shown = key;
        let (why, preview) = match target.map(|t| self.round_command(t, fillet)) {
            None => (None, None),
            Some(Err(why)) => (Some(why), None),
            Some(Ok(c)) => {
                let mut m = self.work_model().clone();
                c.apply(&mut m);
                (None, Some(m))
            }
        };
        self.round_refused = why;
        self.set_preview(preview);
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Take the preview away (leaving the tool, Esc, after a click).
    pub(super) fn round_clear(&mut self) {
        self.round_refused = None;
        self.round_at = None;
        if (self.round_shown.take().is_some() || self.tool == FILLET) && self.preview.is_some() {
            self.set_preview(None);
        }
    }

    pub(super) fn round_press(&mut self, s: DVec2, ctrl: bool) {
        self.round_notice = None;
        let Some(t) = self.round_target(s) else {
            return;
        };
        let fillet = self.round_fillet(ctrl);
        // The click's own result replaces the preview.
        self.round_clear();
        match self.round_command(t, fillet) {
            Ok(c) => {
                let last = match t {
                    Target::Edge(e) => Some((e, fillet)),
                    Target::Corner(_) => None,
                };
                if self.run(c) {
                    self.round_last = last;
                }
            }
            Err(why) => {
                self.round_notice = Some(why);
                self.round_last = None;
            }
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    /// The window's kind or size. Right after a round, it changes that
    /// round in place (one undo step); if the change cannot be done, the
    /// round stays as it was and the status line says why.
    pub(super) fn round_set(&mut self, op: RoundOp) {
        let before = (self.round_chamfer, self.round_size);
        match op {
            RoundOp::Fillet(f) => self.round_chamfer = !f,
            RoundOp::Size(s) => self.round_size = s.map(f64::abs).filter(|s| *s > 0.0),
        }
        if (self.round_chamfer, self.round_size) == before {
            return;
        }
        if let Some((edge, was_fillet)) = self.round_last {
            let fillet = match op {
                RoundOp::Fillet(f) => f,
                RoundOp::Size(_) => was_fillet,
            };
            if self.doc.undo() {
                self.model_changed();
            }
            match self.round_command(Target::Edge(edge), fillet) {
                Ok(c) => {
                    if self.run(c) {
                        self.round_last = Some((edge, fillet));
                    }
                }
                Err(why) => {
                    // Keep the last one as it was.
                    if self.doc.redo() {
                        self.model_changed();
                    }
                    self.round_notice = Some(why);
                }
            }
        }
        self.round_preview();
        self.overlay_dirty = true;
        self.refresh();
    }

    /// A change from the Fillet window, recorded like any input.
    pub fn round_input(&mut self, op: RoundOp) {
        let t = self.time;
        self.handle(&Event {
            t,
            input: Input::Round { op },
        });
    }

    /// The Fillet window's size text: a length, or empty to fit each edge.
    /// Text that is not a length changes nothing.
    pub fn round_text(&mut self, text: &str) {
        let t = text.trim();
        if t.is_empty() {
            self.round_input(RoundOp::Size(None));
        } else if let Some(size) = self.number.length(t).map(f64::abs)
            && size > 0.0
        {
            self.round_input(RoundOp::Size(Some(size)));
        }
    }

    /// What the Fillet window shows, while the tool is active.
    pub fn round_panel(&self) -> Option<RoundPanel> {
        (self.tool == FILLET).then(|| RoundPanel {
            chamfer: self.round_chamfer,
            size: self
                .round_size
                .map_or(String::new(), |r| self.number.show(r)),
            fits: self
                .round_at
                .and_then(|s| match self.round_target(s)? {
                    Target::Edge(e) => prism_ops::round::default_size(self.work_model(), e),
                    Target::Corner(c) => prism_ops::fillet::default_radius(self.work_model(), c),
                })
                .map(|r| self.number.show(r)),
            changes_last: self.round_last.is_some(),
            refused: self.round_refused,
            how: HOW,
        })
    }

    /// Typed text: the size for the next round, and right after one, a new
    /// size for it. True if it was used.
    pub(super) fn round_enter(&mut self, text: &str) -> bool {
        if self.tool != FILLET {
            return false;
        }
        if let Some(size) = self.number.length(text.trim()).map(f64::abs)
            && size > 0.0
        {
            self.round_set(RoundOp::Size(Some(size)));
        }
        true
    }

    pub(super) fn round_hint(&self) -> &'static str {
        self.round_notice.or(self.round_refused).unwrap_or(HOW)
    }

    pub(super) fn round_measurement(&self) -> (&'static str, String) {
        let label = if self.round_fillet(self.ctrl_held) {
            "Radius"
        } else {
            "Distance"
        };
        (
            label,
            self.round_size
                .map_or(String::new(), |r| self.number.show(r)),
        )
    }
}
