//! The Fillet / Chamfer tool (brief 26, step 5a; its window, brief 29):
//! click a solid's edge to round it, or cut it off flat (chamfer), at the
//! window's size (empty fits each edge). Ctrl swaps the kind while held.
//! Hovering shows the result before the click. A new size or kind right
//! after a round changes it. A click near a flat face's corner rounds the
//! corner (brief 24's fillet).
//!
//! Many edges at once (Fillet Joins step C, brief 32; owner, 2026-11-18):
//! a click collects the whole straight run of segments the edge is in (a
//! click on one already collected takes it out); a double-click on a face
//! collects all its edges. The collected edges are the selection (edges or
//! faces selected before the tool start it) and show rounded as a preview.
//! Drag from an edge, or type a size, to set the size. Nothing is made
//! until Enter (a typed size + Enter rounds them at that size): then they
//! are rounded together, so rounds that meet at a corner join. Esc clears
//! them; leaving the tool makes nothing.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_math::{DVec2, DVec3};
use prism_topo::EdgeId;
use prism_view::overlay::{Line, Overlay};
use prism_view::{Hit, View};
use serde::{Deserialize, Serialize};

use super::{Event, Input, Session, pick};

pub const FILLET: &str = "tool.fillet";

const HOW: &str = "Click edges to collect them (a line in pieces as a whole) · double-click a face for all its edges · drag from an edge or type to set the size · Enter rounds them · Ctrl swaps fillet and chamfer";
/// Said when a size past what the edges take is shown as the largest
/// that works (DECISIONS #45, kind 2).
const CLAMPED: &str =
    "That size is more than these edges take: the largest that works is shown, and Enter makes it.";
/// Said when no size fits the edges on its own (curved edges only).
const NEED_SIZE: &str = "Type a size for these edges (curved edges have no size of their own)";
const HOW_FEATURE: &str = "Drag the square handle or type a size + Enter to change the selected fillet or chamfer · click an edge to round more · Esc deselects";
const HOW_SET: &str = "Drag the square handle to size the round · Enter rounds the collected edges · click adds or takes away edges · or type a size + Enter · Esc clears · Ctrl swaps fillet and chamfer";

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

/// A press on an edge: a click collects it, a drag sets the size.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Down {
    at: DVec2,
    edges: BTreeSet<EdgeId>,
    dragging: bool,
    /// Pressed on the collected edges' size handle: the guide it is
    /// measured on and which face of it.
    handle: Option<(prism_doc::SizeGuide, usize)>,
}

/// The largest size that works for a target: (document revision,
/// target, fillet, size, model).
pub(super) type LimitCache =
    std::cell::RefCell<Option<(u64, Target, bool, f64, prism_topo::Model)>>;

/// What a click would round.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Target {
    Corner(DVec3),
    Edge(EdgeId),
    /// Several edges of one solid, rounded together through the kernel.
    Edges(BTreeSet<EdgeId>),
}

impl Session {
    /// The kind a click makes now: the window's, swapped while Ctrl is held.
    fn round_fillet(&self, ctrl: bool) -> bool {
        self.round_chamfer == ctrl
    }

    /// The edges the selection names: its edges as picked, and every edge
    /// of its faces. Empty with nothing selected.
    fn round_selected(&self) -> BTreeSet<EdgeId> {
        // A selected feature is not edges to round (its handle sets its size).
        if self.selected_feature().is_some() {
            return BTreeSet::new();
        }
        let m = self.work_model();
        let mut out = self.selection.edges.clone();
        for f in &self.selection.faces {
            if let Some(face) = m.face(*f) {
                out.extend(face.edge_ids());
            }
        }
        out
    }

    /// What a click at `s` rounds: the selection when there is one, else
    /// the corner or edge (with its straight run) under the cursor.
    fn round_target(&self, s: DVec2) -> Option<Target> {
        let set = self.round_selected();
        if !set.is_empty() {
            return Some(Target::Edges(set));
        }
        self.round_hovered(s)
    }

