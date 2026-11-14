//! Smart Guides in the session (brief 33 part 4, PLAN 5.1): Move and Paste
//! line a group, component or whole body up with its neighbours and snap
//! it to equal gaps (`crate::smart` does the sums). Scale lines up the side
//! being dragged and matches a neighbour's size; Rotate turns it to face
//! the same way as a neighbour, or square to it.
//!
//! - Neighbours are the shown groups and components in the open group, and
//!   its loose bodies (pieces joined by edges), as boxes in the world.
//!   Hidden groups are left out, so they cost nothing.
//! - A point the cursor snaps to (an endpoint, a midpoint, a crossing)
//!   wins: the guides step aside. Typing a distance wins too.
//! - The time budget: with a clock (`Session::measure`), each adjustment is
//!   timed; three in a row over the budget lower the level one step
//!   (spacing off, then far objects, then paused), and the status bar says
//!   so. The next move starts at the full level again.

use std::collections::{BTreeMap, BTreeSet};

use prism_math::DVec3;
use prism_topo::VertexId;
use prism_view::View;
use prism_view::overlay::{Label, Line, Mark, Overlay, Shape};

use super::Session;
use crate::smart::{self, Ask, Bounds, Guide, Heading, Index, Level, ScaleAsk, TurnAsk};

/// The pull, in points at aperture 1 (the feel test's 8 px).
const PULL_PX: f64 = 8.0;
/// The most neighbours looked at for equal spacing (PLAN 5.1).
pub const CAP: usize = 200;
/// The default time budget per adjustment, in milliseconds (PLAN 5.1).
pub const BUDGET_MS: f64 = 2.0;
/// Slow adjustments in a row before the level drops.
const SLOW_IN_A_ROW: u8 = 3;
/// Sizes of the drawing, in points.
const ARROW_PX: f64 = 8.0;
const ARROW_HALF_PX: f64 = 5.0;
const BAR_PX: f64 = 7.0;
const DOT_SIZE: f32 = 4.5;
const END_SIZE: f32 = 3.5;

/// Smart Guides during one move or paste.
#[derive(Clone, Debug)]
pub(super) struct SmartOp {
    index: Index,
    /// The moving box where the move started.
    moving: Bounds,
    level: Level,
    slow: u8,
    /// The move as adjusted, and the guides it shows.
    delta: DVec3,
    guides: Vec<Guide>,
    /// The moving box now (moved or scaled).
    now: Bounds,
    /// Move and Paste: the moving box's centre is marked.
    ring: bool,
    /// Rotate: the neighbours' headings, and the moving object's.
    headings: Vec<Heading>,
    mine: Option<DVec3>,
}

/// What a guided step is.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Move,
    Scale,
    Turn(DVec3),
}

impl Session {
    /// View > Smart Guides (Ctrl+U).
    pub fn set_smart_guides(&mut self, on: bool) {
        if self.smart_on != on {
            self.smart_on = on;
            if !on {
                self.smart = None;
                self.overlay_dirty = true;
                self.dirty = true;
            }
        }
    }

    pub fn smart_guides(&self) -> bool {
        self.smart_on
    }

    /// The time budget per adjustment, in milliseconds.
    pub fn set_smart_budget(&mut self, ms: f64) {
        self.smart_budget = ms;
    }

    /// Whether Smart Guides are working on a move (for tests and MCP).
    pub fn smart_level(&self) -> Option<Level> {
        self.smart.as_ref().map(|s| s.level)
    }

    /// The guides showing now.
    pub fn smart_shown(&self) -> &[Guide] {
        self.smart.as_ref().map_or(&[], |s| &s.guides)
    }

    /// The box of what moves: `verts` (loose, in the world) and the
    /// selected groups. None unless the loose geometry is whole bodies.
    fn smart_moving(&self, verts: &BTreeSet<VertexId>) -> Option<Bounds> {
        let model = self.work_model();
        let bodies = bodies(model, &BTreeSet::new());
        let whole = bodies
            .iter()
            .all(|b| b.iter().all(|v| verts.contains(v)) || b.iter().all(|v| !verts.contains(v)));
        if !whole {
            return None;
        }
        let mut moving = Bounds::around(verts.iter().map(|v| model.vertex(*v)));
        for g in &self.groups {
            if let Some(c) = self.group_box(*g).and_then(Bounds::around) {
                moving = Some(moving.map_or(c, |m| m.union(c)));
            }
        }
        moving
    }

