//! The viewport: turns egui's mouse, wheel and keys into the session's
//! events (`prism_tools::Session`, pure and replayable), and tells the GPU
//! what changed.

use egui::{Context, PointerButton, Pos2, Rect, Response};
use prism_math::DVec2;
use prism_tools::{Button, Event, Input, Key, Session};
use prism_view::overlay::Overlay;

use crate::actions::registry::Selection as Counts;
use crate::chrome::Chrome;

pub struct Viewport {
    pub session: Session,
    /// The viewport's rectangle on screen, in points.
    rect: Rect,
    /// A mouse button went down in the viewport and is still down.
    held: bool,
    /// The time egui last reported, so events carry it.
    now: f64,
    /// Sorts browser scrolls into wheel zooms and trackpad pans.
    wheel: WheelSort,
    /// A modifier pressed alone, so far (a tap toggles copy or new loop).
    tap: Option<Tap>,
    /// Ctrl (Cmd) and Alt held, as last sent to the session.
    held_keys: (bool, bool),
}

/// A modifier that toggles a tool's mode when tapped on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tap {
    Ctrl,
    Alt,
}

/// What tapping a modifier does: Ctrl copies (Move, Rotate) and Alt makes
/// a new loop (Push/Pull) on Windows and Linux; on a Mac, Option does both
/// (each tool uses its own) and Control does neither, as in SketchUp.
fn tap_inputs(tap: Tap, mac: bool) -> Vec<Input> {
    match (tap, mac) {
        (Tap::Ctrl, false) => vec![Input::CtrlTap],
        (Tap::Alt, false) => vec![Input::AltTap],
        (Tap::Alt, true) => vec![Input::CtrlTap, Input::AltTap],
        (Tap::Ctrl, true) => Vec::new(),
    }
}

/// The Paint Bucket's cursor, drawn over the viewport in place of the
/// arrow: a tipped bucket pouring the brush's colour onto the point that
/// gets painted (the pointer's tip). While sampling, an eyedropper.
fn draw_bucket(ctx: &Context, p: Pos2, colour: egui::Color32, sampling: bool) {
    use egui::{Shape, vec2};
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::unique("tool cursor"),
    ));
    use prism_ui::theme::color::{CURSOR_BODY, cursor_halo, cursor_ink, cursor_rim};
    let ink = cursor_ink();
    let halo = cursor_halo();
    let at = |x: f32, y: f32| p + vec2(x, y);
    if sampling {
        // An eyedropper: its tip on the point.
        for s in [halo, ink] {
            painter.line_segment([p, at(9.0, -9.0)], s);
        }
        painter.add(Shape::convex_polygon(
            vec![
                at(8.0, -12.0),
                at(12.0, -8.0),
                at(18.0, -14.0),
                at(14.0, -18.0),
            ],
            colour,
            ink,
        ));
        return;
    }
    // The pail, tipped so its mouth faces the point.
    painter.add(Shape::convex_polygon(
        vec![
            at(13.0, -4.5),
            at(4.5, -13.0),
            at(14.1, -20.4),
            at(20.4, -14.1),
        ],
        CURSOR_BODY,
        ink,
    ));
    // The paint pouring from the mouth onto the point.
    painter.add(Shape::convex_polygon(
        vec![
            at(10.0, -6.5),
            at(6.5, -10.0),
            at(0.8, -2.8),
            at(0.0, 0.0),
            at(2.8, -0.8),
        ],
        colour,
        ink,
    ));
    // The rim.
    painter.line_segment([at(13.0, -4.5), at(4.5, -13.0)], cursor_rim());
}

/// What the status bar shows from the viewport.
pub struct Status {
    pub hint: &'static str,
    pub measure: (String, String, bool),
    pub lock: Option<(String, [u8; 4])>,
}

impl Viewport {
    pub fn new() -> Self {
        Self::with_document(prism_tools::scenes::cube())
    }

    /// A viewport on a document (a new tab).
    pub fn with_document(doc: prism_doc::Document) -> Self {
        let mut session = Session::new(doc);
        session.measure = Some(|| {
            web_time::Instant::now()
                .duration_since(start())
                .as_secs_f64()
        });
        Self {
            session,
            rect: Rect::NOTHING,
            held: false,
            now: 0.0,
            wheel: WheelSort::default(),
            tap: None,
            held_keys: (false, false),
        }
    }

