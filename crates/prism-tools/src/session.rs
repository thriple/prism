//! The viewport session: the document, the camera, the tools, the inference
//! memory, the selection and the measurement box, driven by a stream of
//! [`Event`]s.
//!
//! Pure: the app turns mouse and keys into events with their times, and
//! reads back what to draw. The same events replay to the same model, which
//! is how recorded sessions become tests (PLAN 5).

use prism_doc::{Command, Document};
use prism_infer::{self as infer, AXES, Context, Inference, Kind, Lock, Memory, Settings};
use prism_math::{DVec2, DVec3};
use prism_ops::select::Selection;
use prism_topo::Model;
use prism_view::Scene;
use prism_view::camera::{
    self, Camera, EASE_EXTENTS, EASE_WHEEL, MIN_DISTANCE_TOLS, Motion, View, ZOOM_PER_DRAG_PX,
};
use prism_view::clicks::{ClickSeries, click_select};
use prism_view::overlay::{Label, Line, Mark, Overlay, ScreenBox, Shape, Tri};
use prism_view::pick::{self, Hit, Picked};
use prism_view::style::{Rgba, Style, alpha};
use serde::{Deserialize, Serialize};

use crate::select::box_select;
use crate::units::NumberFormat;

/// Tool ids (the action registry's).
pub const SELECT: &str = "tool.select";
pub const LINE: &str = "tool.line";
pub const RECT: &str = "tool.rect";
pub const ORBIT: &str = "cam.orbit";
pub const PAN: &str = "cam.pan";
pub const ZOOM: &str = "cam.zoom";

fn is_nav(t: &str) -> bool {
    matches!(t, ORBIT | PAN | ZOOM)
}

/// A box drag shorter than this (points) is a click.
pub const BOX_MIN: f64 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Button {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Key {
    Enter,
    Escape,
    Backspace,
    Up,
    Down,
    Left,
    Right,
}

/// One input, in viewport points (0,0 at the viewport's top left).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Input {
    Resize {
        w: f64,
        h: f64,
    },
    Move {
        x: f64,
        y: f64,
    },
    Press {
        button: Button,
        x: f64,
        y: f64,
        shift: bool,
        ctrl: bool,
    },
    Release {
        button: Button,
    },
    /// A mouse wheel, in notches (positive zooms in).
    Wheel {
        x: f64,
        y: f64,
        notches: f64,
    },
    /// Two-finger trackpad scroll, in points.
    Scroll {
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
    },
    Pinch {
        x: f64,
        y: f64,
        factor: f64,
    },
    Key {
        key: Key,
    },
    ShiftDown,
    ShiftUp,
    /// Characters for the measurement box.
    Text {
        text: String,
    },
    Tool {
        id: String,
    },
    Action {
        id: String,
    },
    /// Only time passes (easing, waking points).
    Time,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Seconds.
    pub t: f64,
    pub input: Input,
}

/// A recorded session: where it started, what happened, and the model it
/// ended with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recording {
    pub model: Model,
    pub camera: Camera,
    pub tool: String,
    pub size: (f64, f64),
    pub settings: Settings,
    pub events: Vec<Event>,
    /// The model's geometry at the end (`Model::geometry_json`).
    pub result: String,
}

/// What the app should do after an event.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Response {
    /// Open the right-click menu here (viewport points).
    pub context_menu: Option<DVec2>,
    /// The active tool changed (Esc left Orbit, Pan or Zoom; a line closed).
    pub tool: Option<String>,
    /// Esc was used.
    pub used_escape: bool,
}

#[derive(Clone, Copy, Debug)]
enum Drag {
    Orbit { pivot: DVec3 },
    Pan { grab: DVec3 },
    Zoom { toward: DVec3 },
}

#[derive(Clone, Copy, Debug)]
struct RectOp {
    start: DVec3,
    normal: DVec3,
    u: DVec3,
    v: DVec3,
    du: f64,
    dv: f64,
}

impl RectOp {
    fn corners(&self) -> [DVec3; 4] {
        let s = self.start;
        let (a, b) = (self.u * self.du, self.v * self.dv);
        [s, s + a, s + a + b, s + b]
    }
}

#[derive(Clone, Copy, Debug)]
struct BoxOp {
    down: DVec2,
    now: DVec2,
    pre: usize,
    shift: bool,
    ctrl: bool,
}

/// The last operation, which typing can still change (Appendix A ★).
#[derive(Clone, Copy, Debug)]
enum LastOp {
    Line { from: DVec3, dir: DVec3, len: f64 },
    Rect { op: RectOp },
}

#[derive(Clone, Debug)]
pub struct Timings {
    pub pick_ms: f64,
    pub infer_ms: f64,
    pub scene_ms: f64,
}

