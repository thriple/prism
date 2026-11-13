//! The Loft tool (brief 34): pick sides by hand, then fill the ring they
//! close or loft through them.
//!
//! - A click on an edge picks the whole side: the edges that run on from
//!   it without a turn sharper than the window's threshold. Shift adds or
//!   removes one edge. Next Side (or Enter) finishes the side.
//! - Sides that close a ring fill it; sides that stand apart are lofted
//!   through, in the order picked. While they are valid the surface shows
//!   as a preview; Loft (or Enter) makes it, one command.
//! - Sides that almost close a ring: a tip says where it is open and the
//!   edges that would close it are highlighted; a click takes them.
//! - Esc steps back one side; with none, it leaves the tool.
//! - Right after a loft, the presets (smooth or straight, rows) change it.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_geom::NurbsSurface;
use prism_math::{DVec2, DVec3};
use prism_ops::sides::{self, Preset, Shape, Side};
use prism_topo::EdgeId;
use prism_view::overlay::{Label, Line, Overlay};
use prism_view::{Hit, View};
use serde::{Deserialize, Serialize};

use super::{Event, Input, Response, SELECT, Session};

pub const LOFT: &str = "tool.loft";

const HOW: &str = "Click an edge to pick a side · Shift adds or removes one edge · Enter or Next Side when the side is done · Esc steps back";
const READY: &str = "Enter or Loft makes it · click an edge to pick another side";
const TAKEN: &str =
    "That edge is already in a side. Esc steps back, or remove the side in the window.";

/// Lines across each way in a preview surface.
const PREVIEW_LINES: usize = 6;
/// Points along each preview line.
const PREVIEW_STEPS: usize = 24;

/// A change from the Loft window (recorded as `Input::Loft`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum LoftOp {
    /// Finish the side being picked.
    NextSide,
    /// Make the surface.
    Make,
    /// Take side `i` out (counting from 0, in the order picked).
    Remove(usize),
    /// The sharpest turn a side runs on through, in degrees.
    Turn(f64),
    /// Straight between sections (else smooth).
    Straight(bool),
    /// Rows of faces.
    Rows(usize),
}

/// What the Loft window shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoftPanel {
    /// Each finished side: how many edges it has.
    pub sides: Vec<usize>,
    /// Edges in the side being picked.
    pub current: usize,
    pub turn_deg: f64,
    pub straight: bool,
    pub rows: usize,
    pub can_next: bool,
    pub can_make: bool,
    /// What the sides make now ("Fill a ring of 3 sides", ...).
    pub makes: Option<String>,
    /// Whether a preset changes the last loft.
    pub changes_last: bool,
    /// Where the ring is open, or why the sides make nothing.
    pub tip: Option<String>,
    pub how: &'static str,
}

/// What the sides make, worked out.
#[derive(Default)]
struct Found {
    closing: Vec<EdgeId>,
    tip: Option<&'static str>,
    makes: Option<String>,
    preview: Vec<NurbsSurface>,
}

/// The tool's state while sides are picked.
#[derive(Clone, Debug, Default)]
pub(super) struct Lofting {
    sides: Vec<Vec<EdgeId>>,
    current: Vec<EdgeId>,
    /// The edge the current side was picked from (a new threshold picks
    /// again from it); None once Shift has changed it.
    from: Option<EdgeId>,
    /// The edges that would close an almost-closed ring.
    closing: Vec<EdgeId>,
    tip: Option<&'static str>,
    makes: Option<String>,
    valid: bool,
    preview: Vec<NurbsSurface>,
}

impl Lofting {
    fn all(&self) -> Vec<Vec<EdgeId>> {
        let mut out = self.sides.clone();
        if !self.current.is_empty() {
            out.push(self.current.clone());
        }
        out
    }

    fn used(&self) -> BTreeSet<EdgeId> {
        self.sides.iter().flatten().copied().collect()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.sides.is_empty() && self.current.is_empty()
    }
}

impl Session {
    fn loft_turn_rad(&self) -> f64 {
        self.loft_turn.to_radians()
    }

    pub(super) fn loft_hover(&mut self, s: DVec2, view: &View) {
        let h = super::edit::edge_pick(&self.scene, view, s, self.settings.aperture);
        self.set_hover_pick(h);
    }

