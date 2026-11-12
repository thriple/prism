//! The viewport session: the document, the camera, the tools, the inference
//! memory, the selection and the measurement box, driven by a stream of
//! [`Event`]s.
//!
//! Pure: the app turns mouse and keys into events with their times, and
//! reads back what to draw. The same events replay to the same model, which
//! is how recorded sessions become tests (PLAN 5).

use std::collections::BTreeSet;

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

mod arcs;
mod cage;
mod construct;
mod curve;
mod edit;
mod follow;
mod groups;
mod health;
mod materials;
mod paint;
mod round;
mod scalebox;
mod shapes;
mod texpos;
use crate::units::NumberFormat;
pub use arcs::{ARC, ARC_CENTRE, ARC2, ARC3, PIE};
pub use cage::PICK_FIRST;
pub use construct::{OFFSET, PROTRACTOR, TAPE};
pub use curve::{CURVE, curve_for};
pub use edit::{ERASE, MOVE, PUSH, ROTATE, SCALE};
use edit::{EditOp, is_edit};
pub use follow::FOLLOW;
pub use groups::{EntityInfo, OutlineRow, Owner, TagOp};
pub use health::HealthRow;
pub use materials::{MaterialOp, MaterialRow, NOT_A_SIZE, PaintWith, TextureRow};
pub use paint::{Brush, PAINT};
pub use round::{FILLET, RoundOp, RoundPanel};
pub use shapes::{CIRCLE, DEFAULT_SIDES, POLYGON, polygon_points};
pub use texpos::{NOT_TEXTURED, TEXTURE};

/// Tool ids (the action registry's).
pub const SELECT: &str = "tool.select";
pub const LINE: &str = "tool.line";
pub const RECT: &str = "tool.rect";
pub const ORBIT: &str = "cam.orbit";
pub const PAN: &str = "cam.pan";
pub const ZOOM: &str = "cam.zoom";

/// Every tool the session runs. The action registry greys out every other
/// tool (DECISIONS #20), so this list is the one place that says what is built.
pub const TOOLS: [&str; 26] = [
    SELECT,
    LINE,
    RECT,
    ORBIT,
    PAN,
    ZOOM,
    PUSH,
    MOVE,
    ROTATE,
    SCALE,
    ERASE,
    OFFSET,
    TAPE,
    PROTRACTOR,
    // The curve tools, active since Exact Curves 6e.
    CIRCLE,
    POLYGON,
    arcs::ARC,
    arcs::ARC2,
    arcs::ARC3,
    arcs::ARC_CENTRE,
    arcs::PIE,
    round::FILLET,
    follow::FOLLOW,
    // Basic Materials 1b.
    paint::PAINT,
    // Basic Materials part 3: Texture > Position.
    texpos::TEXTURE,
    // Freeform NURBS part 1.
    curve::CURVE,
];

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
        /// Alt (Option) held: the Paint Bucket samples.
        #[serde(default)]
        alt: bool,
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
    /// Ctrl (Option on a Mac) pressed: copy in Move and Rotate.
    CtrlTap,
    /// Alt pressed: new loop in Push/Pull.
    AltTap,
    /// Ctrl (Cmd on a Mac) and Alt (Option) held or not, sent when they
    /// change (the Paint Bucket's preview follows them).
    Held {
        ctrl: bool,
        alt: bool,
    },
    /// Characters for the measurement box.
    Text {
        text: String,
    },
    /// A change from the Fillet window.
    Round {
        op: RoundOp,
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
    /// Whether Push/Pull's new loop was on when recording started.
    #[serde(default)]
    pub alt_loop: bool,
    /// The Paint Bucket's brush when recording started, and a copy of its
    /// material (the recording's model does not hold the material table).
    #[serde(default)]
    pub brush: Brush,
    #[serde(default)]
    pub brush_copy: Option<prism_doc::Material>,
    /// The document's materials when recording started, so painted faces
    /// replay in their colours.
    #[serde(default)]
    pub materials: Vec<(prism_topo::MaterialId, prism_doc::Material)>,
    /// The Fillet window's size and kind when recording started.
    #[serde(default)]
    pub round_size: Option<f64>,
    #[serde(default)]
    pub round_chamfer: bool,
}

