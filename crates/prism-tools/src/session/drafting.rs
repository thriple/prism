//! The Draft Angle tool (Shapes & Placement part 6): select the flat faces
//! to lean, set the angle in the window, then click the neutral face (the
//! base the part comes out from): the chosen faces lean about where each
//! meets it, so the part narrows away from it for a positive angle.

use prism_doc::Command;
use prism_math::DVec2;
use prism_view::View;
use prism_view::pick::Hit;

use super::Session;

pub const DRAFT_TOOL: &str = "tool.draft";
pub const DRAFT_FIRST: &str = "Select the faces to lean first, then click the neutral face";

/// What the window shows.
#[derive(Clone, Debug)]
pub struct DraftPanel {
    /// The angle in degrees, as text.
    pub angle: String,
    pub how: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DraftSet {
    Angle(String),
}

impl Session {
    pub fn draft_angle(&self) -> f64 {
        self.draft_angle
    }

    pub fn draft_panel(&self) -> Option<DraftPanel> {
        (self.tool == DRAFT_TOOL).then(|| DraftPanel {
            angle: format!(
                "{}",
                (self.draft_angle.to_degrees() * 1000.0).round() / 1000.0
            ),
            how: self.draft_hint(),
        })
    }

    pub fn draft_input(&mut self, op: DraftSet) {
        let DraftSet::Angle(t) = op;
        match t.trim().trim_end_matches('°').parse::<f64>() {
            Ok(d) if d != 0.0 && d.abs() <= 45.0 => self.draft_angle = d.to_radians(),
            _ => self.notice = Some(prism_ops::draft::ANGLE_RANGE),
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    pub(super) fn draft_hover(&mut self, s: DVec2, view: &View) {
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

    pub(super) fn draft_press(&mut self) {
        let Some(p) = self.hover else { return };
        let Hit::Face(neutral) = p.hit else { return };
        if self.selection.faces.is_empty() {
            self.notice = Some(DRAFT_FIRST);
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        let Some(n) = self.scene.face(neutral).and_then(|f| f.flat_normal()) else {
            return;
        };
        if self.locked_selected() {
            return;
        }
        let command = Command::Draft {
            faces: self.selection.faces.clone(),
            origin: p.point,
            // The way the part comes out: away from the neutral face.
            pull: -n,
            angle: self.draft_angle,
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

    pub(super) fn draft_hint(&self) -> &'static str {
        if self.selection.faces.is_empty() {
            DRAFT_FIRST
        } else {
            "Click the neutral face (the base the part comes out from) to lean the selected faces · set the angle in the window"
        }
    }
}
