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

use prism_doc::{Command, DefId, Feature};
use prism_math::{DVec2, DVec3};
use prism_topo::FaceId;
use prism_view::overlay::{Line, Mark, Overlay, Shape};
use prism_view::{Hit, View};
use serde::{Deserialize, Serialize};

use super::Session;
use super::edit::along_line;
use super::features::GRAB;

pub const SHELL: &str = "tool.shell";

const HOW: &str = "Click a face to shell its body or thicken its sheet · Shift+click faces of a closed body to leave them open";
const HANDLE: &str = "Drag the handle to change the shell's thickness (through the face makes it go the other way) · click a rim's diamond to end it flush or square, drag it to tilt the cut · or click a face to shell another";
const FIXED: &str = "Click a face to shell or thicken it by the window's thickness · Shift+click faces of a closed body to leave them open · clear the thickness to size by the pointer";
const SIZING: &str = "Move to set the thickness (out of the face grows outside, into it inside) · click or type a thickness + Enter · Shift+click leaves a face open · Esc steps back";

/// Said while a shell is made a piece at a time.
pub const JOB_RUNNING: &str = "Shelling face by face, a face at a time: it shows as it goes, the box says how far (a body of many faces takes a while). Esc stops it.";
/// Said when Esc stops it.
pub const JOB_STOPPED: &str = "Stopped: the shell was not made.";

/// A shell being made a piece at a time, and the command that makes it
/// part of the model once every piece is made (the pieces are kept, so it
/// does not make them again).
pub(super) struct JobRun {
    job: prism_ops::shelling::ShellJob,
    command: Command,
    warning: Option<&'static str>,
}

