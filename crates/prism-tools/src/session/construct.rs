//! Offset, Tape Measure and Protractor (brief 07), and drawing the guides
//! stored in the model.

use prism_doc::Command;
use prism_infer::{AXES, Kind};
use prism_math::{DQuat, DVec2, DVec3};
use prism_ops::offset;
use prism_topo::Guide;
use prism_view::View;
use prism_view::overlay::{Label, Line, Mark, Overlay, Shape};
use prism_view::pick::Hit;

use super::edit::{EditOp, plane_point};
use super::{LastOp, Session};

pub const OFFSET: &str = "tool.offset";
pub const TAPE: &str = "tool.tape";
pub const PROTRACTOR: &str = "tool.protractor";

/// Offset snaps to zero within this many points.
const OFFSET_ZERO: f64 = 6.0;
/// How far guide lines are drawn, as a multiple of the scene's size.
const GUIDE_REACH: f64 = 60.0;

/// The part of `v` square to the unit direction `along`.
fn square_to(v: DVec3, along: DVec3) -> DVec3 {
    v - along * v.dot(along)
}

impl Session {
    // ---------- guides ----------

    /// The guides stored in the model: dashed lines (hidden by faces in
    /// front) and crosses.
    pub(super) fn guides_overlay(&self, o: &mut Overlay) {
        let fb = &self.style.feedback;
        let scene = self.display_scene();
        if scene.guides.is_empty() {
            return;
        }
        let view = self.view();
        let size = scene
            .bounds
            .map_or(1.0, |(lo, hi)| lo.distance(hi))
            .max(view.eye.length())
            .max(scene.tol.dist);
        let reach = size * GUIDE_REACH;
        for (id, g) in &scene.guides {
            let doomed = matches!(&self.edit, Some(EditOp::Erase { guides, .. }) if guides.contains(id))
                || (self.tool == super::ERASE && self.hover_guide == Some(*id));
            let color = if doomed { fb.erase } else { fb.guide };
            match *g {
                Guide::Line { origin, dir } => o.tested.push(Line {
                    a: origin - dir * reach,
                    b: origin + dir * reach,
                    color,
                    width: if doomed { fb.target_width } else { 1.2 },
                    dash: fb.guide_dash,
                }),
                Guide::Point { at } => o.marks.push(Mark {
                    at,
                    shape: Shape::Cross,
                    color,
                    size: 5.0,
                }),
            }
        }
    }

    /// The guide under the cursor (for the Eraser) and its distance in
    /// points.
    pub(super) fn guide_pick(&self, view: &View, s: DVec2) -> Option<(prism_topo::GuideId, f64)> {
        let r = prism_view::pick::EDGE_RADIUS * self.settings.aperture;
        let (o, d) = view.ray(s);
        let mut best: Option<(prism_topo::GuideId, f64)> = None;
        for (id, g) in &self.scene.guides {
            let p = match *g {
                Guide::Point { at } => at,
                Guide::Line { origin, dir } => {
                    let w = o - origin;
                    let (dd, du) = (d.dot(d), d.dot(dir));
                    let den = dd - du * du;
                    if den.abs() <= 1e-14 * dd {
                        continue;
                    }
                    let t = (dd * dir.dot(w) - du * d.dot(w)) / den;
                    origin + dir * t
                }
            };
            let Some((q, _)) = view.to_screen(p) else {
                continue;
            };
            let dist = q.distance(s);
            if dist < r && best.is_none_or(|b| dist < b.1) && !self.scene.occluded(view.eye, p) {
                best = Some((*id, dist));
            }
        }
        best
    }

    // ---------- Offset ----------

    pub(super) fn offset_hover(&mut self, s: DVec2, view: &View) {
        if let Some(EditOp::Offset {
            face,
            origin,
            n,
            d0,
            d,
            at,
        }) = &mut self.edit
        {
            let Some(p) = plane_point(view, s, *origin, *n) else {
                return;
            };
            let Some(now) = offset::signed_distance(self.doc.model(), *face, p) else {
                return;
            };
            let mut dd = now - *d0;
            let px = view.to_screen(p).map_or(0.0, |(_, z)| view.units_per_px(z));
            if dd.abs() < OFFSET_ZERO * px {
                dd = 0.0;
            }
            *d = dd;
            *at = p;
            self.update_preview();
        } else {
            let (o, d) = view.ray(s);
            let h = self.scene.ray_face(o, d).map(|(f, t)| prism_view::Picked {
                hit: Hit::Face(f),
                point: o + d * t,
            });
            self.set_hover_pick(h);
        }
    }

