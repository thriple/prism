//! The viewport: the document, the camera, hover and selection, and the
//! mouse and keys that move them (brief 04).
//!
//! The maths lives in `prism-render` (camera, picking, clicks) and is
//! tested there. This module turns egui's input into those calls and keeps
//! track of what has to be drawn again.

use egui::{Context, PointerButton, Pos2, Rect, Response};
use prism_doc::{Command, Document};
use prism_math::{DVec2, DVec3};
use prism_ops::select::Selection;
use prism_render::camera::{
    self, Camera, EASE_EXTENTS, EASE_WHEEL, MIN_DISTANCE_TOLS, Motion, View, ZOOM_PER_DRAG_PX,
};
use prism_render::clicks::{ClickSeries, click_select};
use prism_render::gpu::Highlight;
use prism_render::pick::{self, Hit, Picked};
use prism_render::style::HiddenLines;
use prism_render::{Scene, Style};
use prism_topo::Model;

use crate::actions::registry::Selection as Counts;
use crate::chrome::Chrome;

/// Test scenes (Help > Test Scenes).
pub mod scenes;

/// A navigation drag in progress.
#[derive(Clone, Copy, Debug)]
enum Drag {
    Orbit { pivot: DVec3 },
    Pan { grab: DVec3 },
    Zoom { toward: DVec3 },
}

/// The navigation tools (PLAN 1.5a and 1.8).
const ORBIT: &str = "cam.orbit";
const PAN: &str = "cam.pan";
const ZOOM: &str = "cam.zoom";
const SELECT: &str = "tool.select";

/// Settings shown in the debug panel.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    /// Scales the pick radii (the prototype's slider, 0.5 to 2.5).
    pub aperture: f64,
    /// Ease wheel zoom and Zoom Extents (off when reduce motion is on).
    pub ease: bool,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            aperture: 1.0,
            ease: true,
        }
    }
}

/// Timings for the debug panel, in milliseconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct Timings {
    pub pick_ms: f64,
    pub scene_ms: f64,
}

pub struct Viewport {
    doc: Document,
    /// Counts model changes; the scene and the GPU follow it.
    version: u64,
    scene: Scene,
    scene_version: u64,
    uploaded_version: Option<u64>,
    camera: Camera,
    motion: Option<Motion>,
    hover: Option<Picked>,
    selection: Selection,
    highlight_dirty: bool,
    clicks: ClickSeries,
    middle_clicks: ClickSeries,
    drag: Option<Drag>,
    /// The tool to go back to when Esc leaves Orbit, Pan or Zoom.
    before_nav: &'static str,
    pub style: Style,
    pub tuning: Tuning,
    /// Whether the 3D picture needs drawing again.
    dirty: bool,
    /// The viewport's rectangle on screen, in points.
    rect: Rect,
    pub timings: Timings,
}

impl Viewport {
    pub fn new() -> Self {
        let doc = scenes::cube();
        let scene = Scene::build(doc.model());
        Self {
            doc,
            version: 1,
            scene,
            scene_version: 1,
            uploaded_version: None,
            camera: Camera::home(),
            motion: None,
            hover: None,
            selection: Selection::default(),
            highlight_dirty: true,
            clicks: ClickSeries::default(),
            middle_clicks: ClickSeries::default(),
            drag: None,
            before_nav: SELECT,
            style: Style::default(),
            tuning: Tuning::default(),
            dirty: true,
            rect: Rect::NOTHING,
            timings: Timings::default(),
        }
    }

    pub fn model(&self) -> &Model {
        self.doc.model()
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn camera(&self) -> Camera {
        self.motion.as_ref().map_or(self.camera, Motion::current)
    }

    pub fn highlight(&self) -> Highlight {
        Highlight {
            hover: self.hover.map(|p| p.hit),
            selection: self.selection.clone(),
        }
    }

    /// The scene, if the GPU does not have this version of it yet.
    pub fn take_upload(&mut self) -> Option<&Scene> {
        if self.uploaded_version == Some(self.scene_version) {
            return None;
        }
        self.uploaded_version = Some(self.scene_version);
        Some(&self.scene)
    }

    /// Whether the highlight changed since the last call.
    pub fn take_highlight_change(&mut self) -> bool {
        std::mem::take(&mut self.highlight_dirty)
    }

    /// Whether the picture must be drawn again, and clears the flag.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn view(&self) -> View {
        View::new(
            &self.camera(),
            f64::from(self.rect.width()),
            f64::from(self.rect.height()),
        )
    }