pub struct Session {
    doc: Document,
    version: u64,
    scene: Scene,
    camera: Camera,
    motion: Option<Motion>,
    size: (f64, f64),
    time: f64,
    cursor: Option<DVec2>,
    tool: String,
    before_nav: String,
    hover: Option<Picked>,
    selection: Selection,
    /// Selections saved at the start of a box drag.
    saved: Vec<Selection>,
    clicks: ClickSeries,
    middle_clicks: ClickSeries,
    drag: Option<Drag>,
    boxing: Option<BoxOp>,
    line_start: Option<DVec3>,
    rect: Option<RectOp>,
    rect_memory: Option<(f64, f64)>,
    last_op: Option<LastOp>,
    pub settings: Settings,
    mem: Memory,
    lock: Lock,
    shift_locked: bool,
    inference: Option<Inference>,
    typed: String,
    pub number: NumberFormat,
    pub style: Style,
    pub ease: bool,
    pub reduce_motion: bool,
    dirty: bool,
    highlight_dirty: bool,
    overlay_dirty: bool,
    recording: Option<Recording>,
    timings: Timings,
    /// Time measurements are taken by the app; the session never reads a clock.
    pub measure: Option<fn() -> f64>,
}

impl Session {
    pub fn new(doc: Document) -> Self {
        let scene = Scene::build(doc.model());
        Self {
            doc,
            version: 1,
            scene,
            camera: Camera::home(),
            motion: None,
            size: (800.0, 600.0),
            time: 0.0,
            cursor: None,
            tool: SELECT.to_owned(),
            before_nav: SELECT.to_owned(),
            hover: None,
            selection: Selection::default(),
            saved: Vec::new(),
            clicks: ClickSeries::default(),
            middle_clicks: ClickSeries::default(),
            drag: None,
            boxing: None,
            line_start: None,
            rect: None,
            rect_memory: None,
            last_op: None,
            settings: Settings::default(),
            mem: Memory::default(),
            lock: Lock::None,
            shift_locked: false,
            inference: None,
            typed: String::new(),
            number: NumberFormat::default(),
            style: Style::default(),
            ease: true,
            reduce_motion: false,
            dirty: true,
            highlight_dirty: true,
            overlay_dirty: true,
            recording: None,
            timings: Timings {
                pick_ms: 0.0,
                infer_ms: 0.0,
                scene_ms: 0.0,
            },
            measure: None,
        }
    }

    // ---------- reading ----------