impl Recording {
    /// Whether `model` is the one this recording ended with. A recording
    /// saved before loops named their edges is read back as a model (which
    /// finds them), so it still matches.
    pub fn ends_with(&self, model: &Model) -> bool {
        let now = model.geometry_json();
        now == self.result || Model::from_json(&self.result).is_ok_and(|m| m.geometry_json() == now)
    }
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

#[derive(Clone, Debug)]
struct BoxOp {
    down: DVec2,
    now: DVec2,
    pre: usize,
    shift: bool,
    ctrl: bool,
    /// Pressed on empty space: a click (not a drag) closes the open group.
    empty: bool,
    /// The selected groups when the press began.
    groups: BTreeSet<prism_doc::InstanceId>,
}

/// The last operation, which typing can still change (Appendix A ★).
#[derive(Clone, Debug)]
enum LastOp {
    Line {
        from: DVec3,
        dir: DVec3,
        len: f64,
    },
    Rect {
        op: RectOp,
    },
    Push {
        face: prism_topo::FaceId,
        d: f64,
        new_loop: bool,
    },
    Offset {
        face: prism_topo::FaceId,
        d: f64,
    },
    Edit(EditOp),
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
    /// The selection before the current click series began.
    series_pre: Selection,
    clicks: ClickSeries,
    middle_clicks: ClickSeries,
    drag: Option<Drag>,
    boxing: Option<BoxOp>,
    line_start: Option<DVec3>,
    rect: Option<RectOp>,
    rect_memory: Option<(f64, f64)>,
    /// The circle or polygon being drawn, the last radius, and the
    /// polygon's side count.
    shape: Option<shapes::ShapeOp>,
    /// The arc being drawn (the Arc tools).
    arc: Option<arcs::ArcOp>,
    /// The Curve tool's curve in progress, and whether it passes through
    /// its clicks.
    curve: Option<curve::CurveOp>,
    /// Edit Cage (Freeform NURBS part 3), and what it last refused.
    cage: Option<cage::CageEdit>,
    cage_notice: Option<&'static str>,
    curve_through: bool,
    /// The Arc tools' last typed radius (for fillets), the corner of the
    /// last fillet (a typed radius changes it), and why the last fillet
    /// was refused.
    arc_radius: Option<f64>,
    last_fillet: Option<DVec3>,
    arc_notice: Option<&'static str>,
    /// The Fillet / Chamfer tool: its size, the edge it last rounded (and
    /// whether as a fillet), and why it last refused.
    round_size: Option<f64>,
    round_last: Option<(prism_topo::EdgeId, bool)>,
    round_notice: Option<&'static str>,
    /// The Fillet window's kind (chamfer, else fillet), the cursor, what
    /// the preview shows, and why the hovered edge cannot be rounded.
    round_chamfer: bool,
    round_at: Option<DVec2>,
    round_shown: Option<(round::Target, bool, Option<f64>)>,
    round_refused: Option<&'static str>,
    follow_notice: Option<&'static str>,
    /// Follow Me follows the selected face's edge (Alt).
    follow_perimeter: bool,
    /// The Scale tool: the grip under the cursor, and whether it scales
    /// about the centre (Alt).
    scale_grip: Option<[u8; 3]>,
    scale_centre: bool,
    shape_radius: Option<f64>,
    /// The circle or polygon just drawn, its radius, and the document's
    /// revision then (typing a radius changes it while nothing else has).
    last_shape: Option<(shapes::ShapeOp, f64, u64)>,
    sides: u32,
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
    /// An edit tool's step in progress, and the model it previews.
    edit: Option<EditOp>,
    preview: Option<Scene>,
    /// Rotate's protractor plane, once locked (DECISIONS #18).
    rot_lock: Option<DVec3>,
    shift_held: bool,
    /// Copy (Ctrl) and new loop (Alt) toggles.
    ctrl_copy: bool,
    alt_loop: bool,
    /// Alt was held at the last press (the Paint Bucket samples).
    press_alt: bool,
    /// Ctrl and Alt held now (`Input::Held`).
    ctrl_held: bool,
    alt_held: bool,
    /// The Paint Bucket: what it paints with, whether the next click
    /// samples (Alt tapped), and what is under it.
    brush: Brush,
    brush_copy: Option<prism_doc::Material>,
    sampling: bool,
    paint_at: Option<paint::PaintAt>,
    /// Push/Pull's last distance, for double-click.
    last_push: Option<(f64, bool)>,
    /// Offset's and Tape Measure's last distances, for double-click.
    last_offset: Option<f64>,
    last_tape: Option<f64>,
    /// Tape Measure measures without making a guide (Ctrl).
    measure_only: bool,
    /// What is shown and how (View Settings, PLAN 1.14); `style` follows it.
    view: prism_view::ViewStyle,
    /// The guide under the Eraser.
    hover_guide: Option<prism_topo::GuideId>,
    /// Why the edit in progress changes nothing (arcs), for the status bar.
    refused: Option<&'static str>,
    /// Why the Materials window's last request was refused.
    material_refused: Option<&'static str>,
    /// Texture > Position: the texture being placed, and why the last
    /// click could not start it.
    texpos: Option<texpos::TexPos>,
    texpos_notice: Option<&'static str>,
    /// The open group (empty at the root), and the selected groups in it.
    ctx: Vec<prism_doc::InstanceId>,
    groups: BTreeSet<prism_doc::InstanceId>,
    /// Model Health drawn in the model, and its last check.
    health_view: bool,
    health_report: Option<prism_ops::health::Report>,
    /// What the last Hide hid (Unhide Last).
    last_hidden: BTreeSet<prism_doc::InstanceId>,
    /// Owners of the pieces merged into the scene, and the pieces.
    owners: Vec<groups::Owner>,
    pieces: Vec<groups::Piece>,
    /// The open group's model in world coordinates (None at the root).
    work: Option<Model>,
    to_world: prism_math::DAffine3,
}

impl Session {
    pub fn new(doc: Document) -> Self {
        let mut scene = Scene::build(doc.model());
        // Soft edges are hidden until View > Hidden Geometry shows them.
        scene.drop_soft();
        let mut s = Self {
            edit: None,
            preview: None,
            rot_lock: None,
            shift_held: false,
            curve: None,
            cage: None,
            cage_notice: None,
            curve_through: false,
            ctrl_copy: false,
            alt_loop: false,
            press_alt: false,
            ctrl_held: false,
            alt_held: false,
            brush: Brush::default(),
            brush_copy: None,
            sampling: false,
            paint_at: None,
            last_push: None,
            last_offset: None,
            last_tape: None,
            measure_only: false,
            view: prism_view::ViewStyle::default(),
            hover_guide: None,
            refused: None,
            material_refused: None,
            texpos: None,
            texpos_notice: None,
            ctx: Vec::new(),
            groups: BTreeSet::new(),
            last_hidden: BTreeSet::new(),
            health_view: false,
            health_report: None,
            owners: Vec::new(),
            pieces: Vec::new(),
            work: None,
            to_world: prism_math::DAffine3::IDENTITY,
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
            series_pre: Selection::default(),
            clicks: ClickSeries::default(),
            middle_clicks: ClickSeries::default(),
            drag: None,
            boxing: None,
            line_start: None,
            rect: None,
            rect_memory: None,
            shape: None,
            arc: None,
            arc_radius: None,
            last_fillet: None,
            arc_notice: None,
            round_size: None,
            round_last: None,
            round_notice: None,
            round_chamfer: false,
            round_at: None,
            round_shown: None,
            round_refused: None,
            follow_notice: None,
            follow_perimeter: false,
            scale_grip: None,
            scale_centre: false,
            shape_radius: None,
            last_shape: None,
            sides: shapes::DEFAULT_SIDES,
            last_op: None,
            settings: Settings::default(),
            mem: Memory::default(),
            lock: Lock::None,
            shift_locked: false,
            inference: None,
            typed: String::new(),
            number: NumberFormat::default(),
            style: prism_view::ViewStyle::default().apply(&Style::default()),
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
        };
        // Groups, and materials' colours and textures, need the full build.
        let asm = s.doc.assembly();
        if asm.has_instances() || !asm.materials().0.is_empty() {
            s.build_scene();
        }
        s
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
    /// What to draw: the preview of a step in progress, or the model.
    pub fn display_scene(&self) -> &Scene {
        self.preview.as_ref().unwrap_or(&self.scene)
    }

    fn set_preview(&mut self, m: Option<Model>) {
        let scene = m.map(|m| self.compose(Scene::build_with(&m, self.view.guides)));
        self.set_preview_scene(scene);
    }

    /// Show a scene made another way (a rigid move of the drawn scene).
    fn set_preview_scene(&mut self, scene: Option<Scene>) {
        let had = self.preview.is_some();
        self.preview = scene;
        if had || self.preview.is_some() {
            self.version += 1;
            self.highlight_dirty = true;
            self.dirty = true;
        }
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

    /// What else the hover takes, as a click would select it: the whole
    /// smooth surface of a hovered face, the whole curve of a hovered arc
    /// (owner, 2026-11-11: hovering highlighted half a cylinder).
    pub fn hover_more(&self) -> Selection {
        let mut sel = Selection::default();
        let m = self.work_model();
        match self.hover() {
            Some(Hit::Face(f)) if m.face(f).is_some() => {
                sel.faces.extend(prism_ops::select::surface(m, f));
            }
            Some(Hit::Edge(e)) if m.edge(e).is_some() => {
                sel.edges.extend(m.curve_edges(e));
            }
            _ => {}
        }
        sel
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
        self.line_start.is_some()
            || self.rect.is_some()
            || self.shape.is_some()
            || self.arc.is_some()
            || self.curve.is_some()
            || self.cage_start().is_some()
            || self.boxing.is_some()
            || self.edit.is_some()
            || self.texpos_active()
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

    /// Draw everything again: the picture, the highlight and the tool
    /// feedback (after switching to this document's tab).
    pub fn mark_all_dirty(&mut self) {
        self.dirty = true;
        self.highlight_dirty = true;
        self.overlay_dirty = true;
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
            || self.last_op.is_some() && self.typing_tool()
            || self.tool == FILLET;
        editable && (c.is_ascii_digit() || c == '.' || c == '-' || c == self.number.decimal)
    }

    fn typing_tool(&self) -> bool {
        self.tool == LINE
            || self.tool == RECT
            || shapes::is_shape(&self.tool)
            || arcs::is_arc(&self.tool)
            || matches!(
                self.tool.as_str(),
                PUSH | MOVE | ROTATE | SCALE | OFFSET | TAPE | PROTRACTOR
            )
    }

    /// The measurement box: its label, value, and whether it shows typing.
    pub fn measurement(&self) -> (String, String, bool) {
        let nf = &self.number;
        let (label, value) = if let Some((l, v)) = self.edit_measurement() {
            (l, v)
        } else if let Some((l, v)) = self.shape_measurement() {
            (l, v)
        } else if let Some((l, v)) = self.arc_measurement() {
            (l, v)
        } else if let (Some(start), Some(i)) = (self.line_start, &self.inference) {
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
        if let Some(h) = self.cage_hint() {
            return h;
        }
        if let Some(r) = self.refused.filter(|_| self.edit.is_some()) {
            return r;
        }
        if let Some(h) = self.texpos_hint() {
            return h;
        }
        if let Some(h) = self.edit_hint() {
            return h;
        }
        if let Some(h) = self.shape_hint() {
            return h;
        }
        if let Some(h) = self.arc_hint() {
            return h;
        }
        if let Some(h) = self.curve_hint() {
            return h;
        }
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
            alt_loop: self.alt_loop,
            brush: self.brush,
            brush_copy: match self.brush {
                Brush::Material(id) => self
                    .doc
                    .assembly()
                    .material(id)
                    .or(self.brush_copy.as_ref())
                    .cloned(),
                _ => None,
            },
            materials: self
                .doc
                .assembly()
                .materials()
                .0
                .iter()
                .map(|(id, m)| (*id, m.clone()))
                .collect(),
            round_size: self.round_size,
            round_chamfer: self.round_chamfer,
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
        let next = r.materials.last().map_or(0, |(id, _)| id.0 + 1);
        let asm = prism_doc::Assembly::with_model(r.model.clone()).with_materials(
            r.materials.iter().cloned().collect(),
            next,
            Default::default(),
        );
        let mut s = Session::new(Document::with_assembly(asm));
        s.camera = r.camera;
        s.tool = r.tool.clone();
        s.size = r.size;
        s.settings = r.settings;
        s.ease = false;
        s.alt_loop = r.alt_loop;
        s.brush = r.brush;
        s.brush_copy = r.brush_copy.clone();
        s.round_size = r.round_size;
        s.round_chamfer = r.round_chamfer;
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
                alt,
            } => {
                self.press_alt = *alt;
                self.press(*button, DVec2::new(*x, *y), *shift, *ctrl, &mut resp);
            }
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
                self.shift_held = true;
                self.paint_keys_changed();
                if let Some(i) = &self.inference
                    && let Some(a) = i.axis
                    && self.lock == Lock::None
                    && i.kind == Kind::Guide
                {
                    self.set_lock(Lock::Axis(a));
                    self.shift_locked = true;
                }
            }
            Input::CtrlTap => self.toggle_copy(),
            Input::AltTap if self.tool == SCALE => self.toggle_scale_centre(),
            Input::AltTap if self.tool == FOLLOW => self.toggle_follow_perimeter(),
            Input::AltTap if self.tool == PAINT => self.toggle_sampling(),
            Input::Held { ctrl, alt } => {
                if *ctrl && !self.ctrl_held && self.tool == CURVE {
                    self.curve_through = !self.curve_through;
                    self.overlay_dirty = true;
                    self.dirty = true;
                }
                self.ctrl_held = *ctrl;
                self.alt_held = *alt;
                self.paint_keys_changed();
                self.round_preview();
            }
            Input::Round { op } => self.round_set(*op),
            Input::AltTap => self.toggle_loop(),
            Input::ShiftUp => {
                self.shift_held = false;
                self.paint_keys_changed();
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
        self.cage_notice = None;
        if is_nav(id) && !is_nav(&self.tool) {
            self.before_nav = self.tool.clone();
        }
        self.cancel();
        self.tool = id.to_owned();
        // A new loop is for one pull at a time, never left on unseen.
        self.alt_loop = false;
        self.sampling = false;
        self.paint_at = None;
        self.last_shape = None;
        self.rot_lock = None;
        self.ctrl_copy = false;
        self.mem.clear();
        self.lock = Lock::None;
        self.last_op = None;
        self.drag = None;
        if self.hover.take().is_some() {
            self.highlight_dirty = true;
            self.dirty = true;
        }
        self.inference = None;
        if id == TEXTURE {
            self.texpos_picked();
        }
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
        self.cancel_edit();
        self.line_start = None;
        self.rect = None;
        self.shape = None;
        self.arc = None;
        self.curve = None;
        self.cage_cancel();
        self.last_fillet = None;
        self.round_last = None;
        self.round_notice = None;
        self.round_clear();
        self.follow_notice = None;
        self.follow_perimeter = false;
        self.texpos_cancel();
        self.texpos_notice = None;
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
                } else if self.cage.is_some() {
                    self.cage_leave();
                } else if !self.close_group() {
                    resp.used_escape = false;
                }
            }
            Key::Backspace => {
                self.typed.pop();
                self.overlay_dirty = true;
            }
            Key::Enter => self.enter(),
            Key::Up | Key::Right | Key::Left | Key::Down
                if matches!(self.tool.as_str(), ROTATE | PROTRACTOR) && self.edit.is_none() =>
            {
                let axis = match key {
                    Key::Right => Some(0),
                    Key::Left => Some(1),
                    _ => None,
                };
                self.rotate_arrow(axis, key == Key::Up);
            }
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

    /// Finish any camera ease now (an assistant clicking at model points
    /// needs the camera where it is going, not where it is on the way).
    pub fn settle_camera(&mut self) {
        if self.motion.is_some() {
            self.settle();
            self.dirty = true;
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
        if self.run_here(c) {
            self.model_changed();
            true
        } else {
            false
        }
    }

    fn model_changed(&mut self) {
        self.version += 1;
        // The Fillet preview was of the old model.
        if self.round_shown.take().is_some() && self.tool == FILLET {
            self.preview = None;
            self.highlight_dirty = true;
        }
        let t0 = self.measure.map(|f| f());
        self.build_scene();
        self.cage_check();
        // The face being positioned went (undo, another edit).
        if let Some(f) = self.texpos.as_ref().map(|t| t.face())
            && self.work_model().face(f).is_none()
        {
            self.texpos_cancel();
        }
        if self.health_view {
            self.refresh_health();
        }
        if let (Some(f), Some(t0)) = (self.measure, t0) {
            self.timings.scene_ms = (f() - t0) * 1e3;
        }
        let m = self.work.as_ref().unwrap_or_else(|| self.doc.model());
        self.selection.vertices.retain(|v| m.has_vertex(*v));
        self.selection.edges.retain(|e| m.edge(*e).is_some());
        self.selection.faces.retain(|f| m.face(*f).is_some());
        self.selection.guides.retain(|g| m.guide(*g).is_some());
        self.hover_guide = None;
        self.hover = None;
        self.mem.prev = None;
        self.highlight_dirty = true;
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Open a document (File > New and Open): the selection, tool steps and
    /// undo history start fresh. With a camera, the view goes there; else
    /// it fits the model.
    pub fn open_document(&mut self, doc: Document, camera: Option<Camera>) {
        self.load(doc);
        if let Some(c) = camera {
            self.motion = None;
            self.camera = c;
            self.dirty = true;
        }
    }

    /// Add imported geometry beside the model, in one undo step, and fit
    /// the view to it. Returns whether anything was added.
    pub fn import(&mut self, model: Model) -> bool {
        self.import_painted(model, Vec::new())
    }

    /// [`Session::import`] with the file's materials (Basic Materials part
    /// 3): faces painted `MaterialId(k)` get the `k`-th of `materials`,
    /// which are added to the document (with their images) in the same
    /// step.
    pub fn import_painted(
        &mut self,
        mut model: Model,
        materials: Vec<(prism_doc::Material, Option<prism_doc::Image>)>,
    ) -> bool {
        let Some((lo, hi)) = model.bbox() else {
            return false;
        };
        self.cancel();
        let next = self.doc.assembly().materials().1;
        let ids: Vec<prism_topo::FaceId> = model.faces().map(|(id, _)| id).collect();
        for f in ids {
            model.edit_face(f, |x| {
                for side in [&mut x.paint.front, &mut x.paint.back] {
                    *side = side
                        .filter(|k| (k.0 as usize) < materials.len())
                        .map(|k| prism_topo::MaterialId(next + k.0));
                }
            });
        }
        let mut commands = Vec::new();
        for (m, image) in materials {
            if let Some(image) = image {
                commands.push(Command::AddImage { image });
            }
            commands.push(Command::AddMaterial { material: m });
        }
        commands.push(Command::Import { model });
        if !self.run(Command::Batch { commands }) {
            return false;
        }
        let goal = self.camera.extents(lo, hi, self.view().aspect());
        self.ease_to(goal, EASE_EXTENTS);
        self.refresh();
        true
    }

    /// What is shown and how (View Settings).
    pub fn view_style(&self) -> &prism_view::ViewStyle {
        &self.view
    }

    /// Change the view style: the drawing style follows, and faces stop
    /// hiding or being picked in X-ray and Wireframe.
    pub fn set_view_style(&mut self, v: prism_view::ViewStyle) {
        if v == self.view {
            return;
        }
        // Guides and hidden objects change what the scene holds.
        let rebuild = v.guides != self.view.guides || v.hidden != self.view.hidden;
        self.view = v;
        self.style = self.view.apply(&Style::default());
        if rebuild {
            self.model_changed();
        } else {
            self.set_pick_flags();
            self.version += 1;
            self.highlight_dirty = true;
            self.overlay_dirty = true;
            self.dirty = true;
        }
        self.refresh();
    }

    /// Whether faces hide and can be picked, from the view style.
    fn set_pick_flags(&mut self) {
        use prism_view::style::FaceDraw;
        self.scene.faces_hide = self.style.faces == FaceDraw::Solid;
        self.scene.faces_pick = self.style.faces != FaceDraw::None;
        self.scene.materials_shown = self.style.materials;
        self.scene.textures_shown = self.style.textures;
    }

    /// How lengths are shown (File > Document Settings).
    pub fn set_units(&mut self, units: prism_math::DisplayUnits) {
        if self.number.units != units {
            self.number.units = units;
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    fn load(&mut self, doc: Document) {
        self.doc = doc;
        self.ctx.clear();
        self.groups.clear();
        self.selection = Selection::default();
        self.cancel();
        self.last_op = None;
        self.mem.clear();
        self.model_changed();
        if let Some((lo, hi)) = self.scene.bounds {
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
                let m = self.work_model();
                let s = Selection {
                    vertices: Default::default(),
                    edges: m.edges().map(|(id, _)| id).collect(),
                    faces: m.faces().map(|(id, _)| id).collect(),
                    ..Default::default()
                };
                self.set_selection(s);
                self.set_groups(self.all_groups());
            }
            "edit.desel" => {
                self.set_selection(Selection::default());
                self.set_groups(BTreeSet::new());
            }
            "edit.delete" => self.delete(),
            "edit.selmat" => self.select_same_material(),
            "tex.reset" => self.texture_reset(),
            "cage.edit" => self.cage_toggle(),
            "cage.add" => self.cage_add(),
            "edit.group" => self.make_group(),
            "edit.comp" => self.make_component(),
            // Edit > Reverse Faces and Orient Faces (Model Health).
            "edit.revface" => {
                if !self.selection.faces.is_empty() {
                    let faces = self.selection.faces.clone();
                    self.cancel();
                    self.run(Command::ReverseFaces { faces });
                }
            }
            // Edit > Soften/Smooth (Exact Curves 4c): the selected edges,
            // or plain again if all of them are soft already.
            "edit.soften" => {
                let edges = self.selection.edges.clone();
                if !edges.is_empty() {
                    let m = self.work_model();
                    let soft = !edges.iter().all(|e| m.edge(*e).is_some_and(|x| x.soft));
                    self.cancel();
                    self.run(Command::Soften { edges, soft });
                }
            }
            "edit.orient" => {
                let faces =
                    (!self.selection.faces.is_empty()).then(|| self.selection.faces.clone());
                self.cancel();
                self.run(Command::OrientFaces { faces });
            }
            "edit.hide" => self.hide(),
            "edit.unhideall" => self.unhide_all(),
            "edit.unhide" => self.unhide_selected(),
            "edit.unhidelast" => self.unhide_last(),
            "edit.isolate" => self.isolate(),
            "edit.unique" => self.make_unique(),
            "flip.x" => self.flip(0),
            "flip.y" => self.flip(1),
            "flip.z" => self.flip(2),
            "edit.explode" => self.explode(),
            "edit.undo" => {
                self.cancel();
                // Undo of an edit inside a group opens that group again.
                let ctx = self.doc.undo_context();
                if self.doc.undo() {
                    self.ctx = ctx;
                    self.last_op = None;
                    self.model_changed();
                }
            }
            "edit.redo" => {
                self.cancel();
                let ctx = self.doc.redo_context();
                if self.doc.redo() {
                    self.ctx = ctx;
                    self.last_op = None;
                    self.model_changed();
                }
            }
            "file.new" => self.load(Document::new()),
            // Edit > Intersect Faces (DECISIONS #29).
            "isect.model" | "isect.sel" => {
                let faces = (id == "isect.sel").then(|| self.selection.faces.clone());
                if faces.as_ref().is_some_and(BTreeSet::is_empty) {
                    return;
                }
                self.cancel();
                self.run(Command::IntersectFaces { faces });
                self.refresh();
            }
            id if prism_view::view_style::TOGGLES.contains(&id) => {
                let mut v = self.view.clone();
                v.toggle(id);
                self.set_view_style(v);
            }
            _ => {
                if let Some(doc) = crate::scenes::by_action(id) {
                    self.load(doc);
                }
            }
        }
    }

    // ---------- the mouse ----------

    fn wants_inference(&self) -> bool {
        self.tool == LINE
            || self.tool == RECT
            || shapes::is_shape(&self.tool)
            || arcs::is_arc(&self.tool)
            || self.tool == CURVE
            || self.cage_wants_inference()
            || self.edit_wants_inference()
    }

    fn context(&self) -> Context {
        if self.cage_takes() {
            return Context {
                start: self.cage_start(),
                ..Default::default()
            };
        }
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
            CIRCLE | POLYGON => Context {
                start: self.shape.map(|r| r.centre),
                no_lock: true,
                ..Default::default()
            },
            t if arcs::is_arc(t) => Context {
                start: self.arc.as_ref().and_then(|a| a.pts.last().copied()),
                no_lock: true,
                ..Default::default()
            },
            CURVE => Context {
                start: self.curve.as_ref().and_then(|c| c.pts.last().copied()),
                ..Default::default()
            },
            MOVE => match &self.edit {
                Some(EditOp::Move {
                    from, verts, copy, ..
                }) => Context {
                    start: Some(*from),
                    // A copy leaves the original in place, so it can be snapped to.
                    exclude: if *copy {
                        Default::default()
                    } else {
                        verts.clone()
                    },
                    no_lock: false,
                },
                _ => Context::default(),
            },
            TAPE => Context {
                start: match &self.edit {
                    Some(EditOp::Tape { start, .. }) => Some(*start),
                    _ => None,
                },
                ..Default::default()
            },
            ROTATE | PROTRACTOR => match &self.edit {
                Some(EditOp::Rotate { centre, .. } | EditOp::Protractor { centre, .. }) => {
                    Context {
                        start: Some(*centre),
                        no_lock: true,
                        ..Default::default()
                    }
                }
                _ => Context {
                    no_lock: true,
                    ..Default::default()
                },
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
            self.shape_hover(s, &view);
            self.arc_hover(s, &view);
            self.cage_hover();
            if is_edit(&self.tool) {
                self.edit_hover(s, &view);
            }
        } else if is_edit(&self.tool) {
            self.edit_hover(s, &view);
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
            Button::Left if self.cage_takes() => self.cage_press(s, shift, ctrl),
            Button::Left => match self.tool.as_str() {
                SELECT => self.select_press(s, shift, ctrl),
                LINE => self.line_press(s),
                RECT => self.rect_press(s),
                CIRCLE | POLYGON => {
                    let n = self.clicks.press(self.time, s);
                    self.shape_press(s, n);
                }
                t if arcs::is_arc(t) => {
                    let n = self.clicks.press(self.time, s);
                    self.arc_press(s, n);
                }
                CURVE => {
                    let n = self.clicks.press(self.time, s);
                    self.curve_press(s, n);
                }
                t if is_edit(t) => {
                    let n = self.clicks.press(self.time, s);
                    self.update_hover(s);
                    self.edit_press(s, n, shift, ctrl);
                    self.refresh();
                }
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
                    match self.owner(h.hit) {
                        Some(Owner::Child(g)) => {
                            if !self.groups.contains(&g) {
                                self.set_selection(Selection::default());
                                self.set_groups([g].into());
                            }
                        }
                        Some(Owner::Outside) => {}
                        None if !has => {
                            self.set_groups(BTreeSet::new());
                            self.set_selection(click_select(self.work_model(), Some(h.hit), 1));
                        }
                        None => {}
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
                let dragged = (b.now - b.down).abs().max_element() >= BOX_MIN;
                if !dragged && b.empty && !b.shift && !b.ctrl {
                    self.close_group();
                }
                if dragged {
                    let crossing = b.now.x < b.down.x;
                    let (lo, hi) = (b.down.min(b.now), b.down.max(b.now));
                    let mut hits = box_select(&self.scene, &self.view(), lo, hi, crossing);
                    let groups = self.box_groups(&mut hits, crossing, &b);
                    self.set_groups(groups);
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
                    for v in hits.vertices {
                        if b.shift && sel.vertices.contains(&v) {
                            sel.vertices.remove(&v);
                        } else {
                            sel.vertices.insert(v);
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
            Button::Left if is_edit(&self.tool) && self.drag.is_none() => {
                if let Some(c) = self.cursor {
                    self.edit_release(c);
                }
                self.refresh();
            }
            _ => self.drag = None,
        }
    }

    fn select_press(&mut self, s: DVec2, shift: bool, ctrl: bool) {
        let n = self.clicks.press(self.time, s);
        let hit = pick::pick(&self.scene, &self.view(), s, self.settings.aperture).map(|p| p.hit);
        let groups_before = self.groups.clone();
        // A group is picked whole; a double-click opens it. Geometry
        // outside the open group closes it (PLAN 1.2).
        match hit.and_then(|h| self.owner(h)) {
            Some(Owner::Outside) => {
                self.close_group();
                return;
            }
            Some(Owner::Child(g)) => {
                if n >= 2 && !shift && !ctrl {
                    self.open_group(g);
                    return;
                }
                let mut gs = if shift || ctrl {
                    groups_before.clone()
                } else {
                    BTreeSet::new()
                };
                if shift && gs.contains(&g) {
                    gs.remove(&g);
                } else {
                    gs.insert(g);
                }
                if !shift && !ctrl {
                    self.set_selection(Selection::default());
                }
                self.set_groups(gs);
                if n == 1 {
                    self.saved.push(self.selection.clone());
                    self.boxing = Some(BoxOp {
                        down: s,
                        now: s,
                        pre: self.saved.len(),
                        shift,
                        ctrl,
                        empty: false,
                        groups: groups_before,
                    });
                }
                return;
            }
            None => {}
        }
        if !shift && !ctrl {
            self.set_groups(BTreeSet::new());
        }
        let pre = self.selection.clone();
        if n == 1 {
            self.series_pre = pre.clone();
        }
        // What this click names: the item, a face and its edges (double),
        // or everything connected (triple).
        let named = match hit {
            Some(h) => click_select(self.work_model(), Some(h), n.min(3)),
            None => Selection::default(),
        };
        // With Shift or Ctrl, a double or triple click works from the
        // selection as it was before the series started, so other
        // selections are kept. Shift takes the named things out if the
        // clicked item was already selected, and adds them otherwise.
        let base = &self.series_pre;
        let sel = if shift || ctrl {
            let was = hit.is_some_and(|h| contains(base, h));
            let mut out = base.clone();
            if shift && was {
                remove_all(&mut out, &named);
            } else {
                add_all(&mut out, &named);
            }
            out
        } else {
            named
        };
        self.set_selection(sel);
        if n == 1 {
            self.saved.push(pre);
            self.boxing = Some(BoxOp {
                down: s,
                now: s,
                pre: self.saved.len(),
                shift,
                ctrl,
                empty: hit.is_none(),
                groups: groups_before,
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
        let faces = self.work_model().face_count();
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
        let end_at = prism_ops::vertex_at(self.work_model(), end)
            .map(|v| self.work_model().vertex(v))
            .unwrap_or(end);
        self.line_start = (self.work_model().face_count() <= faces).then_some(end_at);
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
        // A curved face has no plane: the ground or the view's.
        if let Some(n) = i
            .face
            .and_then(|f| self.scene.face(f))
            .and_then(|f| f.flat_normal())
        {
            return n;
        }
        if i.kind == Kind::OnGround {
            return DVec3::Z;
        }
        // A point on a guide or an edge: the face under the cursor, if the
        // point lies in its plane.
        if let Some(s) = self.cursor {
            let (o, d) = self.view().ray(s);
            if let Some(f) = self
                .scene
                .ray_face(o, d)
                .and_then(|(f, _)| self.scene.face(f))
                && let Some(n) = f.flat_normal()
                && (i.point - f.origin).dot(n).abs() <= self.scene.tol.dist * 10.0
            {
                return n;
            }
        }
        let f = self.view().f;
        let a = (0..3)
            .max_by(|&i, &j| f[i].abs().total_cmp(&f[j].abs()))
            .unwrap_or(2);
        AXES[a]
    }

    // ---------- typing ----------

    fn enter(&mut self) {
        if self.texpos_active() {
            self.texpos_commit();
            return;
        }
        let text = std::mem::take(&mut self.typed);
        self.overlay_dirty = true;
        if text.is_empty()
            && self.edit.is_none()
            && self.tool == SELECT
            && self.selection.is_empty()
            && self.groups.len() == 1
        {
            let g = *self.groups.iter().next().expect("one group");
            self.open_group(g);
            return;
        }
        if self.edit.is_some() {
            self.edit_enter(&text);
            return;
        }
        if self.shape_enter(&text)
            || self.arc_enter(&text)
            || self.round_enter(&text)
            || self.curve_enter()
            || self.cage_enter(&text)
        {
            return;
        }
        if !text.is_empty() && self.edit_retype(&text) {
            return;
        }
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
        match (self.last_op.clone(), self.typing_tool()) {
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
        self.group_boxes(&mut o);
        self.health_marks(&mut o);
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
        self.guides_overlay(&mut o);
        if is_edit(&self.tool) {
            self.edit_overlay(&mut o);
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
        // The centre of an arc that is hovered or woken, to be snapped to.
        let arcs = self.mem.woken.iter().map(|w| w.key).chain(
            self.inference
                .as_ref()
                .filter(|i| matches!(i.kind, Kind::OnArc | Kind::Midpoint | Kind::Quadrant))
                .and_then(|i| i.key),
        );
        for k in arcs {
            if let prism_infer::Key::Edge(e) | prism_infer::Key::Mid(e) = k
                && let Some(prism_geom::Curve::Arc { centre, .. }) =
                    self.scene.edge(e).map(|e| e.curve.clone())
            {
                o.marks.push(Mark {
                    at: centre,
                    shape: Shape::Cross,
                    color: fb.woken,
                    size: fb.woken_size,
                });
            }
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
                color: match g.source {
                    infer::GuideSource::Stored(_) => fb.guide,
                    infer::GuideSource::Crossing(..) => fb.guide_cross,
                    infer::GuideSource::Tangent(..) => fb.tangent,
                    _ => g.axis.map_or(fb.parallel, axis_color),
                },
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
        self.shape_overlay(&mut o);
        self.arc_overlay(&mut o);
        self.curve_overlay(&mut o);
        self.cage_overlay(&mut o);
        // The snap: its target, glyph and label.
        let glyph_color = match i.kind {
            Kind::Endpoint => fb.endpoint,
            Kind::Midpoint | Kind::Quadrant => fb.midpoint,
            Kind::Centre => fb.endpoint,
            Kind::OnEdge | Kind::OnArc => fb.on_edge,
            Kind::OnFace => fb.on_face,
            Kind::OnGround => fb.on_ground,
            Kind::Intersection => fb.intersection,
            Kind::GuideCross => fb.guide_cross,
            Kind::Guide => fb.intersection,
            Kind::Tangent => fb.tangent,
            Kind::GuidePoint => fb.endpoint,
            Kind::Free => fb.free,
        };
        let color = i.axis.map_or(glyph_color, axis_color);
        if let Some(e) = i.edge.and_then(|e| self.scene.edge(e))
            && matches!(
                i.kind,
                Kind::OnEdge | Kind::OnArc | Kind::Midpoint | Kind::Quadrant
            )
        {
            for (a, b) in e.segments() {
                o.lines.push(Line {
                    a,
                    b,
                    color: color.with_alpha(alpha(fb.target_glow_alpha)),
                    width: fb.target_glow_width,
                    dash: [0.0, 0.0],
                });
                o.lines.push(Line {
                    a,
                    b,
                    color,
                    width: fb.target_width,
                    dash: [0.0, 0.0],
                });
            }
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
            Kind::Endpoint | Kind::Free | Kind::Guide | Kind::Tangent => Shape::Circle,
            Kind::Midpoint | Kind::Quadrant => Shape::Triangle,
            Kind::Centre => Shape::Ring,
            Kind::OnEdge | Kind::OnArc => Shape::Square,
            Kind::OnFace | Kind::OnGround => Shape::Diamond,
            Kind::Intersection | Kind::GuideCross | Kind::GuidePoint => Shape::Cross,
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

fn contains(s: &Selection, h: Hit) -> bool {
    match h {
        Hit::Vertex(v) => s.vertices.contains(&v),
        Hit::Edge(e) => s.edges.contains(&e),
        Hit::Face(f) => s.faces.contains(&f),
    }
}

fn add_all(s: &mut Selection, o: &Selection) {
    s.vertices.extend(o.vertices.iter().copied());
    s.edges.extend(o.edges.iter().copied());
    s.faces.extend(o.faces.iter().copied());
}

fn remove_all(s: &mut Selection, o: &Selection) {
    s.vertices.retain(|v| !o.vertices.contains(v));
    s.edges.retain(|e| !o.edges.contains(e));
    s.faces.retain(|f| !o.faces.contains(f));
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
/// [`rect_point`] for tools drawn on a plane through `start` with normal
/// `n`, ignoring guides along the normal (on the plane each is one point).
fn plane_point(
    scene: &Scene,
    view: &View,
    s: DVec2,
    i: &Inference,
    start: DVec3,
    n: DVec3,
) -> DVec3 {
    let along_normal =
        i.kind == Kind::Guide && i.guides.first().is_some_and(|g| g.dir.dot(n).abs() > 0.99);
    if along_normal {
        let mut free = i.clone();
        free.kind = Kind::Free;
        return rect_point(scene, view, s, &free, start, n);
    }
    rect_point(scene, view, s, i, start, n)
}

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