    pub(super) fn offset_press(&mut self, s: DVec2, n_clicks: u32) {
        if let Some(EditOp::Offset { face, d, .. }) = self.edit.clone() {
            // A quick second click without moving repeats the last distance.
            if n_clicks == 2
                && d.abs() <= self.scene.tol.dist
                && let Some(last) = self.last_offset
            {
                self.edit = None;
                self.set_preview(None);
                self.commit_offset(face, last);
                return;
            }
            self.finish_edit();
            return;
        }
        let view = self.view();
        let (o, dir) = view.ray(s);
        let face = if self.selection.faces.len() == 1 {
            self.selection.faces.first().copied()
        } else {
            self.scene.ray_face(o, dir).map(|(f, _)| f)
        };
        let Some(face) = face else { return };
        let Some(sf) = self.scene.face(face) else {
            return;
        };
        let (origin, n) = (sf.origin, sf.normal);
        let Some(p) = plane_point(&view, s, origin, n) else {
            return;
        };
        let Some(d0) = offset::signed_distance(self.doc.model(), face, p) else {
            return;
        };
        self.edit = Some(EditOp::Offset {
            face,
            origin,
            n,
            d0,
            d: 0.0,
            at: p,
        });
        self.last_op = None;
        self.overlay_dirty = true;
    }

    pub(super) fn commit_offset(&mut self, face: prism_topo::FaceId, d: f64) {
        if self.run(Command::Offset { face, distance: d }) {
            self.last_offset = Some(d);
            self.last_op = Some(LastOp::Offset { face, d });
        }
    }

    // ---------- Tape Measure ----------

    pub(super) fn tape_press(&mut self, n_clicks: u32) {
        let Some(i) = self.inference.clone() else {
            return;
        };
        if let Some(EditOp::Tape {
            start,
            end,
            along,
            edge,
        }) = self.edit.clone()
        {
            // A double-click on an edge: a guide along it at the last offset.
            if n_clicks == 2
                && end.distance(start) <= self.scene.tol.dist
                && let Some(dir) = along
            {
                self.edit = None;
                self.double_click_guide(start, dir, edge);
                return;
            }
            self.finish_edit();
            return;
        }
        let along = (i.kind == Kind::OnEdge).then_some(i.edge_dir).flatten();
        self.edit = Some(EditOp::Tape {
            start: i.point,
            along,
            end: i.point,
            edge: i.edge,
        });
        self.last_op = None;
        self.lock = prism_infer::Lock::None;
        self.mem.clear();
        self.overlay_dirty = true;
        self.refresh();
    }

    /// The guide a double-click on an edge makes: along the edge, moved the
    /// last offset towards the side of its first face.
    fn double_click_guide(&mut self, at: DVec3, dir: DVec3, edge: Option<prism_topo::EdgeId>) {
        let offset = self.last_tape.unwrap_or(0.0);
        let side = edge
            .and_then(|e| self.scene.edge(e))
            .and_then(|e| e.faces[0].and_then(|f| self.scene.face(f)))
            .map(|f| {
                let mut s = f.normal.cross(dir).normalize_or_zero();
                if s.dot(f.lo.lerp(f.hi, 0.5) - at) < 0.0 {
                    s = -s;
                }
                s
            })
            .unwrap_or_else(|| {
                let up = if dir.z.abs() > 0.9 {
                    DVec3::X
                } else {
                    DVec3::Z
                };
                up.cross(dir).normalize_or_zero()
            });
        if self.measure_only {
            return;
        }
        self.run(Command::AddGuide {
            guide: Guide::Line {
                origin: at + side * offset,
                dir,
            },
        });
    }

