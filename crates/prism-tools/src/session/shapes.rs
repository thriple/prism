//! The Circle and Polygon tools (brief 23, step 2d): click the centre, then
//! a point on the radius. The plane is the Rectangle's: the face under the
//! cursor, or the axis an arrow key picks. The measurement box takes the
//! radius, and the Polygon's side count as `6s`. A double-click places
//! the last size again.

use prism_doc::Command;
use prism_math::{DVec2, DVec3, Plane};
use prism_view::View;
use prism_view::overlay::{Label, Line, Overlay};

use super::{Session, plane_point};

pub const CIRCLE: &str = "tool.circle";
pub const POLYGON: &str = "tool.polygon";

/// Built, but greyed in menus, toolbars and the dock until Exact Curves is
/// done (owner, 2026-11-09). Only MCP `use_tool` reaches them.
pub const GREYED_TOOLS: [&str; 8] = [
    CIRCLE,
    POLYGON,
    super::arcs::ARC,
    super::arcs::ARC2,
    super::arcs::ARC3,
    super::arcs::ARC_CENTRE,
    super::arcs::PIE,
    super::round::FILLET,
];

/// A polygon's sides when none were typed.
pub const DEFAULT_SIDES: u32 = 6;
/// The most sides a polygon can have.
pub const MAX_SIDES: u32 = 1000;

/// A circle or polygon being drawn.
#[derive(Clone, Copy, Debug)]
pub(super) struct ShapeOp {
    pub centre: DVec3,
    pub normal: DVec3,
    /// The cursor on the plane: the radius, and a polygon's first corner.
    pub at: DVec3,
}

impl ShapeOp {
    fn radius(&self) -> f64 {
        self.at.distance(self.centre)
    }
}

pub(super) fn is_shape(tool: &str) -> bool {
    tool == CIRCLE || tool == POLYGON
}

/// A polygon's corners: inscribed in the circle of `radius`, the first
/// towards `dir`.
pub fn polygon_points(
    centre: DVec3,
    normal: DVec3,
    dir: DVec3,
    radius: f64,
    sides: u32,
) -> Vec<DVec3> {
    let n = normal.normalize();
    let u = (dir - n * dir.dot(n)).normalize_or_zero();
    let u = if u == DVec3::ZERO {
        Plane::new(centre, n).frame().u
    } else {
        u
    };
    let v = n.cross(u);
    (0..sides)
        .map(|k| {
            let t = std::f64::consts::TAU * f64::from(k) / f64::from(sides);
            centre + (u * t.cos() + v * t.sin()) * radius
        })
        .collect()
}

impl Session {
    pub(super) fn shape_press(&mut self, s: DVec2, n_clicks: u32) {
        self.update_hover(s);
        let Some(i) = self.inference.clone() else {
            return;
        };
        match self.shape {
            None => {
                let normal = self.plane_normal(&i);
                self.shape = Some(ShapeOp {
                    centre: i.point,
                    normal,
                    at: i.point,
                });
                self.last_op = None;
            }
            Some(op) => {
                // A double-click (or a second click on the centre) places
                // the last size again.
                let tiny = self.tiny_at(op.centre);
                let radius = if op.radius() <= tiny || n_clicks == 2 {
                    match self.shape_radius {
                        Some(r) => r,
                        None => return,
                    }
                } else {
                    op.radius()
                };
                self.commit_shape(op, radius);
            }
        }
        self.overlay_dirty = true;
        self.refresh();
    }

    /// The cursor moved: the radius follows it on the plane.
    pub(super) fn shape_hover(&mut self, s: DVec2, view: &View) {
        let (Some(op), Some(i)) = (&mut self.shape, &self.inference) else {
            return;
        };
        op.at = plane_point(&self.scene, view, s, i, op.centre, op.normal);
    }

    fn tiny_at(&self, p: DVec3) -> f64 {
        self.scene.tol.dist.max(p.length() * 1e-12)
    }