    pub fn model(&self) -> &Model {
        self.doc.model()
    }
    pub fn document(&self) -> &Document {
        &self.doc
    }
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
    /// Changes whenever the model does.
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn camera(&self) -> Camera {
        self.motion.as_ref().map_or(self.camera, Motion::current)
    }
    pub fn tool(&self) -> &str {
        &self.tool
    }
    pub fn selection(&self) -> &Selection {
        &self.selection
    }
    pub fn hover(&self) -> Option<Hit> {
        self.hover.map(|p| p.hit)
    }
    pub fn inference(&self) -> Option<&Inference> {
        self.inference.as_ref()
    }
    pub fn timings(&self) -> &Timings {
        &self.timings
    }
    pub fn view(&self) -> View {
        View::new(&self.camera(), self.size.0, self.size.1)
    }
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }
    /// Whether a tool step is in progress (a line or rectangle started).
    pub fn in_operation(&self) -> bool {
        self.line_start.is_some() || self.rect.is_some() || self.boxing.is_some()
    }
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }
    pub fn take_highlight_dirty(&mut self) -> bool {
        std::mem::take(&mut self.highlight_dirty)
    }
    pub fn take_overlay_dirty(&mut self) -> bool {
        std::mem::take(&mut self.overlay_dirty)
    }
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    /// Whether the camera is still easing (keep drawing frames).
    pub fn animating(&self) -> bool {
        self.motion.is_some()
    }
    /// When a hovered point will wake, so the app can wake the session then.
    pub fn next_wake(&self) -> Option<f64> {
        if self.wants_inference() {
            self.mem.wakes_at(&self.settings)
        } else {
            None
        }
    }

    /// Whether this character goes to the measurement box rather than to a
    /// shortcut.
    pub fn accepts_char(&self, c: char) -> bool {
        if !self.typed.is_empty() {
            return !c.is_control();
        }
        let editable = self.in_operation() && self.boxing.is_none()
            || self.last_op.is_some() && self.typing_tool();
        editable && (c.is_ascii_digit() || c == '.' || c == '-' || c == self.number.decimal)
    }

    fn typing_tool(&self) -> bool {
        self.tool == LINE || self.tool == RECT
    }

    /// The measurement box: its label, value, and whether it shows typing.
    pub fn measurement(&self) -> (String, String, bool) {
        let nf = &self.number;
        let (label, value) = if let (Some(start), Some(i)) = (self.line_start, &self.inference) {
            ("Length", nf.show(start.distance(i.point)))
        } else if let Some(r) = &self.rect {
            ("Size", nf.show_pair(r.du.abs(), r.dv.abs()))
        } else if self.tool == RECT {
            match self.rect_memory {
                Some((w, h)) => ("Size", nf.show_pair(w, h)),
                None => ("Size", String::new()),
            }
        } else {
            match (&self.last_op, self.typing_tool()) {
                (Some(LastOp::Line { len, .. }), true) => ("Length", nf.show(*len)),
                (Some(LastOp::Rect { op }), true) => {
                    ("Size", nf.show_pair(op.du.abs(), op.dv.abs()))
                }
                _ => ("", String::new()),
            }
        };
        if !self.typed.is_empty() {
            return (label.to_owned(), self.typed.clone(), true);
        }
        let value = if value.is_empty() {
            "—".to_owned()
        } else {
            value
        };
        (label.to_owned(), value, false)
    }

    /// The status bar's hint for the tool's next step (feel test `hintText(`).
    pub fn hint(&self) -> &'static str {
        match self.tool.as_str() {
            LINE if self.line_start.is_some() => {
                "Click the next point · Shift locks the inference · arrows lock an axis · type a length + Enter · Esc stops"
            }
            LINE => {
                "Click a start point. Hover points to wake them, then draw aligned to them from elsewhere."
            }
            RECT if self.rect.is_some() => {
                "Click the opposite corner · type width,height + Enter (2,1.5) · arrows pick the plane · Esc cancels"
            }
            RECT => "Click the first corner. It draws on the face or plane under the cursor.",
            SELECT => {
                "Click to select · Shift toggles · Ctrl adds · double-click face + edges · triple-click connected · drag a box · Del deletes"
            }
            ORBIT => {
                "Orbit: drag to look around (Shift pans) · wheel zooms · Esc returns to your tool"
            }
            PAN => "Pan: drag to slide the view · wheel zooms · Esc returns to your tool",
            ZOOM => "Zoom: drag up to zoom in, down to zoom out · Esc returns to your tool",
            _ => "Middle-drag orbits, Shift+middle-drag pans, the wheel zooms.",
        }
    }

    /// The lock chip's text.
    pub fn lock_text(&self) -> Option<String> {
        match self.lock {
            Lock::Axis(a) => Some(format!("{} locked", infer::AXIS_NAMES[a as usize])),
            Lock::None => None,
        }
    }

    // ---------- recording ----------

    pub fn start_recording(&mut self) {
        self.recording = Some(Recording {
            model: self.doc.model().clone(),
            camera: self.camera(),
            tool: self.tool.clone(),
            size: self.size,
            settings: self.settings,
            events: Vec::new(),
            result: String::new(),
        });
    }

    /// Stop recording and return it, as JSON.
    pub fn stop_recording(&mut self) -> Option<String> {
        let mut r = self.recording.take()?;
        r.result = self.doc.model().geometry_json();
        serde_json::to_string_pretty(&r).ok()
    }

    /// Play a recording from its start. Returns the session at the end.
    pub fn replay(r: &Recording) -> Session {
        let mut s = Session::new(Document::with_model(r.model.clone()));
        s.camera = r.camera;
        s.tool = r.tool.clone();
        s.size = r.size;
        s.settings = r.settings;
        s.ease = false;
        for e in &r.events {
            s.handle(e);
        }
        s
    }

    // ---------- handling ----------

    pub fn handle(&mut self, e: &Event) -> Response {
        if let Some(r) = &mut self.recording {
            r.events.push(e.clone());
        }
        let dt = (e.t - self.time).max(0.0);
        self.time = e.t;
        if let Some(m) = &mut self.motion {
            self.dirty = true;
            if !m.step(dt) {
                self.motion = None;
            }
        }
        let mut resp = Response::default();
        match &e.input {
            Input::Resize { w, h } => {
                if (*w, *h) != self.size {
                    self.size = (w.max(1.0), h.max(1.0));
                    self.dirty = true;
                }
            }
            Input::Move { x, y } => self.moved(DVec2::new(*x, *y)),
            Input::Press {
                button,
                x,
                y,
                shift,
                ctrl,
            } => self.press(*button, DVec2::new(*x, *y), *shift, *ctrl, &mut resp),
            Input::Release { button } => self.release(*button),
            Input::Wheel { x, y, notches } => {
                let s = DVec2::new(*x, *y);
                self.settle_goal();
                let view = self.view();
                let q = self.surface(&view, s);
                let goal = self.camera.wheel(q, *notches, self.min_distance());
                self.ease_to(goal, EASE_WHEEL);
                self.refresh();
            }
            Input::Scroll { x, y, dx, dy } => {
                self.settle();
                let view = self.view();
                let q = self.surface(&view, DVec2::new(*x, *y));
                self.camera = self.camera.pan(&view, q, DVec2::new(*dx, *dy));
                self.dirty = true;
                self.refresh();
            }
            Input::Pinch { x, y, factor } => {
                self.settle();
                let view = self.view();
                let q = self.surface(&view, DVec2::new(*x, *y));
                self.camera = self.camera.pinch(q, *factor, self.min_distance());
                self.dirty = true;
                self.refresh();
            }
            Input::Key { key } => self.key(*key, &mut resp),
            Input::ShiftDown => {
                if let Some(i) = &self.inference
                    && let Some(a) = i.axis
                    && self.lock == Lock::None
                    && i.kind == Kind::Guide
                {
                    self.set_lock(Lock::Axis(a));
                    self.shift_locked = true;
                }
            }
            Input::ShiftUp => {
                if self.shift_locked {
                    self.shift_locked = false;
                    self.set_lock(Lock::None);
                }
            }
            Input::Text { text } => {
                self.typed.push_str(text);
                self.overlay_dirty = true;
            }
            Input::Tool { id } => self.set_tool(id),
            Input::Action { id } => self.action(id),
            Input::Time => {
                if self.mem.tick(self.time, &self.settings) {
                    self.refresh();
                }
            }
        }
        resp
    }

    fn set_tool(&mut self, id: &str) {
        if id == self.tool {
            return;
        }
        if is_nav(id) && !is_nav(&self.tool) {
            self.before_nav = self.tool.clone();
        }
        self.cancel();
        self.tool = id.to_owned();
        self.mem.clear();
        self.lock = Lock::None;
        self.last_op = None;
        self.drag = None;
        if self.hover.take().is_some() {
            self.highlight_dirty = true;
            self.dirty = true;
        }
        self.inference = None;
        self.overlay_dirty = true;
        self.dirty = true;
        self.refresh();
    }

    fn set_lock(&mut self, l: Lock) {
        self.lock = l;
        self.overlay_dirty = true;
        self.refresh();
    }

    /// Cancel the step in progress; the tool stays.
    fn cancel(&mut self) {
        self.line_start = None;
        self.rect = None;
        self.boxing = None;
        self.typed.clear();
        self.lock = Lock::None;
        self.shift_locked = false;
        self.overlay_dirty = true;
        self.dirty = true;
    }

    fn key(&mut self, key: Key, resp: &mut Response) {
        match key {
            Key::Escape => {
                resp.used_escape = true;
                if !self.typed.is_empty() {
                    self.typed.clear();
                    self.overlay_dirty = true;
                } else if is_nav(&self.tool) {
                    let back = self.before_nav.clone();
                    self.set_tool(&back);
                    resp.tool = Some(back);
                } else if self.in_operation() {
                    self.cancel();
                    self.refresh();
                } else if self.lock != Lock::None || !self.mem.woken.is_empty() {
                    self.lock = Lock::None;
                    self.mem.clear();
                    self.overlay_dirty = true;
                    self.refresh();
                } else {
                    resp.used_escape = false;
                }
            }
            Key::Backspace => {
                self.typed.pop();
                self.overlay_dirty = true;
            }
            Key::Enter => self.enter(),
            Key::Up | Key::Right | Key::Left => {
                let a = match key {
                    Key::Up => 2,
                    Key::Right => 0,
                    _ => 1,
                };
                let l = if self.lock == Lock::Axis(a) {
                    Lock::None
                } else {
                    Lock::Axis(a)
                };
                self.shift_locked = false;
                self.set_lock(l);
            }
            Key::Down => {
                self.shift_locked = false;
                self.set_lock(Lock::None);
            }
        }
    }

    // ---------- camera ----------

    fn min_distance(&self) -> f64 {
        self.scene.tol.dist * MIN_DISTANCE_TOLS
    }

    fn surface(&self, view: &View, s: DVec2) -> DVec3 {
        let fallback = pick::fallback_distance(&self.scene, view, self.camera.target);
        camera::surface_point(view, s, pick::model_point(&self.scene, view, s), fallback)
    }

    fn settle(&mut self) {
        if let Some(m) = self.motion.take() {
            self.camera = m.goal();
        }
    }

    /// Wheel steps during an ease build on its goal.
    fn settle_goal(&mut self) {
        if let Some(m) = &self.motion {
            self.camera = m.goal();
        }
    }

    fn ease_to(&mut self, goal: Camera, duration: f64) {
        let now = self.camera();
        self.motion = (self.ease && !self.reduce_motion).then(|| Motion::new(now, goal, duration));
        self.camera = goal;
        self.dirty = true;
    }

    // ---------- the model ----------

    fn run(&mut self, c: Command) -> bool {
        if self.doc.run(c) {
            self.model_changed();
            true
        } else {
            false
        }
    }

    fn model_changed(&mut self) {
        self.version += 1;
        let t0 = self.measure.map(|f| f());
        self.scene = Scene::build(self.doc.model());
        if let (Some(f), Some(t0)) = (self.measure, t0) {
            self.timings.scene_ms = (f() - t0) * 1e3;
        }
        let m = self.doc.model();
        self.selection.vertices.retain(|v| m.has_vertex(*v));
        self.selection.edges.retain(|e| m.edge(*e).is_some());
        self.selection.faces.retain(|f| m.face(*f).is_some());
        self.hover = None;
        self.mem.prev = None;
        self.highlight_dirty = true;
        self.overlay_dirty = true;
        self.dirty = true;
    }

    fn load(&mut self, doc: Document) {
        self.doc = doc;
        self.selection = Selection::default();
        self.cancel();
        self.last_op = None;
        self.mem.clear();
        self.model_changed();
        if let Some((lo, hi)) = self.doc.model().bbox() {
            let goal = Camera::home().extents(lo, hi, self.view().aspect());
            self.motion = None;
            self.camera = goal;
        }
    }

    fn set_selection(&mut self, s: Selection) {
        if s != self.selection {
            self.selection = s;
            self.highlight_dirty = true;
            self.dirty = true;
        }
    }

    fn action(&mut self, id: &str) {
        match id {
            "cam.extents" => {
                if let Some((lo, hi)) = self.scene.bounds {
                    let goal = self.camera.extents(lo, hi, self.view().aspect());
                    self.ease_to(goal, EASE_EXTENTS);
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
                    self.run(Command::Erase { selection });
                }
            }
            "edit.undo" => {
                self.cancel();
                if self.doc.undo() {
                    self.last_op = None;
                    self.model_changed();
                }
            }
            "edit.redo" => {
                self.cancel();
                if self.doc.redo() {
                    self.last_op = None;
                    self.model_changed();
                }
            }
            "file.new" => self.load(Document::new()),
            "view.shadow" => self.dirty = true,
            _ => {
                if let Some(doc) = crate::scenes::by_action(id) {
                    self.load(doc);
                }
            }
        }
    }

    // ---------- the mouse ----------

    fn wants_inference(&self) -> bool {
        self.tool == LINE || self.tool == RECT
    }

    fn context(&self) -> Context {
        match self.tool.as_str() {
            LINE => Context {
                start: self.line_start,
                ..Default::default()
            },
            RECT => Context {
                start: self.rect.map(|r| r.start),
                no_lock: true,
                ..Default::default()
            },
            _ => Context::default(),
        }
    }

    /// Infer again at the cursor (after the model, lock or woken points
    /// changed), and update the tool's preview.
    fn refresh(&mut self) {
        if let Some(s) = self.cursor
            && self.drag.is_none()
        {
            self.update_hover(s);
        }
    }

    fn update_hover(&mut self, s: DVec2) {
        let view = self.view();
        if self.wants_inference() {
            let t0 = self.measure.map(|f| f());
            let ctx = self.context();
            let mut i = infer::infer(
                &self.scene,
                &view,
                s,
                &ctx,
                self.lock,
                &self.mem,
                &self.settings,
            );
            if self.mem.observe(&i, self.time, &self.settings) {
                i = infer::infer(
                    &self.scene,
                    &view,
                    s,
                    &ctx,
                    self.lock,
                    &self.mem,
                    &self.settings,
                );
                self.mem.prev = i.key;
            }
            if let (Some(f), Some(t0)) = (self.measure, t0) {
                self.timings.infer_ms = (f() - t0) * 1e3;
            }
            if let Some(r) = &mut self.rect {
                let q = rect_point(&self.scene, &view, s, &i, r.start, r.normal);
                let d = q - r.start;
                r.du = d.dot(r.u);
                r.dv = d.dot(r.v);
            }
            if self.inference.as_ref() != Some(&i) {
                self.inference = Some(i);
                self.overlay_dirty = true;
                self.dirty = true;
            }
        } else if self.tool == SELECT && self.boxing.is_none() {
            let t0 = self.measure.map(|f| f());
            let h = pick::pick(&self.scene, &view, s, self.settings.aperture);
            if let (Some(f), Some(t0)) = (self.measure, t0) {
                self.timings.pick_ms = (f() - t0) * 1e3;
            }
            if h.map(|p| p.hit) != self.hover.map(|p| p.hit) {
                self.highlight_dirty = true;
                self.dirty = true;
            }
            self.hover = h;
        }
    }

    fn moved(&mut self, s: DVec2) {
        let last = self.cursor.replace(s);
        if let (Some(drag), Some(last)) = (self.drag, last) {
            let d = s - last;
            let c = match drag {
                Drag::Orbit { pivot } => self.camera.orbit_drag(pivot, d.x, d.y),
                Drag::Pan { grab } => self.camera.pan(&self.view(), grab, d),
                Drag::Zoom { toward } => self.camera.zoom_toward(toward, -d.y * ZOOM_PER_DRAG_PX),
            };
            self.camera = c;
            self.dirty = true;
            return;
        }
        if let Some(b) = &mut self.boxing {
            b.now = s;
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        self.update_hover(s);
    }

    fn press(&mut self, button: Button, s: DVec2, shift: bool, ctrl: bool, resp: &mut Response) {
        self.cursor = Some(s);
        match button {
            Button::Middle => {
                self.settle();
                let view = self.view();
                let q = self.surface(&view, s);
                if self.middle_clicks.press(self.time, s) == 2 {
                    self.camera = self.camera.recentre(q);
                    self.dirty = true;
                    self.drag = None;
                    return;
                }
                self.drag = Some(if shift {
                    Drag::Pan { grab: q }
                } else {
                    Drag::Orbit { pivot: q }
                });
            }
            Button::Left if is_nav(&self.tool) => {
                self.settle();
                let view = self.view();
                let q = self.surface(&view, s);
                self.drag = Some(match self.tool.as_str() {
                    ORBIT if !shift => Drag::Orbit { pivot: q },
                    ZOOM => Drag::Zoom { toward: q },
                    _ => Drag::Pan { grab: q },
                });
            }
            Button::Left => match self.tool.as_str() {
                SELECT => self.select_press(s, shift, ctrl),
                LINE => self.line_press(s),
                RECT => self.rect_press(s),
                _ => {}
            },
            Button::Right => {
                if self.tool == SELECT
                    && let Some(h) =
                        pick::pick(&self.scene, &self.view(), s, self.settings.aperture)
                {
                    let has = match h.hit {
                        Hit::Vertex(v) => self.selection.vertices.contains(&v),
                        Hit::Edge(e) => self.selection.edges.contains(&e),
                        Hit::Face(f) => self.selection.faces.contains(&f),
                    };
                    if !has {
                        self.set_selection(click_select(self.doc.model(), Some(h.hit), 1));
                    }
                }
                resp.context_menu = Some(s);
            }
        }
    }

    fn release(&mut self, button: Button) {
        match button {
            Button::Left if self.boxing.is_some() => {
                let b = self.boxing.take().expect("box");
                let pre = self.saved.pop().unwrap_or_default();
                let _ = b.pre;
                if (b.now - b.down).abs().max_element() >= BOX_MIN {
                    let crossing = b.now.x < b.down.x;
                    let (lo, hi) = (b.down.min(b.now), b.down.max(b.now));
                    let hits = box_select(&self.scene, &self.view(), lo, hi, crossing);
                    let mut sel = if b.shift || b.ctrl {
                        pre
                    } else {
                        Selection::default()
                    };
                    for e in hits.edges {
                        if b.shift && sel.edges.contains(&e) {
                            sel.edges.remove(&e);
                        } else {
                            sel.edges.insert(e);
                        }
                    }
                    for f in hits.faces {
                        if b.shift && sel.faces.contains(&f) {
                            sel.faces.remove(&f);
                        } else {
                            sel.faces.insert(f);
                        }
                    }
                    self.set_selection(sel);
                }
                self.overlay_dirty = true;
                self.dirty = true;
            }
            _ => self.drag = None,
        }
    }

    fn select_press(&mut self, s: DVec2, shift: bool, ctrl: bool) {
        let n = self.clicks.press(self.time, s);
        let hit = pick::pick(&self.scene, &self.view(), s, self.settings.aperture).map(|p| p.hit);
        let pre = self.selection.clone();
        let mut sel = if shift || ctrl {
            pre.clone()
        } else {
            Selection::default()
        };
        if let Some(h) = hit {
            let m = self.doc.model();
            if n >= 3 {
                sel = click_select(m, Some(h), 3);
            } else {
                match h {
                    Hit::Vertex(v) => toggle(&mut sel.vertices, v, shift),
                    Hit::Edge(e) => toggle(&mut sel.edges, e, shift),
                    Hit::Face(f) => {
                        toggle(&mut sel.faces, f, shift);
                        if n == 2 {
                            sel.edges.extend(click_select(m, Some(h), 2).edges);
                        }
                    }
                }
            }
        }
        self.set_selection(sel);
        if n == 1 {
            self.saved.push(pre);
            self.boxing = Some(BoxOp {
                down: s,
                now: s,
                pre: self.saved.len(),
                shift,
                ctrl,
            });
        }
    }

    fn line_press(&mut self, s: DVec2) {
        self.update_hover(s);
        let Some(i) = self.inference.clone() else {
            return;
        };
        match self.line_start {
            None => {
                self.line_start = Some(i.point);
                self.lock = Lock::None;
                self.last_op = None;
                self.overlay_dirty = true;
            }
            Some(start) => {
                if i.point.distance(start) <= self.scene.tol.dist {
                    self.line_start = None;
                    self.overlay_dirty = true;
                    return;
                }
                self.commit_line(start, i.point);
            }
        }
        self.refresh();
    }

    fn commit_line(&mut self, start: DVec3, end: DVec3) {
        let faces = self.doc.model().face_count();
        let made = self.run(Command::Segment { p: start, q: end });
        self.typed.clear();
        self.lock = Lock::None;
        self.shift_locked = false;
        self.mem.clear();
        let len = start.distance(end);
        if made {
            self.last_op = Some(LastOp::Line {
                from: start,
                dir: (end - start) / len.max(f64::MIN_POSITIVE),
                len,
            });
        }
        // The line chains on from its end, unless it just closed a face.
        let end_at = prism_ops::vertex_at(self.doc.model(), end)
            .map(|v| self.doc.model().vertex(v))
            .unwrap_or(end);
        self.line_start = (self.doc.model().face_count() <= faces).then_some(end_at);
        self.overlay_dirty = true;
    }

    fn rect_press(&mut self, s: DVec2) {
        self.update_hover(s);
        let Some(i) = self.inference.clone() else {
            return;
        };
        match self.rect {
            None => {
                let normal = self.plane_normal(&i);
                let (u, v) = rect_basis(normal);
                self.rect = Some(RectOp {
                    start: i.point,
                    normal,
                    u,
                    v,
                    du: 0.0,
                    dv: 0.0,
                });
                self.last_op = None;
                self.overlay_dirty = true;
            }
            Some(op) => self.commit_rect(op),
        }
        self.refresh();
    }

    fn commit_rect(&mut self, op: RectOp) {
        self.rect = None;
        self.typed.clear();
        self.lock = Lock::None;
        self.overlay_dirty = true;
        let tiny = self.scene.tol.dist.max(op.start.length() * 1e-12);
        if op.du.abs() <= tiny || op.dv.abs() <= tiny {
            return;
        }
        if self.run(Command::Loop {
            points: op.corners().to_vec(),
        }) {
            self.rect_memory = Some((op.du.abs(), op.dv.abs()));
            self.last_op = Some(LastOp::Rect { op });
        }
        self.mem.clear();
    }

    /// The plane a rectangle starts on (feel test `rotNormal(`): the locked
    /// axis, the face under the cursor, the ground, else the view's main axis.
    fn plane_normal(&self, i: &Inference) -> DVec3 {
        if let Lock::Axis(a) = self.lock {
            return AXES[a as usize];
        }
        if let Some(f) = i.face.and_then(|f| self.scene.face(f)) {
            return f.normal;
        }
        if i.kind == Kind::OnGround {
            return DVec3::Z;
        }
        let f = self.view().f;
        let a = (0..3)
            .max_by(|&i, &j| f[i].abs().total_cmp(&f[j].abs()))
            .unwrap_or(2);
        AXES[a]
    }

    // ---------- typing ----------

    fn enter(&mut self) {
        let text = std::mem::take(&mut self.typed);
        self.overlay_dirty = true;
        let nf = self.number;
        if let (Some(start), LINE) = (self.line_start, self.tool.as_str()) {
            let Some(v) = nf.length(&text) else { return };
            let mut dir = self
                .inference
                .as_ref()
                .map(|i| i.point - start)
                .unwrap_or(DVec3::X)
                .normalize_or_zero();
            if let Lock::Axis(a) = self.lock {
                let ax = AXES[a as usize];
                dir = if dir.dot(ax) < 0.0 { -ax } else { ax };
            }
            if dir == DVec3::ZERO {
                dir = DVec3::X;
            }
            self.commit_line(start, start + dir * v);
            self.refresh();
            return;
        }
        if let Some(mut op) = self.rect {
            let size = if text.is_empty() {
                self.rect_memory.map(|(w, h)| (w, Some(h)))
            } else {
                nf.lengths(&text)
            };
            let Some((w, h)) = size else { return };
            let h = h.unwrap_or(w);
            op.du = if op.du < 0.0 { -w.abs() } else { w.abs() };
            op.dv = if op.dv < 0.0 { -h.abs() } else { h.abs() };
            self.commit_rect(op);
            self.refresh();
            return;
        }
        // Change the last operation.
        match (self.last_op, self.typing_tool()) {
            (Some(LastOp::Line { from, dir, .. }), true) => {
                let Some(v) = nf.length(&text) else { return };
                if self.doc.undo() {
                    self.model_changed();
                }
                let end = from + dir * v;
                self.run(Command::Segment { p: from, q: end });
                self.last_op = Some(LastOp::Line {
                    from,
                    dir,
                    len: v.abs(),
                });
            }
            (Some(LastOp::Rect { mut op }), true) => {
                let Some((w, h)) = nf.lengths(&text) else {
                    return;
                };
                let h = h.unwrap_or(w);
                if self.doc.undo() {
                    self.model_changed();
                }
                op.du = op.du.signum() * w.abs();
                op.dv = op.dv.signum() * h.abs();
                self.run(Command::Loop {
                    points: op.corners().to_vec(),
                });
                self.rect_memory = Some((w.abs(), h.abs()));
                self.last_op = Some(LastOp::Rect { op });
            }
            _ => {}
        }
        self.refresh();
    }

    // ---------- feedback ----------

    /// What the tools want drawn over the model.
    pub fn overlay(&self) -> Overlay {
        let mut o = Overlay::default();
        let fb = &self.style.feedback;
        let view = self.view();
        if let Some(b) = &self.boxing
            && (b.now - b.down).abs().max_element() >= BOX_MIN
        {
            let crossing = b.now.x < b.down.x;
            o.screen_box = Some(ScreenBox {
                min: b.down.min(b.now),
                max: b.down.max(b.now),
                crossing,
            });
            o.box_label = Some(format!(
                "{}{}",
                if crossing { "Crossing" } else { "Window" },
                if b.shift {
                    "  toggle"
                } else if b.ctrl {
                    "  add"
                } else {
                    ""
                }
            ));
        }
        if !self.wants_inference() {
            return o;
        }
        // Woken points and edges.
        for w in &self.mem.woken {
            o.marks.push(Mark {
                at: w.point,
                shape: Shape::DiamondOutline,
                color: if w.edge_dir.is_some() {
                    fb.parallel
                } else {
                    fb.woken
                },
                size: fb.woken_size,
            });
        }
        let Some(i) = &self.inference else {
            return o;
        };
        let axis_color = |a: u8| self.style.axes[a as usize];
        let px = |p: DVec3| view.to_screen(p).map_or(0.0, |(_, d)| view.units_per_px(d));
        // Guides the snap sits on, a little past the snap.
        for g in &i.guides {
            let r = i.point - g.origin;
            let along = r.dot(g.dir);
            let past = px(i.point) * 20.0;
            let end = g.origin + g.dir * (along + if along >= 0.0 { past } else { -past });
            o.lines.push(Line {
                a: g.origin,
                b: end,
                color: g.axis.map_or(fb.parallel, axis_color),
                width: fb.guide_width,
                dash: fb.guide_dash,
            });
        }
        // The rubber band.
        if let Some(start) = self.line_start {
            let color = i.axis.map_or(fb.rubber_band, axis_color);
            o.lines.push(Line {
                a: start,
                b: i.point,
                color,
                width: fb.rubber_band_width,
                dash: [0.0, 0.0],
            });
            o.labels.push(Label {
                at: start.lerp(i.point, 0.5),
                offset: [10.0, -14.0],
                text: self.number.show(start.distance(i.point)),
                color: fb.pill,
            });
        }
        // The rectangle preview.
        if let Some(r) = &self.rect {
            let c = r.corners();
            o.tris.push(Tri {
                p: [c[0], c[1], c[2]],
                color: fb.preview_fill,
            });
            o.tris.push(Tri {
                p: [c[0], c[2], c[3]],
                color: fb.preview_fill,
            });
            for k in 0..4 {
                o.lines.push(Line {
                    a: c[k],
                    b: c[(k + 1) % 4],
                    color: fb.rubber_band,
                    width: fb.rubber_band_width,
                    dash: [0.0, 0.0],
                });
            }
            o.labels.push(Label {
                at: c[0].lerp(c[2], 0.5),
                offset: [-40.0, 0.0],
                text: format!(
                    "{} × {}",
                    self.number.show(r.du.abs()).trim_end_matches(" m"),
                    self.number.show(r.dv.abs())
                ),
                color: fb.pill,
            });
        }
        // The snap: its target, glyph and label.
        let glyph_color = match i.kind {
            Kind::Endpoint => fb.endpoint,
            Kind::Midpoint => fb.midpoint,
            Kind::OnEdge => fb.on_edge,
            Kind::OnFace => fb.on_face,
            Kind::OnGround => fb.on_ground,
            Kind::Intersection => fb.intersection,
            Kind::GuideCross => fb.guide_cross,
            Kind::Guide => fb.intersection,
            Kind::Free => fb.free,
        };
        let color = i.axis.map_or(glyph_color, axis_color);
        if let Some(e) = i.edge.and_then(|e| self.scene.edge(e))
            && matches!(i.kind, Kind::OnEdge | Kind::Midpoint)
        {
            o.lines.push(Line {
                a: e.a,
                b: e.b,
                color: color.with_alpha(alpha(fb.target_glow_alpha)),
                width: fb.target_glow_width,
                dash: [0.0, 0.0],
            });
            o.lines.push(Line {
                a: e.a,
                b: e.b,
                color,
                width: fb.target_width,
                dash: [0.0, 0.0],
            });
        }
        if i.vertex.is_some() {
            o.marks.push(Mark {
                at: i.point,
                shape: Shape::Ring,
                color,
                size: fb.target_ring,
            });
        }
        let shape = match i.kind {
            Kind::Endpoint | Kind::Free | Kind::Guide => Shape::Circle,
            Kind::Midpoint => Shape::Triangle,
            Kind::OnEdge => Shape::Square,
            Kind::OnFace | Kind::OnGround => Shape::Diamond,
            Kind::Intersection | Kind::GuideCross => Shape::Cross,
        };
        o.marks.push(Mark {
            at: i.point,
            shape,
            color,
            size: fb.glyph_size,
        });
        if i.kind != Kind::Free {
            let pill = if color == fb.intersection {
                fb.pill
            } else {
                color
            };
            o.labels.push(Label {
                at: i.point,
                offset: [14.0, 20.0],
                text: i.label.clone(),
                color: pill,
            });
        }
        o
    }

    /// Colour for the lock chip (the locked axis's colour).
    pub fn lock_color(&self) -> Option<Rgba> {
        match self.lock {
            Lock::Axis(a) => Some(self.style.axes[a as usize]),
            Lock::None => None,
        }
    }
}