    /// Start for a move of `verts` (loose, in the world) and the selected
    /// groups. Loose geometry counts only when it is whole bodies.
    pub(super) fn smart_begin_move(&mut self, verts: &BTreeSet<VertexId>) {
        self.smart_begin_step(verts, Step::Move);
    }

    /// Start for Scale (not in Edit Cage, which moves control points).
    pub(super) fn smart_begin_scale(&mut self, verts: &BTreeSet<VertexId>) {
        self.smart = None;
        if !self.cage_active() {
            self.smart_begin_step(verts, Step::Scale);
        }
    }

    /// Start for Rotate about `normal`.
    pub(super) fn smart_begin_turn(&mut self, verts: &BTreeSet<VertexId>, normal: DVec3) {
        self.smart_begin_step(verts, Step::Turn(normal));
    }

    fn smart_begin_step(&mut self, verts: &BTreeSet<VertexId>, step: Step) {
        self.smart = None;
        if !self.smart_on {
            return;
        }
        if let Some(moving) = self.smart_moving(verts) {
            let skip = self.groups.clone();
            self.smart_begin(moving, verts, &skip, step);
            if let Step::Turn(n) = step {
                let mine = self
                    .groups
                    .first()
                    .and_then(|g| self.group_heading(*g, n))
                    .or_else(|| longest_edge(self.work_model(), n, |v| verts.contains(&v)));
                match (mine, self.smart.as_mut()) {
                    (Some(m), Some(op)) => op.mine = Some(m),
                    _ => self.smart = None,
                }
            }
        }
    }

    /// Start for a paste whose box is `moving` (where it was copied).
    pub(super) fn smart_begin_paste(&mut self, moving: Bounds) {
        self.smart = None;
        if self.smart_on {
            self.smart_begin(moving, &BTreeSet::new(), &BTreeSet::new(), Step::Move);
        }
    }

    /// A group's heading for a turn about `n`: the axis of its own
    /// coordinates that lies most nearly in the plane of the turn.
    fn group_heading(&self, id: prism_doc::InstanceId, n: DVec3) -> Option<DVec3> {
        let mut path = self.ctx.clone();
        path.push(id);
        let w = self.doc.assembly().world_of(&path)?;
        let m = w.matrix3;
        [m.x_axis, m.y_axis, m.z_axis]
            .into_iter()
            .filter(|a| a.length() > 0.0)
            .min_by(|a, b| (a.normalize().dot(n).abs()).total_cmp(&b.normalize().dot(n).abs()))
    }

    fn smart_begin(
        &mut self,
        moving: Bounds,
        verts: &BTreeSet<VertexId>,
        skip: &BTreeSet<prism_doc::InstanceId>,
        step: Step,
    ) {
        let asm = self.doc.assembly();
        let mut boxes = Vec::new();
        let mut headings = Vec::new();
        if let Some(def) = asm.context_def(&self.ctx).and_then(|d| asm.def(d)) {
            for id in def.instances.keys() {
                if skip.contains(id) || !asm.is_shown(*id) {
                    continue;
                }
                if let Some(b) = self.group_box(*id).and_then(Bounds::around) {
                    boxes.push(b);
                    if let Step::Turn(n) = step
                        && let Some(dir) = self.group_heading(*id, n)
                    {
                        headings.push(Heading { dir, bounds: b });
                    }
                }
            }
        }
        let model = self.work_model();
        for b in bodies(model, verts) {
            if let Some(bb) = Bounds::around(b.iter().map(|v| model.vertex(*v))) {
                boxes.push(bb);
                if let Step::Turn(n) = step {
                    let set: BTreeSet<VertexId> = b.iter().copied().collect();
                    if let Some(dir) = longest_edge(model, n, |v| set.contains(&v)) {
                        headings.push(Heading { dir, bounds: bb });
                    }
                }
            }
        }
        self.smart = Some(SmartOp {
            index: Index::new(boxes),
            moving,
            level: Level::Full,
            slow: 0,
            delta: DVec3::ZERO,
            guides: Vec::new(),
            now: moving,
            ring: step == Step::Move,
            headings,
            mine: None,
        });
    }

    pub(super) fn smart_end(&mut self) {
        if self.smart.take().is_some() {
            self.overlay_dirty = true;
        }
    }