    fn commit_shape(&mut self, op: ShapeOp, radius: f64) {
        self.shape = None;
        self.typed.clear();
        self.lock = prism_infer::Lock::None;
        self.overlay_dirty = true;
        if radius <= self.tiny_at(op.centre) {
            return;
        }
        let c = if self.tool == POLYGON {
            Command::Polygon {
                points: polygon_points(op.centre, op.normal, op.at - op.centre, radius, self.sides),
            }
        } else {
            Command::Circle {
                centre: op.centre,
                normal: op.normal,
                radius,
            }
        };
        if self.run(c) {
            self.shape_radius = Some(radius);
        }
        self.mem.clear();
    }

    /// Typed text for these tools: `6s` sets the side count, a length the
    /// radius (finishing the shape in progress). True if it was used.
    pub(super) fn shape_enter(&mut self, text: &str) -> bool {
        if !is_shape(&self.tool) {
            return false;
        }
        let t = text.trim();
        if self.tool == POLYGON
            && let Some(n) = t.strip_suffix(['s', 'S'])
        {
            if let Ok(n) = n.trim().parse::<u32>()
                && (3..=MAX_SIDES).contains(&n)
            {
                self.sides = n;
                self.overlay_dirty = true;
                self.refresh();
            }
            return true;
        }
        if let Some(op) = self.shape
            && let Some(r) = self.number.length(t)
            && r.abs() > 0.0
        {
            self.commit_shape(op, r.abs());
            self.refresh();
        }
        true
    }

    pub(super) fn shape_hint(&self) -> Option<&'static str> {
        Some(match (self.tool.as_str(), self.shape.is_some()) {
            (CIRCLE, true) => {
                "Click to set the radius · type a radius + Enter · arrows pick the plane · Esc cancels"
            }
            (CIRCLE, false) => {
                "Click the centre. It draws on the face or plane under the cursor · double-click places the last size"
            }
            (POLYGON, true) => {
                "Click to set the radius · type a radius, or sides as 8s, + Enter · arrows pick the plane · Esc cancels"
            }
            (POLYGON, false) => {
                "Click the centre · type sides as 8s + Enter · double-click places the last size"
            }
            _ => return None,
        })
    }

    pub(super) fn shape_measurement(&self) -> Option<(&'static str, String)> {
        if !is_shape(&self.tool) {
            return None;
        }
        let r = self
            .shape
            .map(|op| op.radius())
            .filter(|r| *r > 0.0)
            .or(self.shape_radius);
        let radius = r.map(|r| self.number.show(r)).unwrap_or_default();
        Some(if self.tool == POLYGON {
            ("Radius", format!("{radius} · {} sides", self.sides))
        } else {
            ("Radius", radius)
        })
    }

    /// The shape in progress, drawn as its outline and radius.
    pub(super) fn shape_overlay(&self, o: &mut Overlay) {
        let Some(op) = self.shape else { return };
        let r = op.radius();
        if r <= self.tiny_at(op.centre) {
            return;
        }
        let fb = &self.style.feedback;
        let sides = if self.tool == POLYGON {
            self.sides
        } else {
            prism_geom::SEGMENTS_PER_TURN as u32
        };
        let pts = polygon_points(op.centre, op.normal, op.at - op.centre, r, sides);
        for k in 0..pts.len() {
            o.lines.push(Line {
                a: pts[k],
                b: pts[(k + 1) % pts.len()],
                color: fb.rubber_band,
                width: fb.rubber_band_width,
                dash: [0.0, 0.0],
            });
        }
        o.lines.push(Line {
            a: op.centre,
            b: op.at,
            color: fb.rubber_band,
            width: fb.rubber_band_width,
            dash: [4.0, 3.0],
        });
        o.labels.push(Label {
            at: op.centre.lerp(op.at, 0.5),
            offset: [10.0, -14.0],
            text: self.number.show(r),
            color: fb.pill,
        });
    }
}
