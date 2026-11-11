//! The Follow Me tool (brief 26, steps 5c and 5d): select a path of lines
//! and arcs, then click the profile face to sweep it along the path. With
//! nothing selected, it says to select a path first. Alt (tapped) follows
//! a face's edge instead: the path is the outer loop of the selected face.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_math::DVec2;
use prism_view::{Hit, View};

use super::Session;

pub const FOLLOW: &str = "tool.followme";

/// Said with Alt on and not one face selected.
pub const NO_FACE: &str =
    "Alt: select one face, whose edge is the path, then click the profile face.";

impl Session {
    pub(super) fn follow_hover(&mut self, s: DVec2, view: &View) {
        let (o, d) = view.ray(s);
        let h = self.scene.ray_face(o, d).map(|(f, t)| prism_view::Picked {
            hit: Hit::Face(f),
            point: o + d * t,
        });
        self.set_hover_pick(h);
    }

    pub(super) fn follow_press(&mut self) {
        self.follow_notice = None;
        let Some(Hit::Face(profile)) = self.hover.map(|p| p.hit) else {
            return;
        };
        let path: BTreeSet<_> = if self.follow_perimeter {
            match self.perimeter() {
                Some(p) => p,
                None => {
                    self.follow_notice = Some(NO_FACE);
                    self.overlay_dirty = true;
                    return;
                }
            }
        } else {
            self.selection().edges.iter().copied().collect()
        };
        let c = Command::FollowMe { path, profile };
        if let Some(why) = c.refusal(self.work_model()) {
            self.follow_notice = Some(why);
        } else if self.run(c) {
            self.set_selection(Default::default());
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    /// The outer loop of the one selected face, as a path.
    fn perimeter(&self) -> Option<BTreeSet<prism_topo::EdgeId>> {
        let sel = self.selection();
        let [face] = sel.faces.iter().copied().collect::<Vec<_>>()[..] else {
            return None;
        };
        let f = self.work_model().face(face)?;
        Some(f.loop_edge_ids(0).iter().copied().collect())
    }

    /// Alt tapped: follow the selected face's edge, or the selected path.
    pub(super) fn toggle_follow_perimeter(&mut self) {
        self.follow_perimeter = !self.follow_perimeter;
        self.follow_notice = None;
        self.overlay_dirty = true;
    }

    pub(super) fn follow_hint(&self) -> &'static str {
        if let Some(why) = self.follow_notice {
            return why;
        }
        if self.follow_perimeter {
            return if self.perimeter().is_some() {
                "Click the profile face to sweep it round the selected face's edge (Alt: along a selected path)"
            } else {
                NO_FACE
            };
        }
        if self.selection().edges.is_empty() {
            prism_ops::follow::NO_PATH
        } else {
            "Click the profile face to sweep it along the selected path (Alt: round a selected face's edge)"
        }
    }
}
