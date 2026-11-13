//! The Camera menu (Everyday Commands part 2, brief 33): standard views,
//! Perspective and Parallel Projection, Field of View, Zoom Selection, and
//! the Zoom Window, Walk and Look Around tools.

use prism_math::{DVec2, DVec3};
use prism_view::camera::{self, Camera, EASE_EXTENTS, FOV_MAX_DEG, FOV_MIN_DEG, Lens};
use prism_view::overlay::{Overlay, ScreenBox};

use super::{Drag, Session, ZOOM};

/// Tool ids.
pub const WALK: &str = "cam.walk";
pub const LOOK: &str = "cam.look";
pub const WINDOW: &str = "cam.window";

/// Radians turned per pixel dragged (Look Around, Walk).
const TURN_PER_PX: f64 = 0.004;
/// Walk: share of the camera's distance moved per pixel dragged.
const WALK_PER_PX: f64 = 0.004;
/// Walk: share of the camera's distance one arrow press steps.
const WALK_STEP: f64 = 0.05;
/// Walk: radians one arrow press turns (5°).
const WALK_TURN: f64 = 5.0 * std::f64::consts::PI / 180.0;
/// A Zoom Window smaller than this (pixels) is a click: it zooms in a step.
const WINDOW_MIN: f64 = 6.0;
/// The step a click with Zoom Window zooms in by.
const WINDOW_CLICK: f64 = 0.5;

impl Session {
    /// Perspective or parallel, and the field of view.
    pub fn lens(&self) -> Lens {
        self.lens
    }

    /// Set the lens (a file's saved view).
    pub fn set_lens(&mut self, lens: Lens) {
        let lens = Lens {
            fov: lens.fov.clamp(FOV_MIN_DEG, FOV_MAX_DEG),
            ..lens
        };
        if lens != self.lens {
            self.lens = lens;
            self.dirty = true;
            self.highlight_dirty = true;
            self.overlay_dirty = true;
        }
    }

    /// The Camera menu's actions. Returns whether `id` was one.
    pub(super) fn camera_action(&mut self, id: &str) -> bool {
        if let Some(view) = id.strip_prefix("cam.v.") {
            if let Some(dir) = camera::standard_view(view) {
                self.standard_view(dir);
            }
            return true;
        }
        match id {
            "cam.persp" => self.set_lens(Lens {
                parallel: false,
                ..self.lens
            }),
            "cam.parallel" => self.set_lens(Lens {
                parallel: true,
                ..self.lens
            }),
            // Field of View: the Zoom tool takes a typed angle.
            "cam.fov" => self.set_tool(ZOOM),
            "cam.zoomsel" => self.zoom_selection(),
            _ => return false,
        }
        true
    }

    /// Look in a standard direction at the whole model, fitted.
    fn standard_view(&mut self, dir: DVec3) {
        let goal = match self.scene.bounds {
            Some((lo, hi)) => Camera::looking(dir, lo, hi, self.view().aspect()),
            None => {
                let c = self.camera.target;
                let d = self.camera.distance();
                Camera {
                    eye: c - dir * d,
                    target: c,
                }
            }
        };
        self.ease_to(goal, EASE_EXTENTS);
        self.refresh();
    }

    /// Zoom Selection: fit the selection and the selected groups.
    fn zoom_selection(&mut self) {
        let m = self.work_model();
        let mut pts: Vec<DVec3> = self
            .selection
            .all_vertices(m)
            .into_iter()
            .map(|v| m.vertex(v))
            .collect();
        for g in &self.groups {
            if let Some(c) = self.group_box(*g) {
                pts.extend(c);
            }
        }
        let Some(first) = pts.first().copied() else {
            return;
        };
        let (lo, hi) = pts
            .iter()
            .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let goal = self.camera.extents(lo, hi, self.view().aspect());
        self.ease_to(goal, EASE_EXTENTS);
        self.refresh();
    }

    /// Typing goes to the Zoom tool's field of view, or Walk's eye height.
    pub(super) fn fov_measurement(&self) -> Option<(&'static str, String)> {
        match self.tool.as_str() {
            ZOOM => Some(("Field of View", format!("{:.0}°", self.lens.fov))),
            WALK => Some(("Eye Height", self.number.show(self.camera.eye.z))),
            _ => None,
        }
    }