    pub(super) fn loft_press(&mut self, shift: bool) {
        self.loft_notice = None;
        let Some(e) = self.hover.and_then(|p| match p.hit {
            Hit::Edge(e) => Some(e),
            _ => None,
        }) else {
            return;
        };
        if self.lofting.is_empty() {
            // Starting again: the presets now shape the new one.
            self.loft_last = None;
        }
        let l = &mut self.lofting;
        if l.closing.contains(&e) {
            if !l.current.is_empty() {
                let c = std::mem::take(&mut l.current);
                l.sides.push(c);
            }
            let c = std::mem::take(&mut l.closing);
            l.sides.push(c);
            l.from = None;
        } else if l.used().contains(&e) {
            self.loft_notice = Some(TAKEN);
        } else if shift {
            match l.current.iter().position(|k| *k == e) {
                Some(i) => {
                    l.current.remove(i);
                }
                None => l.current.push(e),
            }
            l.from = None;
        } else {
            self.loft_pick_from(e);
        }
        self.loft_update();
    }

    /// The current side: the run of edges from `e`, leaving out edges
    /// already in a side.
    fn loft_pick_from(&mut self, e: EdgeId) {
        let used = self.lofting.used();
        let run = sides::chain(self.work_model(), e, self.loft_turn_rad());
        // Stop the run where it reaches a side already picked.
        let at = run.iter().position(|k| *k == e).unwrap_or(0);
        let mut lo = at;
        while lo > 0 && !used.contains(&run[lo - 1]) {
            lo -= 1;
        }
        let mut hi = at;
        while hi + 1 < run.len() && !used.contains(&run[hi + 1]) {
            hi += 1;
        }
        self.lofting.current = run[lo..=hi].to_vec();
        self.lofting.from = Some(e);
    }

    /// Work out what the sides make: the preview, the tip, the closing
    /// edges.
    pub(super) fn loft_update(&mut self) {
        // Sides whose edges went (undo) are dropped.
        let model = self.work_model();
        let keep = |s: &[EdgeId]| s.iter().all(|e| model.edge(*e).is_some());
        let sides: Vec<Vec<EdgeId>> = self
            .lofting
            .sides
            .iter()
            .filter(|s| keep(s))
            .cloned()
            .collect();
        let current: Vec<EdgeId> = self
            .lofting
            .current
            .iter()
            .copied()
            .filter(|e| model.edge(*e).is_some())
            .collect();
        self.lofting.sides = sides;
        self.lofting.current = current;
        let found = self.loft_found();
        let l = &mut self.lofting;
        l.valid = !found.preview.is_empty();
        l.makes = if l.valid { found.makes } else { None };
        l.preview = found.preview;
        l.tip = found.tip;
        l.closing = found.closing;
        self.overlay_dirty = true;
        self.dirty = true;
    }

    fn loft_found(&self) -> Found {
        let model = self.work_model();
        let all = self.lofting.all();
        let list: Vec<Side> = match all.iter().map(|e| sides::side(model, e)).collect() {
            Ok(list) => list,
            Err(why) => {
                return Found {
                    tip: Some(why),
                    ..Found::default()
                };
            }
        };
        let tol = model.tol().dist * 10.0;
        let mut closing = Vec::new();
        let (mut tip, makes) = match sides::shape(&list, tol) {
            Shape::Few => (None, None),
            Shape::Ring { .. } if list.len() == 2 => (Some(sides::RING_OF_TWO), None),
            Shape::Ring { .. } => (None, Some(format!("Fill a ring of {} sides", list.len()))),
            Shape::Sections => (None, Some(format!("Loft through {} sections", list.len()))),
            Shape::Open { ends } => {
                closing = self.loft_closing(ends);
                let tip = if closing.is_empty() {
                    "The ring is open between the first side's start and the last side's end."
                } else {
                    "The ring is open: click the highlighted edges to close it."
                };
                (Some(tip), None)
            }
            Shape::Mixed => (Some(sides::OPEN), None),
        };
        let mut preview = Vec::new();
        if makes.is_some() {
            match sides::surfaces(model, &list, self.loft_preset) {
                Ok(s) => preview = s,
                Err(why) => tip = Some(why),
            }
        }
        Found {
            closing,
            tip,
            makes,
            preview,
        }
    }

    /// The run of edges, not yet in a side, that joins the ring's two open
    /// ends.
    fn loft_closing(&self, ends: [DVec3; 2]) -> Vec<EdgeId> {
        let model = self.work_model();
        let used: BTreeSet<EdgeId> = self.lofting.all().into_iter().flatten().collect();
        let Some(a) = prism_ops::sticky::vertex_at(model, ends[0]) else {
            return Vec::new();
        };
        let Some(b) = prism_ops::sticky::vertex_at(model, ends[1]) else {
            return Vec::new();
        };
        // A run from an edge at one end that reaches the other: first runs
        // at the window's threshold, then any run of edges.
        for turn in [self.loft_turn_rad(), std::f64::consts::PI] {
            for (id, e) in model.edges() {
                if used.contains(&id) || !(e.has(a) || e.has(b)) {
                    continue;
                }
                let run = sides::chain(model, id, turn);
                if run.iter().any(|k| used.contains(k)) {
                    continue;
                }
                // The run's ends, trimmed to the stretch from a to b.
                if let Some(part) = between_ends(model, &run, a, b) {
                    return part;
                }
            }
        }
        Vec::new()
    }

