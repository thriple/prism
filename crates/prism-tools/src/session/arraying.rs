//! The Array tool (Shapes & Placement part 2): copies of the selected
//! groups, components, faces and edges in a row or round an axis. The
//! Array window sets the kind, the count and the step; a ghost shows the
//! result while the window has it, and Make Array does it in one undo step.
//! (Move and Rotate can already make arrays by copying and typing `x5`;
//! this adds exact numbers and a preview.)

use prism_doc::{Change, Command, Motion};
use prism_math::DVec3;

use super::Session;

pub const ARRAY_TOOL: &str = "tool.array";

/// The most copies one array makes, so a typo cannot freeze the program.
pub const MAX_COPIES: u32 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayKind {
    Linear,
    Polar,
}

/// The Array window's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct ArraySettings {
    pub kind: ArrayKind,
    /// How many there are in all, the original included.
    pub count: u32,
    /// Linear: the step from one to the next, in metres, along `axis`.
    pub spacing: f64,
    /// Polar: the angle all the copies spread over, in degrees. 360 is a
    /// full circle (the last copy stops short of the first).
    pub angle: f64,
    /// The world axis: the row's direction, or what the copies turn about.
    pub axis: usize,
}

impl Default for ArraySettings {
    fn default() -> Self {
        Self {
            kind: ArrayKind::Linear,
            count: 3,
            spacing: 1.0,
            angle: 360.0,
            axis: 0,
        }
    }
}

/// One change in the window.
#[derive(Clone, Debug, PartialEq)]
pub enum ArraySet {
    Kind(ArrayKind),
    /// Typed: a whole number, at least 2.
    Count(String),
    /// Typed in the document's unit.
    Spacing(String),
    /// Typed in degrees.
    Angle(String),
    Axis(usize),
    /// Make the array.
    Make,
}

/// What the Array window shows.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrayPanel {
    pub settings: ArraySettings,
    pub count: String,
    pub spacing: String,
    pub angle: String,
    /// What the window says about the selection.
    pub how: &'static str,
}

impl Session {
    /// The window's content, while the Array tool is active.
    pub fn array_panel(&self) -> Option<ArrayPanel> {
        (self.tool == ARRAY_TOOL).then(|| {
            let s = self.array_set.clone();
            ArrayPanel {
                count: s.count.to_string(),
                spacing: self.number.show(s.spacing),
                angle: format!("{}", (s.angle * 1000.0).round() / 1000.0),
                how: self.array_hint().unwrap_or("Select what to array"),
                settings: s,
            }
        })
    }

    pub(super) fn array_hint(&self) -> Option<&'static str> {
        if self.tool != ARRAY_TOOL {
            return None;
        }
        Some(if self.groups.is_empty() && self.selection.is_empty() {
            "Select groups, components, faces or edges to array, then set the count and step in the window"
        } else {
            "The ghost shows the array · press Make Array in the window to do it"
        })
    }

    /// The command that makes the array, in the world: None when nothing
    /// is selected or the settings make no copies.
    fn array_command(&self) -> Option<Command> {
        let s = &self.array_set;
        let copies = s.count.saturating_sub(1).min(MAX_COPIES);
        if copies == 0 {
            return None;
        }
        let mut axis = DVec3::ZERO;
        axis[s.axis.min(2)] = 1.0;
        // The middle of what is selected, for a turn.
        let mut points = self.groups_corners();
        let work = self.work_model();
        points.extend(
            self.selection
                .all_vertices(work)
                .into_iter()
                .map(|v| work.vertex(v)),
        );
        if points.is_empty() {
            return None;
        }
        let (lo, hi) = points
            .iter()
            .fold((points[0], points[0]), |(l, h), p| (l.min(*p), h.max(*p)));
        let centre = (lo + hi) * 0.5;
        let motion = match s.kind {
            ArrayKind::Linear => Motion::Translate {
                by: axis * s.spacing,
            },
            ArrayKind::Polar => {
                // All the way round: n copies share 360, the last stopping
                // short of the first. Less: they spread over the angle.
                let total = s.angle.to_radians();
                let step = if (s.angle.abs() - 360.0).abs() < 1e-9 {
                    total / f64::from(s.count)
                } else {
                    total / f64::from(s.count - 1)
                };
                Motion::Rotate {
                    centre,
                    axis,
                    angle: step,
                }
            }
        };
        let loose = (!self.selection.is_empty()).then(|| Command::Copy {
            selection: self.selection.clone(),
            step: motion,
            count: copies,
        });
        let groups = (!self.groups.is_empty()).then(|| Command::CopyInstances {
            instances: self.groups.clone(),
            step: Change::from(motion),
            count: copies,
        });
        match (loose, groups) {
            (Some(a), Some(b)) => Some(Command::Batch {
                commands: vec![a, b],
            }),
            (a, b) => a.or(b),
        }
    }

    /// Change a setting, or make the array.
    pub fn array_input(&mut self, op: ArraySet) {
        match op {
            ArraySet::Kind(k) => self.array_set.kind = k,
            ArraySet::Count(t) => {
                if let Ok(n) = t.trim().parse::<u32>() {
                    self.array_set.count = n.clamp(2, MAX_COPIES + 1);
                }
            }
            ArraySet::Spacing(t) => {
                if let Some(d) = self.number.length(t.trim()).filter(|d| *d != 0.0) {
                    self.array_set.spacing = d;
                }
            }
            ArraySet::Angle(t) => {
                if let Ok(a) = t.trim().trim_end_matches('°').parse::<f64>()
                    && a.abs() > 0.0
                    && a.abs() <= 360.0
                {
                    self.array_set.angle = a;
                }
            }
            ArraySet::Axis(a) => self.array_set.axis = a.min(2),
            ArraySet::Make => {
                if let Some(c) = self.array_command() {
                    self.set_preview_scene(None);
                    self.cancel();
                    self.run(c);
                } else {
                    self.notice = Some(ARRAY_NOTHING);
                }
                self.overlay_dirty = true;
                self.dirty = true;
                return;
            }
        }
        self.array_preview();
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Show the array as a ghost while the tool is active.
    pub(super) fn array_preview(&mut self) {
        if self.tool != ARRAY_TOOL {
            return;
        }
        let scene = self.array_command().map(|c| self.preview_of(&c));
        self.set_preview_scene(scene);
    }
}

/// Said when Make Array is pressed with nothing selected.
pub const ARRAY_NOTHING: &str = "Select something to array first";