    /// Whether the cursor snapped to a point (which wins over the guides).
    fn smart_point_snap(&self) -> bool {
        self.inference.as_ref().is_some_and(|i| i.kind.is_point())
    }

    /// The pull in model units at `at`.
    fn smart_tol(&self, view: &View, at: DVec3) -> f64 {
        view.to_screen(at).map_or(0.0, |(_, depth)| {
            view.units_per_px(depth) * PULL_PX * self.settings.aperture
        })
    }

    /// After an adjustment that started at `t0`: three slow ones in a row
    /// lower the level, and the status bar says so.
    fn smart_timed(&mut self, t0: Option<f64>) {
        let (Some(f), Some(t0)) = (self.measure, t0) else {
            return;
        };
        let budget = self.smart_budget;
        let Some(op) = &mut self.smart else { return };
        if op.level == Level::Paused {
            return;
        }
        let mut notice = None;
        if (f() - t0) * 1e3 > budget {
            op.slow += 1;
            if op.slow >= SLOW_IN_A_ROW {
                op.slow = 0;
                op.level = op.level.lower();
                notice = op.level.notice();
            }
        } else {
            op.slow = op.slow.saturating_sub(1);
        }
        if notice.is_some() {
            self.notice = notice;
        }
    }

    /// The move `raw` adjusted by the guides.
    pub(super) fn smart_adjust(&mut self, raw: DVec3, view: &View) -> DVec3 {
        let point_snap = self.smart_point_snap();
        let axes = match self.lock {
            prism_infer::Lock::Axis(a) => [0, 1, 2].map(|q| q == usize::from(a)),
            prism_infer::Lock::None => [true; 3],
        };
        let Some(op) = &self.smart else {
            return raw;
        };
        let tol = self.smart_tol(view, op.moving.moved(raw).centre());
        let t0 = self.measure.map(|f| f());
        let (d, guides) = if point_snap {
            (raw, Vec::new())
        } else {
            smart::snap(
                &op.index,
                &Ask {
                    moving: op.moving,
                    delta: raw,
                    tol,
                    axes,
                    level: op.level,
                    cap: CAP,
                },
            )
        };
        if let Some(op) = &mut self.smart {
            op.delta = d;
            op.now = op.moving.moved(d);
            op.guides = guides;
        }
        self.smart_timed(t0);
        self.overlay_dirty = true;
        d
    }

    /// Scale: the factor `t` on the `scaled` axes about `anchor`, adjusted
    /// so the dragged side lines up or the size matches a neighbour's.
    /// `snapped`: the inference engine already put the grip level with a
    /// point, which wins.
    pub(super) fn smart_scale(
        &mut self,
        t: f64,
        anchor: DVec3,
        scaled: [bool; 3],
        snapped: bool,
        view: &View,
    ) -> f64 {
        let Some(op) = &self.smart else { return t };
        let ask = ScaleAsk {
            moving: op.moving,
            anchor,
            scaled,
            t,
            tol: 0.0,
            level: op.level,
            cap: CAP,
        };
        let k = DVec3::from_array([0, 1, 2].map(|a| if scaled[a] { t } else { 1.0 }));
        let now = Bounds::new(
            anchor + (op.moving.lo - anchor) * k,
            anchor + (op.moving.hi - anchor) * k,
        );
        let tol = self.smart_tol(view, now.centre());
        let t0 = self.measure.map(|f| f());
        let (t, guides) = if snapped || scaled == [false; 3] {
            (t, Vec::new())
        } else {
            smart::snap_scale(&op.index, &ScaleAsk { tol, ..ask })
        };
        if let Some(op) = &mut self.smart {
            let k = DVec3::from_array([0, 1, 2].map(|a| if scaled[a] { t } else { 1.0 }));
            op.now = Bounds::new(
                anchor + (op.moving.lo - anchor) * k,
                anchor + (op.moving.hi - anchor) * k,
            );
            op.guides = guides;
        }
        self.smart_timed(t0);
        self.overlay_dirty = true;
        t
    }

    /// No guides for this step (Scale with Shift on an edge grip).
    pub(super) fn smart_clear(&mut self) {
        if let Some(op) = &mut self.smart
            && !op.guides.is_empty()
        {
            op.guides.clear();
            self.overlay_dirty = true;
        }
    }

