//! Cage editing (Freeform NURBS part 3). Edit Cage shows the control
//! points of the selected freeform curve or face. While it is on:
//! - Select picks control points (Shift or Ctrl adds and removes);
//! - Move takes the picked points from one click to the next (a click on
//!   a control point picks it and moves it), with the usual inference,
//!   axis locks and a typed distance;
//! - Rotate turns them about their middle, from one click round to the
//!   next (about the locked axis, or blue), or by typed degrees;
//! - Scale sizes them with its box and grips, drawn round the picked
//!   points (as for any selection), or by a typed factor;
//! - a Select drag boxes control points (Shift toggles, Ctrl adds).
//!
//! Esc ends a step, then leaves the cage.

use std::collections::BTreeSet;

use prism_doc::Command;
use prism_math::{DAffine3, DVec2, DVec3};
use prism_ops::cage::{self, Cage};
use prism_view::overlay::{Line, Mark, Overlay, Shape};

use super::{MOVE, ROTATE, SCALE, SELECT, Session};

/// The cage being edited.
#[derive(Clone, Debug)]
pub(super) struct CageEdit {
    pub cage: Cage,
    /// The picked control points (places in `cage::points`).
    pub picked: BTreeSet<usize>,
    /// A step's first click.
    pub from: Option<DVec3>,
    /// The picks before the press that may start a box.
    pub pre: BTreeSet<usize>,
}

impl Session {
    /// Whether Edit Cage is on.
    pub fn cage_active(&self) -> bool {
        self.cage.is_some()
    }

    /// The picked control points, while Edit Cage is on.
    pub fn cage_picked(&self) -> Vec<usize> {
        self.cage
            .as_ref()
            .map_or(Vec::new(), |c| c.picked.iter().copied().collect())
    }

    /// The cage's control points, in the world.
    pub fn cage_points(&self) -> Vec<DVec3> {
        let Some(c) = &self.cage else {
            return Vec::new();
        };
        cage::points(self.work_model(), c.cage)
            .into_iter()
            .map(|p| self.to_world.transform_point3(p))
            .collect()
    }

