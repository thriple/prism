//! The Rotated Rectangle and Freehand tools (Everyday Commands part 3,
//! brief 33).
//!
//! - **Rotated Rectangle:** click one corner, click the next (that side can
//!   run any way, in 3D, with inference), then click the width. The width
//!   is measured square to the first side, on the plane the first click
//!   started on, unless it snaps to a point (then the rectangle tilts to
//!   reach it) or an arrow picks the plane. Typed: the side's length, then
//!   the width, or `width,angle` (the tilt in degrees from that plane).
//! - **Freehand:** press and drag. The stroke stays flat on the face, the
//!   ground or the plane it started on, and is fitted to a smooth NURBS
//!   curve on release; ending near the start closes it (and a closed
//!   stroke on a face makes a face).

use prism_doc::Command;
use prism_geom::Nurbs;
use prism_infer::{AXES, Kind, Lock};
use prism_math::{DQuat, DVec2, DVec3};
use prism_view::View;
use prism_view::overlay::{Label, Line, Overlay};

use super::{Session, plane_point};

pub const RRECT: &str = "tool.rrect";
pub const FREEHAND: &str = "tool.freehand";

/// Freehand keeps a point at most every this many pixels of drag.
const FREEHAND_STEP_PX: f64 = 3.0;
/// The fitted curve stays within this many pixels of the stroke.
const FREEHAND_FIT_PX: f64 = 1.5;
/// A stroke that ends within this many pixels of its start closes.
const FREEHAND_CLOSE_PX: f64 = 10.0;
/// The most points the fitted curve passes through.
const FREEHAND_MAX_POINTS: usize = 200;

/// A rotated rectangle being drawn: its first corner, then its first side.
#[derive(Clone, Copy, Debug)]
pub(super) struct RRect {
    pub a: DVec3,
    /// The second corner, once clicked.
    pub b: Option<DVec3>,
    /// The plane the first click started on.
    pub normal: DVec3,
    /// The cursor's point: the second corner, then the width.
    pub at: DVec3,
}

/// A freehand stroke in progress: its points, on the plane through the
/// first with this normal, and the size of a pixel there.
#[derive(Clone, Debug)]
pub(super) struct Freehand {
    pub pts: Vec<DVec3>,
    pub normal: DVec3,
    pub px: f64,
    /// Started with Shift: the stroke follows what is under the cursor in
    /// 3D (faces, edges, points) instead of staying on one plane.
    pub free: bool,
}

/// The rectangle's four corners from its first side `a`–`b` and a point
/// `p` on the width side (measured square to the side).
pub fn rrect_corners(a: DVec3, b: DVec3, p: DVec3) -> [DVec3; 4] {
    let s = (b - a).normalize_or_zero();
    let d = p - a;
    let w = d - s * d.dot(s);
    [a, b, b + w, a + w]
}

/// The plane square to the first side that the width is measured on, with
/// `n` the plane's normal before squaring (falls back to `alt` when the
/// side runs along `n`).
fn width_normal(side: DVec3, n: DVec3, alt: DVec3) -> DVec3 {
    let s = side.normalize_or_zero();
    for c in [n, alt, DVec3::Z, DVec3::X] {
        let q = c - s * c.dot(s);
        if q.length() > 1e-6 {
            return q.normalize();
        }
    }
    DVec3::Z
}

/// Fewer points along a stroke, keeping every one that strays more than
/// `tol` from the chords between those kept (Douglas–Peucker).
pub fn simplify(pts: &[DVec3], tol: f64) -> Vec<DVec3> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((i, j)) = stack.pop() {
        let (a, b) = (pts[i], pts[j]);
        let ab = b - a;
        let len2 = ab.length_squared();
        let mut far = (0.0, i);
        for (k, p) in pts.iter().enumerate().take(j).skip(i + 1) {
            let d = if len2 > 0.0 {
                let t = ((*p - a).dot(ab) / len2).clamp(0.0, 1.0);
                p.distance(a + ab * t)
            } else {
                p.distance(a)
            };
            if d > far.0 {
                far = (d, k);
            }
        }
        if far.0 > tol {
            keep[far.1] = true;
            stack.push((i, far.1));
            stack.push((far.1, j));
        }
    }
    pts.iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