    /// Rotate: the angle `raw` (the cursor's arm `arm` from `centre`)
    /// adjusted to face the same way as a neighbour, or square to it. The
    /// protractor's soft step `soft`, if it caught the angle, wins when it
    /// is nearer.
    pub(super) fn smart_turn(
        &mut self,
        raw: f64,
        soft: Option<f64>,
        centre: DVec3,
        normal: DVec3,
        arm: DVec3,
        view: &View,
    ) -> f64 {
        let plain = soft.unwrap_or(raw);
        let Some(op) = &self.smart else { return plain };
        let Some(mine) = op.mine else { return plain };
        let len = arm.length();
        if len <= 0.0 {
            return plain;
        }
        let tol = self.smart_tol(view, centre + arm) / len;
        let t0 = self.measure.map(|f| f());
        let found = if self.smart_point_snap() {
            None
        } else {
            smart::snap_turn(
                &op.headings,
                &TurnAsk {
                    mine,
                    moving: op.moving,
                    centre,
                    normal,
                    angle: raw,
                    tol,
                    level: op.level,
                },
            )
        };
        let found = found.filter(|(a, _)| soft.is_none_or(|s| (a - raw).abs() <= (s - raw).abs()));
        let angle = found.as_ref().map_or(plain, |(a, _)| *a);
        if let Some(op) = &mut self.smart {
            op.guides = found.into_iter().map(|(_, g)| g).collect();
        }
        self.smart_timed(t0);
        self.overlay_dirty = true;
        angle
    }

    /// The centre dot, guide lines, arrows and labels.
    pub(super) fn smart_overlay(&self, o: &mut Overlay) {
        let Some(op) = &self.smart else {
            return;
        };
        let fb = &self.style.feedback;
        let now = op.now;
        let mc = now.centre();
        if op.ring {
            o.marks.push(Mark {
                at: mc,
                shape: Shape::Ring,
                color: fb.rubber_band,
                size: DOT_SIZE,
            });
        }
        for g in &op.guides {
            match *g {
                Guide::Align {
                    axis,
                    at,
                    other,
                    centre,
                    show,
                } => {
                    if !show {
                        continue;
                    }
                    let oc = other.centre();
                    // The two nearest points of the boxes at the matched
                    // value (the centres, for a centre match).
                    let p1 = DVec3::from_array([0, 1, 2].map(|i| {
                        if i == axis {
                            at
                        } else if centre {
                            oc[i]
                        } else {
                            mc[i].clamp(other.lo[i], other.hi[i])
                        }
                    }));
                    let p2 = DVec3::from_array([0, 1, 2].map(|i| {
                        if i == axis {
                            at
                        } else if centre {
                            mc[i]
                        } else {
                            oc[i].clamp(now.lo[i], now.hi[i])
                        }
                    }));
                    o.lines.push(Line {
                        a: p1,
                        b: p2,
                        color: fb.smart_align,
                        width: fb.guide_width,
                        dash: fb.guide_dash,
                    });
                    for p in [p1, p2] {
                        o.marks.push(Mark {
                            at: p,
                            shape: Shape::Circle,
                            color: fb.smart_align,
                            size: END_SIZE,
                        });
                    }
                    o.labels.push(Label {
                        at: (p1 + p2) * 0.5,
                        offset: [10.0, -14.0],
                        text: format!("Align {}", ["X", "Y", "Z"][axis]),
                        color: fb.smart_align,
                    });
                }
                Guide::Space { axis, gaps, gap } => {
                    for (s, e) in gaps {
                        let at = |v: f64| {
                            let mut p = mc;
                            p[axis] = v;
                            p
                        };
                        let text = format!("d: {}", self.number.show(gap));
                        self.smart_dimension(o, axis, at(s), at(e), text);
                    }
                }
                Guide::Size {
                    axis,
                    size,
                    mine,
                    other,
                } => {
                    // Along the bottom front edge of each box.
                    let word = ["Width", "Depth", "Height"][axis];
                    for b in [mine, other] {
                        let (mut p, mut q) = (b.lo, b.lo);
                        q[axis] = b.hi[axis];
                        p[axis] = b.lo[axis];
                        let text = format!("{word} {}", self.number.show(size));
                        self.smart_dimension(o, axis, p, q, text);
                    }
                }
                Guide::Turn {
                    other,
                    along,
                    at,
                    mine,
                    square,
                } => {
                    let half = |b: &Bounds| (b.hi - b.lo).length() * 0.5;
                    let oc = other.centre();
                    let mh = half(&now).max(half(&other) * 0.5);
                    for (c, d, h) in [(oc, along, half(&other)), (at, mine, mh)] {
                        o.lines.push(Line {
                            a: c - d * h,
                            b: c + d * h,
                            color: fb.smart_align,
                            width: fb.guide_width,
                            dash: fb.guide_dash,
                        });
                        o.marks.push(Mark {
                            at: c,
                            shape: Shape::Circle,
                            color: fb.smart_align,
                            size: END_SIZE,
                        });
                    }
                    o.labels.push(Label {
                        at: at + mine * mh,
                        offset: [10.0, -14.0],
                        text: if square { "Square" } else { "Parallel" }.to_owned(),
                        color: fb.smart_align,
                    });
                }
            }
        }
    }

