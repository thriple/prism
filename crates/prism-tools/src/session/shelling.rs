//! The Shell / Thicken tool (brief 42, part 3b). Click a face: its sheet
//! or body is taken. Move the pointer, or type a thickness and press
//! Enter: out of the clicked face makes the new wall outside, into it
//! inside. Shift+click a face of a closed body to leave it open (again to
//! close it). A ghost of the result shows while sizing; a click or Enter
//! makes it in one undo step. Esc steps back.
//!
//! Its window (part 3d): a thickness typed there makes a click on a face
//! shell it at once (a ghost shows on the face under the pointer first),
//! inside or outside as the window says; empty sizes by the pointer. It
//! also counts the open faces and closes them all.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_math::{DVec2, DVec3};
use prism_topo::FaceId;
use prism_view::overlay::{Line, Overlay};
use prism_view::{Hit, View};
use serde::{Deserialize, Serialize};

use super::Session;
use super::edit::along_line;

pub const SHELL: &str = "tool.shell";

const HOW: &str = "Click a face to shell its body or thicken its sheet · Shift+click faces of a closed body to leave them open";
const FIXED: &str = "Click a face to shell or thicken it by the window's thickness · Shift+click faces of a closed body to leave them open · clear the thickness to size by the pointer";
const SIZING: &str = "Move to set the thickness (out of the face grows outside, into it inside) · click or type a thickness + Enter · Shift+click leaves a face open · Esc steps back";

/// A change from the Shell / Thicken window (recorded as `Input::Shell`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShellSet {
    /// The thickness a click makes; `None` sizes by the pointer.
    Thickness(Option<f64>),
    /// Whether that thickness grows outside (true) or goes inside.
    Outside(bool),
    /// Close every face left open.
    CloseAll,
}

/// What the Shell / Thicken window shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShellPanel {
    /// The window's thickness as text; empty sizes by the pointer.
    pub size: String,
    pub outside: bool,
    /// How many faces are left open.
    pub open: usize,
    /// Why the face under the pointer (or being sized) cannot be shelled.
    pub refused: Option<&'static str>,
    pub how: &'static str,
}

/// A shell being sized (or, with the window's thickness, the one a click
/// on the face under the pointer would make).
#[derive(Clone, Debug)]
pub(super) struct ShellOp {
    /// Made by the window's thickness: follows the pointer from face to
    /// face, and a click makes it.
    fixed: bool,
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
        if self.shell_size.is_some() && self.shell_op.as_ref().is_none_or(|op| op.fixed) {
            self.shell_preview();
            return;
        }
        if let Some(op) = &mut self.shell_op {
            if let Some(t) = along_line(o, dir, op.p0, op.n) {
                op.d = t;
            }
            self.shell_update();
        }
    }

    /// The window's thickness, signed by its direction.
    fn shell_fixed(&self) -> Option<f64> {
        self.shell_size
            .map(|t| if self.shell_outside { t } else { -t })
    }

    /// With the window's thickness: the ghost of the face under the pointer.
    fn shell_preview(&mut self) {
        let Some(d) = self.shell_fixed() else {
            return;
        };
        match self.shell_face() {
            Some(f) => {
                if self
                    .shell_op
                    .as_ref()
                    .is_some_and(|op| op.face == f && op.d == d)
                {
                    return;
                }
                self.shell_op = Some(ShellOp {
                    fixed: true,
                    face: f,
                    p0: DVec3::ZERO,
                    n: DVec3::ZERO,
                    d,
                    shown: None,
                    ghost: Vec::new(),
                    refused: None,
                });
                self.shell_update();
            }
            None => {
                if self.shell_op.take().is_some() {
                    self.overlay_dirty = true;
                    self.dirty = true;
                }
            }
        }
    }

    /// A change from the window, recorded so the session replays it.
    pub fn shell_input(&mut self, op: ShellSet) {
        let t = self.time;
        self.handle(&super::Event {
            t,
            input: super::Input::Shell { op },
        });
    }

    /// The window's thickness text: a length, or empty to size by the
    /// pointer. Text that is not a length changes nothing.
    pub fn shell_text(&mut self, text: &str) {
        let t = text.trim();
        if t.is_empty() {
            self.shell_input(ShellSet::Thickness(None));
        } else if let Some(size) = self.number.length(t).map(f64::abs)
            && size > 0.0
        {
            self.shell_input(ShellSet::Thickness(Some(size)));
        }
    }

    pub(super) fn shell_set(&mut self, op: ShellSet) {
        match op {
            ShellSet::Thickness(t) => {
                self.shell_size = t;
                // Typed while sizing by the pointer: as if typed in the
                // measurement box.
                if let (Some(t), Some(op)) = (t, &mut self.shell_op)
                    && !op.fixed
                {
                    let way = if op.d > 0.0 { 1.0 } else { -1.0 };
                    op.d = t * way;
                    self.shell_go();
                    return;
                }
                if self.shell_op.as_ref().is_some_and(|op| op.fixed) {
                    self.shell_op = None;
                }
            }
            ShellSet::Outside(out) => {
                self.shell_outside = out;
                if self.shell_op.as_ref().is_some_and(|op| op.fixed) {
                    self.shell_op = None;
                }
            }
            ShellSet::CloseAll => {
                self.shell_open.clear();
                if let Some(op) = &mut self.shell_op {
                    op.shown = None;
                }
                self.shell_update();
            }
        }
        if self.tool == SHELL && self.shell_size.is_some() {
            self.shell_preview();
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// What the window shows (with the tool active).
    pub fn shell_panel(&self) -> Option<ShellPanel> {
        (self.tool == SHELL).then(|| ShellPanel {
            size: self
                .shell_size
                .map_or(String::new(), |t| self.number.show(t)),
            outside: self.shell_outside,
            open: self.shell_open.len(),
            refused: self
                .shell_op
                .as_ref()
                .and_then(|op| op.refused)
                .filter(|r| *r != prism_ops::shelling::ZERO),
            how: self.shell_hint(),
        })
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
        if self.shell_size.is_some() {
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
            fixed: false,
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
        if self.shell_op.as_ref().is_none_or(|op| op.fixed) {
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
                .unwrap_or(if op.fixed { FIXED } else { SIZING }),
            None if self.shell_size.is_some() => FIXED,
            None => HOW,
        }
    }

    pub(super) fn shell_measurement(&self) -> (&'static str, String) {
        let d = self
            .shell_op
            .as_ref()
            .map(|op| op.d.abs())
            .or(self.shell_size)
            .unwrap_or(0.0);
        ("Thickness", self.number.show(d))
    }

    /// The thickness being sized by the pointer (for tests).
    pub fn shell_sizing(&self) -> Option<f64> {
        self.shell_op.as_ref().filter(|op| !op.fixed).map(|op| op.d)
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