/// The smooth curve fitted to a stroke (`tol`: how close, in world units),
/// closed when asked. None for a stroke too short to be a curve.
pub fn freehand_curve(pts: &[DVec3], tol: f64, closed: bool) -> Option<Nurbs> {
    let mut tol = tol;
    let mut kept = simplify(pts, tol);
    while kept.len() > FREEHAND_MAX_POINTS {
        tol *= 2.0;
        kept = simplify(pts, tol);
    }
    if closed {
        kept.pop();
        if kept.len() < 3 {
            return None;
        }
    } else if kept.len() < 2 {
        return None;
    }
    Nurbs::through(&kept, closed)
}

impl Session {
    // ---------- Rotated Rectangle ----------

    pub(super) fn rrect_press(&mut self, s: DVec2) {
        self.update_hover(s);
        let Some(i) = self.inference.clone() else {
            return;
        };
        let tiny = self.scene.tol.dist.max(i.point.length() * 1e-12);
        match self.rrect {
            None => {
                let normal = self.plane_normal(&i);
                self.rrect = Some(RRect {
                    a: i.point,
                    b: None,
                    normal,
                    at: i.point,
                });
                self.last_op = None;
            }
            Some(mut r) => match r.b {
                None if i.point.distance(r.a) > tiny => {
                    r.b = Some(i.point);
                    r.at = i.point;
                    // The arrows now pick the width's plane.
                    self.lock = Lock::None;
                    self.rrect = Some(r);
                }
                None => {}
                Some(b) => self.commit_rrect(r.a, b, r.at),
            },
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    /// The cursor moved: the second corner, then the width, follow it.
    pub(super) fn rrect_hover(&mut self, s: DVec2, view: &View) {
        let (Some(r), Some(i)) = (self.rrect, &self.inference) else {
            return;
        };
        let at = match r.b {
            None => i.point,
            Some(b) => {
                let snapped = !matches!(i.kind, Kind::OnFace | Kind::OnGround | Kind::Free);
                let n = self.rrect_plane(r.a, b, r.normal, view);
                if snapped && self.lock == Lock::None {
                    i.point
                } else {
                    plane_point(&self.scene, view, s, i, r.a, n)
                }
            }
        };
        if let Some(r) = &mut self.rrect {
            r.at = at;
        }
    }

    /// The plane the width is drawn on: the arrow's axis, else the plane
    /// the first click started on, each turned square to the first side.
    fn rrect_plane(&self, a: DVec3, b: DVec3, normal: DVec3, view: &View) -> DVec3 {
        let n = match self.lock {
            Lock::Axis(k) => AXES[k as usize],
            Lock::None => normal,
        };
        width_normal(b - a, n, view.f)
    }

    fn commit_rrect(&mut self, a: DVec3, b: DVec3, p: DVec3) {
        self.rrect = None;
        self.typed.clear();
        self.lock = Lock::None;
        self.overlay_dirty = true;
        let c = rrect_corners(a, b, p);
        let tiny = self.scene.tol.dist.max(a.length() * 1e-12);
        if c[0].distance(c[1]) <= tiny || c[1].distance(c[2]) <= tiny {
            return;
        }
        self.run(Command::Loop { points: c.to_vec() });
        self.mem.clear();
    }

    /// Typed: the first side's length, then `width` or `width,angle`.
    /// True if it was used.
    pub(super) fn rrect_enter(&mut self, text: &str) -> bool {
        if self.tool != RRECT {
            return false;
        }
        let Some(r) = self.rrect else {
            return true;
        };
        let t = text.trim();
        match r.b {
            None => {
                let Some(len) = self.number.length(t).filter(|l| *l != 0.0) else {
                    return true;
                };
                let mut dir = (r.at - r.a).normalize_or_zero();
                if let Lock::Axis(k) = self.lock {
                    let ax = AXES[k as usize];
                    dir = if dir.dot(ax) < 0.0 { -ax } else { ax };
                }
                if dir == DVec3::ZERO {
                    dir = super::rect_basis(r.normal).0;
                }
                let b = r.a + dir * len;
                self.rrect = Some(RRect {
                    b: Some(b),
                    at: b,
                    ..r
                });
                self.lock = Lock::None;
            }
            Some(b) => {
                let mut parts = t.split(|c| self.number.is_separator(c));
                let Some(w) = parts.next().and_then(|p| self.number.length(p)) else {
                    return true;
                };
                let angle = match parts.next() {
                    None => None,
                    Some(p) => match p.trim().trim_end_matches('°').parse::<f64>() {
                        Ok(d) => Some(d.to_radians()),
                        Err(_) => return true,
                    },
                };
                let side = (b - r.a).normalize_or_zero();
                let n = self.rrect_plane(r.a, b, r.normal, &self.view());
                // Square to the side on the plane, towards the cursor.
                let mut across = n.cross(side).normalize_or_zero();
                if (r.at - r.a).dot(across) < 0.0 {
                    across = -across;
                }
                if let Some(t) = angle {
                    across = DQuat::from_axis_angle(side, t) * across;
                }
                self.commit_rrect(r.a, b, r.a + across * w);
            }
        }
        self.overlay_dirty = true;
        self.refresh();
        true
    }

    pub(super) fn rrect_measurement(&self) -> Option<(&'static str, String)> {
        if self.tool != RRECT {
            return None;
        }
        let r = self.rrect?;
        Some(match r.b {
            None => ("Length", self.number.show(r.a.distance(r.at))),
            Some(b) => {
                let c = rrect_corners(r.a, b, r.at);
                ("Width", self.number.show(c[1].distance(c[2])))
            }
        })
    }

    pub(super) fn rrect_hint(&self) -> Option<&'static str> {
        if self.tool != RRECT {
            return None;
        }
        Some(match self.rrect.map(|r| r.b.is_some()) {
            None => "Click the first corner. Any plane: the next click sets the first side.",
            Some(false) => {
                "Click the second corner · type a length + Enter · arrows lock an axis · Esc cancels"
            }
            Some(true) => {
                "Click the width · type a width, or width,angle + Enter · arrows pick the plane · Esc cancels"
            }
        })
    }

    pub(super) fn rrect_overlay(&self, o: &mut Overlay) {
        let Some(r) = self.rrect else { return };
        let fb = &self.style.feedback;
        let mut line = |a: DVec3, b: DVec3| {
            o.lines.push(Line {
                a,
                b,
                color: fb.rubber_band,
                width: fb.rubber_band_width,
                dash: [0.0, 0.0],
            });
        };
        match r.b {
            None => line(r.a, r.at),
            Some(b) => {
                let c = rrect_corners(r.a, b, r.at);
                for k in 0..4 {
                    line(c[k], c[(k + 1) % 4]);
                }
                o.labels.push(Label {
                    at: c[1].lerp(c[2], 0.5),
                    offset: [10.0, -14.0],
                    text: self.number.show(c[1].distance(c[2])),
                    color: fb.pill,
                });
            }
        }
    }

    // ---------- Freehand ----------

    pub(super) fn freehand_press(&mut self, s: DVec2, shift: bool) {
        self.update_hover(s);
        let Some(i) = self.inference.clone() else {
            return;
        };
        let view = self.view();
        let normal = self.plane_normal(&i);
        let depth = (i.point - view.eye).dot(view.f).abs();
        self.freehand = Some(Freehand {
            pts: vec![i.point],
            normal,
            px: view.units_per_px(depth).max(f64::MIN_POSITIVE),
            free: shift,
        });
        self.last_op = None;
        self.overlay_dirty = true;
    }

    /// The drag adds points on the stroke's plane. True if a stroke took it.
    pub(super) fn freehand_drag(&mut self, s: DVec2) -> bool {
        let Some(f) = &self.freehand else {
            return false;
        };
        let p = if f.free {
            // In 3D: the point the inference engine finds under the cursor.
            self.update_hover(s);
            let Some(i) = &self.inference else {
                return true;
            };
            i.point
        } else {
            let view = self.view();
            let (o, d) = view.ray(s);
            let start = f.pts[0];
            let den = f.normal.dot(d);
            if den.abs() < 1e-6 {
                return true;
            }
            let t = f.normal.dot(start - o) / den;
            if t <= 0.0 {
                return true;
            }
            o + d * t
        };
        let f = self.freehand.as_mut().expect("stroke");
        if f.pts
            .last()
            .is_none_or(|q| q.distance(p) >= f.px * FREEHAND_STEP_PX)
        {
            f.pts.push(p);
            self.overlay_dirty = true;
            self.dirty = true;
        }
        true
    }

    /// The stroke ends: fit it and add the curve.
    pub(super) fn freehand_release(&mut self) -> bool {
        let Some(f) = self.freehand.take() else {
            return false;
        };
        self.overlay_dirty = true;
        self.dirty = true;
        let n = f.pts.len();
        let closed = n >= 4 && f.pts[0].distance(f.pts[n - 1]) <= f.px * FREEHAND_CLOSE_PX && {
            // Not a dab: it went somewhere before coming back.
            let far = f
                .pts
                .iter()
                .map(|p| p.distance(f.pts[0]))
                .fold(0.0, f64::max);
            far > f.px * FREEHAND_CLOSE_PX * 2.0
        };
        let mut pts = f.pts;
        if closed {
            // End exactly where it began.
            let first = pts[0];
            *pts.last_mut().expect("points") = first;
        }
        let tol = f.px * FREEHAND_FIT_PX;
        let c = match freehand_curve(&pts, tol, closed) {
            Some(c) if c.points().len() == 2 && !closed => Command::Segment {
                p: pts[0],
                q: pts[pts.len() - 1],
            },
            Some(curve) => Command::Spline { curve },
            None => return true,
        };
        self.run(c);
        self.mem.clear();
        self.refresh();
        true
    }

    pub(super) fn freehand_hint(&self) -> Option<&'static str> {
        (self.tool == FREEHAND).then_some(if self.freehand.is_some() {
            "Drag to draw · release to finish · end at the start to close · Esc cancels"
        } else {
            "Press and drag to draw a smooth curve on the face or plane under the cursor · hold Shift as you press to draw over anything in 3D"
        })
    }