    /// A measured span from `a` to `b` along `axis`: a line with an arrow
    /// and an end bar at each end, and its label.
    fn smart_dimension(&self, o: &mut Overlay, axis: usize, a: DVec3, b: DVec3, text: String) {
        let fb = &self.style.feedback;
        let view = self.view();
        let px = |p: DVec3| {
            view.to_screen(p)
                .map_or(0.0, |(_, depth)| view.units_per_px(depth))
        };
        let along = DVec3::from_array([0, 1, 2].map(|i| f64::from(u8::from(i == axis))));
        let mut side = along.cross(view.f);
        if side.length_squared() < 1e-12 {
            side = along.any_orthonormal_vector();
        }
        let side = side.normalize();
        let line = |a: DVec3, b: DVec3| Line {
            a,
            b,
            color: fb.smart_space,
            width: fb.rubber_band_width,
            dash: [0.0, 0.0],
        };
        o.lines.push(line(a, b));
        for (p, dir) in [(a, b - a), (b, a - b)] {
            let k = px(p);
            let u = dir.normalize_or_zero();
            let back = p + u * (ARROW_PX * k);
            o.lines.push(line(p, back + side * (ARROW_HALF_PX * k)));
            o.lines.push(line(p, back - side * (ARROW_HALF_PX * k)));
            o.lines
                .push(line(p - side * (BAR_PX * k), p + side * (BAR_PX * k)));
        }
        o.labels.push(Label {
            at: (a + b) * 0.5,
            offset: [0.0, -16.0],
            text,
            color: fb.smart_space,
        });
    }
}

/// The direction of the longest edge among vertices for which `keep`
/// holds, flattened onto the plane square to `n` (a loose body's heading).
fn longest_edge(
    model: &prism_topo::Model,
    n: DVec3,
    keep: impl Fn(VertexId) -> bool,
) -> Option<DVec3> {
    let n = n.normalize_or_zero();
    model
        .edges()
        .filter(|(_, e)| keep(e.a) && keep(e.b))
        .map(|(_, e)| {
            let d = model.vertex(e.b) - model.vertex(e.a);
            d - n * n.dot(d)
        })
        .filter(|d| d.length() > 0.0)
        .max_by(|a, b| a.length().total_cmp(&b.length()))
        .map(DVec3::normalize)
}

/// The loose bodies (pieces joined by edges) of `model`, leaving out any
/// that uses a vertex in `skip`.
fn bodies(model: &prism_topo::Model, skip: &BTreeSet<VertexId>) -> Vec<Vec<VertexId>> {
    let mut parent: BTreeMap<VertexId, VertexId> = model.vertices().map(|(v, _)| (v, v)).collect();
    fn find(p: &mut BTreeMap<VertexId, VertexId>, mut a: VertexId) -> VertexId {
        while p[&a] != a {
            let up = p[&p[&a]];
            p.insert(a, up);
            a = up;
        }
        a
    }
    for (_, e) in model.edges() {
        let (ra, rb) = (find(&mut parent, e.a), find(&mut parent, e.b));
        if ra != rb {
            parent.insert(ra, rb);
        }
    }
    let mut groups: BTreeMap<VertexId, Vec<VertexId>> = BTreeMap::new();
    let all: Vec<VertexId> = parent.keys().copied().collect();
    for v in all {
        let r = find(&mut parent, v);
        groups.entry(r).or_default().push(v);
    }
    groups
        .into_values()
        .filter(|b| !b.iter().any(|v| skip.contains(v)))
        .collect()
}