    /// Pick control points by their places (for MCP and tests).
    pub fn cage_pick(&mut self, places: &[usize]) {
        let n = self.cage_points().len();
        if let Some(c) = &mut self.cage {
            c.picked = places.iter().copied().filter(|k| *k < n).collect();
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    /// Edit Cage: on for the selected freeform curve or face, or off.
    pub(super) fn cage_toggle(&mut self) {
        self.cage_notice = None;
        if self.cage.take().is_some() {
            self.cage_leave();
            return;
        }
        let m = self.work_model();
        match cage::cage_of(m, &self.selection.edges, &self.selection.faces) {
            Some(c) => {
                self.cancel();
                self.selection = Default::default();
                self.cage = Some(CageEdit {
                    cage: c,
                    picked: BTreeSet::new(),
                    from: None,
                    pre: BTreeSet::new(),
                });
                self.highlight_dirty = true;
            }
            None => self.cage_notice = Some(cage::NO_CAGE),
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    pub(super) fn cage_leave(&mut self) {
        self.cage = None;
        self.set_preview(None);
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Add control points to the cage (or to the selected curve or face).
    pub(super) fn cage_add(&mut self) {
        let target = self.cage.as_ref().map(|c| c.cage).or_else(|| {
            cage::cage_of(
                self.work_model(),
                &self.selection.edges,
                &self.selection.faces,
            )
        });
        let Some(target) = target else {
            self.cage_notice = Some(cage::NO_CAGE);
            return;
        };
        let mut w = self.work_model().clone();
        self.cage_notice = cage::add_points(&mut w, target).err();
        if self.cage_notice.is_none() {
            if let Some(c) = &mut self.cage {
                c.picked.clear();
            }
            self.run(Command::AddCagePoints { cage: target });
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Loft, Revolve, Sweep or Patch from the selected curves. The new faces are selected; a refusal says why.
    pub(super) fn surface_from_selection(&mut self, action: &str) {
        let edges = self.selection.edges.clone();
        let c = match action {
            "surf.loft" => Command::Loft { edges },
            "surf.revolve" => Command::Revolve { edges },
            "surf.sweep" => Command::Sweep { edges },
            _ => Command::Patch { edges },
        };
        self.cage_notice = c.refusal(self.work_model());
        if self.cage_notice.is_none() {
            let before: BTreeSet<prism_topo::FaceId> =
                self.work_model().faces().map(|(id, _)| id).collect();
            if self.run(c) {
                self.selection = prism_ops::Selection {
                    faces: self
                        .work_model()
                        .faces()
                        .map(|(id, _)| id)
                        .filter(|id| !before.contains(id))
                        .collect(),
                    ..Default::default()
                };
                self.highlight_dirty = true;
            }
        }
        self.overlay_dirty = true;
        self.dirty = true;
        self.refresh();
    }

    /// The cage went (undo, another edit): leave it.
    pub(super) fn cage_check(&mut self) {
        if self.cage.is_some() && self.cage_points().is_empty() {
            self.cage = None;
        }
    }

    /// Whether the cage takes the click instead of the tool.
    /// (Scale works through its own box and grips, round the picked
    /// points.)
    pub(super) fn cage_takes(&self) -> bool {
        self.cage.is_some() && matches!(self.tool.as_str(), SELECT | MOVE | ROTATE)
    }

    pub(super) fn cage_wants_inference(&self) -> bool {
        self.cage.is_some() && matches!(self.tool.as_str(), MOVE | ROTATE | SCALE)
    }

    pub(super) fn cage_start(&self) -> Option<DVec3> {
        self.cage.as_ref().and_then(|c| c.from)
    }

    /// The control point under the cursor, within the pick aperture.
    fn cage_point_at(&self, s: DVec2) -> Option<usize> {
        let view = self.view();
        self.cage_points()
            .iter()
            .enumerate()
            .filter_map(|(k, p)| {
                let (q, _) = view.to_screen(*p)?;
                let d = q.distance(s);
                (d <= self.settings.aperture.max(8.0)).then_some((d, k))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|x| x.1)
    }

    fn cage_middle(&self) -> Option<DVec3> {
        let c = self.cage.as_ref()?;
        let pts = self.cage_points();
        let picked: Vec<DVec3> = c
            .picked
            .iter()
            .filter_map(|k| pts.get(*k).copied())
            .collect();
        (!picked.is_empty()).then(|| picked.iter().copied().sum::<DVec3>() / picked.len() as f64)
    }

    fn cage_axis(&self) -> DVec3 {
        match self.lock {
            prism_infer::Lock::Axis(a) => [DVec3::X, DVec3::Y, DVec3::Z][a as usize % 3],
            prism_infer::Lock::None => DVec3::Z,
        }
    }

    /// The motion the step in progress would make, ending at `to`.
    fn cage_motion(&self, to: DVec3) -> Option<DAffine3> {
        let from = self.cage.as_ref()?.from?;
        let tiny = self.scene.tol.dist;
        match self.tool.as_str() {
            MOVE => Some(DAffine3::from_translation(to - from)),
            ROTATE => {
                let c = self.cage_middle()?;
                let n = self.cage_axis();
                let flat = |p: DVec3| {
                    let d = p - c;
                    d - n * d.dot(n)
                };
                let (a, b) = (flat(from), flat(to));
                if a.length() <= tiny || b.length() <= tiny {
                    return None;
                }
                let angle = a.cross(b).dot(n).atan2(a.dot(b));
                Some(prism_ops::transform::turn(c, n, angle))
            }
            _ => None,
        }
    }

    pub(super) fn cage_command(&self, m: DAffine3) -> Option<Command> {
        let c = self.cage.as_ref()?;
        (!c.picked.is_empty()).then(|| Command::MoveCage {
            cage: c.cage,
            points: c.picked.clone(),
            m,
        })
    }

    /// Why the step would be refused, if it would.
    fn cage_refusal(&self, m: &DAffine3) -> Option<&'static str> {
        let c = self.cage.as_ref()?;
        let local = self.to_world.inverse() * *m * self.to_world;
        cage::moved(self.work_model(), c.cage, &c.picked, &local).err()
    }

    fn cage_commit(&mut self, m: DAffine3) {
        self.cage_notice = self.cage_refusal(&m);
        if let Some(c) = &mut self.cage {
            c.from = None;
        }
        self.set_preview(None);
        self.typed.clear();
        self.lock = prism_infer::Lock::None;
        if self.cage_notice.is_none()
            && let Some(c) = self.cage_command(m)
        {
            self.run(c);
        }
        self.mem.clear();
        self.overlay_dirty = true;
        self.refresh();
    }

    pub(super) fn cage_press(&mut self, s: DVec2, shift: bool, ctrl: bool) {
        self.update_hover(s);
        self.cage_notice = None;
        let hit = self.cage_point_at(s);
        let tool = self.tool.clone();
        if tool == SELECT {
            if let Some(c) = &mut self.cage {
                c.pre = c.picked.clone();
                match (hit, shift || ctrl) {
                    (Some(k), true) => {
                        if !c.picked.remove(&k) {
                            c.picked.insert(k);
                        }
                    }
                    (Some(k), false) => c.picked = BTreeSet::from([k]),
                    (None, false) => c.picked.clear(),
                    (None, true) => {}
                }
            }
            // A drag from here is a box.
            self.boxing = Some(super::BoxOp {
                down: s,
                now: s,
                pre: self.saved.len(),
                shift,
                ctrl,
                empty: false,
                groups: self.groups.clone(),
            });
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        let Some(at) = self.inference.as_ref().map(|i| i.point) else {
            return;
        };
        let started = self.cage.as_ref().and_then(|c| c.from);
        match started {
            None => {
                let pts = self.cage_points();
                if let Some(c) = &mut self.cage {
                    // Move: a click on a control point that is not picked
                    // picks it, and moves it from where it is.
                    let mut from = at;
                    if tool == MOVE
                        && let Some(k) = hit
                    {
                        if !c.picked.contains(&k) {
                            c.picked = BTreeSet::from([k]);
                        }
                        from = pts[k];
                    }
                    if c.picked.is_empty() {
                        self.cage_notice = Some(PICK_FIRST);
                    } else {
                        c.from = Some(from);
                    }
                }
            }
            Some(_) => {
                if let Some(m) = self.cage_motion(at) {
                    self.cage_commit(m);
                    return;
                }
            }
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    /// A Select box let go while the cage is open: the control points
    /// inside it are picked (Shift toggles them, Ctrl adds them). A press
    /// that did not drag keeps what the click picked.
    pub(super) fn cage_box(&mut self, down: DVec2, now: DVec2, shift: bool, ctrl: bool) {
        if (now - down).abs().max_element() < super::BOX_MIN {
            return;
        }
        let (lo, hi) = (down.min(now), down.max(now));
        let view = self.view();
        let inside: Vec<usize> = self
            .cage_points()
            .iter()
            .enumerate()
            .filter_map(|(k, p)| {
                let (q, _) = view.to_screen(*p)?;
                (q.cmpge(lo).all() && q.cmple(hi).all()).then_some(k)
            })
            .collect();
        if let Some(c) = &mut self.cage {
            let mut picked = if shift || ctrl {
                c.pre.clone()
            } else {
                BTreeSet::new()
            };
            for k in inside {
                if shift && picked.contains(&k) {
                    picked.remove(&k);
                } else {
                    picked.insert(k);
                }
            }
            c.picked = picked;
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The cursor moved: show what the step would make.
    pub(super) fn cage_hover(&mut self) {
        if self.cage.as_ref().is_none_or(|c| c.from.is_none()) {
            return;
        }
        let Some(to) = self.inference.as_ref().map(|i| i.point) else {
            return;
        };
        let shown = self
            .cage_motion(to)
            .filter(|m| self.cage_refusal(m).is_none())
            .and_then(|m| self.cage_command(m))
            .map(|c| self.preview_of(&c));
        self.set_preview_scene(shown);
        self.overlay_dirty = true;
    }

    /// A typed distance (Move), degrees (Rotate) or factor (Scale). True
    /// if it was used.
    pub(super) fn cage_enter(&mut self, text: &str) -> bool {
        let Some(from) = self.cage.as_ref().and_then(|c| c.from) else {
            return false;
        };
        let to = self.inference.as_ref().map(|i| i.point);
        let m = match self.tool.as_str() {
            MOVE => self.number.length(text).zip(to).and_then(|(d, to)| {
                let dir = (to - from).try_normalize()?;
                Some(DAffine3::from_translation(dir * d))
            }),
            ROTATE => text
                .trim()
                .trim_end_matches('°')
                .parse::<f64>()
                .ok()
                .zip(self.cage_middle())
                .map(|(deg, c)| prism_ops::transform::turn(c, self.cage_axis(), deg.to_radians())),
            _ => None,
        };
        if let Some(m) = m {
            self.cage_commit(m);
        }
        true
    }

    /// Esc: end the step. True if there was one.
    pub(super) fn cage_cancel(&mut self) -> bool {
        let Some(c) = &mut self.cage else {
            return false;
        };
        if c.from.take().is_none() {
            return false;
        }
        self.set_preview(None);
        self.overlay_dirty = true;
        true
    }

    pub(super) fn cage_hint(&self) -> Option<&'static str> {
        if let Some(why) = self.cage_notice {
            return Some(why);
        }
        let c = self.cage.as_ref()?;
        Some(match (self.tool.as_str(), c.from.is_some()) {
            (SELECT, _) => {
                "Cage: click or drag a box round control points to pick them (Shift toggles, Ctrl adds) · then Move, Rotate or Scale · Esc leaves"
            }
            (MOVE, false) => {
                "Cage: click a control point to move it, or a start point to move the picked ones"
            }
            (MOVE, true) => "Click where it goes · type a distance + Enter · arrows lock an axis",
            (ROTATE, false) => {
                "Cage: click a start point to turn the picked points about their middle"
            }
            (ROTATE, true) => "Click how far round · type degrees + Enter · arrows pick the axis",
            (SCALE, _) if self.edit.is_some() => {
                "Click where the grip goes · type a factor + Enter · Alt: about the middle"
            }
            (SCALE, _) => {
                "Cage: click a grip of the box round the picked points to size them (corners evenly)"
            }
            _ => "Cage: pick Select, Move, Rotate or Scale · Esc leaves",
        })
    }

    /// The curvature combs of the selected freeform curves (and of the
    /// curve whose cage is open), in the world: for each, the teeth (from
    /// the curve, away from the centre of its bend, as long as it bends
    /// there) and the line joining their tips. Scaled so the longest tooth
    /// is 15 % of the curve's length. Empty unless View > Curvature Combs
    /// is on.
    pub fn curvature_combs(&self) -> Vec<Vec<(DVec3, DVec3)>> {
        if !self.view.combs {
            return Vec::new();
        }
        let m = self.work_model();
        let mut edges: Vec<prism_topo::EdgeId> = self.selection.edges.iter().copied().collect();
        if let Some(Cage::Curve(e)) = self.cage.as_ref().map(|c| c.cage) {
            edges.push(e);
        }
        let mut curves: Vec<std::sync::Arc<prism_geom::Nurbs>> = Vec::new();
        for e in edges {
            if let Some(s) = m.edge(e).and_then(|x| x.curve.spline())
                && !curves.iter().any(|c| std::sync::Arc::ptr_eq(c, &s.curve))
            {
                curves.push(s.curve.clone());
            }
        }
        let mut out = Vec::new();
        for c in curves {
            let c = c.transformed(&self.to_world);
            let (lo, hi) = c.domain();
            let n = (c.steps(lo, hi) * 2).clamp(16, 400);
            let at = |i: usize| lo + (hi - lo) * i as f64 / n as f64;
            let pts: Vec<DVec3> = (0..=n).map(|i| c.eval(at(i))).collect();
            let bends: Vec<DVec3> = (0..=n).map(|i| c.curvature(at(i))).collect();
            let length: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
            let most = bends.iter().map(|k| k.length()).fold(0.0, f64::max);
            if most * length <= 1e-9 {
                continue;
            }
            let scale = 0.15 * length / most;
            out.push(
                pts.iter()
                    .zip(&bends)
                    .map(|(p, k)| (*p, *p - *k * scale))
                    .collect(),
            );
        }
        out
    }

    pub(super) fn combs_overlay(&self, o: &mut Overlay) {
        let fb = &self.style.feedback;
        for comb in self.curvature_combs() {
            let mut line = |a: DVec3, b: DVec3| {
                o.lines.push(Line {
                    a,
                    b,
                    color: fb.tangent,
                    width: fb.guide_width,
                    dash: [0.0, 0.0],
                });
            };
            for (p, tip) in &comb {
                line(*p, *tip);
            }
            for w in comb.windows(2) {
                line(w[0].1, w[1].1);
            }
        }
    }

    /// The cage: its lines, and its points (picked ones filled).
    pub(super) fn cage_overlay(&self, o: &mut Overlay) {
        let Some(c) = &self.cage else { return };
        let fb = &self.style.feedback;
        // During a step the cage is drawn where it would go.
        let moved = match &self.edit {
            Some(super::edit::EditOp::Scale { anchor, k, .. }) => {
                Some(prism_ops::transform::scaling(*anchor, *k))
            }
            _ => c
                .from
                .zip(self.inference.as_ref().map(|i| i.point))
                .and_then(|(_, to)| self.cage_motion(to)),
        };
        let pts: Vec<DVec3> = self
            .cage_points()
            .into_iter()
            .enumerate()
            .map(|(k, p)| match moved {
                Some(m) if c.picked.contains(&k) => m.transform_point3(p),
                _ => p,
            })
            .collect();
        for (a, b) in cage::lines(self.work_model(), c.cage) {
            o.lines.push(Line {
                a: pts[a],
                b: pts[b],
                color: fb.rubber_band,
                width: fb.guide_width,
                dash: [4.0, 3.0],
            });
        }
        if self.tool == ROTATE {
            self.cage_protractor(o);
        }
        for (k, p) in pts.iter().enumerate() {
            let picked = c.picked.contains(&k);
            o.marks.push(Mark {
                at: *p,
                shape: if picked { Shape::Square } else { Shape::Circle },
                color: if picked { fb.endpoint } else { fb.rubber_band },
                size: if picked {
                    fb.glyph_size
                } else {
                    fb.glyph_size * 0.7
                },
            });
        }
    }
}

impl Session {
    /// Rotate's protractor about the picked points' middle: the turn's
    /// plane, the line to the start and to where it is now, and the angle.
    fn cage_protractor(&self, o: &mut Overlay) {
        let Some(c) = self.cage_middle() else { return };
        let fb = &self.style.feedback;
        let view = self.view();
        let Some((_, depth)) = view.to_screen(c) else {
            return;
        };
        let r = view.units_per_px(depth) * super::edit::PROTRACTOR_RADIUS;
        let n = self.cage_axis();
        let (a, b) = super::rect_basis(n);
        let pt = |t: f64| c + (a * t.cos() + b * t.sin()) * r;
        let seg = 72;
        for k in 0..seg {
            let t = |k: i32| f64::from(k) / f64::from(seg) * std::f64::consts::TAU;
            o.lines.push(Line {
                a: pt(t(k)),
                b: pt(t(k + 1)),
                color: fb.protractor,
                width: 1.2,
                dash: [0.0, 0.0],
            });
        }
        o.marks.push(Mark {
            at: c,
            shape: Shape::Circle,
            color: self.style.selected,
            size: 4.5,
        });
        let flat = |p: DVec3| {
            let d = p - c;
            (d - n * d.dot(n)).try_normalize()
        };
        let Some(s0) = self.cage_start().and_then(flat) else {
            return;
        };
        o.lines.push(Line {
            a: c,
            b: c + s0 * r,
            color: fb.rubber_band,
            width: 1.6,
            dash: [0.0, 0.0],
        });
        let to = self.inference.as_ref().map(|i| i.point);
        let Some(s1) = to.and_then(flat) else { return };
        let angle = s0.cross(s1).dot(n).atan2(s0.dot(s1));
        o.lines.push(Line {
            a: c,
            b: c + s1 * r,
            color: self.style.selected,
            width: 2.0,
            dash: [0.0, 0.0],
        });
        o.labels.push(prism_view::overlay::Label {
            at: c + s1 * r,
            offset: [12.0, -14.0],
            text: format!("{:.1}°", angle.to_degrees()),
            color: fb.pill,
        });
    }
}

/// Said when a cage step starts with nothing picked.
pub const PICK_FIRST: &str =
    "Pick control points first: click them with Select (Shift adds), or click one with Move.";
