//! The Trim / Extend and Project Curves tools (Shapes & Placement part 5).
//!
//! **Trim / Extend** (`tool.trim`): click a piece of a loose line or arc and
//! it goes (an edge that bounds a face is left for the Eraser). Hold Alt
//! and click near a free end of a straight edge to carry it on to the first
//! edge or face in the way.
//!
//! **Project Curves** (`tool.project`): select the lines (or arcs turned the
//! same way as the face) first, then click a flat face: they are dropped
//! straight onto it and cut it like drawn edges.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_math::DVec2;
use prism_ops::select::Selection;
use prism_view::View;
use prism_view::pick::Hit;

use super::Session;
use super::edit::edge_pick;

pub const TRIM_TOOL: &str = "tool.trim";
pub const PROJECT_TOOL: &str = "tool.project";
pub const PROJECT_FIRST: &str = "Select the lines to project first, then click the face";

impl Session {
    pub(super) fn trim_hover(&mut self, s: DVec2, view: &View) {
        let mut h = edge_pick(&self.scene, view, s, self.settings.aperture);
        // Trimming only lights up what it can take; Alt (extending) any
        // straight edge.
        if let Some(Hit::Edge(e)) = h.map(|p| p.hit) {
            let m = self.work_model();
            let ok = if self.press_alt {
                m.edge(e).is_some_and(|x| !x.curve.is_curved())
            } else {
                prism_ops::trim::is_loose(m, e)
            };
            if !ok {
                h = None;
            }
        }
        self.set_hover_pick(h);
    }

    pub(super) fn trim_press(&mut self, s: DVec2) {
        let view = self.view();
        let Some(p) = edge_pick(&self.scene, &view, s, self.settings.aperture) else {
            return;
        };
        let Hit::Edge(e) = p.hit else { return };
        if self.locked_selected() {
            return;
        }
        let m = self.work_model();
        let Some(x) = m.edge(e) else { return };
        let command = if self.press_alt {
            // The end nearer the click.
            let (a, b) = (m.vertex(x.a), m.vertex(x.b));
            let end = if p.point.distance(a) <= p.point.distance(b) {
                x.a
            } else {
                x.b
            };
            Command::Extend {
                edge: e,
                vertex: end,
            }
        } else {
            if let Some(why) = prism_ops::trim::trim_refusal(m, e) {
                self.notice = Some(why);
                self.overlay_dirty = true;
                self.dirty = true;
                return;
            }
            Command::Erase {
                selection: Selection {
                    edges: BTreeSet::from([e]),
                    ..Selection::default()
                },
            }
        };
        if let Some(why) = command.refusal(m) {
            self.notice = Some(why);
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        self.notice = None;
        self.cancel();
        self.run(command);
    }

    pub(super) fn trim_hint(&self) -> &'static str {
        if self.press_alt {
            "Click near the free end of a straight edge to carry it on to the first edge or face in its way"
        } else {
            "Click a piece of a loose line or arc to trim it away · hold Alt and click near a free end to extend it · an edge of a face is the Eraser's"
        }
    }

    pub(super) fn project_hover(&mut self, s: DVec2, view: &View) {
        let (o, d) = view.ray(s);
        let h = self
            .scene
            .ray_face(o, d)
            .filter(|(f, _)| !self.scene.face_owner.contains_key(f))
            .filter(|(f, _)| {
                self.scene
                    .face(*f)
                    .is_some_and(|x| x.flat_normal().is_some())
            })
            .map(|(f, t)| prism_view::Picked {
                hit: Hit::Face(f),
                point: o + d * t,
            });
        self.set_hover_pick(h);
    }

    pub(super) fn project_press(&mut self) {
        let Some(Hit::Face(face)) = self.hover.map(|p| p.hit) else {
            return;
        };
        if self.selection.edges.is_empty() {
            self.notice = Some(PROJECT_FIRST);
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        if self.locked_selected() {
            return;
        }
        let command = Command::Project {
            edges: self.selection.edges.clone(),
            face,
        };
        if let Some(why) = command.refusal(self.work_model()) {
            self.notice = Some(why);
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        self.notice = None;
        self.cancel();
        self.run(command);
    }

    pub(super) fn project_hint(&self) -> &'static str {
        if self.selection.edges.is_empty() {
            PROJECT_FIRST
        } else {
            "Click a flat face to drop the selected lines straight onto it"
        }
    }
}
