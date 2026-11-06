//! Push/Pull, Move, Rotate, Scale and the Eraser (brief 06), as in the feel
//! test. Each step previews on a copy of the model and commits one command.

use std::collections::BTreeSet;

use prism_doc::{Command, Motion};
use prism_infer::{AXES, Kind};
use prism_math::{DQuat, DVec2, DVec3};
use prism_ops::select::Selection;
use prism_topo::{EdgeId, FaceId, GuideId, Model, VertexId};
use prism_view::View;
use prism_view::clicks::click_select;
use prism_view::overlay::{Label, Line, Mark, Overlay, Shape, Tri};
use prism_view::pick::{self, Hit};
use prism_view::style::alpha;

use super::construct::{OFFSET, PROTRACTOR, TAPE};
use super::{LastOp, Session, rect_basis};

pub const PUSH: &str = "tool.push";
pub const MOVE: &str = "tool.move";
pub const ROTATE: &str = "tool.rotate";
pub const SCALE: &str = "tool.scale";
pub const ERASE: &str = "tool.erase";

/// Push/Pull's snaps: match an endpoint within this many pixels (times the
/// aperture), the ground within 12, and zero within 6.
const PUSH_MATCH: f64 = 14.0;
const PUSH_GROUND: f64 = 12.0;
const PUSH_ZERO: f64 = 6.0;
/// Rotate: soft snap to 15° within this many radians; Shift gives 45° steps.
const SOFT_STEP: f64 = std::f64::consts::PI / 12.0;
const SOFT_REACH: f64 = 0.045;
const SHIFT_STEP: f64 = std::f64::consts::PI / 4.0;
/// Scale: clamp, and snap to 1 within this.
const SCALE_MIN: f64 = 0.05;
const SCALE_MAX: f64 = 20.0;
const SCALE_SNAP: f64 = 0.02;
/// Protractor radius on screen, in points.
const PROTRACTOR_RADIUS: f64 = 80.0;
/// A press that moves less than this before release is a click.
const DRAG_MIN: f64 = 6.0;

#[derive(Clone, Debug)]
pub(super) enum EditOp {
    Push {
        face: FaceId,
        p0: DVec3,
        n: DVec3,
        d: f64,
        new_loop: bool,
        snap: Option<(String, DVec3)>,
    },
    Move {
        from: DVec3,
        selection: Selection,
        verts: BTreeSet<VertexId>,
        delta: DVec3,
        copy: bool,
    },
    Rotate {
        centre: DVec3,
        normal: DVec3,
        /// The start arm, once set.
        u0: Option<DVec3>,
        /// The current arm.
        u1: Option<DVec3>,
        angle: f64,
        selection: Selection,
        verts: BTreeSet<VertexId>,
        copy: bool,
        /// Where the centre was pressed, while the button is still down.
        pressed: Option<DVec2>,
    },
    Scale {
        centre: DVec3,
        r0: f64,
        k: f64,
        verts: BTreeSet<VertexId>,
    },
    Erase {
        edges: BTreeSet<EdgeId>,
        guides: BTreeSet<GuideId>,
    },
    Offset {
        face: FaceId,
        /// The face's plane.
        origin: DVec3,
        n: DVec3,
        /// The cursor's distance from the outline when clicked.
        d0: f64,
        d: f64,
        /// The cursor on the plane.
        at: DVec3,
    },
    Tape {
        start: DVec3,
        /// Started on an edge: its direction (the guide is parallel to it).
        along: Option<DVec3>,
        edge: Option<EdgeId>,
        end: DVec3,
    },
    Protractor {
        centre: DVec3,
        normal: DVec3,
        u0: Option<DVec3>,
        u1: Option<DVec3>,
        angle: f64,
        pressed: Option<DVec2>,
    },
}

pub(super) fn is_edit(t: &str) -> bool {
    matches!(
        t,
        PUSH | MOVE | ROTATE | SCALE | ERASE | OFFSET | TAPE | PROTRACTOR
    )
}

/// The rotation of `p` about the line through `c` along unit `n`.
fn rotate_point(p: DVec3, c: DVec3, n: DVec3, a: f64) -> DVec3 {
    c + DQuat::from_axis_angle(n, a) * (p - c)
}

/// Where the ray through `s` meets the plane through `c` with normal `n`.
pub(super) fn plane_point(view: &View, s: DVec2, c: DVec3, n: DVec3) -> Option<DVec3> {
    let (o, d) = view.ray(s);
    let den = n.dot(d);
    if den.abs() < 1e-3 {
        return None;
    }
    let t = n.dot(c - o) / den;
    (t > 0.0).then(|| o + d * t)
}

/// The closest point on the line `a + u t` to the ray: its parameter t.
fn along_line(o: DVec3, d: DVec3, a: DVec3, u: DVec3) -> Option<f64> {
    let w = o - a;
    let (dd, du, uu) = (d.dot(d), d.dot(u), u.dot(u));
    let den = dd * uu - du * du;
    if den.abs() <= 1e-14 * dd * uu {
        return None;
    }
    Some((dd * u.dot(w) - du * d.dot(w)) / den)
}

