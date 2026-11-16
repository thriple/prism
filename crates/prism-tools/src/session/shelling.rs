//! The Shell / Thicken tool (brief 42, part 3b). Click a face: its sheet
//! or body is taken. Move the pointer, or type a thickness and press
//! Enter: out of the clicked face makes the new wall outside, into it
//! inside. Shift+click a face of a closed body to leave it open (again to
//! close it). A ghost of the result shows while sizing; a click or Enter
//! makes it in one undo step. Esc steps back.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_math::{DVec2, DVec3};
use prism_topo::FaceId;
use prism_view::overlay::{Line, Overlay};
use prism_view::{Hit, View};

use super::Session;
use super::edit::along_line;

pub const SHELL: &str = "tool.shell";

const HOW: &str = "Click a face to shell its body or thicken its sheet · Shift+click faces of a closed body to leave them open";
const SIZING: &str = "Move to set the thickness (out of the face grows outside, into it inside) · click or type a thickness + Enter · Shift+click leaves a face open · Esc steps back";

/// A shell being sized.
#[derive(Clone, Debug)]
pub(super) struct ShellOp {
    face: FaceId,
    p0: DVec3,
    n: DVec3,
    d: f64,
    /// The thickness and open faces the ghost was made for.
    shown: Option<(f64, BTreeSet<FaceId>)>,
    ghost: Vec<[DVec3; 2]>,
    refused: Option<&'static str>,
}

impl Session {
    /// The face under the pointer, if it is the open group's own.
    fn shell_face(&self) -> Option<FaceId> {
        match self.hover.map(|p| p.hit) {
            Some(Hit::Face(f)) if !self.scene.face_owner.contains_key(&f) => Some(f),
            _ => None,
        }
    }

    pub(super) fn shell_hover(&mut self, s: DVec2, view: &View) {
        let (o, dir) = view.ray(s);
        let h = self
            .scene
            .ray_face(o, dir)
            .filter(|(f, _)| !self.scene.face_owner.contains_key(f))
            .map(|(f, t)| prism_view::Picked {
                hit: Hit::Face(f),
                point: o + dir * t,
            });
        self.set_hover_pick(h);
        if let Some(op) = &mut self.shell_op {
            if let Some(t) = along_line(o, dir, op.p0, op.n) {
                op.d = t;
            }
            self.shell_update();
        }
    }

