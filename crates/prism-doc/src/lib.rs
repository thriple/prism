//! The document: the model, and every edit as a command in a history, so
//! undo, redo, autosave and test replay all come from one log.
//!
//! Each step keeps a snapshot of the model before and after (the model is
//! plain data). Undo and redo put a snapshot back exactly. Id counters are
//! never wound back, so an id is never handed out twice.

use std::collections::BTreeSet;

use prism_math::DVec3;
use prism_ops::select::Selection;
use prism_ops::{pushpull, transform};
use prism_topo::{FaceId, Model, VertexId};
use serde::{Deserialize, Serialize};

/// One edit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Segment {
        p: DVec3,
        q: DVec3,
    },
    Rect {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        z: f64,
    },
    /// A closed loop of segments through these points, in one step (a
    /// rectangle on any plane).
    Loop {
        points: Vec<DVec3>,
    },
    Erase {
        selection: Selection,
    },
    PushPull {
        face: FaceId,
        distance: f64,
        new_loop: bool,
    },
    Translate {
        vertices: BTreeSet<VertexId>,
        by: DVec3,
    },
    Rotate {
        vertices: BTreeSet<VertexId>,
        centre: DVec3,
        axis: DVec3,
        angle: f64,
    },
    Scale {
        vertices: BTreeSet<VertexId>,
        centre: DVec3,
        factor: DVec3,
    },
    Duplicate {
        selection: Selection,
    },
    /// Copies of a selection, each moved or rotated one step further than
    /// the last (`count` 1 is a plain copy; more is an array), in one step.
    Copy {
        selection: Selection,
        step: Motion,
        count: u32,
    },
}

/// How a copy is placed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Motion {
    Translate {
        by: DVec3,
    },
    Rotate {
        centre: DVec3,
        axis: DVec3,
        angle: f64,
    },
}

impl Command {
    /// A short name for the Edit menu ("Undo Push/Pull").
    pub fn name(&self) -> &'static str {
        match self {
            Command::Segment { .. } => "Line",
            Command::Rect { .. } => "Rectangle",
            Command::Loop { points } if points.len() == 4 => "Rectangle",
            Command::Loop { .. } => "Shape",
            Command::Erase { .. } => "Erase",
            Command::PushPull { .. } => "Push/Pull",
            Command::Translate { .. } => "Move",
            Command::Rotate { .. } => "Rotate",
            Command::Scale { .. } => "Scale",
            Command::Duplicate { .. } => "Copy",
            Command::Copy { count, .. } if *count > 1 => "Array",
            Command::Copy { .. } => "Copy",
        }
    }

    /// Run the command on a model.
    pub fn apply(&self, m: &mut Model) {
        match self {
            Command::Segment { p, q } => {
                prism_ops::add_segment(m, *p, *q);
            }
            Command::Rect { x0, y0, x1, y1, z } => {
                prism_ops::add_rect(m, *x0, *y0, *x1, *y1, *z);
            }
            Command::Loop { points } => {
                for i in 0..points.len() {
                    prism_ops::add_segment(m, points[i], points[(i + 1) % points.len()]);
                }
            }
            Command::Erase { selection } => prism_ops::remove(m, selection),
            Command::PushPull {
                face,
                distance,
                new_loop,
            } => {
                pushpull::push_pull(m, *face, *distance, *new_loop);
            }
            Command::Translate { vertices, by } => transform::translate(m, vertices, *by),
            Command::Rotate {
                vertices,
                centre,
                axis,
                angle,
            } => transform::rotate(m, vertices, *centre, *axis, *angle),
            Command::Scale {
                vertices,
                centre,
                factor,
            } => transform::scale(m, vertices, *centre, *factor),
            Command::Duplicate { selection } => {
                prism_ops::duplicate(m, selection);
            }
            Command::Copy {
                selection,
                step,
                count,
            } => {
                for k in 1..=*count {
                    let copy = prism_ops::duplicate(m, selection);
                    let vs = copy.all_vertices(m);
                    let k = f64::from(k);
                    match *step {
                        Motion::Translate { by } => transform::translate(m, &vs, by * k),
                        Motion::Rotate {
                            centre,
                            axis,
                            angle,
                        } => transform::rotate(m, &vs, centre, axis, angle * k),
                    }
                }
            }
        }
    }
}

/// A command and the model before and after it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Step {
    command: Command,
    before: Model,
    after: Model,
}