    pub(super) fn freehand_overlay(&self, o: &mut Overlay) {
        let Some(f) = &self.freehand else { return };
        let fb = &self.style.feedback;
        for w in f.pts.windows(2) {
            o.lines.push(Line {
                a: w[0],
                b: w[1],
                color: fb.rubber_band,
                width: fb.rubber_band_width,
                dash: [0.0, 0.0],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_are_square_to_the_first_side() {
        let c = rrect_corners(
            DVec3::ZERO,
            DVec3::new(2.0, 0.0, 0.0),
            DVec3::new(0.5, 1.0, 1.0),
        );
        assert_eq!(c[2], DVec3::new(2.0, 1.0, 1.0));
        assert_eq!(c[3], DVec3::new(0.0, 1.0, 1.0));
        assert!((c[1] - c[0]).dot(c[2] - c[1]).abs() < 1e-12);
    }

    #[test]
    fn simplify_keeps_corners_and_drops_straight_runs() {
        let mut pts: Vec<DVec3> = (0..=10)
            .map(|k| DVec3::new(f64::from(k), 0.0, 0.0))
            .collect();
        pts.extend((1..=10).map(|k| DVec3::new(10.0, f64::from(k), 0.0)));
        let s = simplify(&pts, 0.01);
        assert_eq!(
            s,
            vec![
                pts[0],
                DVec3::new(10.0, 0.0, 0.0),
                DVec3::new(10.0, 10.0, 0.0)
            ]
        );
    }

    #[test]
    fn a_stroke_fits_a_curve_close_to_it() {
        let pts: Vec<DVec3> = (0..=100)
            .map(|k| {
                let t = f64::from(k) / 100.0 * std::f64::consts::PI;
                DVec3::new(t.cos(), t.sin(), 0.0)
            })
            .collect();
        let c = freehand_curve(&pts, 0.002, false).expect("curve");
        let (t0, t1) = c.domain();
        assert!(c.eval(t0).distance(pts[0]) < 1e-9);
        assert!(c.eval(t1).distance(pts[100]) < 1e-9);
        for k in 0..=50 {
            let p = c.eval(t0 + (t1 - t0) * f64::from(k) / 50.0);
            assert!((p.length() - 1.0).abs() < 0.01, "{p}");
            assert!(p.z.abs() < 1e-12);
        }
        assert!(c.points().len() < 40, "{}", c.points().len());
    }
}
