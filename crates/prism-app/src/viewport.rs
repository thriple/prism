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
        }
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
        if !pressed {
            return false;
        }
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
                        // Browsers report a mouse wheel in points too: big
                        // vertical steps zoom (DECISIONS #4, the wheel only
                        // zooms); small or sideways ones are a trackpad, which pans.
                        egui::MouseWheelUnit::Point if is_mouse_wheel(dx, dy) => Input::Wheel {
                            x,
                            y,
                            notches: dy / WHEEL_POINTS_PER_NOTCH,
                        },
                        egui::MouseWheelUnit::Point => Input::Scroll { x, y, dx, dy },
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

/// A fixed start for timings (the session only measures differences).
fn start() -> web_time::Instant {
    use std::sync::OnceLock;
    static START: OnceLock<web_time::Instant> = OnceLock::new();
    *START.get_or_init(web_time::Instant::now)
}

#[cfg(test)]
mod tests {
    use super::is_mouse_wheel;

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