impl Session {
    pub(super) fn edit_wants_inference(&self) -> bool {
        matches!(
            self.tool.as_str(),
            MOVE | ROTATE | SCALE | TAPE | PROTRACTOR
        )
    }

    /// The selection a tool works on: the selection, or else (Move, Rotate)
    /// the item under the cursor, or (Scale) what is connected to it.
    fn working_selection(&self, s: DVec2, connected: bool) -> Selection {
        if !self.selection.is_empty() {
            return self.selection.clone();
        }
        let hit = pick::pick(&self.scene, &self.view(), s, self.settings.aperture).map(|p| p.hit);
        match hit {
            Some(h) if connected => click_select(self.doc.model(), Some(h), 3),
            Some(h) => click_select(self.doc.model(), Some(h), 1),
            None => Selection::default(),
        }
    }

    /// The protractor's plane before a rotation starts: the lock, the face
    /// under the cursor, the ground, else the view's main plane.
    fn protractor_normal(&self) -> DVec3 {
        if let Some(n) = self.rot_lock {
            return n;
        }
        if let Some(i) = &self.inference {
            if let Some(f) = i.face.and_then(|f| self.scene.face(f)) {
                return f.normal;
            }
            if i.kind == Kind::OnGround {
                return DVec3::Z;
            }
            // Snapped to a point or an edge: the face under the cursor, or
            // the ground if the point is on it.
            if let Some(s) = self.cursor {
                let (o, d) = self.view().ray(s);
                if let Some(f) = self
                    .scene
                    .ray_face(o, d)
                    .and_then(|(f, _)| self.scene.face(f))
                {
                    return f.normal;
                }
            }
            if i.point.z.abs() <= self.scene.tol.dist {
                return DVec3::Z;
            }
        }
        let f = self.view().f;
        let a = (0..3)
            .max_by(|&i, &j| f[i].abs().total_cmp(&f[j].abs()))
            .unwrap_or(2);
        AXES[a]
    }

