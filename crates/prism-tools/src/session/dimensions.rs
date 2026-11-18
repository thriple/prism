//! Dimensions (Annotation & Sections part 2): the Dimensions tool and the
//! Edit > Dimension command.
//!
//! - **Tool:** click the first point, click the second (both with
//!   inference), then click where the dimension line goes: the line is
//!   offset square to the measured line, towards that point. The label
//!   shows the length in the document's unit. Click a dimension's line to
//!   pick it up and move it the same way; Esc leaves it.
//! - A dimension is a document object (world coordinates). It changes no
//!   geometry and does not follow the model if it is moved (a later pass).

use prism_doc::{Command, DimId, Dimension};
use prism_math::{DVec2, DVec3};
use prism_view::View;
use prism_view::overlay::{Label, Line, Overlay};

use super::Session;

pub const DIM_TOOL: &str = "tool.dim";

/// How near a click must be to a dimension to pick it up, in pixels.
const PICK_PX: f64 = 8.0;
/// An arrowhead's length, in pixels.
const ARROW_PX: f64 = 11.0;

/// A dimension being made, or an existing one being moved.
#[derive(Clone, Copy, Debug)]
pub(super) struct DimOp {
    pub a: DVec3,
    pub b: Option<DVec3>,
    /// The cursor's point.
    pub at: DVec3,
    /// Set when moving an existing dimension.
    pub moving: Option<DimId>,
}

/// The part of `p - a` square to the line along `ab`: where a dimension
/// line goes. Zero if `p` is on the line.
pub fn offset_towards(a: DVec3, b: DVec3, p: DVec3) -> DVec3 {
    let dir = (b - a).normalize_or_zero();
    let d = p - a;
    d - dir * d.dot(dir)
}

impl Session {
    fn dim_tiny(&self, p: DVec3) -> f64 {
        self.scene.tol.dist.max(p.length() * 1e-12)
    }

    /// The dimension line's ends and the extension lines' feet.
    fn dim_geometry(d: &Dimension) -> [DVec3; 4] {
        [d.a, d.b, d.a + d.offset, d.b + d.offset]
    }