    /// The corner, or the edge and its straight run, under the cursor.
    fn round_hovered(&self, s: DVec2) -> Option<Target> {
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
            None => {
                let run = prism_ops::rounds::straight_run(m, e);
                if run.len() > 1 {
                    Target::Edges(run)
                } else {
                    Target::Edge(e)
                }
            }
        })
    }

    /// The command a click on `t` would run, or why there is none.
    fn round_command(&self, t: &Target, fillet: bool) -> Result<Command, &'static str> {
        let m = self.work_model();
        let c = match t.clone() {
            Target::Edges(edges) => {
                let size = self
                    .round_size
                    .or_else(|| prism_ops::rounds::fit(m, &edges))
                    .ok_or(NEED_SIZE)?;
                Command::RoundEdges {
                    edges,
                    size,
                    fillet,
                }
            }
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
        if self.handle_drag(s, view) || self.round_drag(s, view) {
            return;
        }
        if self.selected_feature().is_some() {
            // The handle lights up when the cursor is near it.
            self.overlay_dirty = true;
        }
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
        let key = target.clone().map(|t| (t, fillet, self.round_size));
        if key == self.round_shown {
            return;
        }
        self.round_shown = key;
        let collected = !self.round_selected().is_empty();
        // While a size is being dragged, a size that cannot be made keeps
        // the last one that could on screen, and the reason shows at the
        // cursor (owner, 2026-11-26: the preview blanked mid-drag with
        // only small text in the window to say why).
        let dragging = self.round_down.as_ref().is_some_and(|d| d.dragging);
        self.round_clamped = None;
        let built = target.clone().map(|t| self.round_command(&t, fillet));
        // Past what the edges take: the largest size that works is built
        // and shown, and says so (DECISIONS #45, kind 2).
        let clamped = match (&built, &target, self.round_size) {
            (Some(Err(why)), Some(t), Some(asked)) if clampable(why) => self
                .round_largest(t, fillet, asked)
                .map(|(made, m)| (*why, asked, made, m)),
            _ => None,
        };
        let built = match clamped {
            Some((why, asked, made, m)) => {
                self.round_clamped = Some((asked, made, why));
                Some(Ok((CLAMPED, m)))
            }
            None => built.map(|b| b.map(|c| ("", self.applied(&c)))),
        };
        let (why, preview) = match built {
            None => (None, None),
            Some(Ok((why, m))) => ((!why.is_empty()).then_some(why), Some(m)),
            Some(Err(why)) if dragging && self.preview.is_some() => {
                self.round_refused = Some(why);
                self.overlay_dirty = true;
                return;
            }
            Some(Err(why)) => (Some(why), None),
        };
        // The collected edges' round is not made yet: what it would make
        // (the faces the model does not have, and their lines) shows
        // selected until Enter. A hovered edge's preview is not marked.
        self.round_pending = match (&preview, collected) {
            (Some(m), true) => {
                let have = self.work_model();
                let faces: BTreeSet<_> = m
                    .faces()
                    .map(|(id, _)| id)
                    .filter(|id| have.face(*id).is_none())
                    .collect();
                let edges = faces
                    .iter()
                    .filter_map(|f| m.face(*f))
                    .flat_map(|f| f.edge_ids())
                    .collect();
                prism_ops::Selection {
                    faces,
                    edges,
                    ..Default::default()
                }
            }
            _ => prism_ops::Selection::default(),
        };
        self.round_refused = why;
        self.set_preview(preview);
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The work model with `c` applied.
    fn applied(&self, c: &Command) -> prism_topo::Model {
        let mut m = self.work_model().clone();
        c.apply(&mut m);
        m
    }

    /// The largest size up to `asked` that `t`'s edges take, and the model
    /// rounded at it: halving from `asked` until one works, then halving
    /// the gap. None if not even a small one works. Kept while the size
    /// asked stays past it, so a drag past the limit stays quick.
    fn round_largest(
        &self,
        t: &Target,
        fillet: bool,
        asked: f64,
    ) -> Option<(f64, prism_topo::Model)> {
        let edges = match t {
            Target::Edge(e) => BTreeSet::from([*e]),
            Target::Edges(set) => set.clone(),
            Target::Corner(_) => return None,
        };
        let revision = self.doc.revision();
        if let Some((r, kt, kf, made, m)) = &*self.round_limit.borrow()
            && *r == revision
            && kt == t
            && *kf == fillet
            && asked >= *made
        {
            return Some((*made, m.clone()));
        }
        let work = self.work_model();
        let build = |size: f64| {
            let mut m = work.clone();
            prism_ops::rounds::round_edges(&mut m, &edges, size, fillet)
                .ok()
                .map(|_| m)
        };
        let floor = self.scene.tol.dist * 10.0;
        let (mut hi, mut size) = (asked, asked / 2.0);
        let mut good = None;
        while size > floor {
            if let Some(m) = build(size) {
                good = Some((size, m));
                break;
            }
            hi = size;
            size /= 2.0;
        }
        let (mut made, mut model) = good?;
        for _ in 0..7 {
            let mid = (made + hi) / 2.0;
            match build(mid) {
                Some(m) => {
                    made = mid;
                    model = m;
                }
                None => hi = mid,
            }
        }
        *self.round_limit.borrow_mut() = Some((revision, t.clone(), fillet, made, model.clone()));
        Some((made, model))
    }

    /// What to draw as selected: the round waiting for Enter while there
    /// is one (its faces and lines, in the preview), else the selection.
    pub fn shown_selection(&self) -> &prism_ops::Selection {
        if self.tool == FILLET && self.preview.is_some() && !self.round_pending.is_empty() {
            &self.round_pending
        } else {
            &self.selection
        }
    }

    /// Take the preview away (leaving the tool, Esc, after a click).
    pub(super) fn round_clear(&mut self) {
        self.round_pending = prism_ops::Selection::default();
        self.round_handle = None;
        self.round_refused = None;
        self.round_clamped = None;
        self.round_at = None;
        if (self.round_shown.take().is_some() || self.tool == FILLET) && self.preview.is_some() {
            self.set_preview(None);
        }
    }

    pub(super) fn round_press(&mut self, s: DVec2, n_clicks: u32, _shift: bool, ctrl: bool) {
        self.round_notice = None;
        self.round_down = None;
        if self.handle_press(s) || self.set_handle_press(s) {
            return;
        }
        // A double-click on a face: all its edges join the set.
        if n_clicks == 2
            && let Some(Hit::Face(f)) =
                pick::pick(&self.scene, &self.view(), s, self.settings.aperture).map(|p| p.hit)
            && self.owner(Hit::Face(f)).is_none()
        {
            let edges: BTreeSet<EdgeId> = self
                .work_model()
                .face(f)
                .map(|x| x.edge_ids().collect())
                .unwrap_or_default();
            self.round_collect(&edges, Some(true));
            return;
        }
        match self.round_hovered(s) {
            // A flat face's corner is rounded at once (a drawing step).
            Some(Target::Corner(c)) => self.round_go(Target::Corner(c), self.round_fillet(ctrl)),
            Some(Target::Edge(e)) => {
                self.round_down = Some(Down {
                    at: s,
                    edges: BTreeSet::from([e]),
                    dragging: false,
                    handle: None,
                });
            }
            Some(Target::Edges(edges)) => {
                self.round_down = Some(Down {
                    at: s,
                    edges,
                    dragging: false,
                    handle: None,
                });
            }
            None => {}
        }
    }

    /// The button let go: a click (no drag) collects the edges, or takes
    /// them out if they were all in.
    pub(super) fn round_release(&mut self) {
        if self.handle_release() {
            return;
        }
        if let Some(d) = self.round_down.take()
            && !d.dragging
        {
            self.round_collect(&d.edges, None);
        }
    }

    /// Add edges to the set (`Some(true)`), take them out, or (None) take
    /// them out if all are in, else add them.
    fn round_collect(&mut self, edges: &BTreeSet<EdgeId>, add: Option<bool>) {
        if edges.is_empty() {
            return;
        }
        // A selected feature is left; the edges start a new set.
        let mut sel = if self.selected_feature().is_some() {
            prism_ops::Selection::default()
        } else {
            self.selection.clone()
        };
        let add = add.unwrap_or_else(|| !edges.iter().all(|e| sel.edges.contains(e)));
        if add {
            sel.edges.extend(edges.iter().copied());
        } else {
            sel.edges.retain(|e| !edges.contains(e));
        }
        // A new size or kind now applies to the set, not the last round.
        self.round_last = None;
        self.set_selection(sel);
        self.round_shown = None;
        self.round_preview();
        self.overlay_dirty = true;
        self.refresh();
    }

    /// While the button is held after a press on an edge: past a few
    /// pixels it is a drag, which collects the edge and sets the size from
    /// how far the cursor is from it across its face. True while dragging.
    fn round_drag(&mut self, s: DVec2, view: &View) -> bool {
        let Some(d) = self.round_down.clone() else {
            return false;
        };
        if !d.dragging {
            if s.distance(d.at) < super::edit::DRAG_MIN {
                return false;
            }
            self.round_down = Some(Down {
                dragging: true,
                ..d.clone()
            });
            self.round_collect(&d.edges, Some(true));
        }
        let size = match &d.handle {
            Some((guide, side)) => guide_size(guide, *side, s, view),
            None => d
                .edges
                .first()
                .and_then(|e| self.round_size_at(*e, s, view)),
        };
        if let Some(size) = size.filter(|x| *x > self.scene.tol.dist) {
            self.round_size = Some(size);
            self.round_shown = None;
            self.round_preview();
        }
        self.overlay_dirty = true;
        self.dirty = true;
        true
    }

    /// The collected edges' size handle (brief 55): the longest straight
    /// one with a flat face beside it, its guide, the face it is measured
    /// across (the one facing the view most) and the size it shows.
    fn set_handle(&self) -> Option<(prism_doc::SizeGuide, usize, f64)> {
        if self.tool != FILLET || self.selected_feature().is_some() {
            return None;
        }
        let set = self.round_selected();
        let m = self.work_model();
        let guide = set
            .iter()
            .filter_map(|e| prism_doc::edge_guide(m, *e))
            .max_by(|x, y| x.a.distance(x.b).total_cmp(&y.a.distance(y.b)))?;
        let size = self
            .round_size
            .or_else(|| prism_ops::rounds::fit(m, &set))?;
        let view = self.view();
        let to_eye = view.eye - (guide.a + guide.b) * 0.5;
        let side = (0..guide.sides.len()).max_by(|x, y| {
            let f = |k: usize| guide.sides[k].normal.dot(to_eye).abs();
            f(*x).total_cmp(&f(*y))
        })?;
        Some((guide, side, size))
    }

    /// Where the collected edges' size handle is drawn.
    fn set_handle_at(&self) -> Option<(prism_doc::SizeGuide, usize, DVec3)> {
        let (guide, side, size) = match self.round_down.as_ref().and_then(|d| d.handle.clone()) {
            Some((g, side)) => (g, side, self.round_size?),
            None => self.set_handle()?,
        };
        let at = (guide.a + guide.b) * 0.5 + guide.sides[side].across * size;
        Some((guide, side, at))
    }

    /// A press on the collected edges' size handle starts dragging it.
    fn set_handle_press(&mut self, s: DVec2) -> bool {
        let Some((guide, side, at)) = self.set_handle_at() else {
            return false;
        };
        let view = self.view();
        let near = view
            .to_screen(at)
            .is_some_and(|(q, _)| q.distance(s) < super::features::GRAB * self.settings.aperture);
        // A handle close to its edge (a small size) leaves a press on the
        // edge itself to drag from the edge, as before.
        let on_edge = match (
            view.to_screen(guide.a),
            view.to_screen(guide.b),
            view.to_screen(at),
        ) {
            (Some((a, _)), Some((b, _)), Some((q, _))) => {
                let ab = b - a;
                let t =
                    ((s - a).dot(ab) / ab.length_squared().max(f64::MIN_POSITIVE)).clamp(0.0, 1.0);
                s.distance(a + ab * t) < s.distance(q)
            }
            _ => false,
        };
        let grab = near && !on_edge;
        if grab {
            self.round_down = Some(Down {
                at: s,
                edges: self.round_selected(),
                dragging: true,
                handle: Some((guide, side)),
            });
            self.overlay_dirty = true;
        }
        grab
    }

    /// The collected edges drawn as the ones being rounded, and their size
    /// handle: across the face beside the longest from its middle, with
    /// the size written by it (orange, with why, when that size cannot be
    /// made).
    fn set_overlay(&self, o: &mut Overlay) {
        if self.tool != FILLET || self.selected_feature().is_some() {
            return;
        }
        let set = self.round_selected();
        if set.is_empty() {
            return;
        }
        let m = self.work_model();
        let fb = &self.style.feedback;
        for e in &set {
            for w in m.edge_points(*e).windows(2) {
                o.lines.push(Line {
                    a: w[0],
                    b: w[1],
                    color: self.style.selected,
                    width: fb.target_width * 1.5,
                    dash: [0.0, 0.0],
                });
            }
        }
        let Some((guide, _, at)) = self.set_handle_at() else {
            return;
        };
        let view = self.view();
        let held = self.round_down.as_ref().is_some_and(|d| d.handle.is_some());
        let over = held
            || self.round_at.is_some_and(|s| {
                view.to_screen(at).is_some_and(|(q, _)| {
                    q.distance(s) < super::features::GRAB * self.settings.aperture
                })
            });
        let color = if over { self.style.hover } else { fb.guide };
        let mid = (guide.a + guide.b) * 0.5;
        o.lines.push(Line {
            a: mid,
            b: at,
            color,
            width: fb.target_width,
            dash: [6.0, 4.0],
        });
        o.marks.push(prism_view::overlay::Mark {
            at,
            shape: prism_view::overlay::Shape::Square,
            color,
            size: fb.glyph_size * 1.25,
        });
        if let Some(size) = self.round_size.or_else(|| prism_ops::rounds::fit(m, &set)) {
            let kind = if self.round_fillet(self.ctrl_held) {
                "Fillet"
            } else {
                "Chamfer"
            };
            // While dragging, the reason is said at the cursor already.
            let text = match (self.round_refused, self.round_clamped) {
                (Some(_), Some((_, made, why))) if !held => {
                    format!("{kind} {}", clamp_text(why, &self.number.show(made)))
                }
                (_, Some(_)) => format!("{kind} {}", self.number.show(size)),
                (Some(why), None) if !held => format!(
                    "{kind} {}: {}",
                    self.number.show(size),
                    why.strip_prefix("Refused: ").unwrap_or(why)
                ),
                _ => format!("{kind} {}", self.number.show(size)),
            };
            o.labels.push(prism_view::overlay::Label {
                at,
                offset: [14.0, 10.0],
                text,
                color: if self.round_refused.is_some() {
                    self.style.health_many
                } else {
                    self.style.hover
                },
            });
        }
    }

    /// How far from edge `e` the cursor points, across the face beside it
    /// that faces the view most.
    fn round_size_at(&self, e: EdgeId, s: DVec2, view: &View) -> Option<f64> {
        let edge = self.scene.edge(e)?;
        let (o, d) = view.ray(s);
        let dir = (edge.b - edge.a).try_normalize()?;
        let face = edge
            .faces
            .iter()
            .flatten()
            .filter_map(|f| self.scene.face(*f))
            .filter_map(|f| Some((f.flat_normal()?, f.origin)))
            .max_by(|x, y| x.0.dot(d).abs().total_cmp(&y.0.dot(d).abs()))?;
        let (n, origin) = face;
        let den = n.dot(d);
        if den.abs() < prism_math::tol::UNIT_GUARD {
            return None;
        }
        let p = o + d * (n.dot(origin - o) / den);
        let w = p - edge.a;
        let size = (w - dir * w.dot(dir)).length();
        (size > self.scene.tol.dist).then_some(size)
    }

    /// Esc with edges collected: clear them. True if it did.
    pub(super) fn round_escape(&mut self) -> bool {
        if self.handle_cancel() {
            return true;
        }
        if self.tool != FILLET || (self.selection.is_empty() && self.round_down.is_none()) {
            return false;
        }
        self.round_down = None;
        self.set_selection(prism_ops::Selection::default());
        self.round_clear();
        self.overlay_dirty = true;
        self.refresh();
        true
    }

    /// Round `t` now (a click, or Enter on the selection).
    fn round_go(&mut self, t: Target, fillet: bool) {
        // A size past what the edges take makes the largest that works.
        if let Some((asked, made, _)) = self.round_clamped
            && self.round_size == Some(asked)
        {
            self.round_size = Some(made);
        }
        // The click's own result replaces the preview.
        self.round_clear();
        let from_selection = matches!(t, Target::Edges(_)) && !self.selection.is_empty();
        match self.round_command(&t, fillet) {
            Ok(c) => {
                let last = match &t {
                    Target::Corner(_) => None,
                    t => Some((t.clone(), fillet)),
                };
                if self.run(c) {
                    self.round_last = last;
                    if from_selection {
                        // The rounded edges are gone.
                        self.set_selection(prism_ops::Selection::default());
                    }
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

    /// The edges a click would round, drawn over the model while hovering
    /// (the selection shows as selected already).
    pub(super) fn round_overlay(&self, o: &mut Overlay) {
        self.handle_overlay(o);
        self.set_overlay(o);
        self.round_refusal_label(o);
        if self.tool != FILLET || !self.selection.is_empty() || self.round_refused.is_some() {
            return;
        }
        let Some(t) = self.round_at.and_then(|s| self.round_hovered(s)) else {
            return;
        };
        let edges = match t {
            Target::Edge(e) => vec![e],
            Target::Edges(set) => set.into_iter().collect(),
            Target::Corner(_) => return,
        };
        let m = self.work_model();
        let fb = &self.style.feedback;
        for e in edges {
            for w in m.edge_points(e).windows(2) {
                o.lines.push(Line {
                    a: w[0],
                    b: w[1],
                    color: self.style.hover,
                    width: fb.target_width,
                    dash: [0.0, 0.0],
                });
            }
        }
    }

    /// While a size is dragged past what can be made: why, at the cursor,
    /// in the warning colour, and that the last size that could is shown.
    fn round_refusal_label(&self, o: &mut Overlay) {
        let dragging =
            self.round_down.as_ref().is_some_and(|d| d.dragging) || self.round_handle.is_some();
        let (Some(why), Some(s), true) = (self.round_refused, self.cursor, dragging) else {
            return;
        };
        let view = self.view();
        let (eye, dir) = view.ray(s);
        let c = self.camera();
        let text = match self.round_clamped {
            Some((_, made, why)) => clamp_text(why, &self.number.show(made)),
            None => refusal_text(why),
        };
        o.labels.push(prism_view::overlay::Label {
            at: eye + dir * c.eye.distance(c.target),
            offset: [14.0, -16.0],
            text,
            color: self.style.health_many,
        });
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
        if let Some((target, was_fillet)) = self.round_last.clone() {
            let fillet = match op {
                RoundOp::Fillet(f) => f,
                RoundOp::Size(_) => was_fillet,
            };
            if self.doc.undo() {
                self.model_changed();
            }
            match self.round_command(&target, fillet) {
                Ok(c) => {
                    if self.run(c) {
                        self.round_last = Some((target, fillet));
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
                    Target::Edges(set) => prism_ops::rounds::fit(self.work_model(), &set),
                    Target::Corner(c) => prism_ops::fillet::default_radius(self.work_model(), c),
                })
                .map(|r| self.number.show(r)),
            changes_last: self.round_last.is_some(),
            refused: self.round_refused,
            how: self.round_how(),
        })
    }

    /// Typed text: the size for the next round, and right after one, a new
    /// size for it. True if it was used.
    pub(super) fn round_enter(&mut self, text: &str) -> bool {
        if self.tool != FILLET {
            return false;
        }
        // A selected feature: a typed size is its new size.
        if let Some(index) = self.selected_feature()
            && let Some(def) = self.doc.assembly().context_def(&self.ctx)
        {
            if let Some(size) = self.number.length(text.trim()).map(f64::abs)
                && size > 0.0
            {
                if self.run(Command::SetFeatureSize { def, index, size }) {
                    self.select_feature(index);
                } else {
                    self.round_notice = Some("The feature cannot be that size");
                }
            }
            return true;
        }
        // Enter alone rounds the selected edges.
        if text.trim().is_empty() {
            let set = self.round_selected();
            if !set.is_empty() {
                self.round_go(Target::Edges(set), self.round_fillet(self.ctrl_held));
            }
            return true;
        }
        if let Some(size) = self.number.length(text.trim()).map(f64::abs)
            && size > 0.0
        {
            let set = self.round_selected();
            if set.is_empty() {
                // Right after a round: a new size for it.
                self.round_set(RoundOp::Size(Some(size)));
            } else {
                // The collected edges are rounded at that size.
                self.round_size = Some(size);
                self.round_go(Target::Edges(set), self.round_fillet(self.ctrl_held));
            }
        }
        true
    }

    pub(super) fn round_hint(&self) -> &'static str {
        self.round_notice
            .or(self.round_refused)
            .unwrap_or_else(|| self.round_how())
    }

    fn round_how(&self) -> &'static str {
        if self.selected_feature().is_some() {
            HOW_FEATURE
        } else if self.selection.is_empty() {
            HOW
        } else {
            HOW_SET
        }
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

/// How far across side `side` of `guide` from its edge the pointer at
/// `s` is.
fn guide_size(guide: &prism_doc::SizeGuide, side: usize, s: DVec2, view: &View) -> Option<f64> {
    let face = guide.sides.get(side)?;
    let (o, d) = view.ray(s);
    let den = face.normal.dot(d);
    if den.abs() < prism_math::tol::UNIT_GUARD {
        return None;
    }
    let p = o + d * (face.normal.dot(face.origin - o) / den);
    Some((p - guide.a).dot(face.across))
}

/// Whether a refusal is of a size the edges cannot take, so a smaller
/// one may be made instead.
fn clampable(why: &str) -> bool {
    why == prism_ops::rounds::TOO_LARGE
        || why == prism_ops::rounds::NOT_ROUNDED
        || why == prism_ops::round::TOO_WIDE
}

/// Said when the largest size that works is made instead of the one asked.
pub(super) fn clamp_text(why: &str, made: &str) -> String {
    let why = why.strip_prefix("Refused: ").unwrap_or(why);
    format!(
        "made at {made}, the most these edges take ({})",
        why.trim_end_matches('.')
    )
}

/// A refusal said at the cursor while dragging: without "Refused:", and
/// that the size shown is the last one that worked.
pub(super) fn refusal_text(why: &str) -> String {
    let why = why.strip_prefix("Refused: ").unwrap_or(why);
    let mut c = why.chars();
    let why = match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect::<String>(),
        None => String::new(),
    };
    format!("{} Showing the last size that worked.", why.trim_end())
}