    /// Send an input as if it came from the mouse or keys (the MCP
    /// server's `use_tool`).
    pub fn drive(&mut self, input: Input) -> prism_tools::Response {
        self.send(input)
    }

    fn send(&mut self, input: Input) -> prism_tools::Response {
        self.session.handle(&Event { t: self.now, input })
    }

    fn local(&self, p: Pos2) -> (f64, f64) {
        (
            f64::from(p.x - self.rect.left()),
            f64::from(p.y - self.rect.top()),
        )
    }

    pub fn counts(&self) -> Counts {
        let s = self.session.selection();
        Counts {
            faces: s.faces.len(),
            edges: s.edges.len(),
            points: s.vertices.len(),
            groups: self.session.selected_groups().len(),
        }
    }

    pub fn can_undo(&self) -> bool {
        self.session.document().can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.session.document().can_redo()
    }

    pub fn overlay(&self) -> Overlay {
        self.session.overlay()
    }

    pub fn status(&self) -> Status {
        Status {
            hint: self.session.hint(),
            measure: self.session.measurement(),
            lock: self
                .session
                .lock_text()
                .zip(self.session.lock_color())
                .map(|(t, c)| (t, c.0)),
        }
    }

    /// An action from the menus, dock, keys or search.
    pub fn action(&mut self, id: &str, chrome: &mut Chrome) {
        if id == "help.record" {
            if self.session.is_recording() {
                if let Some(json) = self.session.stop_recording() {
                    chrome.status_note(match crate::storage::export_session(&json) {
                        Ok(where_to) => format!("Session saved to {where_to}"),
                        Err(e) => format!("Could not save the session: {e}"),
                    });
                }
            } else {
                self.session.start_recording();
                chrome.status_note(
                    "Recording the session. Pick Record Session again to stop and save it.".into(),
                );
            }
            return;
        }
        self.send(Input::Action { id: id.to_owned() });
    }

    pub fn tool_changed(&mut self, to: &'static str) {
        self.send(Input::Tool { id: to.to_owned() });
    }

    /// A modifier held now takes part in a shortcut or went with a key to
    /// another window: letting it go is not a tap (owner, 2026-11-11: Alt
    /// turned Push/Pull's new loop on unasked).
    pub fn cancel_tap(&mut self) {
        self.tap = None;
    }

    /// A key the chrome did not use. Returns true if the viewport used it.
    pub fn key(&mut self, code: &str, pressed: bool, chrome: &mut Chrome) -> bool {
        if code.starts_with("Shift") {
            self.send(if pressed {
                Input::ShiftDown
            } else {
                Input::ShiftUp
            });
            return false;
        }
        // Ctrl, Alt or Option pressed and let go on their own.
        let modifier = match code {
            "ControlLeft" | "ControlRight" => Some(Tap::Ctrl),
            "AltLeft" | "AltRight" => Some(Tap::Alt),
            _ => None,
        };
        if let Some(m) = modifier {
            if pressed {
                self.tap = Some(m);
            } else if self.tap.take() == Some(m) {
                let mac = matches!(
                    chrome.platform,
                    crate::actions::keymap::Platform::Mac
                        | crate::actions::keymap::Platform::Web { mac: true }
                );
                for input in tap_inputs(m, mac) {
                    self.send(input);
                }
            }
            return false;
        }
        if !pressed {
            return false;
        }
        // Any other key: the modifier was part of a shortcut.
        self.tap = None;
        let key = match code {
            "Enter" | "NumpadEnter" => Some(Key::Enter),
            "Escape" => Some(Key::Escape),
            "Backspace" if self.session.measurement().2 => Some(Key::Backspace),
            "ArrowUp" => Some(Key::Up),
            "ArrowDown" => Some(Key::Down),
            "ArrowLeft" => Some(Key::Left),
            "ArrowRight" => Some(Key::Right),
            _ => None,
        };
        if let Some(key) = key {
            let r = self.send(Input::Key { key });
            if let Some(t) = &r.tool
                && let Some(id) = chrome.tool_id(t)
            {
                chrome.active_tool = id;
            }
            return key != Key::Escape || r.used_escape;
        }
        false
    }

