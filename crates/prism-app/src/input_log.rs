//! A short, readable log of the input events the window receives.
//!
//! This is plain data with no winit types, so it can be tested without a
//! window. The window code turns winit events into [`InputEvent`]s.

use std::collections::VecDeque;
use std::fmt;

/// Which modifier keys are held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Cmd on macOS, the Windows key on Windows.
    pub logo: bool,
}

impl fmt::Display for Modifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = [
            (self.shift, "Shift"),
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.logo, "Logo"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        if names.is_empty() {
            f.write_str("none")
        } else {
            f.write_str(&names.join("+"))
        }
    }
}

/// How a wheel delta is measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WheelUnit {
    /// Lines or notches, typical of a mouse wheel.
    Lines,
    /// Physical pixels, typical of a trackpad.
    Pixels,
}

/// One input event, already simplified for display.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    Key {
        /// The physical key position, such as `KeyL` (not the typed letter).
        code: String,
        /// What the key typed under the current layout, if anything.
        text: Option<String>,
        pressed: bool,
        repeat: bool,
        modifiers: Modifiers,
    },
    Button {
        button: String,
        pressed: bool,
        /// Cursor position in physical pixels.
        x: f64,
        y: f64,
        modifiers: Modifiers,
    },
    Wheel {
        dx: f64,
        dy: f64,
        unit: WheelUnit,
        modifiers: Modifiers,
    },
    /// A trackpad pinch. Positive means zoom in.
    Pinch {
        delta: f64,
    },
    ModifiersChanged(Modifiers),
}

fn press(pressed: bool) -> &'static str {
    if pressed { "down" } else { "up" }
}

impl fmt::Display for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputEvent::Key {
                code,
                text,
                pressed,
                repeat,
                modifiers,
            } => {
                write!(f, "key {code} {}", press(*pressed))?;
                if *repeat {
                    f.write_str(" (repeat)")?;
                }
                if let Some(t) = text {
                    write!(f, " text {t:?}")?;
                }
                write!(f, " mods {modifiers}")
            }
            InputEvent::Button {
                button,
                pressed,
                x,
                y,
                modifiers,
            } => write!(
                f,
                "mouse {button} {} at ({x:.0}, {y:.0}) mods {modifiers}",
                press(*pressed)
            ),
            InputEvent::Wheel {
                dx,
                dy,
                unit,
                modifiers,
            } => {
                let unit = match unit {
                    WheelUnit::Lines => "lines",
                    WheelUnit::Pixels => "px",
                };
                write!(f, "wheel ({dx:.2}, {dy:.2}) {unit} mods {modifiers}")
            }
            InputEvent::Pinch { delta } => write!(f, "pinch {delta:+.3}"),
            InputEvent::ModifiersChanged(m) => write!(f, "modifiers {m}"),
        }
    }
}

/// Keeps the most recent events, newest last, with a running count.
#[derive(Clone, Debug)]
pub struct InputLog {
    capacity: usize,
    entries: VecDeque<(u64, InputEvent)>,
    total: u64,
}

impl InputLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: VecDeque::new(),
            total: 0,
        }
    }

    pub fn push(&mut self, event: InputEvent) {
        self.total += 1;
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back((self.total, event));
    }

    /// The number of events received since start, including dropped ones.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// The kept events with their sequence numbers, oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &(u64, InputEvent)> {
        self.entries.iter()
    }

    /// The kept events as display lines, oldest first.
    pub fn lines(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|(n, e)| format!("{n:>5}  {e}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: &str) -> InputEvent {
        InputEvent::Key {
            code: code.into(),
            text: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }
    }

    #[test]
    fn keeps_only_the_newest_events() {
        let mut log = InputLog::new(3);
        for c in ["KeyA", "KeyB", "KeyC", "KeyD", "KeyE"] {
            log.push(key(c));
        }
        assert_eq!(log.total(), 5);
        let kept: Vec<u64> = log.entries().map(|(n, _)| *n).collect();
        assert_eq!(kept, vec![3, 4, 5]);
        assert!(log.lines()[0].contains("KeyC"));
        assert!(log.lines()[2].contains("KeyE"));
    }

    #[test]
    fn zero_capacity_still_keeps_one() {
        let mut log = InputLog::new(0);
        log.push(key("KeyA"));
        log.push(key("KeyB"));
        assert_eq!(log.lines().len(), 1);
    }

    #[test]
    fn key_line_shows_physical_code_text_and_modifiers() {
        let e = InputEvent::Key {
            code: "KeyL".into(),
            text: Some("l".into()),
            pressed: true,
            repeat: true,
            modifiers: Modifiers {
                shift: true,
                ctrl: true,
                ..Default::default()
            },
        };
        assert_eq!(
            e.to_string(),
            "key KeyL down (repeat) text \"l\" mods Shift+Ctrl"
        );
    }

    #[test]
    fn mouse_and_wheel_lines() {
        let b = InputEvent::Button {
            button: "Middle".into(),
            pressed: false,
            x: 120.4,
            y: 33.6,
            modifiers: Modifiers::default(),
        };
        assert_eq!(b.to_string(), "mouse Middle up at (120, 34) mods none");
        let w = InputEvent::Wheel {
            dx: 0.0,
            dy: -1.0,
            unit: WheelUnit::Lines,
            modifiers: Modifiers {
                alt: true,
                ..Default::default()
            },
        };
        assert_eq!(w.to_string(), "wheel (0.00, -1.00) lines mods Alt");
        assert_eq!(
            InputEvent::Pinch { delta: 0.25 }.to_string(),
            "pinch +0.250"
        );
    }
}