    /// The dimension under the cursor.
    fn dim_at(&self, s: DVec2, view: &View) -> Option<DimId> {
        let mut best: Option<(f64, DimId)> = None;
        for (id, d) in self.doc.assembly().dimensions().0 {
            let [a, b, c, e] = Self::dim_geometry(d);
            for (p, q) in [(c, e), (a, c), (b, e)] {
                let (Some((p, _)), Some((q, _))) = (view.to_screen(p), view.to_screen(q)) else {
                    continue;
                };
                let pq = q - p;
                let len2 = pq.length_squared();
                let t = if len2 > 0.0 {
                    ((s - p).dot(pq) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let dist = s.distance(p + pq * t);
                if dist <= PICK_PX && best.is_none_or(|b| dist < b.0) {
                    best = Some((dist, *id));
                }
            }
        }
        best.map(|b| b.1)
    }

    pub(super) fn dim_press(&mut self, s: DVec2) {
        self.update_hover(s);
        let view = self.view();
        let point = self.inference.as_ref().map(|i| i.point);
        match self.dim_op {
            None => {
                if let Some(id) = self.dim_at(s, &view)
                    && let Some(d) = self.doc.assembly().dimension(id).copied()
                {
                    self.dim_sel = Some(id);
                    self.dim_op = Some(DimOp {
                        a: d.a,
                        b: Some(d.b),
                        at: d.a + d.offset,
                        moving: Some(id),
                    });
                } else if let Some(p) = point {
                    self.dim_op = Some(DimOp {
                        a: p,
                        b: None,
                        at: p,
                        moving: None,
                    });
                }
            }
            Some(mut op) => {
                let Some(p) = point else {
                    return;
                };
                let tiny = self.dim_tiny(p);
                match op.b {
                    None => {
                        if p.distance(op.a) > tiny {
                            op.b = Some(p);
                            op.at = p;
                            self.dim_op = Some(op);
                        }
                    }
                    Some(b) => {
                        let offset = offset_towards(op.a, b, p);
                        if offset.length() > tiny {
                            let c = match op.moving {
                                Some(id) => Command::MoveDimension { id, offset },
                                None => Command::AddDimension { a: op.a, b, offset },
                            };
                            if self.doc.run(c) {
                                self.model_changed();
                                if op.moving.is_none() {
                                    self.dim_sel = self
                                        .doc
                                        .assembly()
                                        .dimensions()
                                        .0
                                        .keys()
                                        .next_back()
                                        .copied();
                                }
                            }
                            self.dim_op = None;
                        }
                    }
                }
            }
        }
        self.overlay_dirty = true;
        self.dirty = true;
        self.refresh();
    }

    pub(super) fn dim_hover(&mut self) {
        let Some(p) = self.inference.as_ref().map(|i| i.point) else {
            return;
        };
        if let Some(op) = &mut self.dim_op {
            op.at = p;
        }
    }

    pub(super) fn dim_hint(&self) -> Option<&'static str> {
        if self.tool != DIM_TOOL {
            return None;
        }
        Some(
            match self.dim_op.map(|o| (o.moving.is_some(), o.b.is_some())) {
                None => "Click the first point · click a dimension's line to move it",
                Some((true, _)) => "Click where the dimension line goes · Esc leaves it",
                Some((false, false)) => "Click the second point · Esc cancels",
                Some((false, true)) => "Click where the dimension line goes · Esc cancels",
            },
        )
    }

    /// Whether the document has any dimension (for menus).
    pub fn has_dimensions(&self) -> bool {
        !self.doc.assembly().dimensions().0.is_empty()
    }

    /// Add a dimension (for the assistant): its id.
    pub fn add_dimension_line(&mut self, a: DVec3, b: DVec3, offset: DVec3) -> Option<u32> {
        self.cancel();
        if !self.doc.run(Command::AddDimension { a, b, offset }) {
            return None;
        }
        self.model_changed();
        let id = self
            .doc
            .assembly()
            .dimensions()
            .0
            .keys()
            .next_back()
            .copied()?;
        self.dim_sel = Some(id);
        Some(id.0)
    }

    /// Edit > Dimension > Delete Dimension: the picked one, else the newest.
    pub(super) fn dim_delete(&mut self) {
        let all = self.doc.assembly().dimensions().0;
        let Some(id) = self
            .dim_sel
            .filter(|id| all.contains_key(id))
            .or_else(|| all.keys().next_back().copied())
        else {
            return;
        };
        self.cancel();
        if self.doc.run(Command::DeleteDimension { id }) {
            self.dim_sel = None;
            self.model_changed();
        }
    }

    /// The dimensions, and the one being made.
    pub(super) fn dim_overlay(&self, o: &mut Overlay) {
        let colour = self.style.feedback.dimension;
        let view = self.view();
        let draw = |d: Dimension, hot: bool, o: &mut Overlay| {
            let [a, b, c, e] = Self::dim_geometry(&d);
            let color = if hot { self.style.selected } else { colour };
            let width = if hot { 2.0 } else { 1.2 };
            let line = |p: DVec3, q: DVec3| Line {
                a: p,
                b: q,
                color,
                width,
                dash: [0.0, 0.0],
            };
            o.lines.push(line(a, c));
            o.lines.push(line(b, e));
            o.lines.push(line(c, e));
            // Arrowheads on the dimension line, sized in pixels.
            let mid = (c + e) * 0.5;
            if let Some((_, depth)) = view.to_screen(mid) {
                let s = view.units_per_px(depth) * ARROW_PX;
                let along = (e - c).normalize_or_zero();
                let side = d.offset.normalize_or_zero();
                for (tip, dir) in [(c, 1.0), (e, -1.0)] {
                    let back = tip + along * (dir * s);
                    o.lines.push(line(tip, back + side * (s * 0.3)));
                    o.lines.push(line(tip, back - side * (s * 0.3)));
                }
            }
            o.labels.push(Label {
                at: mid,
                offset: [0.0, -14.0],
                text: self.number.show(d.length()),
                color: self.style.feedback.pill,
            });
        };
        if self.view.dimensions {
            for (id, d) in self.doc.assembly().dimensions().0 {
                if self.dim_op.is_some_and(|op| op.moving == Some(*id)) {
                    continue;
                }
                draw(*d, self.dim_sel == Some(*id), o);
            }
        }
        if self.tool != super::dimensions::DIM_TOOL {
            return;
        }
        let Some(op) = self.dim_op else {
            return;
        };
        match op.b {
            None => o.lines.push(Line {
                a: op.a,
                b: op.at,
                color: colour,
                width: 1.2,
                dash: [6.0, 4.0],
            }),
            Some(b) => {
                let offset = offset_towards(op.a, b, op.at);
                draw(Dimension { a: op.a, b, offset }, true, o);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dimension_line_goes_square_to_the_measured_line() {
        let off = offset_towards(
            DVec3::ZERO,
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(3.0, 2.0, 1.0),
        );
        assert_eq!(off, DVec3::new(0.0, 2.0, 1.0));
        assert_eq!(
            offset_towards(DVec3::ZERO, DVec3::X, DVec3::new(2.0, 0.0, 0.0)),
            DVec3::ZERO
        );
        // A measured line that is a point has no direction: the whole step.
        assert_eq!(offset_towards(DVec3::ZERO, DVec3::ZERO, DVec3::Y), DVec3::Y);
    }
}