/// The model and its history.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    model: Model,
    done: Vec<Step>,
    undone: Vec<Step>,
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start from an existing model (with an empty history).
    pub fn with_model(model: Model) -> Self {
        Self {
            model,
            ..Self::default()
        }
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Run a command. A command that changes nothing is not recorded.
    /// Returns whether it changed the model.
    pub fn run(&mut self, command: Command) -> bool {
        let before = self.model.clone();
        command.apply(&mut self.model);
        if self.model == before {
            return false;
        }
        self.done.push(Step {
            command,
            before,
            after: self.model.clone(),
        });
        self.undone.clear();
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// The command Undo would undo.
    pub fn undo_name(&self) -> Option<&'static str> {
        self.done.last().map(|s| s.command.name())
    }

    pub fn redo_name(&self) -> Option<&'static str> {
        self.undone.last().map(|s| s.command.name())
    }

    pub fn undo(&mut self) -> bool {
        let Some(step) = self.done.pop() else {
            return false;
        };
        let counters = self.model.counters();
        self.model = step.before.clone();
        self.model.keep_counters_at_least(counters);
        self.undone.push(step);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(step) = self.undone.pop() else {
            return false;
        };
        let counters = self.model.counters();
        self.model = step.after.clone();
        self.model.keep_counters_at_least(counters);
        self.done.push(step);
        true
    }

    /// The commands run so far, oldest first (for replay and autosave).
    pub fn log(&self) -> impl Iterator<Item = &Command> {
        self.done.iter().map(|s| &s.command)
    }

    /// Run a log of commands from an empty model.
    pub fn replay<'a>(commands: impl IntoIterator<Item = &'a Command>) -> Document {
        let mut d = Document::new();
        for c in commands {
            d.run(c.clone());
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmds() -> Vec<Command> {
        vec![
            Command::Rect {
                x0: 0.0,
                y0: 0.0,
                x1: 4.0,
                y1: 4.0,
                z: 0.0,
            },
            Command::Rect {
                x0: 2.0,
                y0: 2.0,
                x1: 6.0,
                y1: 6.0,
                z: 0.0,
            },
            Command::Segment {
                p: DVec3::new(0.0, 1.0, 0.0),
                q: DVec3::new(6.0, 1.0, 0.0),
            },
        ]
    }

    #[test]
    fn undo_and_redo_restore_the_model_exactly() {
        let mut d = Document::new();
        for c in cmds() {
            let before = d.model().geometry_json();
            assert!(d.run(c));
            let after = d.model().to_json();
            assert!(d.undo());
            assert_eq!(
                d.model().geometry_json(),
                before,
                "undo restores the geometry"
            );
            assert!(d.redo());
            assert_eq!(
                d.model().to_json(),
                after,
                "redo restores everything, ids included"
            );
        }
    }

    #[test]
    fn ids_are_not_handed_out_twice_after_undo() {
        let mut d = Document::new();
        d.run(cmds()[0].clone());
        let first: Vec<_> = d.model().faces().map(|(id, _)| id).collect();
        d.undo();
        d.run(cmds()[1].clone());
        let second: Vec<_> = d.model().faces().map(|(id, _)| id).collect();
        assert!(first.iter().all(|f| !second.contains(f)));
    }

    #[test]
    fn a_command_that_changes_nothing_is_not_recorded() {
        let mut d = Document::new();
        d.run(cmds()[0].clone());
        assert!(!d.run(cmds()[0].clone()));
        assert_eq!(d.log().count(), 1);
        assert_eq!(d.undo_name(), Some("Rectangle"));
    }

    #[test]
    fn replaying_the_log_gives_the_same_model() {
        let mut d = Document::new();
        for c in cmds() {
            d.run(c);
        }
        let again = Document::replay(d.log());
        assert_eq!(again.model().to_json(), d.model().to_json());
    }

    #[test]
    fn a_new_command_clears_redo() {
        let mut d = Document::new();
        d.run(cmds()[0].clone());
        d.undo();
        assert!(d.can_redo());
        d.run(cmds()[1].clone());
        assert!(!d.can_redo());
    }

    #[test]
    fn the_document_saves_and_loads() {
        let mut d = Document::new();
        for c in cmds() {
            d.run(c);
        }
        let text = serde_json::to_string(&d).expect("saves");
        let back: Document = serde_json::from_str(&text).expect("loads");
        assert_eq!(back, d);
    }
}
