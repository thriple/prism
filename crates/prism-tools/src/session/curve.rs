//! The Curve tool (Freeform NURBS part 1): a NURBS curve from clicked
//! points. Each click is a control point (the curve is pulled towards it);
//! Ctrl switches to a curve that passes through the clicks. A double-click
//! or Enter ends the curve; a click on its start closes it. Every point
//! comes from the inference engine, so a curve can run in 3D.

use prism_doc::Command;
use prism_geom::Nurbs;
use prism_math::{DVec2, DVec3};
use prism_view::overlay::{Line, Overlay};

use super::Session;

pub const CURVE: &str = "tool.curve";

/// A curve being drawn.
#[derive(Clone, Debug)]
pub(super) struct CurveOp {
    /// The clicks so far.
    pub pts: Vec<DVec3>,
}

/// The curve through or shaped by `pts`.
pub fn curve_for(pts: &[DVec3], closed: bool, through: bool) -> Option<Nurbs> {
    match (through, closed) {
        (true, _) => Nurbs::through(pts, closed),
        (false, true) => Nurbs::closed(pts, 3),
        (false, false) => Nurbs::open(pts, 3),
    }
}

impl Session {
    /// Whether the Curve tool passes through its clicks (Ctrl switches).
    pub fn curve_through(&self) -> bool {
        self.curve_through
    }

    pub(super) fn curve_press(&mut self, s: DVec2, n_clicks: u32) {
        self.update_hover(s);
        let Some(i) = self.inference.clone() else {
            return;
        };
        let tiny = self.scene.tol.dist;
        let Some(mut op) = self.curve.take() else {
            self.last_op = None;
            self.curve = Some(CurveOp { pts: vec![i.point] });
            self.overlay_dirty = true;
            self.refresh();
            return;
        };
        let p = i.point;
        let last = *op.pts.last().unwrap_or(&p);
        if op.pts.len() >= 3 && p.distance(op.pts[0]) <= tiny {
            self.commit_curve(&op.pts, true);
            return;
        }
        if p.distance(last) > tiny {
            op.pts.push(p);
        }
        if n_clicks >= 2 {
            self.commit_curve(&op.pts, false);
            return;
        }
        self.curve = Some(op);
        self.overlay_dirty = true;
        self.refresh();
    }

    fn commit_curve(&mut self, pts: &[DVec3], closed: bool) {
        self.curve = None;
        self.typed.clear();
        self.lock = prism_infer::Lock::None;
        self.overlay_dirty = true;
        let c = match pts {
            [] | [_] => None,
            [p, q] => Some(Command::Segment { p: *p, q: *q }),
            _ => curve_for(pts, closed, self.curve_through).map(|curve| Command::Spline { curve }),
        };
        if let Some(c) = c {
            self.run(c);
        }
        self.mem.clear();
        self.refresh();
    }

    /// Enter ends the curve where it is. True if it was used.
    pub(super) fn curve_enter(&mut self) -> bool {
        if self.tool != CURVE {
            return false;
        }
        if let Some(op) = self.curve.take() {
            self.commit_curve(&op.pts, false);
        }
        true
    }

    pub(super) fn curve_hint(&self) -> Option<&'static str> {
        if self.tool != CURVE {
            return None;
        }
        let n = self.curve.as_ref().map_or(0, |op| op.pts.len());
        Some(match (n, self.curve_through) {
            (0, false) => "Click the start of the curve · Ctrl: a curve through the clicks",
            (0, true) => "Click the start of the curve (through the clicks) · Ctrl: control points",
            (1 | 2, _) => "Click the next point · double-click or Enter ends the curve",
            _ => "Click the next point · double-click or Enter ends · click the start to close",
        })
    }

    /// The curve in progress: its control polygon dashed, the curve solid.
    pub(super) fn curve_overlay(&self, o: &mut Overlay) {
        let Some(op) = &self.curve else { return };
        let fb = &self.style.feedback;
        let mut pts = op.pts.clone();
        if let Some(i) = &self.inference
            && pts
                .last()
                .is_none_or(|l| l.distance(i.point) > self.scene.tol.dist)
        {
            pts.push(i.point);
        }
        let closing = pts.len() >= 4 && pts[0].distance(pts[pts.len() - 1]) <= self.scene.tol.dist;
        if closing {
            pts.pop();
        }
        let mut line = |a: DVec3, b: DVec3, dash: [f32; 2]| {
            o.lines.push(Line {
                a,
                b,
                color: fb.rubber_band,
                width: fb.rubber_band_width,
                dash,
            });
        };
        if !self.curve_through || pts.len() < 3 {
            for w in pts.windows(2) {
                line(w[0], w[1], [4.0, 3.0]);
            }
        }
        if pts.len() < 3 {
            return;
        }
        let Some(c) = curve_for(&pts, closing, self.curve_through) else {
            return;
        };
        let (t0, t1) = c.domain();
        let n = c.steps(t0, t1);
        let at = |k: usize| c.eval(t0 + (t1 - t0) * k as f64 / n as f64);
        for k in 0..n {
            line(at(k), at(k + 1), [0.0, 0.0]);
        }
    }
}