    fn local(&self, p: Pos2) -> DVec2 {
        DVec2::new(
            f64::from(p.x - self.rect.left()),
            f64::from(p.y - self.rect.top()),
        )
    }

    fn min_distance(&self) -> f64 {
        self.scene.tol.dist * MIN_DISTANCE_TOLS
    }

    /// The point under the cursor for zoom, orbit and pan.
    fn surface(&self, view: &View, s: DVec2) -> DVec3 {
        let fallback = pick::fallback_distance(&self.scene, view, self.camera.target);
        camera::surface_point(view, s, pick::model_point(&self.scene, view, s), fallback)
    }

    /// Settle any ease, so a drag starts from where the camera really is.
    fn settle(&mut self) {
        if let Some(m) = self.motion.take() {
            self.camera = m.goal();
        }
    }

    fn set_camera(&mut self, c: Camera) {
        self.camera = c;
        self.dirty = true;
    }

    fn ease_to(&mut self, goal: Camera, duration: f64, reduce_motion: bool) {
        let now = self.camera();
        if self.tuning.ease && !reduce_motion {
            self.motion = Some(Motion::new(now, goal, duration));
        } else {
            self.motion = None;
        }
        self.camera = goal;
        self.dirty = true;
    }

    /// The model changed: rebuild the scene and drop what no longer exists.
    fn model_changed(&mut self) {
        self.version += 1;
        let start = web_time::Instant::now();
        self.scene = Scene::build(self.doc.model());
        self.timings.scene_ms = start.elapsed().as_secs_f64() * 1e3;
        self.scene_version = self.version;
        let m = self.doc.model();
        self.selection.vertices.retain(|v| m.has_vertex(*v));
        self.selection.edges.retain(|e| m.edge(*e).is_some());
        self.selection.faces.retain(|f| m.face(*f).is_some());
        self.hover = None;
        self.highlight_dirty = true;
        self.dirty = true;
    }

    fn load(&mut self, doc: Document, reduce_motion: bool) {
        self.doc = doc;
        self.selection = Selection::default();
        self.model_changed();
        if let Some((lo, hi)) = self.doc.model().bbox() {
            let goal = Camera::home().extents(lo, hi, self.view().aspect());
            self.ease_to(goal, 0.0, reduce_motion);
        }
    }

    fn set_selection(&mut self, s: Selection) {
        if s != self.selection {
            self.selection = s;
            self.highlight_dirty = true;
            self.dirty = true;
        }
    }

    fn set_hover(&mut self, h: Option<Picked>) {
        if h.map(|p| p.hit) != self.hover.map(|p| p.hit) {
            self.highlight_dirty = true;
            self.dirty = true;
        }
        self.hover = h;
    }

    /// How many of each thing are selected, for the context menu and rules.
    pub fn counts(&self) -> Counts {
        Counts {
            faces: self.selection.faces.len(),
            edges: self.selection.edges.len(),
            points: self.selection.vertices.len(),
        }
    }

    pub fn can_undo(&self) -> bool {
        self.doc.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.doc.can_redo()
    }