    /// Finish the side being picked.
    fn loft_next(&mut self) {
        let l = &mut self.lofting;
        if l.current.is_empty() {
            return;
        }
        let c = std::mem::take(&mut l.current);
        l.sides.push(c);
        l.from = None;
        self.loft_update();
    }

    /// Make the surface: one command.
    fn loft_make(&mut self) {
        if !self.lofting.valid {
            self.loft_notice = self.lofting.tip.is_none().then_some(sides::TOO_FEW);
            return;
        }
        let all = self.lofting.all();
        let c = Command::LoftSides {
            sides: all.clone(),
            preset: self.loft_preset,
        };
        if let Some(why) = c.refusal(self.work_model()) {
            self.loft_notice = Some(why);
            return;
        }
        if self.run(c) {
            self.lofting = Lofting::default();
            self.loft_last = Some(all);
            self.overlay_dirty = true;
            self.refresh();
        }
    }

    /// A preset: changes the last loft in place (one undo step) when
    /// nothing new has been picked since; else the preview.
    fn loft_set_preset(&mut self, p: Preset) {
        if p == self.loft_preset {
            return;
        }
        self.loft_preset = p;
        if let (Some(last), true) = (self.loft_last.clone(), self.lofting.is_empty()) {
            if self.doc.undo() {
                self.model_changed();
            }
            let c = Command::LoftSides {
                sides: last,
                preset: p,
            };
            match c.refusal(self.work_model()) {
                None => {
                    self.run(c);
                }
                Some(why) => {
                    if self.doc.redo() {
                        self.model_changed();
                    }
                    self.loft_notice = Some(why);
                }
            }
        }
        self.loft_update();
        self.refresh();
    }

