//! Draw > Surface's Loft, Revolve, Sweep and Patch when the selection does
//! not make them yet (tool pass part 2, owner 2026-11-16: "visual feedback
//! to a tool like that is nice"). Picking one with the curves selected
//! makes it at once, as before. Otherwise it waits: the hint says what to
//! click, a click on a curve adds it or takes it out (no Shift needed),
//! and once the curves make the surface a ghost of its faces shows and
//! Enter makes it. Esc, or another tool, leaves.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_math::DVec3;
use prism_topo::EdgeId;
use prism_view::overlay::{Line, Overlay};

use super::{SELECT, Session};

/// The command waiting for its curves.
#[derive(Clone, Debug)]
pub(super) struct SurfacePick {
    action: &'static str,
    /// The selected edges the ghost was made for.
    key: Vec<EdgeId>,
    /// Why the selection does not make it yet, if it does not.
    why: Option<&'static str>,
    /// The edges of the faces it would make.
    ghost: Vec<Vec<DVec3>>,
}

/// The action id as a static string, if it is one of these.
fn surface_action(id: &str) -> Option<&'static str> {
    ["surf.loft", "surf.revolve", "surf.sweep", "surf.patch"]
        .into_iter()
        .find(|a| *a == id)
}

fn command(action: &str, edges: BTreeSet<EdgeId>) -> Command {
    match action {
        "surf.loft" => Command::Loft { edges },
        "surf.revolve" => Command::Revolve { edges },
        "surf.sweep" => Command::Sweep { edges },
        _ => Command::Patch { edges },
    }
}

/// What to click, for each.
fn how(action: &str) -> &'static str {
    match action {
        "surf.loft" => "Loft: click the curves to loft through · Enter makes it · Esc leaves",
        "surf.revolve" => {
            "Revolve: click the outline or curves to turn (any shape, open or closed) and the straight line to turn them about · Enter makes it · Esc leaves"
        }
        "surf.sweep" => {
            "Sweep: click the profile and the path it runs along · Enter makes it · Esc leaves"
        }
        _ => "Patch: click curves joined end to end in a ring · Enter makes it · Esc leaves",
    }
}

fn ready(action: &str) -> &'static str {
    match action {
        "surf.loft" => "Enter makes the loft · click curves to add or take out · Esc leaves",
        "surf.revolve" => "Enter makes the revolve · click edges to add or take out · Esc leaves",
        "surf.sweep" => "Enter makes the sweep · click curves to add or take out · Esc leaves",
        _ => "Enter makes the patch · click curves to add or take out · Esc leaves",
    }
}

impl Session {
    /// Draw > Surface's commands: made at once when the selection makes
    /// them, else they wait for clicks.
    pub(super) fn surface_action(&mut self, id: &str) {
        let Some(action) = surface_action(id) else {
            return;
        };
        let edges = self.selection.edges.clone();
        let refused = command(action, edges).refusal(self.work_model());
        if refused.is_none() {
            self.surface_pick = None;
            self.surface_from_selection(action);
            return;
        }
        // Mid-way through something else (sides picked in the Loft tool, a
        // move): only say what it needs, as before, and lose nothing.
        if self.in_operation() || self.loft_busy() {
            self.cage_notice = refused;
            self.overlay_dirty = true;
            return;
        }
        if self.tool != SELECT {
            self.set_tool(SELECT);
        }
        self.cage_notice = None;
        self.surface_pick = Some(SurfacePick {
            action,
            key: Vec::new(),
            why: None,
            ghost: Vec::new(),
        });
        self.surface_update();
    }

    /// Whether clicks add to the selection (no Shift needed).
    pub(super) fn surface_picking(&self) -> bool {
        self.surface_pick.is_some() && self.tool == SELECT
    }

    /// After any input: the ghost follows the selection.
    pub(super) fn surface_update(&mut self) {
        if self.tool != SELECT {
            self.surface_pick = None;
            return;
        }
        let Some(p) = &self.surface_pick else {
            return;
        };
        let key: Vec<EdgeId> = self.selection.edges.iter().copied().collect();
        if key == p.key && (!p.ghost.is_empty() || p.why.is_some() || key.is_empty()) {
            return;
        }
        let action = p.action;
        let model = self.work_model();
        let c = command(action, key.iter().copied().collect());
        let (why, ghost) = match c.refusal(model) {
            Some(why) => (Some(why), Vec::new()),
            None => {
                let mut w = model.clone();
                let before: BTreeSet<_> = w.faces().map(|(id, _)| id).collect();
                c.apply(&mut w);
                let new: Vec<_> = w
                    .faces()
                    .map(|(id, _)| id)
                    .filter(|id| !before.contains(id))
                    .collect();
                (None, super::lofting::face_lines(&w, &new))
            }
        };
        self.surface_pick = Some(SurfacePick {
            action,
            key,
            why,
            ghost,
        });
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Enter: make it, if the curves do.
    pub(super) fn surface_enter(&mut self) -> bool {
        let Some(p) = &self.surface_pick else {
            return false;
        };
        if self.tool != SELECT {
            return false;
        }
        if p.why.is_none() && !p.ghost.is_empty() {
            let action = p.action;
            self.surface_pick = None;
            self.surface_from_selection(action);
        }
        true
    }

    /// Esc: stop waiting (the selection stays).
    pub(super) fn surface_escape(&mut self) -> bool {
        if self.surface_pick.take().is_some() {
            self.overlay_dirty = true;
            self.dirty = true;
            return true;
        }
        false
    }

    pub(super) fn surface_hint(&self) -> Option<&'static str> {
        let p = self.surface_pick.as_ref().filter(|_| self.tool == SELECT)?;
        Some(match p.why {
            // Curves picked that cannot make it: why (the "Select ..."
            // reasons only say what to click, which the hint says better).
            Some(why) if !why.starts_with("Select") => why,
            _ if p.ghost.is_empty() => how(p.action),
            _ => ready(p.action),
        })
    }

    /// Why the curves picked so far do not make it (for the window and
    /// tests).
    pub fn surface_waiting(&self) -> Option<(&'static str, Option<&'static str>, bool)> {
        self.surface_pick
            .as_ref()
            .map(|p| (p.action, p.why, !p.ghost.is_empty()))
    }

    pub(super) fn surface_overlay(&self, o: &mut Overlay) {
        let Some(p) = &self.surface_pick else {
            return;
        };
        let fb = &self.style.feedback;
        for pts in &p.ghost {
            for w in pts.windows(2) {
                o.lines.push(Line {
                    a: w[0],
                    b: w[1],
                    color: fb.loft_preview,
                    width: fb.target_width,
                    dash: [0.0, 0.0],
                });
            }
        }
    }
}
