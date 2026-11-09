//! The Arc tools (brief 24, step 3a): 2-point, 3-point, centre and pie.
//! The plane comes from the first click, as for Rectangle and Circle; later
//! clicks and the cursor are put on it.

use std::f64::consts::{PI, TAU};

use prism_doc::Command;
use prism_geom::{ArcSpec, Curve, arc_bulge, arc_through, bulge_for_radius};
use prism_math::{DVec2, DVec3};
use prism_view::View;
use prism_view::overlay::{Label, Line, Overlay};

use super::{Session, plane_point};

/// The 2-point arc; `tool.arc` (A) picks it too.
pub const ARC: &str = "tool.arc";
pub const ARC2: &str = "arc.2";
pub const ARC3: &str = "arc.3";
pub const ARC_CENTRE: &str = "arc.c";
pub const PIE: &str = "arc.p";

pub(super) fn is_arc(tool: &str) -> bool {
    matches!(tool, ARC | ARC2 | ARC3 | ARC_CENTRE | PIE)
}

fn by_centre(tool: &str) -> bool {
    tool == ARC_CENTRE || tool == PIE
}

/// An arc being drawn.
#[derive(Clone, Debug)]
pub(super) struct ArcOp {
    pub normal: DVec3,
    /// The clicks so far, on the plane.
    pub pts: Vec<DVec3>,
    /// The cursor on the plane.
    pub at: DVec3,
    /// A centre arc's sweep so far (signed: it follows the cursor round).
    pub sweep: f64,
    /// The 2-point arc's bulge is snapped to a half circle.
    pub half: bool,
    /// The 2-point arc's bulge is snapped tangent to an edge at its start.
    pub tangent: Option<f64>,
}

impl Session {
    pub(super) fn arc_press(&mut self, s: DVec2, n_clicks: u32) {
        self.update_hover(s);
        self.arc_notice = None;
        let Some(i) = self.inference.clone() else {
            return;
        };
        // A double-click on a corner rounds it.
        if n_clicks == 2
            && let Some(op) = &self.arc
            && op.pts.len() == 1
            && let Some(v) = prism_ops::vertex_at(self.work_model(), op.pts[0])
        {
            let corner = self.work_model().vertex(v);
            self.arc = None;
            self.fillet(corner, None);
            return;
        }
        let op = match self.arc.take() {
            None => {
                self.last_op = None;
                self.last_fillet = None;
                ArcOp {
                    normal: self.plane_normal(&i),
                    pts: vec![i.point],
                    at: i.point,
                    sweep: 0.0,
                    half: false,
                    tangent: None,
                }
            }
            Some(mut op) => {
                let tiny = self.scene.tol.dist;
                if op.pts.iter().any(|p| p.distance(op.at) <= tiny) {
                    self.arc = Some(op);
                    return;
                }
                op.pts.push(op.at);
                if op.pts.len() == 3 {
                    if let Some(c) = self.arc_command(&op) {
                        self.commit_arc(c);
                    }
                    self.arc = None;
                    self.refresh();
                    return;
                }
                op
            }
        };
        self.arc = Some(op);
        self.overlay_dirty = true;
        self.refresh();
    }