    /// Handle an action from the menus, dock, keys or search.
    pub fn action(&mut self, id: &str, reduce_motion: bool) {
        match id {
            "cam.extents" => {
                if let Some((lo, hi)) = self.scene.bounds {
                    let goal = self.camera.extents(lo, hi, self.view().aspect());
                    self.ease_to(goal, EASE_EXTENTS, reduce_motion);
                }
            }
            "edit.selall" => {
                let m = self.doc.model();
                let s = Selection {
                    vertices: Default::default(),
                    edges: m.edges().map(|(id, _)| id).collect(),
                    faces: m.faces().map(|(id, _)| id).collect(),
                };
                self.set_selection(s);
            }
            "edit.desel" => self.set_selection(Selection::default()),
            "edit.delete" => {
                if !self.selection.is_empty() {
                    let selection = std::mem::take(&mut self.selection);
                    if self.doc.run(Command::Erase { selection }) {
                        self.model_changed();
                    }
                }
            }
            "edit.undo" => {
                if self.doc.undo() {
                    self.model_changed();
                }
            }
            "edit.redo" => {
                if self.doc.redo() {
                    self.model_changed();
                }
            }
            "file.new" => self.load(Document::new(), reduce_motion),
            "view.shadow" => self.dirty = true,
            _ => {
                if let Some(doc) = scenes::by_action(id) {
                    self.load(doc, reduce_motion);
                }
            }
        }
    }

    /// Note the tool picked before a navigation tool, for Esc.
    pub fn tool_changed(&mut self, from: &'static str, to: &'static str) {
        let nav = |t: &str| matches!(t, ORBIT | PAN | ZOOM);
        if nav(to) && !nav(from) {
            self.before_nav = from;
        }
        self.drag = None;
        if self.hover.is_some() && to != SELECT {
            self.set_hover(None);
        }
    }

    /// Esc with nothing else to close: leave Orbit, Pan or Zoom.
    pub fn escape(&mut self, chrome: &mut Chrome) -> bool {
        if matches!(chrome.active_tool, ORBIT | PAN | ZOOM) {
            let back = self.before_nav;
            chrome.active_tool = back;
            self.drag = None;
            return true;
        }
        false
    }

    /// Advance an ease. Returns true while the camera is still moving.
    pub fn step(&mut self, dt: f64) -> bool {
        if let Some(m) = &mut self.motion {
            let moving = m.step(dt);
            self.dirty = true;
            if !moving {
                self.motion = None;
            }
            return moving;
        }
        false
    }