    /// Up in Rotate: lock the protractor to the face under the cursor (or to
    /// blue); Right and Left lock it to red and green; Down releases.
    pub(super) fn rotate_arrow(&mut self, axis: Option<u8>, up: bool) {
        self.rot_lock = match (up, axis) {
            (true, _) => Some(
                self.inference
                    .as_ref()
                    .and_then(|i| i.face)
                    .and_then(|f| self.scene.face(f))
                    .map_or(DVec3::Z, |f| f.normal),
            ),
            (false, Some(a)) => Some(AXES[a as usize]),
            (false, None) => None,
        };
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Hover and previews for the edit tools, after inference ran.
    pub(super) fn edit_hover(&mut self, s: DVec2, view: &View) {
        match self.tool.as_str() {
            PUSH => {
                if let Some(EditOp::Push { .. }) = &self.edit {
                    self.push_drag(s, view);
                } else {
                    let h = self
                        .scene
                        .ray_face(view.ray(s).0, view.ray(s).1)
                        .map(|(f, t)| prism_view::Picked {
                            hit: Hit::Face(f),
                            point: view.ray(s).0 + view.ray(s).1 * t,
                        });
                    self.set_hover_pick(h);
                }
            }
            ERASE => {
                let mut h = edge_pick(&self.scene, view, s, self.settings.aperture);
                // A guide nearer than the edge wins.
                let g = self.guide_pick(view, s).filter(|(_, gd)| {
                    h.is_none_or(|p| {
                        view.to_screen(p.point)
                            .is_none_or(|(q, _)| q.distance(s) > *gd)
                    })
                });
                if g.is_some() {
                    h = None;
                }
                let g = g.map(|(id, _)| id);
                if g != self.hover_guide {
                    self.hover_guide = g;
                    self.overlay_dirty = true;
                    self.dirty = true;
                }
                self.set_hover_pick(h);
                if let Some(EditOp::Erase { edges, guides }) = &mut self.edit {
                    let mut more = false;
                    if let Some(p) = h
                        && let Hit::Edge(e) = p.hit
                    {
                        more |= edges.insert(e);
                    }
                    if let Some(g) = g {
                        more |= guides.insert(g);
                    }
                    if more {
                        self.overlay_dirty = true;
                        self.dirty = true;
                    }
                }
            }
            OFFSET => self.offset_hover(s, view),
            TAPE => self.tape_drag(),
            PROTRACTOR => self.transform_drag(s, view),
            MOVE | ROTATE | SCALE => {
                if self.edit.is_none() && self.selection.is_empty() {
                    let h = pick::pick(&self.scene, view, s, self.settings.aperture);
                    self.set_hover_pick(h);
                } else if self.hover.is_some() {
                    self.set_hover_pick(None);
                }
                self.transform_drag(s, view);
            }
            _ => {}
        }
    }

    pub(super) fn set_hover_pick(&mut self, h: Option<prism_view::Picked>) {
        if h.map(|p| p.hit) != self.hover.map(|p| p.hit) {
            self.highlight_dirty = true;
            self.dirty = true;
        }
        self.hover = h;
    }

    fn push_drag(&mut self, s: DVec2, view: &View) {
        let Some(EditOp::Push { p0, n, d, snap, .. }) = &mut self.edit else {
            return;
        };
        let (o, dir) = view.ray(s);
        let mut t = along_line(o, dir, *p0, *n).unwrap_or(*d);
        *snap = None;
        let ap = self.settings.aperture;
        let mut best = f64::MAX;
        for &(_, p) in &self.scene.vertices {
            if let Some((q, _)) = view.to_screen(p) {
                let dd = q.distance(s);
                if dd < PUSH_MATCH * ap && dd < best {
                    best = dd;
                    t = (p - *p0).dot(*n);
                    *snap = Some(("Match Endpoint".to_owned(), p));
                }
            }
        }
        if snap.is_none() && n.z.abs() > 0.3 {
            let tg = -p0.z / n.z;
            let gp = *p0 + *n * tg;
            if view
                .to_screen(gp)
                .is_some_and(|(q, _)| q.distance(s) < PUSH_GROUND * ap)
            {
                t = tg;
                *snap = Some(("On Ground".to_owned(), gp));
            }
        }
        if snap.is_none()
            && let (Some((a, _)), Some((b, _))) =
                (view.to_screen(*p0 + *n * t), view.to_screen(*p0))
            && a.distance(b) < PUSH_ZERO
        {
            t = 0.0;
        }
        *d = t;
        self.update_preview();
    }

    fn transform_drag(&mut self, s: DVec2, view: &View) {
        let Some(i) = self.inference.clone() else {
            return;
        };
        let shift = self.shift_held;
        match &mut self.edit {
            Some(EditOp::Move { from, delta, .. }) => {
                *delta = i.point - *from;
            }
            Some(
                EditOp::Rotate {
                    centre,
                    normal,
                    u0,
                    u1,
                    angle,
                    ..
                }
                | EditOp::Protractor {
                    centre,
                    normal,
                    u0,
                    u1,
                    angle,
                    ..
                },
            ) => {
                let Some(p) = plane_point(view, s, *centre, *normal) else {
                    return;
                };
                let v = p - *centre;
                if v.length() <= 0.0 {
                    return;
                }
                let u = v.normalize();
                *u1 = Some(u);
                if let Some(a0) = *u0 {
                    let mut a = normal.dot(a0.cross(u)).atan2(a0.dot(u));
                    if shift {
                        a = (a / SHIFT_STEP).round() * SHIFT_STEP;
                    } else {
                        let r = (a / SOFT_STEP).round() * SOFT_STEP;
                        if (a - r).abs() < SOFT_REACH {
                            a = r;
                        }
                    }
                    *angle = a;
                }
            }
            Some(EditOp::Scale { centre, r0, k, .. }) => {
                let mut f = (i.point.distance(*centre) / r0.max(f64::MIN_POSITIVE))
                    .clamp(SCALE_MIN, SCALE_MAX);
                if (f - 1.0).abs() < SCALE_SNAP {
                    f = 1.0;
                }
                *k = f;
            }
            _ => return,
        }
        self.update_preview();
    }

    /// Show the step in progress on a copy of the model.
    pub(super) fn update_preview(&mut self) {
        // A whole body moved or turned rigidly: move what is drawn instead
        // of rebuilding the model (big imported meshes stay smooth).
        if let Some(scene) = self.rigid_preview() {
            self.set_preview_scene(Some(scene));
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        let base = self.doc.model();
        let preview: Option<Model> = match &self.edit {
            Some(EditOp::Push {
                face, d, new_loop, ..
            }) => {
                let mut m = base.clone();
                if d.abs() > 0.0 {
                    prism_ops::push_pull(&mut m, *face, *d, *new_loop);
                }
                Some(m)
            }
            Some(EditOp::Offset { face, d, .. }) => {
                let mut m = base.clone();
                if d.abs() > 0.0 {
                    prism_ops::offset::offset(&mut m, *face, *d);
                }
                Some(m)
            }
            Some(e @ (EditOp::Move { .. } | EditOp::Rotate { .. } | EditOp::Scale { .. })) => {
                self.transform_command(e).map(|c| {
                    let mut m = base.clone();
                    c.apply(&mut m);
                    m
                })
            }
            _ => None,
        };
        self.set_preview(preview);
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The preview of a move or rotation that moves whole faces and edges
    /// only, made by moving the drawn scene. None if it changes any face's
    /// shape (then the model is rebuilt), or is a copy.
    fn rigid_preview(&self) -> Option<prism_view::Scene> {
        match &self.edit {
            Some(EditOp::Move {
                verts,
                delta,
                copy: false,
                ..
            }) => {
                let d = *delta;
                self.scene.rigidly_moved(verts, |p| p + d, |n| n)
            }
            Some(EditOp::Rotate {
                verts,
                centre,
                normal,
                angle,
                copy: false,
                u0: Some(_),
                ..
            }) => {
                let q = DQuat::from_axis_angle(normal.normalize(), *angle);
                let c = *centre;
                self.scene
                    .rigidly_moved(verts, |p| c + q * (p - c), |n| q * n)
            }
            _ => None,
        }
    }

    /// The command a move, rotation or scale in progress would commit.
    fn transform_command(&self, e: &EditOp) -> Option<Command> {
        Some(match e {
            EditOp::Move {
                selection,
                verts,
                delta,
                copy,
                ..
            } => {
                if *copy {
                    Command::Copy {
                        selection: selection.clone(),
                        step: Motion::Translate { by: *delta },
                        count: 1,
                    }
                } else {
                    Command::Translate {
                        vertices: verts.clone(),
                        by: *delta,
                    }
                }
            }
            EditOp::Rotate {
                centre,
                normal,
                angle,
                selection,
                verts,
                copy,
                u0,
                ..
            } => {
                u0.as_ref()?;
                if *copy {
                    Command::Copy {
                        selection: selection.clone(),
                        step: Motion::Rotate {
                            centre: *centre,
                            axis: *normal,
                            angle: *angle,
                        },
                        count: 1,
                    }
                } else {
                    Command::Rotate {
                        vertices: verts.clone(),
                        centre: *centre,
                        axis: *normal,
                        angle: *angle,
                    }
                }
            }
            EditOp::Scale {
                centre, k, verts, ..
            } => Command::Scale {
                vertices: verts.clone(),
                centre: *centre,
                factor: DVec3::splat(*k),
            },
            _ => return None,
        })
    }

    /// A left press with an edit tool.
    pub(super) fn edit_press(&mut self, s: DVec2, n_clicks: u32) {
        match self.tool.as_str() {
            PUSH => self.push_press(s, n_clicks),
            ERASE => {
                let mut edges = BTreeSet::new();
                if let Some(Hit::Edge(e)) = self.hover.map(|p| p.hit) {
                    edges.insert(e);
                }
                let guides = self.hover_guide.into_iter().collect();
                self.edit = Some(EditOp::Erase { edges, guides });
                self.overlay_dirty = true;
            }
            OFFSET => self.offset_press(s, n_clicks),
            TAPE => self.tape_press(n_clicks),
            PROTRACTOR => self.protractor_press(s),
            MOVE => self.move_press(s),
            ROTATE => self.rotate_press(s),
            SCALE => self.scale_press(s),
            _ => {}
        }
    }

    pub(super) fn edit_release(&mut self, s: DVec2) {
        let view = self.view();
        match &mut self.edit {
            Some(EditOp::Erase { edges, guides }) => {
                let selection = Selection {
                    edges: std::mem::take(edges),
                    guides: std::mem::take(guides),
                    ..Default::default()
                };
                self.edit = None;
                if !selection.is_empty() {
                    self.run(Command::Erase { selection });
                }
                self.overlay_dirty = true;
            }
            Some(
                EditOp::Rotate {
                    pressed,
                    u0,
                    centre,
                    normal,
                    ..
                }
                | EditOp::Protractor {
                    pressed,
                    u0,
                    centre,
                    normal,
                    ..
                },
            ) => {
                // Released after a drag from the centre: the drag sets the start arm.
                if let Some(p) = pressed.take()
                    && p.distance(s) >= DRAG_MIN
                    && u0.is_none()
                    && let Some(q) = plane_point(&view, s, *centre, *normal)
                    && q.distance(*centre) > 0.0
                {
                    *u0 = Some((q - *centre).normalize());
                    self.overlay_dirty = true;
                }
            }
            _ => {}
        }
    }

    fn push_press(&mut self, s: DVec2, n_clicks: u32) {
        if let Some(EditOp::Push { face, d, .. }) = self.edit.clone() {
            // A quick second click without pulling repeats the last distance.
            if n_clicks == 2
                && d.abs() <= self.scene.tol.dist
                && let Some(last) = self.last_push
            {
                self.edit = None;
                self.set_preview(None);
                self.commit_push(face, last.0, last.1);
                return;
            }
            self.finish_edit();
            return;
        }
        let view = self.view();
        let (o, dir) = view.ray(s);
        let Some((face, t)) = self.scene.ray_face(o, dir) else {
            return;
        };
        let n = self.scene.face(face).map_or(DVec3::Z, |f| f.normal);
        self.edit = Some(EditOp::Push {
            face,
            p0: o + dir * t,
            n,
            d: 0.0,
            new_loop: self.alt_loop,
            snap: None,
        });
        self.last_op = None;
        self.overlay_dirty = true;
    }

    fn commit_push(&mut self, face: FaceId, d: f64, new_loop: bool) {
        if self.run(Command::PushPull {
            face,
            distance: d,
            new_loop,
        }) {
            self.last_push = Some((d, new_loop));
            self.last_op = Some(LastOp::Push { face, d, new_loop });
        }
    }

    fn move_press(&mut self, s: DVec2) {
        if self.edit.is_some() {
            self.finish_edit();
            return;
        }
        let selection = self.working_selection(s, false);
        if selection.is_empty() {
            return;
        }
        let Some(i) = self.inference.clone() else {
            return;
        };
        let verts = selection.all_vertices(self.doc.model());
        self.edit = Some(EditOp::Move {
            from: i.point,
            selection,
            verts,
            delta: DVec3::ZERO,
            copy: false,
        });
        self.after_start();
    }

    fn rotate_press(&mut self, s: DVec2) {
        match &mut self.edit {
            None => {
                // The selection, or with nothing selected the item under
                // the cursor alone, as in SketchUp (DECISIONS #26): a
                // hovered face twists, and the faces attached to it fold.
                let selection = self.working_selection(s, false);
                if selection.is_empty() {
                    return;
                }
                let Some(i) = self.inference.clone() else {
                    return;
                };
                let normal = self.protractor_normal();
                let verts = selection.all_vertices(self.doc.model());
                self.edit = Some(EditOp::Rotate {
                    centre: i.point,
                    normal,
                    u0: None,
                    u1: None,
                    angle: 0.0,
                    selection,
                    verts,
                    copy: false,
                    pressed: Some(s),
                });
                self.after_start();
            }
            Some(EditOp::Rotate { u0, u1, .. }) if u0.is_none() => {
                if let Some(u) = *u1 {
                    *u0 = Some(u);
                    self.overlay_dirty = true;
                }
            }
            Some(_) => self.finish_edit(),
        }
    }

    fn protractor_press(&mut self, s: DVec2) {
        match &mut self.edit {
            None => {
                let Some(i) = self.inference.clone() else {
                    return;
                };
                let normal = self.protractor_normal();
                self.edit = Some(EditOp::Protractor {
                    centre: i.point,
                    normal,
                    u0: None,
                    u1: None,
                    angle: 0.0,
                    pressed: Some(s),
                });
                self.after_start();
            }
            Some(EditOp::Protractor { u0, u1, .. }) if u0.is_none() => {
                if let Some(u) = *u1 {
                    *u0 = Some(u);
                    self.overlay_dirty = true;
                }
            }
            Some(_) => self.finish_edit(),
        }
    }

    fn scale_press(&mut self, s: DVec2) {
        if self.edit.is_some() {
            self.finish_edit();
            return;
        }
        let selection = self.working_selection(s, true);
        if selection.is_empty() {
            return;
        }
        let Some(i) = self.inference.clone() else {
            return;
        };
        let m = self.doc.model();
        let verts = selection.all_vertices(m);
        let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        for v in &verts {
            lo = lo.min(m.vertex(*v));
            hi = hi.max(m.vertex(*v));
        }
        let centre = lo.lerp(hi, 0.5);
        let r0 = i.point.distance(centre);
        if r0 <= self.scene.tol.dist {
            return;
        }
        self.edit = Some(EditOp::Scale {
            centre,
            r0,
            k: 1.0,
            verts,
        });
        self.after_start();
    }

    fn after_start(&mut self) {
        self.last_op = None;
        self.lock = prism_infer::Lock::None;
        self.mem.clear();
        self.overlay_dirty = true;
        self.refresh();
    }

    /// Commit the step in progress.
    pub(super) fn finish_edit(&mut self) {
        let Some(e) = self.edit.take() else {
            return;
        };
        self.set_preview(None);
        self.typed.clear();
        self.lock = prism_infer::Lock::None;
        match e {
            EditOp::Push {
                face, d, new_loop, ..
            } => {
                if d.abs() > self.scene.tol.dist {
                    self.commit_push(face, d, new_loop);
                }
            }
            EditOp::Erase { .. } => {}
            ref t if self.finish_construct(t) => {}
            ref t => {
                let still = match t {
                    EditOp::Move { delta, .. } => delta.length() <= self.scene.tol.dist,
                    EditOp::Rotate { angle, .. } => angle.abs() < 1e-12,
                    EditOp::Scale { k, .. } => (*k - 1.0).abs() < 1e-12,
                    _ => true,
                };
                if !still
                    && let Some(c) = self.transform_command(t)
                    && self.run(c)
                {
                    self.last_op = Some(LastOp::Edit(t.clone()));
                }
            }
        }
        self.mem.clear();
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Esc during an edit step.
    pub(super) fn cancel_edit(&mut self) {
        if self.edit.take().is_some() {
            self.set_preview(None);
        }
    }

    /// Ctrl tapped: toggle copy (Move, Rotate).
    pub(super) fn toggle_copy(&mut self) {
        // Only while moving or rotating, so Ctrl+Z and other shortcuts do
        // not flip it (as in the feel test).
        if self.tool == TAPE {
            self.measure_only = !self.measure_only;
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        if let Some(EditOp::Move { copy, .. } | EditOp::Rotate { copy, .. }) = &mut self.edit {
            *copy = !*copy;
            self.ctrl_copy = *copy;
            self.update_preview();
            self.overlay_dirty = true;
        }
    }

    /// Alt tapped: toggle Push/Pull's new loop.
    pub(super) fn toggle_loop(&mut self) {
        if self.tool != PUSH {
            return;
        }
        self.alt_loop = !self.alt_loop;
        if let Some(EditOp::Push { new_loop, .. }) = &mut self.edit {
            *new_loop = self.alt_loop;
            self.update_preview();
        }
        self.overlay_dirty = true;
    }

    /// Enter with typed text while an edit step is in progress. Returns
    /// true if it was used.
    pub(super) fn edit_enter(&mut self, text: &str) -> bool {
        let nf = self.number;
        let Some(e) = self.edit.clone() else {
            return false;
        };
        if self.construct_enter(&e, text).is_some() {
            self.refresh();
            return true;
        }
        match e {
            EditOp::Push {
                face, d, new_loop, ..
            } => {
                let Some(v) = nf.length(text) else {
                    return true;
                };
                self.edit = None;
                self.set_preview(None);
                // The pull's side, and a typed minus turns it round.
                let sign = if d < 0.0 { -1.0 } else { 1.0 };
                self.commit_push(face, sign * v, new_loop);
            }
            EditOp::Move { mut delta, .. } => {
                let Some(v) = nf.length(text) else {
                    return true;
                };
                let mut dir = delta.normalize_or_zero();
                if let prism_infer::Lock::Axis(a) = self.lock {
                    let ax = AXES[a as usize];
                    dir = if dir.dot(ax) < 0.0 { -ax } else { ax };
                }
                if dir == DVec3::ZERO {
                    dir = DVec3::X;
                }
                delta = dir * v;
                if let Some(EditOp::Move { delta: d, .. }) = &mut self.edit {
                    *d = delta;
                }
                self.finish_edit();
            }
            EditOp::Rotate { u0: Some(_), .. } => {
                let Ok(deg) = text.trim().trim_end_matches('°').parse::<f64>() else {
                    return true;
                };
                if let Some(EditOp::Rotate { angle, .. }) = &mut self.edit {
                    *angle = deg.to_radians();
                }
                self.finish_edit();
            }
            EditOp::Scale { .. } => {
                let Ok(k) = text.trim().trim_end_matches(['x', '×']).parse::<f64>() else {
                    return true;
                };
                if k > 0.0 {
                    if let Some(EditOp::Scale { k: kk, .. }) = &mut self.edit {
                        *kk = k;
                    }
                    self.finish_edit();
                }
            }
            _ => return false,
        }
        self.refresh();
        true
    }

    /// Enter with typed text right after an edit: change it, or make an
    /// array after a copy (`x5`, `/5`). Returns true if it was used.
    pub(super) fn edit_retype(&mut self, text: &str) -> bool {
        let nf = self.number;
        let t = text.trim();
        if let Some(LastOp::Edit(e)) = &self.last_op
            && self.construct_retype(&e.clone(), t).is_some()
        {
            self.refresh();
            return true;
        }
        match self.last_op.clone() {
            Some(LastOp::Offset { face, d }) => {
                let Some(v) = nf.length(t) else { return true };
                if self.doc.undo() {
                    self.model_changed();
                }
                let sign = if d < 0.0 { -1.0 } else { 1.0 };
                self.commit_offset(face, sign * v.abs());
            }
            Some(LastOp::Push { face, d, new_loop }) => {
                let Some(v) = nf.length(t) else { return true };
                if self.doc.undo() {
                    self.model_changed();
                }
                let sign = if d < 0.0 { -1.0 } else { 1.0 };
                self.commit_push(face, sign * v.abs(), new_loop);
            }
            Some(LastOp::Edit(mut e)) => {
                // Arrays after a copy.
                if let EditOp::Move {
                    copy: true,
                    selection,
                    delta,
                    ..
                } = &e
                {
                    let (count, step) = if let Some(n) = t.strip_prefix(['x', '*']) {
                        let Ok(n) = n.trim().parse::<u32>() else {
                            return true;
                        };
                        (n, *delta)
                    } else if let Some(n) = t.strip_prefix('/') {
                        let Ok(n) = n.trim().parse::<u32>() else {
                            return true;
                        };
                        (n, *delta / f64::from(n.max(1)))
                    } else {
                        (0, DVec3::ZERO)
                    };
                    if count > 0 {
                        if self.doc.undo() {
                            self.model_changed();
                        }
                        self.run(Command::Copy {
                            selection: selection.clone(),
                            step: Motion::Translate { by: step },
                            count,
                        });
                        return true;
                    }
                }
                match &mut e {
                    EditOp::Move { delta, .. } => {
                        let Some(v) = nf.length(t) else { return true };
                        *delta = delta.normalize_or_zero() * v.abs();
                    }
                    EditOp::Rotate { angle, .. } => {
                        let Ok(deg) = t.trim_end_matches('°').parse::<f64>() else {
                            return true;
                        };
                        *angle = deg.to_radians();
                    }
                    EditOp::Scale { k, .. } => {
                        let Ok(f) = t.trim_end_matches(['x', '×']).parse::<f64>() else {
                            return true;
                        };
                        if f <= 0.0 {
                            return true;
                        }
                        *k = f;
                    }
                    _ => return false,
                }
                if self.doc.undo() {
                    self.model_changed();
                }
                if let Some(c) = self.transform_command(&e) {
                    self.run(c);
                    self.last_op = Some(LastOp::Edit(e));
                }
            }
            _ => return false,
        }
        self.refresh();
        true
    }

    /// The measurement box while an edit tool is active.
    pub(super) fn edit_measurement(&self) -> Option<(&'static str, String)> {
        if let Some(m) = self.construct_measurement() {
            return Some(m);
        }
        let nf = &self.number;
        Some(match (&self.edit, &self.last_op) {
            (Some(EditOp::Push { d, .. }), _) => ("Distance", nf.show(*d)),
            (Some(EditOp::Move { delta, .. }), _) => ("Distance", nf.show(delta.length())),
            (
                Some(EditOp::Rotate {
                    angle, u0: Some(_), ..
                }),
                _,
            ) => ("Angle", format!("{:.1}°", angle.to_degrees())),
            (Some(EditOp::Rotate { .. }), _) => ("Angle", String::new()),
            (Some(EditOp::Scale { k, .. }), _) => ("Scale", format!("{k:.3}×")),
            (None, Some(LastOp::Push { d, .. })) => ("Distance", nf.show(*d)),
            (None, Some(LastOp::Edit(EditOp::Move { delta, .. }))) => {
                ("Distance", nf.show(delta.length()))
            }
            (None, Some(LastOp::Edit(EditOp::Rotate { angle, .. }))) => {
                ("Angle", format!("{:.1}°", angle.to_degrees()))
            }
            (None, Some(LastOp::Edit(EditOp::Scale { k, .. }))) => ("Scale", format!("{k:.3}×")),
            (None, _) if self.tool == PUSH => (
                "Distance",
                self.last_push.map_or(String::new(), |(d, _)| nf.show(d)),
            ),
            _ => return None,
        })
    }

    pub(super) fn edit_hint(&self) -> Option<&'static str> {
        if let Some(h) = self.construct_hint() {
            return Some(h);
        }
        Some(match (self.tool.as_str(), &self.edit) {
            (PUSH, Some(_)) => {
                "Move to pull · click to finish · type a distance + Enter · Alt toggles new loop · Esc cancels"
            }
            (PUSH, None) => {
                "Click a face to push or pull it · double-click repeats the last distance · Alt toggles new loop"
            }
            (MOVE, Some(_)) => {
                "Move to place · click to finish · Ctrl copies · arrows lock an axis · type a distance + Enter · Esc cancels"
            }
            (MOVE, None) => {
                "Click a point on the selection (or on what you point at) to move it · Ctrl copies, then type x5 or /5 for an array"
            }
            (ROTATE, Some(EditOp::Rotate { u0: None, .. })) => {
                "Click (or drag from the centre and release) to set the start direction"
            }
            (ROTATE, Some(_)) => {
                "Move to rotate (15° snaps, Shift for 45°) · click to finish · Ctrl copies · type degrees + Enter"
            }
            (ROTATE, None) => {
                "Click the rotation centre · Up locks the protractor to the face under it, then move it anywhere · Right and Left lock red and green"
            }
            (SCALE, Some(_)) => {
                "Move to scale about the centre · click to finish · type a factor + Enter · Esc cancels"
            }
            (SCALE, None) => "Click a point to scale from (about the selection's centre)",
            (ERASE, _) => "Click or drag over edges to erase them (with the faces they bound)",
            _ => return None,
        })
    }

    /// What the edit tools draw over the model.
    pub(super) fn edit_overlay(&self, o: &mut Overlay) {
        let fb = &self.style.feedback;
        let view = self.view();
        let px = |p: DVec3| view.to_screen(p).map_or(0.0, |(_, d)| view.units_per_px(d));
        match &self.edit {
            Some(EditOp::Push {
                p0,
                n,
                d,
                new_loop,
                snap,
                ..
            }) => {
                let e = *p0 + *n * *d;
                o.lines.push(Line {
                    a: *p0,
                    b: e,
                    color: fb.rubber_band,
                    width: 1.4,
                    dash: [4.0, 4.0],
                });
                let mut text = self.number.show(*d);
                if *new_loop {
                    text.push_str("  New loop");
                }
                if let Some((label, from)) = snap {
                    o.lines.push(Line {
                        a: *from,
                        b: e,
                        color: fb.parallel,
                        width: fb.guide_width,
                        dash: fb.guide_dash,
                    });
                    o.marks.push(Mark {
                        at: e,
                        shape: Shape::Circle,
                        color: fb.endpoint,
                        size: fb.glyph_size,
                    });
                    text.push_str("  ");
                    text.push_str(label);
                }
                o.labels.push(Label {
                    at: e,
                    offset: [14.0, -16.0],
                    text,
                    color: fb.pill,
                });
            }
            Some(EditOp::Move {
                from, delta, copy, ..
            }) => {
                let to = *from + *delta;
                o.lines.push(Line {
                    a: *from,
                    b: to,
                    color: fb.rubber_band,
                    width: 1.4,
                    dash: [4.0, 4.0],
                });
                o.labels.push(Label {
                    at: to,
                    offset: [14.0, -16.0],
                    text: format!(
                        "{}{}",
                        if *copy { "Copy " } else { "" },
                        self.number.show(delta.length())
                    ),
                    color: fb.pill,
                });
            }
            Some(EditOp::Scale { centre, k, .. }) => {
                if let Some(i) = &self.inference {
                    o.lines.push(Line {
                        a: *centre,
                        b: i.point,
                        color: fb.rubber_band,
                        width: 1.4,
                        dash: [4.0, 4.0],
                    });
                }
                o.marks.push(Mark {
                    at: *centre,
                    shape: Shape::Circle,
                    color: self.style.selected,
                    size: 4.5,
                });
                o.labels.push(Label {
                    at: *centre,
                    offset: [12.0, -16.0],
                    text: format!("× {k:.3}"),
                    color: fb.pill,
                });
            }
            Some(EditOp::Erase { edges, .. }) => {
                for e in edges.iter().filter_map(|e| self.scene.edge(*e)) {
                    o.lines.push(Line {
                        a: e.a,
                        b: e.b,
                        color: fb.erase,
                        width: fb.target_width,
                        dash: [0.0, 0.0],
                    });
                }
            }
            _ => {}
        }
        if self.tool == ERASE
            && let Some(Hit::Edge(e)) = self.hover.map(|p| p.hit)
            && let Some(e) = self.scene.edge(e)
        {
            o.lines.push(Line {
                a: e.a,
                b: e.b,
                color: fb.erase,
                width: fb.target_width,
                dash: [0.0, 0.0],
            });
        }
        self.construct_overlay(o);
        // The protractor.
        if self.tool == ROTATE || self.tool == PROTRACTOR {
            let (c, n, u0, u1, angle, copy) = match &self.edit {
                Some(EditOp::Rotate {
                    centre,
                    normal,
                    u0,
                    u1,
                    angle,
                    copy,
                    ..
                }) => (*centre, *normal, *u0, *u1, *angle, *copy),
                Some(EditOp::Protractor {
                    centre,
                    normal,
                    u0,
                    u1,
                    angle,
                    ..
                }) => (*centre, *normal, *u0, *u1, *angle, false),
                _ => match &self.inference {
                    Some(i) => (i.point, self.protractor_normal(), None, None, 0.0, false),
                    None => return,
                },
            };
            let r = px(c) * PROTRACTOR_RADIUS;
            if r <= 0.0 {
                return;
            }
            let (a, b) = rect_basis(n);
            let pt = |t: f64| c + (a * t.cos() + b * t.sin()) * r;
            let seg = 72;
            for k in 0..seg {
                let (t0, t1) = (
                    f64::from(k) / f64::from(seg) * std::f64::consts::TAU,
                    f64::from(k + 1) / f64::from(seg) * std::f64::consts::TAU,
                );
                o.lines.push(Line {
                    a: pt(t0),
                    b: pt(t1),
                    color: fb.protractor,
                    width: 1.2,
                    dash: [0.0, 0.0],
                });
            }
            for k in 0..24 {
                let t = f64::from(k) / 24.0 * std::f64::consts::TAU;
                let inner = if k % 6 == 0 { 0.86 } else { 0.93 };
                o.lines.push(Line {
                    a: c.lerp(pt(t), inner),
                    b: pt(t),
                    color: fb.protractor,
                    width: 1.0,
                    dash: [0.0, 0.0],
                });
            }
            o.marks.push(Mark {
                at: c,
                shape: Shape::Circle,
                color: self.style.selected,
                size: 4.5,
            });
            if let Some(l) = self.rot_lock {
                let _ = l;
                o.labels.push(Label {
                    at: c,
                    offset: [12.0, 22.0],
                    text: "Plane locked".into(),
                    color: fb.pill,
                });
            }
            match (u0, u1) {
                (None, Some(u)) => o.lines.push(Line {
                    a: c,
                    b: c + u * r,
                    color: fb.rubber_band,
                    width: 1.6,
                    dash: [0.0, 0.0],
                }),
                (Some(s0), _) => {
                    o.lines.push(Line {
                        a: c,
                        b: c + s0 * r,
                        color: fb.rubber_band,
                        width: 1.6,
                        dash: [0.0, 0.0],
                    });
                    let now = rotate_point(c + s0 * r, c, n, angle);
                    o.lines.push(Line {
                        a: c,
                        b: now,
                        color: self.style.selected,
                        width: 2.0,
                        dash: [0.0, 0.0],
                    });
                    let steps = 24;
                    for k in 0..steps {
                        let p0 = rotate_point(
                            c + s0 * r * 0.5,
                            c,
                            n,
                            angle * f64::from(k) / f64::from(steps),
                        );
                        let p1 = rotate_point(
                            c + s0 * r * 0.5,
                            c,
                            n,
                            angle * f64::from(k + 1) / f64::from(steps),
                        );
                        o.tris.push(Tri {
                            p: [c, p0, p1],
                            color: self.style.selected.with_alpha(alpha(0.18)),
                        });
                    }
                    o.labels.push(Label {
                        at: now,
                        offset: [12.0, -14.0],
                        text: format!(
                            "{:.1}°{}",
                            angle.to_degrees(),
                            if copy { " copy" } else { "" }
                        ),
                        color: fb.pill,
                    });
                }
                _ => {}
            }
        }
    }
}

/// The edge under the cursor (the Eraser picks only edges).
fn edge_pick(
    scene: &prism_view::Scene,
    view: &View,
    s: DVec2,
    aperture: f64,
) -> Option<prism_view::Picked> {
    let (o, d) = view.ray(s);
    let mut best: Option<(f64, EdgeId, DVec3)> = None;
    for e in &scene.edges {
        let u = e.b - e.a;
        let Some(t) = along_line(o, d, e.a, u) else {
            continue;
        };
        let p = e.a + u * t.clamp(0.0, 1.0);
        let Some((q, _)) = view.to_screen(p) else {
            continue;
        };
        let dist = q.distance(s);
        if dist < pick::EDGE_RADIUS * aperture
            && best.is_none_or(|b| dist < b.0)
            && !scene.occluded(view.eye, p)
        {
            best = Some((dist, e.id, p));
        }
    }
    best.map(|(_, id, p)| prism_view::Picked {
        hit: Hit::Edge(id),
        point: p,
    })
}