    /// The cursor moved: put it on the plane, and follow a centre arc round.
    pub(super) fn arc_hover(&mut self, s: DVec2, view: &View) {
        let (Some(op), Some(i)) = (&mut self.arc, &self.inference) else {
            return;
        };
        op.at = plane_point(&self.scene, view, s, i, op.pts[0], op.normal);
        if by_centre(&self.tool) && op.pts.len() == 2 {
            let (c, a) = (op.pts[0], op.pts[1]);
            let u = (a - c).normalize_or_zero();
            let v = op.normal.cross(u);
            let w = op.at - c;
            let raw = w.dot(v).atan2(w.dot(u));
            // The turn nearest the last one, so it can pass half a turn.
            let k = ((op.sweep - raw) / TAU).round();
            op.sweep = (raw + k * TAU).clamp(-TAU, TAU);
        }
        op.half = false;
        op.tangent = None;
        if matches!(self.tool.as_str(), ARC | ARC2) && op.pts.len() == 2 {
            let (a, b) = (op.pts[0], op.pts[1]);
            let h = bulge(op);
            let near = view
                .to_screen(op.at)
                .map_or(0.0, |(_, d)| view.units_per_px(d))
                * 8.0;
            // Tangent to an edge at the start (cyan) wins over a half circle
            // (magenta), within a few pixels.
            let tol = self.scene.tol.dist;
            op.tangent = self
                .scene
                .edges
                .iter()
                .filter_map(|e| {
                    let out = if e.b.distance(a) <= tol {
                        e.curve.end_tangent(e.a, e.b)
                    } else if e.a.distance(a) <= tol {
                        -e.curve.start_tangent(e.a, e.b)
                    } else {
                        return None;
                    };
                    tangent_bulge(a, b, op.normal, out)
                })
                .filter(|t| (t - h).abs() <= near)
                .min_by(|x, y| (x - h).abs().total_cmp(&(y - h).abs()));
            if op.tangent.is_none() && (h.abs() - a.distance(b) / 2.0).abs() <= near {
                op.half = true;
            }
        }
    }

    /// The arc the clicks so far and the cursor make, as a command.
    fn arc_command(&self, op: &ArcOp) -> Option<Command> {
        let eps = self.scene.tol.dist;
        let spec = |s: ArcSpec, start: DVec3| Command::Arc {
            centre: s.centre,
            normal: s.normal,
            radius: s.radius,
            start,
            sweep: s.sweep,
            pie: false,
        };
        let third = op.pts.get(2).copied().unwrap_or(op.at);
        match self.tool.as_str() {
            ARC | ARC2 => {
                let (a, b) = (*op.pts.first()?, *op.pts.get(1)?);
                let h = match (op.tangent, op.half) {
                    (Some(t), _) => t,
                    (None, true) => bulge(op).signum() * a.distance(b) / 2.0,
                    (None, false) => bulge(op),
                };
                Some(spec(arc_bulge(a, b, op.normal, h, eps)?, a))
            }
            ARC3 => {
                let (a, m) = (*op.pts.first()?, *op.pts.get(1)?);
                Some(spec(arc_through(a, m, third, eps)?, a))
            }
            _ => {
                let (c, a) = (*op.pts.first()?, *op.pts.get(1)?);
                let radius = c.distance(a);
                if radius <= eps || op.sweep.abs() * radius <= eps {
                    return None;
                }
                // Clockwise about the normal is counter-clockwise about its
                // opposite.
                let normal = op.normal * op.sweep.signum();
                Some(Command::Arc {
                    centre: c,
                    normal,
                    radius,
                    start: a,
                    sweep: op.sweep.abs(),
                    pie: self.tool == PIE,
                })
            }
        }
    }