    pub(super) fn loft_set(&mut self, op: LoftOp) {
        self.loft_notice = None;
        match op {
            LoftOp::NextSide => self.loft_next(),
            LoftOp::Make => self.loft_make(),
            LoftOp::Remove(i) => {
                if i < self.lofting.sides.len() {
                    self.lofting.sides.remove(i);
                    self.loft_update();
                }
            }
            LoftOp::Turn(deg) => {
                self.loft_turn = deg.clamp(1.0, 90.0);
                if let Some(e) = self.lofting.from {
                    self.loft_pick_from(e);
                    self.loft_update();
                }
            }
            LoftOp::Straight(s) => self.loft_set_preset(Preset {
                straight: s,
                ..self.loft_preset
            }),
            LoftOp::Rows(r) => self.loft_set_preset(Preset {
                rows: r.clamp(1, sides::MAX_ROWS),
                ..self.loft_preset
            }),
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// A change from the Loft window, recorded like any input.
    pub fn loft_input(&mut self, op: LoftOp) {
        let t = self.time;
        self.handle(&Event {
            t,
            input: Input::Loft { op },
        });
    }

    /// Enter: finish the side being picked, else make the surface.
    pub(super) fn loft_enter(&mut self) -> bool {
        if self.tool != LOFT {
            return false;
        }
        if self.lofting.current.is_empty() {
            self.loft_make();
        } else {
            self.loft_next();
        }
        true
    }

    /// Esc: drop the side being picked, else the last side; with none,
    /// leave the tool. True if it was used here.
    pub(super) fn loft_escape(&mut self, resp: &mut Response) -> bool {
        if self.tool != LOFT {
            return false;
        }
        let l = &mut self.lofting;
        if !l.current.is_empty() {
            l.current.clear();
            l.from = None;
        } else if l.sides.pop().is_none() {
            self.set_tool(SELECT);
            resp.tool = Some(SELECT.to_owned());
            return true;
        }
        self.loft_update();
        self.refresh();
        true
    }

    /// Forget the sides (leaving the tool).
    pub(super) fn loft_clear(&mut self) {
        if !self.lofting.is_empty() || !self.lofting.preview.is_empty() {
            self.lofting = Lofting::default();
            self.overlay_dirty = true;
        }
        self.loft_notice = None;
    }

    /// What the Loft window shows, while the tool is active.
    pub fn loft_panel(&self) -> Option<LoftPanel> {
        (self.tool == LOFT).then(|| {
            let l = &self.lofting;
            LoftPanel {
                sides: l.sides.iter().map(Vec::len).collect(),
                current: l.current.len(),
                turn_deg: self.loft_turn,
                straight: self.loft_preset.straight,
                rows: self.loft_preset.rows,
                can_next: !l.current.is_empty(),
                can_make: l.valid,
                makes: l.makes.clone(),
                changes_last: self.loft_last.is_some() && l.is_empty(),
                tip: l.tip.map(str::to_owned),
                how: HOW,
            }
        })
    }

    /// The sides picked so far, the side being picked last (for tests and
    /// MCP).
    pub fn loft_sides(&self) -> Vec<Vec<EdgeId>> {
        self.lofting.all()
    }

    /// The edges that would close an almost-closed ring.
    pub fn loft_closing_edges(&self) -> &[EdgeId] {
        &self.lofting.closing
    }

    /// Whether the preview shows a surface.
    pub fn loft_previewing(&self) -> bool {
        !self.lofting.preview.is_empty()
    }

    pub(super) fn loft_hint(&self) -> &'static str {
        let l = &self.lofting;
        if let Some(n) = self.loft_notice.or(l.tip) {
            return n;
        }
        if l.valid && l.current.is_empty() {
            return READY;
        }
        HOW
    }

    /// The sides in their colours and numbered, the side being picked, the
    /// closing edges, and the preview.
    pub(super) fn loft_overlay(&self, o: &mut Overlay) {
        if self.tool != LOFT {
            return;
        }
        let fb = &self.style.feedback;
        let model = self.work_model();
        let mut draw = |edges: &[EdgeId], color, dash: [f32; 2], width: f32| {
            let mut pts = Vec::new();
            for e in edges {
                let p = model.edge_points(*e);
                for w in p.windows(2) {
                    o.lines.push(Line {
                        a: w[0],
                        b: w[1],
                        color,
                        width,
                        dash,
                    });
                }
                pts.extend(p);
            }
            pts
        };
        let solid = [0.0, 0.0];
        for (i, s) in self.lofting.sides.iter().enumerate() {
            let color = fb.loft_sides[i % fb.loft_sides.len()];
            let pts = draw(s, color, solid, fb.target_width);
            if let Some(at) = pts.get(pts.len() / 2) {
                o.labels.push(Label {
                    at: *at,
                    offset: [8.0, -12.0],
                    text: format!("{}", i + 1),
                    color,
                });
            }
        }
        draw(
            &self.lofting.current,
            fb.loft_current,
            solid,
            fb.target_width,
        );
        draw(
            &self.lofting.closing,
            fb.loft_close,
            fb.guide_dash,
            fb.target_width,
        );
        for s in &self.lofting.preview {
            let [(u0, u1), (v0, v1)] = s.domain();
            for k in 0..=PREVIEW_LINES {
                let f = k as f64 / PREVIEW_LINES as f64;
                for along_u in [true, false] {
                    let at = |t: f64| {
                        if along_u {
                            s.eval(u0 + (u1 - u0) * t, v0 + (v1 - v0) * f)
                        } else {
                            s.eval(u0 + (u1 - u0) * f, v0 + (v1 - v0) * t)
                        }
                    };
                    for j in 0..PREVIEW_STEPS {
                        o.lines.push(Line {
                            a: at(j as f64 / PREVIEW_STEPS as f64),
                            b: at((j + 1) as f64 / PREVIEW_STEPS as f64),
                            color: fb.loft_preview,
                            width: fb.guide_width,
                            dash: solid,
                        });
                    }
                }
            }
        }
    }
}

/// The stretch of `run` (edges in order, end to end) from vertex `a` to
/// vertex `b`, if both are on it.
fn between_ends(
    model: &prism_topo::Model,
    run: &[EdgeId],
    a: prism_topo::VertexId,
    b: prism_topo::VertexId,
) -> Option<Vec<EdgeId>> {
    // The vertices along the run.
    let first = model.edge(*run.first()?)?;
    let mut v = match run.get(1).and_then(|k| model.edge(*k)) {
        Some(next) if next.has(first.b) => first.a,
        Some(_) => first.b,
        None => first.a,
    };
    let mut verts = vec![v];
    for e in run {
        v = model.edge(*e)?.other(v);
        verts.push(v);
    }
    let i = verts.iter().position(|x| *x == a)?;
    let j = verts.iter().position(|x| *x == b)?;
    let (lo, hi) = (i.min(j), i.max(j));
    (lo < hi).then(|| run[lo..hi].to_vec())
}