    /// Walk with the arrow keys: up and down step forward and back along
    /// the ground, left and right turn. True if it was one.
    pub(super) fn walk_key(&mut self, key: super::Key) -> bool {
        if self.tool != WALK {
            return false;
        }
        let step = WALK_STEP * self.camera.distance();
        self.camera = match key {
            super::Key::Up => self.camera.walk(step, 0.0),
            super::Key::Down => self.camera.walk(-step, 0.0),
            super::Key::Left => self.camera.walk(0.0, WALK_TURN),
            super::Key::Right => self.camera.walk(0.0, -WALK_TURN),
            _ => return false,
        };
        self.dirty = true;
        self.highlight_dirty = true;
        self.overlay_dirty = true;
        true
    }

    /// Enter with the Zoom tool: a typed field of view, in degrees.
    pub(super) fn fov_enter(&mut self, text: &str) -> bool {
        if self.tool == WALK {
            // A typed eye height above the ground: the view keeps looking
            // the same way.
            if let Some(h) = self.number.length(text) {
                let up = DVec3::Z * (h - self.camera.eye.z);
                self.camera = Camera {
                    eye: self.camera.eye + up,
                    target: self.camera.target + up,
                };
                self.dirty = true;
                self.highlight_dirty = true;
                self.overlay_dirty = true;
            }
            return true;
        }
        if self.tool != ZOOM {
            return false;
        }
        let t = text
            .trim()
            .trim_end_matches('°')
            .trim_end_matches("deg")
            .trim()
            .replace(self.number.decimal, ".");
        if let Ok(v) = t.parse::<f64>()
            && v.is_finite()
        {
            self.set_lens(Lens {
                fov: v,
                ..self.lens
            });
        }
        true
    }

    /// A press with Walk, Look Around or Zoom Window.
    pub(super) fn camera_press(&mut self, s: DVec2) -> Option<Drag> {
        match self.tool.as_str() {
            WALK => Some(Drag::Walk),
            LOOK => Some(Drag::Look),
            WINDOW => {
                self.zoom_box = Some((s, s));
                Some(Drag::Window)
            }
            _ => None,
        }
    }

    /// A camera drag step by `d` pixels for Walk and Look Around.
    pub(super) fn camera_drag(&mut self, drag: Drag, s: DVec2, d: DVec2) -> Option<Camera> {
        match drag {
            Drag::Look => Some(
                self.camera
                    .look_around(-d.x * TURN_PER_PX, -d.y * TURN_PER_PX),
            ),
            Drag::Walk => Some(self.camera.walk(
                -d.y * WALK_PER_PX * self.camera.distance(),
                -d.x * TURN_PER_PX,
            )),
            Drag::Window => {
                if let Some(b) = &mut self.zoom_box {
                    b.1 = s;
                    self.overlay_dirty = true;
                    self.dirty = true;
                }
                None
            }
            _ => None,
        }
    }

    /// Zoom Window released: fit the box to the view (a click zooms in a
    /// step at the cursor).
    pub(super) fn window_release(&mut self) {
        let Some((a, b)) = self.zoom_box.take() else {
            return;
        };
        self.overlay_dirty = true;
        self.dirty = true;
        let view = self.view();
        let size = (b - a).abs();
        let centre = (a + b) * 0.5;
        let q = self.surface(&view, centre);
        // The share of the view the box covers: the view comes that much
        // closer to the box's middle (a parallel view shrinks as much).
        let share = if size.max_element() < WINDOW_MIN {
            WINDOW_CLICK
        } else {
            (size.x / view.w).max(size.y / view.h).min(1.0)
        };
        let centred = self.camera.recentre(q);
        let d = (centred.distance() * share).max(self.min_distance());
        let goal = Camera {
            eye: q - centred.forward() * d,
            target: q,
        };
        self.ease_to(goal, EASE_EXTENTS);
        self.refresh();
    }

    /// The Zoom Window box being dragged.
    pub(super) fn window_overlay(&self, o: &mut Overlay) {
        if let Some((a, b)) = self.zoom_box
            && (b - a).abs().max_element() >= WINDOW_MIN
        {
            o.screen_box = Some(ScreenBox {
                min: a.min(b),
                max: a.max(b),
                crossing: false,
            });
        }
    }

    pub(super) fn camera_hint(&self) -> Option<&'static str> {
        Some(match self.tool.as_str() {
            WALK => {
                "Walk: drag up to go forward, down to go back, sideways to turn · or the arrow keys · type an eye height + Enter · Esc returns to your tool"
            }
            LOOK => {
                "Look Around: drag to turn the view about where you stand · Esc returns to your tool"
            }
            WINDOW => "Zoom Window: drag a box round what to see · Esc returns to your tool",
            ZOOM => {
                "Zoom: drag up to zoom in, down to zoom out · type a field of view in degrees + Enter · Esc returns to your tool"
            }
            _ => return None,
        })
    }
}
