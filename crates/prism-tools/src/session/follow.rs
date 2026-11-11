//! The Follow Me tool (brief 26, step 5c): select a path of lines, then
//! click the profile face to sweep it along the path. With nothing
//! selected, it says to select a path first.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_math::DVec2;
use prism_view::{Hit, View};

use super::Session;

pub const FOLLOW: &str = "tool.followme";

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
        let path: BTreeSet<_> = self.selection().edges.iter().copied().collect();
        let c = Command::FollowMe { path, profile };
        if let Some(why) = c.refusal(self.work_model()) {
            self.follow_notice = Some(why);
        } else if self.run(c) {
            self.set_selection(Default::default());
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    pub(super) fn follow_hint(&self) -> &'static str {
        self.follow_notice
            .unwrap_or(if self.selection().edges.is_empty() {
                prism_ops::follow::NO_PATH
            } else {
                "Click the profile face to sweep it along the selected path"
            })
    }
}
