//! The Fillet / Chamfer tool (brief 26, step 5a): click a solid's edge to
//! round it with the current radius, Ctrl to cut it off flat instead. Type
//! a size + Enter to change the last one (or set the next). A click near a
//! flat face's corner rounds the corner (brief 24's fillet).

use prism_doc::Command;
use prism_math::DVec2;
use prism_topo::EdgeId;
use prism_view::{Hit, View};

use super::Session;

pub const FILLET: &str = "tool.fillet";

impl Session {
    pub(super) fn round_hover(&mut self, s: DVec2, view: &View) {
        let h = super::edit::edge_pick(&self.scene, view, s, self.settings.aperture);
        self.set_hover_pick(h);
    }

    pub(super) fn round_press(&mut self, s: DVec2, ctrl: bool) {
        self.round_notice = None;
        let view = self.view();
        let Some(p) = self.hover else {
            return;
        };
        let Hit::Edge(e) = p.hit else { return };
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
        if let Some(v) = corner {
            let corner = m.vertex(v);
            let Some(r) = self
                .round_size
                .or_else(|| prism_ops::fillet::default_radius(m, corner))
            else {
                return;
            };
            self.run_round(Command::Fillet { corner, radius: r }, None);
            return;
        }
        let fillet = !ctrl;
        let Some(size) = self
            .round_size
            .or_else(|| prism_ops::round::default_size(m, e))
        else {
            self.round_notice = Some(prism_ops::round::NOT_AN_EDGE);
            self.refresh();
            return;
        };
        self.run_round(
            Command::RoundEdge {
                edge: e,
                size,
                fillet,
            },
            Some((e, fillet)),
        );
    }

    fn run_round(&mut self, c: Command, last: Option<(EdgeId, bool)>) {
        if let Some(why) = c.refusal(self.work_model()) {
            self.round_notice = Some(why);
            self.round_last = None;
        } else if self.run(c) {
            self.round_last = last;
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    /// Typed text: the size for the next round, and right after one, a new
    /// size for it. True if it was used.
    pub(super) fn round_enter(&mut self, text: &str) -> bool {
        if self.tool != FILLET {
            return false;
        }
        let Some(size) = self.number.length(text.trim()).map(f64::abs) else {
            return true;
        };
        if size <= 0.0 {
            return true;
        }
        self.round_size = Some(size);
        if let Some((edge, fillet)) = self.round_last {
            if self.doc.undo() {
                self.model_changed();
            }
            let c = Command::RoundEdge { edge, size, fillet };
            if let Some(why) = c.refusal(self.work_model()) {
                // Keep the last one as it was.
                if self.doc.redo() {
                    self.model_changed();
                }
                self.round_notice = Some(why);
                self.refresh();
            } else {
                self.run_round(c, Some((edge, fillet)));
            }
        }
        true
    }

    pub(super) fn round_hint(&self) -> &'static str {
        self.round_notice.unwrap_or(
            "Click an edge to round it · Ctrl cuts it off flat (chamfer) · type a size + Enter · a click near a flat corner rounds the corner",
        )
    }

    pub(super) fn round_measurement(&self) -> (&'static str, String) {
        let chamfer = self.round_last.is_some_and(|(_, fillet)| !fillet);
        let label = if chamfer { "Distance" } else { "Radius" };
        (
            label,
            self.round_size
                .map_or(String::new(), |r| self.number.show(r)),
        )
    }
}