    /// Round `corner` with `radius`, else the last typed radius, else a
    /// tenth of the shorter edge there; say why if it cannot be done.
    fn fillet(&mut self, corner: DVec3, radius: Option<f64>) {
        let m = self.work_model();
        let Some(r) = radius
            .or(self.arc_radius)
            .or_else(|| prism_ops::fillet::default_radius(m, corner))
        else {
            self.arc_notice = Some(prism_ops::fillet::NOT_A_CORNER);
            return;
        };
        let c = Command::Fillet { corner, radius: r };
        if let Some(why) = c.refusal(self.work_model()) {
            self.arc_notice = Some(why);
            self.last_fillet = None;
        } else if self.run(c) {
            self.last_fillet = Some(corner);
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    fn commit_arc(&mut self, c: Command) {
        self.typed.clear();
        self.lock = prism_infer::Lock::None;
        self.overlay_dirty = true;
        self.run(c);
        self.mem.clear();
    }

    /// Typed text: the 2-point arc's bulge (or radius as `2r`) after its
    /// second click; a centre arc's radius after the first, its angle in
    /// degrees after the second. True if it was used.
    pub(super) fn arc_enter(&mut self, text: &str) -> bool {
        if !is_arc(&self.tool) {
            return false;
        }
        let t = text.trim();
        let nf = self.number;
        let radius = |t: &str| nf.length(t.strip_suffix(['r', 'R']).unwrap_or(t).trim());
        let Some(mut op) = self.arc.clone() else {
            // A radius for the next fillet; right after one, it changes it.
            if let Some(r) = radius(t).map(f64::abs).filter(|r| *r > 0.0) {
                self.arc_radius = Some(r);
                if let Some(corner) = self.last_fillet {
                    if self.doc.undo() {
                        self.model_changed();
                    }
                    self.fillet(corner, Some(r));
                }
            }
            return true;
        };
        match (self.tool.as_str(), op.pts.len()) {
            (ARC | ARC2, 2) => {
                let (a, b) = (op.pts[0], op.pts[1]);
                let side = if bulge(&op) < 0.0 { -1.0 } else { 1.0 };
                let h = match t.strip_suffix(['r', 'R']) {
                    Some(r) => nf.length(r.trim()).and_then(|r| {
                        self.arc_radius = Some(r.abs());
                        bulge_for_radius(a.distance(b), r.abs())
                    }),
                    None => nf.length(t).map(f64::abs),
                };
                let Some(h) = h else { return true };
                let left = op.normal.cross(b - a).normalize_or_zero();
                op.at = a.lerp(b, 0.5) + left * h * side;
                op.half = false;
                op.tangent = None;
                op.pts.push(op.at);
            }
            (ARC_CENTRE | PIE, 1) => {
                let Some(r) = nf.length(t).map(f64::abs).filter(|r| *r > 0.0) else {
                    return true;
                };
                let c = op.pts[0];
                let d = (op.at - c).normalize_or_zero();
                let d = if d == DVec3::ZERO {
                    prism_math::Plane::new(c, op.normal).frame().u
                } else {
                    d
                };
                op.pts.push(c + d * r);
                self.arc = Some(op);
                self.overlay_dirty = true;
                self.refresh();
                return true;
            }
            (ARC_CENTRE | PIE, 2) => {
                let Ok(deg) = t.trim_end_matches('°').trim().parse::<f64>() else {
                    return true;
                };
                // The way the cursor went round.
                let side = if op.sweep < 0.0 { -1.0 } else { 1.0 };
                op.sweep = (deg.to_radians() * side).clamp(-TAU, TAU);
                op.pts.push(op.at);
            }
            _ => return true,
        }
        if let Some(c) = self.arc_command(&op) {
            self.commit_arc(c);
        }
        self.arc = None;
        self.refresh();
        true
    }

    pub(super) fn arc_hint(&self) -> Option<&'static str> {
        if let Some(why) = self.arc_notice.filter(|_| is_arc(&self.tool)) {
            return Some(why);
        }
        let n = self.arc.as_ref().map_or(0, |op| op.pts.len());
        Some(match (self.tool.as_str(), n) {
            (ARC | ARC2, 0) => {
                "Click the start of the arc · double-click a corner to round it (type a radius as 2r)"
            }
            (ARC | ARC2, 1) => "Click the end of the arc",
            (ARC | ARC2, _) => {
                "Move to bulge the arc (magenta: a half circle) · click to finish · type a bulge, or a radius as 2r, + Enter"
            }
            (ARC3, 0) => "Click the start of the arc",
            (ARC3, 1) => "Click a point the arc passes through",
            (ARC3, _) => "Click the end of the arc",
            (ARC_CENTRE | PIE, 0) => "Click the centre",
            (ARC_CENTRE | PIE, 1) => "Click the start (it sets the radius) · type a radius + Enter",
            (ARC_CENTRE | PIE, _) => {
                "Move round to the end · click to finish · type an angle in degrees + Enter"
            }
            _ => return None,
        })
    }

    pub(super) fn arc_measurement(&self) -> Option<(&'static str, String)> {
        if !is_arc(&self.tool) {
            return None;
        }
        let nf = &self.number;
        let Some(op) = &self.arc else {
            return Some(("", String::new()));
        };
        Some(match (self.tool.as_str(), op.pts.len()) {
            (ARC | ARC2 | ARC3, 1) => ("Length", nf.show(op.pts[0].distance(op.at))),
            (ARC | ARC2, _) => ("Bulge", nf.show(bulge(op).abs())),
            (ARC_CENTRE | PIE, 1) => ("Radius", nf.show(op.pts[0].distance(op.at))),
            (ARC_CENTRE | PIE, _) => ("Angle", format!("{:.1}°", op.sweep.abs().to_degrees())),
            _ => ("", String::new()),
        })
    }

    /// The arc in progress, drawn along its curve.
    pub(super) fn arc_overlay(&self, o: &mut Overlay) {
        let Some(op) = &self.arc else { return };
        let fb = &self.style.feedback;
        let color = if op.tangent.is_some() {
            fb.tangent
        } else if op.half {
            fb.half_circle
        } else {
            fb.rubber_band
        };
        let line = |o: &mut Overlay, a: DVec3, b: DVec3, dash: [f32; 2]| {
            o.lines.push(Line {
                a,
                b,
                color,
                width: fb.rubber_band_width,
                dash,
            });
        };
        let Some(Command::Arc {
            centre,
            normal,
            radius,
            start,
            sweep,
            pie,
        }) = self.arc_command(op)
        else {
            // Only the first leg so far.
            line(o, *op.pts.last().unwrap_or(&op.at), op.at, [4.0, 3.0]);
            return;
        };
        let curve = Curve::Arc {
            centre,
            axis: normal,
            radius,
        };
        let u = (start - centre).normalize_or_zero();
        let v = normal.cross(u);
        let end = centre + (u * sweep.cos() + v * sweep.sin()) * radius;
        let pts: Vec<DVec3> = std::iter::once(start)
            .chain(curve.inner_points(start, end))
            .chain(std::iter::once(end))
            .collect();
        for w in pts.windows(2) {
            line(o, w[0], w[1], [0.0, 0.0]);
        }
        if pie || by_centre(&self.tool) {
            line(o, centre, start, if pie { [0.0, 0.0] } else { [4.0, 3.0] });
            line(o, centre, end, if pie { [0.0, 0.0] } else { [4.0, 3.0] });
        }
        let label = if op.tangent.is_some() {
            "Tangent".to_owned()
        } else if op.half && sweep > PI - 1e-9 {
            "Half Circle".to_owned()
        } else {
            self.number.show(radius)
        };
        o.labels.push(Label {
            at: curve.point(start, end, 0.5),
            offset: [10.0, -14.0],
            text: label,
            color: fb.pill,
        });
    }
}

/// The 2-point arc's bulge: how far the cursor is from the chord, to its
/// left about the normal.
fn bulge(op: &ArcOp) -> f64 {
    let (a, b) = (op.pts[0], op.pts[1]);
    let left = op.normal.cross(b - a).normalize_or_zero();
    (op.at - a.lerp(b, 0.5)).dot(left)
}

/// The bulge of the arc from `a` to `b` (plane `normal`) that leaves `a`
/// along `out`: half its sweep is the angle between `out` and the chord.
/// None if `out` leaves the plane or runs along the chord.
fn tangent_bulge(a: DVec3, b: DVec3, normal: DVec3, out: DVec3) -> Option<f64> {
    let n = normal.normalize_or_zero();
    let d = (b - a).normalize_or_zero();
    if out.dot(n).abs() > 1e-6 || d == DVec3::ZERO {
        return None;
    }
    let left = n.cross(d);
    let theta = out.dot(d).clamp(-1.0, 1.0).acos();
    let side = out.dot(left);
    if theta < 1e-6 || theta > PI - 1e-6 || side.abs() < 1e-12 {
        return None;
    }
    Some(side.signum() * a.distance(b) / 2.0 * (theta / 2.0).tan())
}