/// A drag of a live shell's thickness handle in progress.
#[derive(Clone, Debug)]
pub(super) struct ShellHandle {
    def: DefId,
    index: usize,
    centre: DVec3,
    normal: DVec3,
    start: f64,
    size: f64,
    /// The rim styles picked for the drag's side (outside or in), kept
    /// while it lasts: picking them builds the shell many times, so each
    /// frame would take ten times as long. The size let go of picks again.
    rims: Option<(bool, prism_ops::shelling::RimStyles)>,
}

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
    /// What to check in the shell it would make (built anyway: owner,
    /// 2026-11-26).
    pub warning: Option<&'static str>,
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
    /// The rims of the ghost that end flush in a plane (brief 49 part 3):
    /// drawn in their own colour, so a rim the program placed is seen.
    flush: Vec<[DVec3; 2]>,
    /// The program's rim styles for the thickness' direction and open
    /// faces they were made for (picking them builds the shell many times,
    /// so they are kept while the thickness only grows or shrinks).
    rims: Option<(bool, BTreeSet<FaceId>, prism_ops::shelling::RimStyles)>,
    refused: Option<&'static str>,
    /// What the kernel warns of in the shell it would make, and where:
    /// the lines it flagged, drawn in the warning colour.
    warning: Option<&'static str>,
    flagged: Vec<[DVec3; 2]>,
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
        if self.job.is_some() {
            return;
        }
        if self.shell_handle_drag(s, view) || self.rim_hover(s, view) {
            return;
        }
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
                // Another face of the same body makes the same shell: the
                // ghost stays (building it again can be slow).
                if let Some(op) = &self.shell_op
                    && op.fixed
                    && op.d == d
                    && op.shown.is_some()
                    && prism_ops::shelling::body(self.work_model(), &[op.face].into()).contains(&f)
                {
                    if let Some(op) = &mut self.shell_op {
                        op.face = f;
                    }
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
                    flush: Vec::new(),
                    rims: None,
                    refused: None,
                    warning: None,
                    flagged: Vec::new(),
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
                // A very thin wall is made, with a warning.
                self.notice = t.and_then(prism_ops::shelling::thin_warning);
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
            warning: self.shell_op.as_ref().and_then(|op| op.warning),
            how: self.shell_hint(),
        })
    }

    pub(super) fn shell_press(&mut self, s: DVec2, shift: bool) {
        if !shift && (self.shell_handle_press(s) || self.rim_press(s)) {
            return;
        }
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
            flush: Vec::new(),
            rims: None,
            refused: None,
            warning: None,
            flagged: Vec::new(),
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
        let faces: BTreeSet<FaceId> = [face].into();
        // The program's pick of rim styles, kept while it still builds.
        let kept = op
            .rims
            .clone()
            .filter(|(out, o, _)| *out == (d > 0.0) && *o == open)
            .map(|r| r.2);
        let m = self.work_model();
        let mut styles = kept
            .clone()
            .unwrap_or_else(|| prism_ops::shelling::default_rims(m, &faces, &open, d));
        let mut built = prism_ops::shelling::slab_preview(m, &faces, &open, d, &styles);
        let flagged_any = |b: &Result<prism_ops::shelling::Flagged, _>| {
            b.as_ref().is_ok_and(|x| !x.flags.is_empty())
        };
        if (built.is_err() || flagged_any(&built)) && kept.is_some() {
            styles = prism_ops::shelling::default_rims(m, &faces, &open, d);
            built = prism_ops::shelling::slab_preview(m, &faces, &open, d, &styles);
        }
        let (ghost, refused) = match &built {
            Ok(b) => (
                b.model
                    .edges()
                    .map(|(_, e)| [b.model.vertex(e.a), b.model.vertex(e.b)])
                    .collect(),
                None,
            ),
            Err(why) => (Vec::new(), Some(*why)),
        };
        let (warning, flagged) = match &built {
            Ok(b) => shell_warning(m, b, d),
            Err(_) => (None, Vec::new()),
        };
        let flush: Vec<[DVec3; 2]> = styles
            .iter()
            .filter(|(_, st)| matches!(st, prism_ops::shelling::RimStyle::Flush(_)))
            .filter_map(|(e, _)| m.edge(*e).map(|x| (m.edge_points(*e), x)))
            .flat_map(|(pts, _)| pts.windows(2).map(|w| [w[0], w[1]]).collect::<Vec<_>>())
            .collect();
        if let Some(op) = &mut self.shell_op {
            op.ghost = ghost;
            op.flush = if refused.is_none() { flush } else { Vec::new() };
            op.rims = Some((d > 0.0, open.clone(), styles));
            op.refused = refused;
            op.warning = warning;
            op.flagged = flagged;
            op.shown = Some((d, open));
        }
        // The warning in the status bar too, while it holds.
        if self.notice.is_none() || self.notice.is_some_and(is_shell_warning) {
            self.notice = warning;
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Make it at the thickness sized, or say why not.
    fn shell_go(&mut self) {
        // The ghost and its warning for the thickness being made (a typed
        // one may not have been shown yet).
        self.shell_update();
        let Some(op) = &self.shell_op else {
            return;
        };
        let warning = op.warning;
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
        // Face by face from many faces: made a piece at a time, a step
        // each frame, shown as it goes (owner, 2026-11-26).
        if let Command::Shell {
            faces,
            open,
            thickness,
        } = &c
            && let Some(job) = prism_ops::shelling::shell_job(
                self.work_model(),
                faces,
                open,
                *thickness,
                &prism_ops::shelling::RimStyles::new(),
            )
        {
            if self.sync_jobs {
                let _ = job.finish();
            } else {
                self.job = Some(JobRun {
                    job,
                    command: c,
                    warning,
                });
                self.notice = Some(JOB_RUNNING);
                self.overlay_dirty = true;
                self.dirty = true;
                return;
            }
        }
        self.run(c);
        // Made: what to check stays in the status bar.
        self.notice = warning;
        self.overlay_dirty = true;
    }

    /// While a long edit is being made: a frame's time makes the next
    /// piece, Esc stops it, the view can still be moved, and anything else
    /// waits. True if the input was taken.
    pub(super) fn job_input(&mut self, input: &super::Input, resp: &mut super::Response) -> bool {
        use super::{Button, Input, Key};
        if self.job.is_none() {
            return false;
        }
        match input {
            Input::Time => {
                self.job_step();
                true
            }
            Input::Key { key: Key::Escape } => {
                self.job = None;
                self.set_preview(None);
                self.notice = Some(JOB_STOPPED);
                resp.used_escape = true;
                self.overlay_dirty = true;
                self.refresh();
                true
            }
            Input::Move { .. }
            | Input::Wheel { .. }
            | Input::Resize { .. }
            | Input::Held { .. }
            | Input::Press {
                button: Button::Middle,
                ..
            }
            | Input::Release {
                button: Button::Middle,
            } => false,
            _ => {
                self.notice = Some(JOB_RUNNING);
                true
            }
        }
    }

    /// The next piece of the edit being made, and what it looks like so
    /// far; the last makes it, in one undo step.
    fn job_step(&mut self) {
        let Some(run) = &mut self.job else {
            return;
        };
        if run.job.step() {
            let shown = run.job.shown();
            self.set_preview(Some(shown));
            self.dirty = true;
            self.overlay_dirty = true;
            return;
        }
        let Some(run) = self.job.take() else {
            return;
        };
        let _ = run.job.finish();
        self.set_preview(None);
        self.run(run.command);
        self.notice = run.warning;
        self.overlay_dirty = true;
        self.refresh();
    }

    /// The measurement box while an edit is being made: how far it is.
    pub(super) fn job_measurement(&self) -> Option<(&'static str, String)> {
        let run = self.job.as_ref()?;
        let (done, all) = run.job.progress();
        Some(("Shelling", format!("{done} of {all} faces")))
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
        if self.shell_handle.take().is_some() {
            self.set_preview(None);
            self.overlay_dirty = true;
            self.refresh();
            return true;
        }
        if self.rim_drag_held() {
            self.rim_clear();
            self.notice = None;
            self.overlay_dirty = true;
            self.refresh();
            return true;
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

    /// The selected live shell's handle: its definition, index, where the
    /// handle's line starts (the middle of the first face shelled, as it
    /// was), the face's normal and the thickness. Only with the Shell tool,
    /// and nothing being sized.
    fn shell_anchor(&self) -> Option<(DefId, usize, DVec3, DVec3, f64)> {
        if self.tool != SHELL || self.shell_op.is_some() {
            return None;
        }
        let index = self.selected_feature()?;
        let asm = self.doc.assembly();
        let def = asm.context_def(&self.ctx)?;
        let Feature::Shell {
            faces,
            thickness,
            broken: false,
            ..
        } = asm.def(def)?.stack.as_ref()?.features.get(index)?
        else {
            return None;
        };
        let f = faces.first().filter(|f| f.normal != DVec3::ZERO)?;
        Some((def, index, f.centre, f.normal, *thickness))
    }

    /// A press on the thickness handle starts dragging it. True if it did.
    fn shell_handle_press(&mut self, s: DVec2) -> bool {
        let Some((def, index, centre, normal, t)) = self.shell_anchor() else {
            return false;
        };
        let near = self
            .view()
            .to_screen(centre + normal * t)
            .is_some_and(|(q, _)| q.distance(s) < GRAB * self.settings.aperture);
        if near {
            self.shell_handle = Some(ShellHandle {
                def,
                index,
                centre,
                normal,
                start: t,
                size: t,
                rims: None,
            });
        }
        near
    }

    /// While the handle is held: the thickness is how far along the face's
    /// normal the pointer is, shown on the body. True while dragging.
    fn shell_handle_drag(&mut self, s: DVec2, view: &View) -> bool {
        let Some(h) = self.shell_handle.clone() else {
            return false;
        };
        let (o, d) = view.ray(s);
        if let Some(t) = along_line(o, d, h.centre, h.normal)
            && t.abs() > self.scene.tol.dist
            && t != h.size
            && let Some((m, rims)) = self.shell_at_thickness(h.def, h.index, t, h.rims.as_ref())
        {
            // A very thin wall is made, with a warning.
            self.notice = prism_ops::shelling::thin_warning(t);
            if let Some(h) = &mut self.shell_handle {
                h.size = t;
                h.rims = Some(rims);
            }
            self.set_preview(Some(m));
        }
        self.overlay_dirty = true;
        self.dirty = true;
        true
    }

    /// The body with shell `index` at thickness `t`, and the rim styles it
    /// ends with (for `t`'s side), or None if the kernel refuses it.
    /// `kept`: the styles picked earlier in the drag, used again while `t`
    /// is on the same side and they still build (exact on release, which
    /// picks afresh).
    fn shell_at_thickness(
        &self,
        def: DefId,
        index: usize,
        t: f64,
        kept: Option<&(bool, prism_ops::shelling::RimStyles)>,
    ) -> Option<(prism_topo::Model, (bool, prism_ops::shelling::RimStyles))> {
        let mut stack = self.doc.assembly().def(def)?.stack.clone()?;
        match stack.features.get_mut(index)? {
            Feature::Shell { thickness, .. } => *thickness = t,
            _ => return None,
        }
        let before = self.before(def, index, &stack);
        let side = t > 0.0;
        if let Some((_, styles)) = kept.filter(|(s, _)| *s == side) {
            let mut trial = stack.clone();
            let m = trial.evaluate_from_kept(&before, index, styles);
            if !trial.features[index].is_broken() {
                return Some((m, (side, styles.clone())));
            }
        }
        let styles = stack.shell_rims(&before, index)?;
        let m = stack.evaluate_from_kept(&before, index, &styles);
        (!stack.features[index].is_broken()).then_some((m, (side, styles)))
    }

    /// The button let go on the handle: the thickness it was dragged to is
    /// set, in one undo step. True if a handle was held.
    pub(super) fn shell_handle_release(&mut self) -> bool {
        let Some(h) = self.shell_handle.take() else {
            return false;
        };
        self.set_preview(None);
        if h.size != h.start {
            self.run(Command::SetFeatureSize {
                def: h.def,
                index: h.index,
                size: h.size,
            });
            // The feature's faces are new ones: keep it selected.
            self.select_feature(h.index);
        }
        self.overlay_dirty = true;
        self.refresh();
        true
    }

    pub(super) fn shell_clear(&mut self) {
        self.shell_op = None;
        self.shell_handle = None;
        self.rim_clear();
        self.shell_open.clear();
    }

    pub(super) fn shell_hint(&self) -> &'static str {
        if let Some(h) = self.rim_hint() {
            return h;
        }
        match &self.shell_op {
            // No thickness yet is the start of sizing, not a mistake.
            Some(op) => op
                .refused
                .filter(|r| *r != prism_ops::shelling::ZERO)
                .or(op.warning)
                .unwrap_or(if op.fixed { FIXED } else { SIZING }),
            None if self.shell_size.is_some() => FIXED,
            None if self.shell_anchor().is_some() => HANDLE,
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
        self.rim_overlay(o);
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
        if let Some((_, _, centre, normal, t)) = self.shell_anchor() {
            let drag = self.shell_handle.as_ref();
            let at = centre + normal * drag.map_or(t, |h| h.size);
            let color = if drag.is_some() {
                self.style.hover
            } else {
                fb.guide
            };
            o.lines.push(Line {
                a: centre,
                b: at,
                color,
                width: fb.target_width,
                dash: [6.0, 4.0],
            });
            o.marks.push(Mark {
                at,
                shape: Shape::Square,
                color,
                size: fb.glyph_size,
            });
        }
        if let Some(op) = &self.shell_op {
            for [a, b] in &op.flush {
                o.lines.push(Line {
                    a: *a,
                    b: *b,
                    color: fb.rim_flush,
                    width: fb.target_width * 1.5,
                    dash: [6.0, 4.0],
                });
            }
            for [a, b] in &op.ghost {
                o.lines.push(Line {
                    a: *a,
                    b: *b,
                    color: fb.loft_preview,
                    width: fb.target_width,
                    dash: [0.0, 0.0],
                });
            }
            // What the warning is about, over the ghost.
            for [a, b] in &op.flagged {
                o.lines.push(Line {
                    a: *a,
                    b: *b,
                    color: self.style.health_many,
                    width: fb.target_width * 1.5,
                    dash: [0.0, 0.0],
                });
            }
        }
    }
}

/// The warnings Shell / Thicken can give (the kernel's flags and the thin
/// wall), so the tool knows a notice is one of its own.
fn is_shell_warning(n: &'static str) -> bool {
    use prism_ops::shelling as sh;
    [
        sh::THIN,
        sh::APART,
        sh::APART_CORNER,
        sh::APART_SHARP,
        sh::APART_THICK,
        sh::APART_FREEFORM,
        sh::APART_BRANCHES,
        sh::SKIPPED,
        sh::UNJOINED,
        sh::BRANCHED,
        sh::BRIDGED,
        sh::TURNED,
        sh::SQUARED,
        sh::SQUARED_OFF,
        sh::SQUARED_ALONG,
    ]
    .contains(&n)
}

/// What to warn of in a shell about to be made (`b`, of the body in `m`,
/// `d` thick), and the lines to mark: the most serious flag first (faces
/// left as they were, then a face-by-face build, then the rest), or the
/// thin wall's warning. Lines are those the kernel flagged in the shell,
/// and the outlines of faces it left as they were.
fn shell_warning(
    m: &prism_topo::Model,
    b: &prism_ops::shelling::Flagged,
    d: f64,
) -> (Option<&'static str>, Vec<[DVec3; 2]>) {
    use prism_ops::shelling as sh;
    let rank = |w: &str| {
        if w == sh::SKIPPED || w == sh::UNJOINED {
            0
        } else if w.starts_with("Built face by face") {
            1
        } else {
            2
        }
    };
    let warning = b
        .flags
        .keys()
        .copied()
        .min_by_key(|w| rank(w))
        .or_else(|| sh::thin_warning(d));
    let mut lines = Vec::new();
    for (why, found) in &b.flags {
        if *why == sh::SKIPPED {
            for f in &found.faces {
                let Some(face) = m.face(*f) else {
                    continue;
                };
                for e in face.edge_ids() {
                    lines.extend(m.edge_points(e).windows(2).map(|w| [w[0], w[1]]));
                }
            }
            continue;
        }
        for e in &found.edges {
            if b.model.edge(*e).is_some() {
                lines.extend(b.model.edge_points(*e).windows(2).map(|w| [w[0], w[1]]));
            }
        }
    }
    (warning, lines)
}