    /// Whether typed text goes to the measurement box (then it is sent).
    pub fn text(&mut self, text: &str) -> bool {
        let Some(c) = text.chars().next() else {
            return false;
        };
        if text.chars().count() == 1 && self.session.accepts_char(c) {
            self.send(Input::Text {
                text: text.to_owned(),
            });
            return true;
        }
        false
    }

    /// Advance time: easing and waking points. Returns when to wake next.
    pub fn time(&mut self, now: f64) -> Option<f64> {
        self.now = now;
        let wake = self.session.next_wake();
        if self.session.animating() || wake.is_some_and(|t| t <= now) {
            self.send(Input::Time);
        }
        if self.session.animating() {
            return Some(0.0);
        }
        self.session.next_wake().map(|t| (t - now).max(0.0))
    }

    /// Mouse and wheel input over the viewport this frame.
    pub fn input(&mut self, ctx: &Context, resp: &Response, rect: Rect, chrome: &mut Chrome) {
        if rect != self.rect {
            self.rect = rect;
            self.send(Input::Resize {
                w: f64::from(rect.width()),
                h: f64::from(rect.height()),
            });
        }
        let over = resp.hovered() || self.held;
        // Ctrl and Alt held: the Paint Bucket's preview follows them.
        let m = ctx.input(|i| i.modifiers);
        if (m.command, m.alt) != self.held_keys {
            self.held_keys = (m.command, m.alt);
            self.send(Input::Held {
                ctrl: m.command,
                alt: m.alt,
            });
        }
        if self.session.tool() == prism_tools::session::PAINT
            && let Some(p) = resp.hover_pos()
        {
            ctx.set_cursor_icon(egui::CursorIcon::None);
            let c = self.session.brush_colour().0;
            let colour = prism_ui::theme::srgba([c[0], c[1], c[2], 255]);
            draw_bucket(ctx, p, colour, self.session.paint_sampling() || m.alt);
        }
        let events = ctx.input(|i| i.events.clone());
        for e in &events {
            match e {
                egui::Event::PointerMoved(p) if over || rect.contains(*p) && resp.hovered() => {
                    let (x, y) = self.local(*p);
                    self.send(Input::Move { x, y });
                }
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } => {
                    let b = match button {
                        PointerButton::Primary => Button::Left,
                        PointerButton::Middle => Button::Middle,
                        PointerButton::Secondary => Button::Right,
                        _ => continue,
                    };
                    if *pressed {
                        // Ctrl-click and the like: not a tap.
                        self.tap = None;
                    }
                    if *pressed && rect.contains(*pos) && resp.hovered() {
                        self.held = true;
                        if b == Button::Left {
                            chrome.menu_close();
                        }
                        let (x, y) = self.local(*pos);
                        let r = self.send(Input::Press {
                            button: b,
                            x,
                            y,
                            shift: modifiers.shift,
                            ctrl: modifiers.command,
                            alt: modifiers.alt,
                        });
                        if let Some(at) = r.context_menu {
                            let p = rect.min + egui::vec2(at.x as f32, at.y as f32);
                            chrome.open_context(p, self.counts());
                        }
                    } else if !*pressed && self.held {
                        self.held = false;
                        self.send(Input::Release { button: b });
                    }
                }
                egui::Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } if resp.hovered() => {
                    let Some(p) = ctx.input(|i| i.pointer.latest_pos()) else {
                        continue;
                    };
                    let (x, y) = self.local(p);
                    let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
                    let input = match unit {
                        egui::MouseWheelUnit::Point if modifiers.ctrl || modifiers.command => {
                            // A trackpad pinch, as browsers report it.
                            Input::Pinch {
                                x,
                                y,
                                factor: (dy * 0.01).exp(),
                            }
                        }
                        // Browsers report a mouse wheel in points too: a
                        // gesture with a big vertical step zooms (DECISIONS
                        // #4, the wheel only zooms); otherwise it is a
                        // trackpad, which pans.
                        egui::MouseWheelUnit::Point => {
                            let t = ctx.input(|i| i.time);
                            for step in self.wheel.feed(t, dx, dy) {
                                self.send(match step {
                                    Step::Zoom(dy) => Input::Wheel {
                                        x,
                                        y,
                                        notches: dy / WHEEL_POINTS_PER_NOTCH,
                                    },
                                    Step::Pan(dx, dy) => Input::Scroll { x, y, dx, dy },
                                });
                            }
                            continue;
                        }
                        _ => Input::Wheel { x, y, notches: dy },
                    };
                    self.send(input);
                }
                egui::Event::Zoom(f) if resp.hovered() => {
                    let Some(p) = ctx.input(|i| i.pointer.latest_pos()) else {
                        continue;
                    };
                    let (x, y) = self.local(p);
                    self.send(Input::Pinch {
                        x,
                        y,
                        factor: f64::from(*f),
                    });
                }
                _ => {}
            }
        }
    }

    /// Where a model point is on screen (for labels), in egui points.
    pub fn to_screen(&self, p: prism_math::DVec3) -> Option<Pos2> {
        let (s, _) = self.session.view().to_screen(p)?;
        Some(self.rect.min + egui::vec2(s.x as f32, s.y as f32))
    }

    pub fn local_to_screen(&self, p: DVec2) -> Pos2 {
        self.rect.min + egui::vec2(p.x as f32, p.y as f32)
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}

