//! Smart Guides in the session (brief 33 part 4, PLAN 5.1): Move and Paste
//! line a group, component or whole body up with its neighbours and snap
//! it to equal gaps (`crate::smart` does the sums).
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

use prism_infer::Kind;
use prism_math::DVec3;
use prism_topo::VertexId;
use prism_view::View;
use prism_view::overlay::{Label, Line, Mark, Overlay, Shape};

use super::Session;
use crate::smart::{self, Ask, Bounds, Guide, Index, Level};

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

    /// Start for a move of `verts` (loose, in the world) and the selected
    /// groups. Loose geometry counts only when it is whole bodies.
    pub(super) fn smart_begin_move(&mut self, verts: &BTreeSet<VertexId>) {
        self.smart = None;
        if !self.smart_on {
            return;
        }
        let model = self.work_model();
        let bodies = bodies(model, &BTreeSet::new());
        let whole = bodies
            .iter()
            .all(|b| b.iter().all(|v| verts.contains(v)) || b.iter().all(|v| !verts.contains(v)));
        if !whole {
            return;
        }
        let mut moving = Bounds::around(verts.iter().map(|v| model.vertex(*v)));
        for g in &self.groups {
            if let Some(c) = self.group_box(*g).and_then(Bounds::around) {
                moving = Some(moving.map_or(c, |m| m.union(c)));
            }
        }
        if let Some(moving) = moving {
            let skip = self.groups.clone();
            self.smart_begin(moving, verts, &skip);
        }
    }

    /// Start for a paste whose box is `moving` (where it was copied).
    pub(super) fn smart_begin_paste(&mut self, moving: Bounds) {
        self.smart = None;
        if self.smart_on {
            self.smart_begin(moving, &BTreeSet::new(), &BTreeSet::new());
        }
    }

    fn smart_begin(
        &mut self,
        moving: Bounds,
        verts: &BTreeSet<VertexId>,
        skip: &BTreeSet<prism_doc::InstanceId>,
    ) {
        let asm = self.doc.assembly();
        let mut boxes = Vec::new();
        if let Some(def) = asm.context_def(&self.ctx).and_then(|d| asm.def(d)) {
            for id in def.instances.keys() {
                if skip.contains(id) || !asm.is_shown(*id) {
                    continue;
                }
                if let Some(b) = self.group_box(*id).and_then(Bounds::around) {
                    boxes.push(b);
                }
            }
        }
        let model = self.work_model();
        for b in bodies(model, verts) {
            if let Some(b) = Bounds::around(b.iter().map(|v| model.vertex(*v))) {
                boxes.push(b);
            }
        }
        self.smart = Some(SmartOp {
            index: Index::new(boxes),
            moving,
            level: Level::Full,
            slow: 0,
            delta: DVec3::ZERO,
            guides: Vec::new(),
        });
    }

    pub(super) fn smart_end(&mut self) {
        if self.smart.take().is_some() {
            self.overlay_dirty = true;
        }
    }

    /// The move `raw` adjusted by the guides.
    pub(super) fn smart_adjust(&mut self, raw: DVec3, view: &View) -> DVec3 {
        let point_snap = self.inference.as_ref().is_some_and(|i| {
            matches!(
                i.kind,
                Kind::Endpoint
                    | Kind::Midpoint
                    | Kind::Intersection
                    | Kind::Centre
                    | Kind::Quadrant
                    | Kind::GuidePoint
                    | Kind::GuideCross
            )
        });
        let axes = match self.lock {
            prism_infer::Lock::Axis(a) => [0, 1, 2].map(|q| q == usize::from(a)),
            prism_infer::Lock::None => [true; 3],
        };
        let aperture = self.settings.aperture;
        let clock = self.measure;
        let budget = self.smart_budget;
        let Some(op) = &mut self.smart else {
            return raw;
        };
        let centre = op.moving.moved(raw).centre();
        let tol = view.to_screen(centre).map_or(0.0, |(_, depth)| {
            view.units_per_px(depth) * PULL_PX * aperture
        });
        let t0 = clock.map(|f| f());
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
        let mut notice = None;
        if let (Some(f), Some(t0)) = (clock, t0)
            && op.level != Level::Paused
        {
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
        }
        op.delta = d;
        op.guides = guides;
        if notice.is_some() {
            self.notice = notice;
        }
        self.overlay_dirty = true;
        d
    }

    /// The centre dot, guide lines, arrows and labels.
    pub(super) fn smart_overlay(&self, o: &mut Overlay) {
        let Some(op) = &self.smart else {
            return;
        };
        let fb = &self.style.feedback;
        let view = self.view();
        let now = op.moving.moved(op.delta);
        let mc = now.centre();
        o.marks.push(Mark {
            at: mc,
            shape: Shape::Ring,
            color: fb.rubber_band,
            size: DOT_SIZE,
        });
        let px = |p: DVec3| {
            view.to_screen(p)
                .map_or(0.0, |(_, depth)| view.units_per_px(depth))
        };
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
                    let along =
                        DVec3::from_array([0, 1, 2].map(|i| f64::from(u8::from(i == axis))));
                    let mut side = along.cross(view.f);
                    if side.length_squared() < 1e-12 {
                        side = along.any_orthonormal_vector();
                    }
                    let side = side.normalize();
                    for (s, e) in gaps {
                        let at = |v: f64| {
                            let mut p = mc;
                            p[axis] = v;
                            p
                        };
                        let (a, b) = (at(s), at(e));
                        let line = |a: DVec3, b: DVec3| Line {
                            a,
                            b,
                            color: fb.smart_space,
                            width: fb.rubber_band_width,
                            dash: [0.0, 0.0],
                        };
                        o.lines.push(line(a, b));
                        // An arrow and an end bar at each end, pointing out.
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
                            text: format!("d: {}", self.number.show(gap)),
                            color: fb.smart_space,
                        });
                    }
                }
            }
        }
    }
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