    pub(super) fn shell_press(&mut self, shift: bool) {
        let face = self.shell_face();
        if shift {
            if let Some(f) = face {
                if !self.shell_open.remove(&f) {
                    self.shell_open.insert(f);
                }
                if let Some(op) = &mut self.shell_op {
                    op.shown = None;
                }
                self.shell_update();
                self.overlay_dirty = true;
                self.dirty = true;
            }
            return;
        }
        if self.shell_op.is_some() {
            self.shell_go();
            return;
        }
        let Some(f) = face else {
            return;
        };
        let p0 = self.hover.map_or(DVec3::ZERO, |p| p.point);
        // Thickness is measured along the face's direction where it was
        // clicked (a curved face points a different way at each place).
        let Some(n) = self.work_model().face(f).map(|x| x.surface.normal_at(p0)) else {
            return;
        };
        // Open faces of another body do not count.
        let body = prism_ops::shelling::body(self.work_model(), &[f].into());
        self.shell_open.retain(|g| body.contains(g));
        self.shell_open.remove(&f);
        self.notice = None;
        self.shell_op = Some(ShellOp {
            face: f,
            p0,
            n,
            d: 0.0,
            shown: None,
            ghost: Vec::new(),
            refused: None,
        });
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The ghost follows the thickness and the open faces.
    fn shell_update(&mut self) {
        let open = self.shell_open.clone();
        let Some(op) = &self.shell_op else {
            return;
        };
        if op
            .shown
            .as_ref()
            .is_some_and(|(d, o)| *d == op.d && *o == open)
        {
            return;
        }
        let (face, d) = (op.face, op.d);
        let (ghost, refused) =
            match prism_ops::shelling::slab(self.work_model(), &[face].into(), &open, d) {
                Ok((s, _)) => (
                    s.edges()
                        .map(|(_, e)| [s.vertex(e.a), s.vertex(e.b)])
                        .collect(),
                    None,
                ),
                Err(why) => (Vec::new(), Some(why)),
            };
        if let Some(op) = &mut self.shell_op {
            op.ghost = ghost;
            op.refused = refused;
            op.shown = Some((d, open));
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Make it at the thickness sized, or say why not.
    fn shell_go(&mut self) {
        let Some(op) = &self.shell_op else {
            return;
        };
        let c = Command::Shell {
            faces: [op.face].into(),
            open: self.shell_open.clone(),
            thickness: op.d,
        };
        if let Some(why) = c.refusal(self.work_model()) {
            self.notice = Some(why);
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        self.shell_op = None;
        self.shell_open.clear();
        self.notice = None;
        self.run(c);
    }

    /// Enter with a typed thickness (or none: the one sized).
    pub(super) fn shell_enter(&mut self, text: &str) -> bool {
        if self.tool != SHELL {
            return false;
        }
        if self.shell_op.is_none() {
            return true;
        }
        let text = text.trim();
        if !text.is_empty() {
            let Some(t) = self.number.length(text) else {
                return true;
            };
            if let Some(op) = &mut self.shell_op {
                // A typed thickness goes the way the pointer is (inside
                // until it has moved out); a minus sign turns it round.
                let way = if op.d > 0.0 { 1.0 } else { -1.0 };
                op.d = t * way;
            }
        }
        self.shell_go();
        true
    }

    /// Esc: stop sizing, then forget the open faces.
    pub(super) fn shell_escape(&mut self) -> bool {
        if self.tool != SHELL {
            return false;
        }
        if self.shell_op.take().is_none() {
            if self.shell_open.is_empty() {
                return false;
            }
            self.shell_open.clear();
        }
        self.notice = None;
        self.overlay_dirty = true;
        self.dirty = true;
        true
    }

    pub(super) fn shell_clear(&mut self) {
        self.shell_op = None;
        self.shell_open.clear();
    }

    pub(super) fn shell_hint(&self) -> &'static str {
        match &self.shell_op {
            // No thickness yet is the start of sizing, not a mistake.
            Some(op) => op
                .refused
                .filter(|r| *r != prism_ops::shelling::ZERO)
                .unwrap_or(SIZING),
            None => HOW,
        }
    }

    pub(super) fn shell_measurement(&self) -> (&'static str, String) {
        let d = self.shell_op.as_ref().map_or(0.0, |op| op.d.abs());
        ("Thickness", self.number.show(d))
    }

    /// The thickness being sized (for tests and the window).
    pub fn shell_sizing(&self) -> Option<f64> {
        self.shell_op.as_ref().map(|op| op.d)
    }

    pub(super) fn shell_overlay(&self, o: &mut Overlay) {
        if self.tool != SHELL {
            return;
        }
        let fb = &self.style.feedback;
        let m = self.work_model();
        for f in &self.shell_open {
            let Some(face) = m.face(*f) else {
                continue;
            };
            for e in face.edge_ids() {
                for w in m.edge_points(e).windows(2) {
                    o.lines.push(Line {
                        a: w[0],
                        b: w[1],
                        color: self.style.hover,
                        width: fb.target_width,
                        dash: [5.0, 4.0],
                    });
                }
            }
        }
        if let Some(op) = &self.shell_op {
            for [a, b] in &op.ghost {
                o.lines.push(Line {
                    a: *a,
                    b: *b,
                    color: fb.loft_preview,
                    width: fb.target_width,
                    dash: [0.0, 0.0],
                });
            }
        }
    }
}