/// Points a browser reports for one notch of a mouse wheel (100 in
/// Chrome, Edge and Firefox).
const WHEEL_POINTS_PER_NOTCH: f64 = 100.0;
/// A vertical step at least this big, with no sideways part, is a wheel
/// notch; trackpads send many small steps.
const WHEEL_MIN_STEP: f64 = 40.0;

/// Whether a scroll reported in points comes from a mouse wheel.
fn is_mouse_wheel(dx: f64, dy: f64) -> bool {
    dx == 0.0 && dy.abs() >= WHEEL_MIN_STEP
}

/// Scroll events this close together (seconds) are one gesture.
const GESTURE_GAP: f64 = 0.25;
/// Small vertical steps held at a gesture's start before it counts as a
/// trackpad (a wheel's first steps can be small in some browsers).
const HOLD_STEPS: usize = 3;

/// What a browser scroll becomes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    /// Points of wheel (100 a notch).
    Zoom(f64),
    Pan(f64, f64),
}

/// Sorts a browser's scrolls, gesture by gesture, into wheel zooms and
/// trackpad pans (owner, 2026-11-09: on the web a wheel panned for a
/// moment before zooming). Native builds tell the two apart themselves.
#[derive(Clone, Debug, Default)]
struct WheelSort {
    last: Option<f64>,
    /// This gesture is a wheel, a trackpad, or not known yet.
    wheel: Option<bool>,
    held: Vec<(f64, f64)>,
}

impl WheelSort {
    fn feed(&mut self, t: f64, dx: f64, dy: f64) -> Vec<Step> {
        let mut out = Vec::new();
        if self.last.is_none_or(|l| t - l > GESTURE_GAP) {
            // A new gesture: what was held from the last one was a pan.
            out.extend(self.held.drain(..).map(|(x, y)| Step::Pan(x, y)));
            self.wheel = None;
        }
        self.last = Some(t);
        match self.wheel {
            Some(true) if dx == 0.0 => out.push(Step::Zoom(dy)),
            Some(_) => {
                self.wheel = Some(false);
                out.push(Step::Pan(dx, dy));
            }
            None if is_mouse_wheel(dx, dy) => {
                self.wheel = Some(true);
                out.extend(self.held.drain(..).map(|(_, y)| Step::Zoom(y)));
                out.push(Step::Zoom(dy));
            }
            None if dx == 0.0 && self.held.len() < HOLD_STEPS => self.held.push((dx, dy)),
            None => {
                self.wheel = Some(false);
                out.extend(self.held.drain(..).map(|(x, y)| Step::Pan(x, y)));
                out.push(Step::Pan(dx, dy));
            }
        }
        out
    }
}

/// A fixed start for timings (the session only measures differences).
fn start() -> web_time::Instant {
    use std::sync::OnceLock;
    static START: OnceLock<web_time::Instant> = OnceLock::new();
    *START.get_or_init(web_time::Instant::now)
}