    /// Mouse and wheel input over the viewport this frame.
    pub fn input(
        &mut self,
        ctx: &Context,
        resp: &Response,
        rect: Rect,
        chrome: &mut Chrome,
        reduce_motion: bool,
    ) {
        if rect != self.rect {
            self.rect = rect;
            self.dirty = true;
        }
        let tool = chrome.active_tool;
        let over = resp.hovered() || resp.is_pointer_button_down_on();
        let (events, time, pointer, delta) = ctx.input(|i| {
            (
                i.events.clone(),
                i.time,
                i.pointer.latest_pos(),
                i.pointer.delta(),
            )
        });
        for e in &events {
            match e {
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } => {
                    if *pressed && rect.contains(*pos) && over {
                        self.press(*pos, *button, modifiers.shift, time, tool, chrome);
                    } else if !*pressed {
                        self.drag = None;
                    }
                }
                egui::Event::MouseWheel {
                    unit,
                    delta: d,
                    modifiers,
                    ..
                } if over => {
                    let Some(p) = pointer else { continue };
                    let s = self.local(p);
                    self.settle_for_wheel();
                    let view = self.view();
                    let q = self.surface(&view, s);
                    match unit {
                        egui::MouseWheelUnit::Point if modifiers.ctrl || modifiers.command => {
                            // A trackpad pinch, as the browser reports it.
                            let goal = self.camera.pinch(
                                q,
                                (f64::from(d.y) * 0.01).exp(),
                                self.min_distance(),
                            );
                            self.set_camera(goal);
                        }
                        egui::MouseWheelUnit::Point => {
                            // Two-finger scroll on a trackpad pans.
                            let goal = self.camera.pan(
                                &view,
                                q,
                                DVec2::new(f64::from(d.x), f64::from(d.y)),
                            );
                            self.set_camera(goal);
                        }
                        egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => {
                            let notches = f64::from(d.y);
                            let goal = self.camera.wheel(q, notches, self.min_distance());
                            self.ease_to(goal, EASE_WHEEL, reduce_motion);
                        }
                    }
                }
                egui::Event::Zoom(f) if over => {
                    let Some(p) = pointer else { continue };
                    self.settle();
                    let view = self.view();
                    let q = self.surface(&view, self.local(p));
                    let goal = self.camera.pinch(q, f64::from(*f), self.min_distance());
                    self.set_camera(goal);
                }
                _ => {}
            }
        }
        // Drags.
        if let Some(drag) = self.drag
            && (delta.x != 0.0 || delta.y != 0.0)
        {
            let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
            let c = match drag {
                Drag::Orbit { pivot } => self.camera.orbit_drag(pivot, dx, dy),
                Drag::Pan { grab } => self.camera.pan(&self.view(), grab, DVec2::new(dx, dy)),
                Drag::Zoom { toward } => self.camera.zoom_toward(toward, -dy * ZOOM_PER_DRAG_PX),
            };
            self.set_camera(c);
        }
        // Hover: only with the Select tool and no drag.
        if self.drag.is_none() && tool == SELECT {
            let h = match pointer {
                Some(p) if over && rect.contains(p) => {
                    let start = web_time::Instant::now();
                    let s = self.local(p);
                    let got = pick::pick(&self.scene, &self.view(), s, self.tuning.aperture);
                    self.timings.pick_ms = start.elapsed().as_secs_f64() * 1e3;
                    got
                }
                _ => None,
            };
            self.set_hover(h);
        } else if self.hover.is_some() {
            self.set_hover(None);
        }
        if resp.secondary_clicked() {
            // Right-click selects what is under the cursor first.
            if let Some(h) = self.hover {
                let has = match h.hit {
                    Hit::Vertex(v) => self.selection.vertices.contains(&v),
                    Hit::Edge(e) => self.selection.edges.contains(&e),
                    Hit::Face(f) => self.selection.faces.contains(&f),
                };
                if !has {
                    self.set_selection(click_select(self.doc.model(), Some(h.hit), 1));
                }
            }
            if let Some(p) = resp.interact_pointer_pos() {
                chrome.open_context(p, self.counts());
            }
        }
    }

    /// Wheel zoom during an ease builds on the goal, so fast scrolling adds up.
    fn settle_for_wheel(&mut self) {
        if let Some(m) = &self.motion {
            self.camera = m.goal();
        }
    }

    fn press(
        &mut self,
        pos: Pos2,
        button: PointerButton,
        shift: bool,
        time: f64,
        tool: &str,
        chrome: &mut Chrome,
    ) {
        let s = self.local(pos);
        match button {
            PointerButton::Middle => {
                self.settle();
                let view = self.view();
                let q = self.surface(&view, s);
                if self.middle_clicks.press(time, s) == 2 {
                    // Double middle-click: re-centre on the point.
                    let goal = self.camera.recentre(q);
                    self.set_camera(goal);
                    self.drag = None;
                    return;
                }
                self.drag = Some(if shift {
                    Drag::Pan { grab: q }
                } else {
                    Drag::Orbit { pivot: q }
                });
            }
            PointerButton::Primary => {
                if matches!(tool, ORBIT | PAN | ZOOM) {
                    self.settle();
                    let view = self.view();
                    let q = self.surface(&view, s);
                    self.drag = Some(match tool {
                        ORBIT if !shift => Drag::Orbit { pivot: q },
                        ZOOM => Drag::Zoom { toward: q },
                        _ => Drag::Pan { grab: q },
                    });
                } else if tool == SELECT {
                    chrome.menu = None;
                    let n = self.clicks.press(time, s);
                    let hit = pick::pick(&self.scene, &self.view(), s, self.tuning.aperture)
                        .map(|p| p.hit);
                    let sel = click_select(self.doc.model(), hit, n);
                    self.set_selection(sel);
                }
            }
            _ => {}
        }
    }

    /// Whether lines hidden behind faces are drawn (debug setting).
    pub fn hidden_lines_index(&self) -> usize {
        match self.style.hidden_lines {
            HiddenLines::Off => 0,
            HiddenLines::Dashed => 1,
            HiddenLines::Faint => 2,
        }
    }

    pub fn set_hidden_lines_index(&mut self, i: usize) {
        let h = [HiddenLines::Off, HiddenLines::Dashed, HiddenLines::Faint][i.min(2)];
        if h != self.style.hidden_lines {
            self.style.hidden_lines = h;
            self.dirty = true;
        }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}