    pub(super) fn tape_drag(&mut self) {
        let Some(i) = &self.inference else { return };
        if let Some(EditOp::Tape { end, .. }) = &mut self.edit {
            *end = i.point;
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    /// The tape's measured distance: square to the edge it started on, or
    /// straight from the start.
    pub(super) fn tape_vector(start: DVec3, end: DVec3, along: Option<DVec3>) -> DVec3 {
        match along {
            Some(a) => square_to(end - start, a),
            None => end - start,
        }
    }

    /// The guide a tape step would make.
    fn tape_guide(start: DVec3, end: DVec3, along: Option<DVec3>) -> Option<Guide> {
        match along {
            Some(dir) => Some(Guide::Line {
                origin: start + Self::tape_vector(start, end, along),
                dir,
            }),
            None => (end != start).then_some(Guide::Point { at: end }),
        }
    }

    // ---------- finishing, typing ----------

    /// Commit an Offset, Tape Measure or Protractor step. Returns false if
    /// `e` is not one of them.
    pub(super) fn finish_construct(&mut self, e: &EditOp) -> bool {
        match *e {
            EditOp::Offset { face, d, .. } => {
                if d.abs() > self.scene.tol.dist {
                    self.commit_offset(face, d);
                }
            }
            EditOp::Tape {
                start, end, along, ..
            } => {
                let v = Self::tape_vector(start, end, along);
                if v.length() > self.scene.tol.dist {
                    self.last_tape = Some(v.length());
                }
                if !self.measure_only
                    && let Some(g) = Self::tape_guide(start, end, along)
                    && self.run(Command::AddGuide { guide: g })
                {
                    self.last_op = Some(LastOp::Edit(e.clone()));
                }
            }
            EditOp::Protractor {
                centre,
                normal,
                u0: Some(u0),
                angle,
                ..
            } => {
                let dir = DQuat::from_axis_angle(normal, angle) * u0;
                if self.run(Command::AddGuide {
                    guide: Guide::Line {
                        origin: centre,
                        dir: dir.normalize(),
                    },
                }) {
                    self.last_op = Some(LastOp::Edit(e.clone()));
                }
            }
            EditOp::Protractor { .. } => {}
            _ => return false,
        }
        true
    }

    /// Enter with typed text during an Offset, Tape Measure or Protractor
    /// step. None if `e` is not one of them.
    pub(super) fn construct_enter(&mut self, e: &EditOp, text: &str) -> Option<()> {
        let nf = self.number;
        match *e {
            EditOp::Offset { face, d, .. } => {
                if let Some(v) = nf.length(text) {
                    self.edit = None;
                    self.set_preview(None);
                    let sign = if d < 0.0 { -1.0 } else { 1.0 };
                    self.commit_offset(face, sign * v);
                }
            }
            EditOp::Tape {
                start, end, along, ..
            } => {
                if let Some(v) = nf.length(text) {
                    let mut dir = Self::tape_vector(start, end, along).normalize_or_zero();
                    if let prism_infer::Lock::Axis(a) = self.lock {
                        let ax = AXES[a as usize];
                        dir = if dir.dot(ax) < 0.0 { -ax } else { ax };
                    }
                    if dir != DVec3::ZERO {
                        if let Some(EditOp::Tape { end, .. }) = &mut self.edit {
                            *end = start + dir * v;
                        }
                        self.finish_edit();
                    }
                }
            }
            EditOp::Protractor { u0: Some(_), .. } => {
                if let Ok(deg) = text.trim().trim_end_matches('°').parse::<f64>() {
                    if let Some(EditOp::Protractor { angle, .. }) = &mut self.edit {
                        *angle = deg.to_radians();
                    }
                    self.finish_edit();
                }
            }
            EditOp::Protractor { .. } => {}
            _ => return None,
        }
        Some(())
    }

    /// Typing right after a tape guide or protractor guide: make it again
    /// at the new distance or angle. None if the last step was not one.
    pub(super) fn construct_retype(&mut self, e: &EditOp, text: &str) -> Option<()> {
        let nf = self.number;
        let mut e = e.clone();
        match &mut e {
            EditOp::Tape {
                start, end, along, ..
            } => {
                let v = nf.length(text)?;
                let dir = Self::tape_vector(*start, *end, *along).normalize_or_zero();
                if dir == DVec3::ZERO {
                    return Some(());
                }
                *end = *start + dir * v.abs();
            }
            EditOp::Protractor { angle, .. } => {
                let Ok(deg) = text.trim().trim_end_matches('°').parse::<f64>() else {
                    return Some(());
                };
                *angle = deg.to_radians();
            }
            _ => return None,
        }
        if self.doc.undo() {
            self.model_changed();
        }
        self.finish_construct(&e);
        Some(())
    }

    pub(super) fn construct_measurement(&self) -> Option<(&'static str, String)> {
        let nf = &self.number;
        Some(match (&self.edit, &self.last_op) {
            (Some(EditOp::Offset { d, .. }), _) => ("Distance", nf.show(*d)),
            (None, Some(LastOp::Offset { d, .. })) => ("Distance", nf.show(*d)),
            (
                Some(EditOp::Tape {
                    start, end, along, ..
                }),
                _,
            ) => (
                "Length",
                nf.show(Self::tape_vector(*start, *end, *along).length()),
            ),
            (
                None,
                Some(LastOp::Edit(EditOp::Tape {
                    start, end, along, ..
                })),
            ) => (
                "Length",
                nf.show(Self::tape_vector(*start, *end, *along).length()),
            ),
            (
                Some(EditOp::Protractor {
                    angle, u0: Some(_), ..
                }),
                _,
            )
            | (None, Some(LastOp::Edit(EditOp::Protractor { angle, .. }))) => {
                ("Angle", format!("{:.1}°", angle.to_degrees()))
            }
            (Some(EditOp::Protractor { .. }), _) => ("Angle", String::new()),
            (None, _) if self.tool == OFFSET => (
                "Distance",
                self.last_offset.map_or(String::new(), |d| nf.show(d)),
            ),
            (None, _) if self.tool == TAPE => (
                "Length",
                self.last_tape.map_or(String::new(), |d| nf.show(d)),
            ),
            _ => return None,
        })
    }

    pub(super) fn construct_hint(&self) -> Option<&'static str> {
        Some(match (self.tool.as_str(), &self.edit) {
            (OFFSET, Some(_)) => {
                "Move to offset (inside shrinks, outside grows) · click to finish · type a distance + Enter · Esc cancels"
            }
            (OFFSET, None) => {
                "Click a face to offset its outline · double-click repeats the last distance"
            }
            (TAPE, Some(_)) if self.measure_only => {
                "Measuring only (Ctrl makes guides) · click to finish · type a length + Enter"
            }
            (TAPE, Some(_)) => {
                "Click to place the guide · Ctrl measures only · arrows lock an axis · type a length + Enter · Esc cancels"
            }
            (TAPE, None) => {
                "Click an edge for a parallel guide, or a point for a guide point · double-click an edge for a guide along it · Ctrl measures only"
            }
            (PROTRACTOR, Some(EditOp::Protractor { u0: None, .. })) => {
                "Click (or drag from the centre and release) to set the base direction"
            }
            (PROTRACTOR, Some(_)) => {
                "Move to set the guide's angle (15° snaps, Shift for 45°) · click to place · type degrees + Enter"
            }
            (PROTRACTOR, None) => {
                "Click the protractor's centre · Up locks it to the face under it · Right and Left lock red and green"
            }
            _ => return None,
        })
    }

    pub(super) fn construct_overlay(&self, o: &mut Overlay) {
        let fb = &self.style.feedback;
        match &self.edit {
            Some(EditOp::Offset { d, at, .. }) => {
                o.labels.push(Label {
                    at: *at,
                    offset: [14.0, -16.0],
                    text: self.number.show(*d),
                    color: fb.pill,
                });
            }
            Some(EditOp::Tape {
                start, end, along, ..
            }) => {
                let v = Self::tape_vector(*start, *end, *along);
                o.lines.push(Line {
                    a: *start,
                    b: *end,
                    color: fb.rubber_band,
                    width: 1.4,
                    dash: [4.0, 4.0],
                });
                if !self.measure_only {
                    match Self::tape_guide(*start, *end, *along) {
                        Some(Guide::Line { origin, dir }) => {
                            let r = self
                                .scene
                                .bounds
                                .map_or(1.0, |(lo, hi)| lo.distance(hi))
                                .max(self.view().eye.length())
                                * GUIDE_REACH;
                            o.lines.push(Line {
                                a: origin - dir * r,
                                b: origin + dir * r,
                                color: fb.guide,
                                width: 1.2,
                                dash: fb.guide_dash,
                            });
                        }
                        Some(Guide::Point { at }) => o.marks.push(Mark {
                            at,
                            shape: Shape::Cross,
                            color: fb.guide,
                            size: 5.0,
                        }),
                        None => {}
                    }
                }
                o.labels.push(Label {
                    at: *end,
                    offset: [14.0, -16.0],
                    text: format!(
                        "{}{}",
                        if self.measure_only { "Measure " } else { "" },
                        self.number.show(v.length())
                    ),
                    color: fb.pill,
                });
            }
            _ => {}
        }
    }
}