fn toggle<T: Ord>(set: &mut std::collections::BTreeSet<T>, x: T, shift: bool) {
    if shift && set.contains(&x) {
        set.remove(&x);
    } else {
        set.insert(x);
    }
}

/// Two directions on a rectangle's plane, along the world axes where they
/// can be (feel test `rectBasis(`).
pub fn rect_basis(n: DVec3) -> (DVec3, DVec3) {
    if n.z.abs() > 0.99 {
        return (DVec3::X, DVec3::Y);
    }
    if n.x.abs() > 0.99 {
        return (DVec3::Y, DVec3::Z);
    }
    if n.y.abs() > 0.99 {
        return (DVec3::X, DVec3::Z);
    }
    let u = n.cross(DVec3::Z).normalize();
    (u, n.cross(u))
}

/// Where the rectangle's far corner goes: a snapped point projected onto
/// its plane, otherwise the cursor's ray meeting the plane.
fn rect_point(
    scene: &Scene,
    view: &View,
    s: DVec2,
    i: &Inference,
    start: DVec3,
    n: DVec3,
) -> DVec3 {
    let snapped = !matches!(i.kind, Kind::OnFace | Kind::OnGround | Kind::Free);
    if !snapped {
        let (o, d) = view.ray(s);
        let den = n.dot(d);
        if den.abs() > 1e-3 {
            let t = n.dot(start - o) / den;
            let reach = scene
                .bounds
                .map_or(1.0, |(lo, hi)| lo.distance(hi))
                .max(view.eye.distance(start))
                * 100.0;
            if t > 0.0 && t < reach {
                return o + d * t;
            }
        }
    }
    let p = i.point;
    p - n * (p - start).dot(n)
}