#[cfg(test)]
mod tests {
    use super::{Step, Tap, WheelSort, is_mouse_wheel, tap_inputs};
    use prism_tools::Input;

    #[test]
    fn a_modifier_used_with_another_key_is_not_a_tap() {
        let mut chrome = crate::chrome::Chrome::new(crate::actions::keymap::Platform::Windows);
        let mut v = super::Viewport::new();
        v.session.handle(&prism_tools::session::Event {
            t: 0.0,
            input: Input::Tool {
                id: "tool.push".into(),
            },
        });
        let tapped = |v: &super::Viewport| v.session.hint().contains("NEW LOOP");
        // Alt alone: a tap, new loop on; again: off.
        v.key("AltLeft", true, &mut chrome);
        v.key("AltLeft", false, &mut chrome);
        assert!(tapped(&v));
        v.key("AltLeft", true, &mut chrome);
        v.key("AltLeft", false, &mut chrome);
        assert!(!tapped(&v));
        // Alt with a key the menus took, or Alt+Tab away: not a tap.
        v.key("AltLeft", true, &mut chrome);
        v.cancel_tap();
        v.key("AltLeft", false, &mut chrome);
        assert!(!tapped(&v));
    }

    #[test]
    fn modifier_taps_per_platform() {
        assert_eq!(tap_inputs(Tap::Ctrl, false), [Input::CtrlTap]);
        assert_eq!(tap_inputs(Tap::Alt, false), [Input::AltTap]);
        assert_eq!(tap_inputs(Tap::Alt, true), [Input::CtrlTap, Input::AltTap]);
        assert!(tap_inputs(Tap::Ctrl, true).is_empty());
    }

    #[test]
    fn a_wheel_gesture_zooms_from_its_first_small_step() {
        let mut w = WheelSort::default();
        // Small steps first, then a notch: all of it zooms, nothing pans.
        assert!(w.feed(0.0, 0.0, 6.0).is_empty());
        assert!(w.feed(0.01, 0.0, 10.0).is_empty());
        assert_eq!(
            w.feed(0.02, 0.0, 100.0),
            [Step::Zoom(6.0), Step::Zoom(10.0), Step::Zoom(100.0)]
        );
        // The smooth tail of the same gesture zooms too.
        assert_eq!(w.feed(0.05, 0.0, 8.0), [Step::Zoom(8.0)]);
    }

    #[test]
    fn a_trackpad_gesture_pans_and_nothing_is_lost() {
        let mut w = WheelSort::default();
        assert!(w.feed(0.0, 0.0, 3.0).is_empty());
        assert!(w.feed(0.01, 0.0, 4.0).is_empty());
        assert!(w.feed(0.02, 0.0, 5.0).is_empty());
        // A fourth small step: it is a trackpad.
        assert_eq!(
            w.feed(0.03, 0.0, 6.0),
            [
                Step::Pan(0.0, 3.0),
                Step::Pan(0.0, 4.0),
                Step::Pan(0.0, 5.0),
                Step::Pan(0.0, 6.0)
            ]
        );
        // A big step later in a trackpad gesture still pans.
        assert_eq!(w.feed(0.04, 0.0, 60.0), [Step::Pan(0.0, 60.0)]);
        // Sideways motion is a trackpad at once.
        let mut w = WheelSort::default();
        assert_eq!(w.feed(0.0, 2.0, 1.0), [Step::Pan(2.0, 1.0)]);
        // A held step from an earlier gesture pans when the next begins.
        let mut w = WheelSort::default();
        assert!(w.feed(0.0, 0.0, 3.0).is_empty());
        assert_eq!(
            w.feed(1.0, 0.0, 100.0),
            [Step::Pan(0.0, 3.0), Step::Zoom(100.0)]
        );
    }

    #[test]
    fn wheel_notches_zoom_and_trackpad_scrolls_pan() {
        assert!(is_mouse_wheel(0.0, 100.0));
        assert!(is_mouse_wheel(0.0, -120.0));
        assert!(is_mouse_wheel(0.0, 53.0));
        assert!(!is_mouse_wheel(0.0, 6.0), "a small trackpad step");
        assert!(
            !is_mouse_wheel(3.0, 80.0),
            "a trackpad swipe with some sideways"
        );
    }
}
