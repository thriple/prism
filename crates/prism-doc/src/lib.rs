//! The document: the geometry, and every edit as a command in a history, so
//! undo, redo, autosave and test replay all come from one log.
//!
//! The geometry is an [`Assembly`]: the root model and its groups. Each step
//! keeps the assembly before and after; definitions it did not change are
//! shared, not copied. Undo and redo put a snapshot back exactly. Id
//! counters are never wound back, so an id is never handed out twice.

use std::collections::BTreeSet;

pub mod assembly;

pub use assembly::{
    Assembly, Attributes, DefId, Definition, Instance, InstanceId, Placed, ROOT, Tag, TagId,
};
use prism_math::{DAffine3, DQuat, DVec3};
use prism_ops::select::Selection;
use prism_ops::{pushpull, transform};
use prism_topo::{FaceId, Guide, Model, VertexId};
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
    /// Copies of a selection, each moved or rotated one step further than
    /// the last (`count` 1 is a plain copy; more is an array), in one step.
    Copy {
        selection: Selection,
        step: Motion,
        count: u32,
    },
    /// Offset a face's loops (positive into the face).
    Offset {
        face: FaceId,
        distance: f64,
    },
    /// Add a guide line or point.
    AddGuide {
        guide: Guide,
    },
    /// Add imported geometry beside the model (not joined to it).
    Import {
        model: Model,
    },
    /// Turn where faces cross into edges (all faces, or those crossings
    /// that involve a face in `faces`).
    IntersectFaces {
        faces: Option<BTreeSet<FaceId>>,
    },
    /// Model Health (PLAN 1.10): turn faces round so each shell agrees and
    /// faces out (all, or only these).
    OrientFaces {
        faces: Option<BTreeSet<FaceId>>,
    },
    /// Turn these faces round.
    ReverseFaces {
        faces: BTreeSet<FaceId>,
    },
    /// Remove interior faces (all, or only these).
    CleanEnvelope {
        faces: Option<BTreeSet<FaceId>>,
    },
    /// Remove edges in no face, and points left on nothing.
    RemoveStray,
    /// A command run inside an open group (the path of instances to it).
    In {
        context: Vec<InstanceId>,
        command: Box<Command>,
    },
    /// Several commands as one step (geometry and groups selected together).
    Batch {
        commands: Vec<Command>,
    },
    /// Make Group from a selection and selected groups.
    MakeGroup {
        selection: Selection,
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        instances: BTreeSet<InstanceId>,
    },
    /// Explode a group into its parent.
    Explode {
        instance: InstanceId,
    },
    /// Make Component from a selection and selected groups.
    MakeComponent {
        selection: Selection,
        #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
        instances: BTreeSet<InstanceId>,
        name: String,
    },
    /// Give each instance its own copy of its component.
    MakeUnique {
        instances: BTreeSet<InstanceId>,
    },
    /// Rename a component.
    Rename {
        def: DefId,
        name: String,
    },
    /// Hide or show instances (anywhere: instance ids are unique).
    SetHidden {
        instances: BTreeSet<InstanceId>,
        hidden: bool,
    },
    /// Name an instance (empty clears it).
    NameInstance {
        instance: InstanceId,
        name: String,
    },
    /// Add a tag.
    AddTag {
        name: String,
    },
    RenameTag {
        tag: TagId,
        name: String,
    },
    /// Show or hide everything on a tag.
    SetTagVisible {
        tag: TagId,
        visible: bool,
    },
    /// Put instances on a tag (None: on no tag).
    SetTag {
        instances: BTreeSet<InstanceId>,
        tag: Option<TagId>,
    },
    /// Move, rotate or scale whole groups.
    Place {
        instances: BTreeSet<InstanceId>,
        change: Change,
    },
    /// Copies of groups, each one step further than the last.
    CopyInstances {
        instances: BTreeSet<InstanceId>,
        step: Change,
        count: u32,
    },
    /// Erase groups.
    EraseInstances {
        instances: BTreeSet<InstanceId>,
    },
}

/// How groups are moved, turned or scaled.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Change {
    Translate {
        by: DVec3,
    },
    Rotate {
        centre: DVec3,
        axis: DVec3,
        angle: f64,
    },
    Scale {
        centre: DVec3,
        factor: DVec3,
    },
}

impl Change {
    /// The change applied `k` times.
    pub fn affine(&self, k: f64) -> DAffine3 {
        match *self {
            Change::Translate { by } => DAffine3::from_translation(by * k),
            Change::Rotate {
                centre,
                axis,
                angle,
            } => {
                DAffine3::from_translation(centre)
                    * DAffine3::from_quat(DQuat::from_axis_angle(axis.normalize(), angle * k))
                    * DAffine3::from_translation(-centre)
            }
            Change::Scale { centre, factor } => {
                DAffine3::from_translation(centre)
                    * DAffine3::from_scale(factor.powf(k))
                    * DAffine3::from_translation(-centre)
            }
        }
    }
}

/// A change of groups, in a group's own coordinates.
fn change_to_local(c: &Change, to_local: &DAffine3) -> Change {
    match *c {
        Change::Translate { by } => Change::Translate {
            by: to_local.transform_vector3(by),
        },
        Change::Rotate {
            centre,
            axis,
            angle,
        } => Change::Rotate {
            centre: to_local.transform_point3(centre),
            axis: to_local.transform_vector3(axis).normalize(),
            angle,
        },
        Change::Scale { centre, factor } => Change::Scale {
            centre: to_local.transform_point3(centre),
            factor,
        },
    }
}

impl From<Motion> for Change {
    fn from(m: Motion) -> Change {
        match m {
            Motion::Translate { by } => Change::Translate { by },
            Motion::Rotate {
                centre,
                axis,
                angle,
            } => Change::Rotate {
                centre,
                axis,
                angle,
            },
        }
    }
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
            Command::Copy { count, .. } if *count > 1 => "Array",
            Command::Copy { .. } => "Copy",
            Command::Offset { .. } => "Offset",
            Command::AddGuide { .. } => "Guide",
            Command::Import { .. } => "Import",
            Command::IntersectFaces { .. } => "Intersect Faces",
            Command::In { command, .. } => command.name(),
            Command::OrientFaces { .. } => "Orient Faces",
            Command::ReverseFaces { .. } => "Reverse Faces",
            Command::CleanEnvelope { .. } => "Clean Envelope",
            Command::RemoveStray => "Remove Stray Edges",
            Command::Batch { commands } => commands.first().map_or("Edit", |c| c.name()),
            Command::MakeGroup { .. } => "Make Group",
            Command::Explode { .. } => "Explode",
            Command::MakeComponent { .. } => "Make Component",
            Command::MakeUnique { .. } => "Make Unique",
            Command::Rename { .. } | Command::RenameTag { .. } => "Rename",
            Command::SetHidden { hidden: true, .. } => "Hide",
            Command::SetHidden { .. } => "Unhide",
            Command::NameInstance { .. } => "Name",
            Command::AddTag { .. } => "Add Tag",
            Command::SetTagVisible { .. } => "Tag Visibility",
            Command::SetTag { .. } => "Tag",
            Command::Place { change, .. } => match change {
                Change::Translate { .. } => "Move",
                Change::Rotate { .. } => "Rotate",
                Change::Scale { .. } => "Scale",
            },
            Command::CopyInstances { count, .. } if *count > 1 => "Array",
            Command::CopyInstances { .. } => "Copy",
            Command::EraseInstances { .. } => "Erase",
        }
    }

    /// The command in a group's own coordinates: tools work in the world,
    /// and `to_local` maps the world into the open group (`model` is the
    /// group's model, for the faces a distance is measured along).
    pub fn to_local(&self, to_local: &DAffine3, model: &Model) -> Command {
        let p = |v: DVec3| to_local.transform_point3(v);
        let dir = |v: DVec3| to_local.transform_vector3(v);
        // A distance along a face's normal, in the group's units.
        let along = |face: FaceId, d: f64| -> f64 {
            let Some(f) = model.face(face) else { return d };
            let n = f.plane().normal;
            // The world normal is the local one through the inverse
            // transpose; a world step d along it, brought back.
            let world_n = to_local.matrix3.transpose().mul_vec3(n).normalize();
            dir(world_n * d).dot(n)
        };
        let scale = to_local.matrix3.determinant().abs().cbrt();
        match self {
            Command::Segment { p: a, q: b } => Command::Segment { p: p(*a), q: p(*b) },
            Command::Rect { x0, y0, x1, y1, z } => Command::Loop {
                points: [
                    DVec3::new(*x0, *y0, *z),
                    DVec3::new(*x1, *y0, *z),
                    DVec3::new(*x1, *y1, *z),
                    DVec3::new(*x0, *y1, *z),
                ]
                .into_iter()
                .map(p)
                .collect(),
            },
            Command::Loop { points } => Command::Loop {
                points: points.iter().map(|v| p(*v)).collect(),
            },
            Command::PushPull {
                face,
                distance,
                new_loop,
            } => Command::PushPull {
                face: *face,
                distance: along(*face, *distance),
                new_loop: *new_loop,
            },
            Command::Translate { vertices, by } => Command::Translate {
                vertices: vertices.clone(),
                by: dir(*by),
            },
            Command::Rotate {
                vertices,
                centre,
                axis,
                angle,
            } => Command::Rotate {
                vertices: vertices.clone(),
                centre: p(*centre),
                axis: dir(*axis).normalize(),
                angle: *angle,
            },
            // Scale works along the group's own axes, as in SketchUp.
            Command::Scale {
                vertices,
                centre,
                factor,
            } => Command::Scale {
                vertices: vertices.clone(),
                centre: p(*centre),
                factor: *factor,
            },
            Command::Copy {
                selection,
                step,
                count,
            } => Command::Copy {
                selection: selection.clone(),
                step: match *step {
                    Motion::Translate { by } => Motion::Translate { by: dir(by) },
                    Motion::Rotate {
                        centre,
                        axis,
                        angle,
                    } => Motion::Rotate {
                        centre: p(centre),
                        axis: dir(axis).normalize(),
                        angle,
                    },
                },
                count: *count,
            },
            Command::Offset { face, distance } => Command::Offset {
                face: *face,
                distance: distance * scale,
            },
            Command::AddGuide { guide } => Command::AddGuide {
                guide: match *guide {
                    Guide::Line { origin, dir: d } => Guide::Line {
                        origin: p(origin),
                        dir: dir(d).normalize(),
                    },
                    Guide::Point { at } => Guide::Point { at: p(at) },
                },
            },
            Command::Place { instances, change } => Command::Place {
                instances: instances.clone(),
                change: change_to_local(change, to_local),
            },
            Command::CopyInstances {
                instances,
                step,
                count,
            } => Command::CopyInstances {
                instances: instances.clone(),
                step: change_to_local(step, to_local),
                count: *count,
            },
            Command::Batch { commands } => Command::Batch {
                commands: commands
                    .iter()
                    .map(|c| c.to_local(to_local, model))
                    .collect(),
            },
            other => other.clone(),
        }
    }

    /// Run the command inside definition `ctx` of an assembly.
    pub fn apply_in(&self, asm: &mut Assembly, ctx: DefId) {
        match self {
            Command::In { context, command } => {
                if let Some(def) = asm.context_def(context) {
                    command.apply_in(asm, def);
                }
            }
            Command::Batch { commands } => {
                for c in commands {
                    c.apply_in(asm, ctx);
                }
            }
            Command::MakeGroup {
                selection,
                instances,
            } => {
                asm.make_group(ctx, selection, instances);
            }
            Command::Explode { instance } => {
                asm.explode(ctx, *instance);
            }
            Command::MakeComponent {
                selection,
                instances,
                name,
            } => {
                asm.make_component(ctx, selection, instances, name);
            }
            Command::MakeUnique { instances } => asm.make_unique(ctx, instances),
            Command::Rename { def, name } => {
                asm.rename(*def, name);
            }
            Command::SetHidden { instances, hidden } => asm.set_hidden(instances, *hidden),
            Command::NameInstance { instance, name } => asm.set_name(*instance, name),
            Command::AddTag { name } => {
                asm.add_tag(name);
            }
            Command::RenameTag { tag, name } => {
                asm.rename_tag(*tag, name);
            }
            Command::SetTagVisible { tag, visible } => asm.set_tag_visible(*tag, *visible),
            Command::SetTag { instances, tag } => asm.set_tag(instances, *tag),
            Command::Place { instances, change } => {
                asm.place(ctx, instances, &change.affine(1.0));
            }
            Command::CopyInstances {
                instances,
                step,
                count,
            } => {
                let steps: Vec<DAffine3> =
                    (1..=*count).map(|k| step.affine(f64::from(k))).collect();
                asm.copy(ctx, instances, &steps);
            }
            Command::EraseInstances { instances } => asm.erase(ctx, instances),
            geometry => {
                if let Some(d) = asm.def_mut(ctx) {
                    geometry.apply(&mut d.model);
                }
            }
        }
    }

    /// Run a geometry command on a model (commands that change groups do
    /// nothing here; see [`Command::apply_in`]).
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
            Command::Copy {
                selection,
                step,
                count,
            } => {
                // Place every copy first, then run the coplanar rule once,
                // so overlapping copies split each other, not the original
                // before the next copy is taken.
                let mut placed: Vec<BTreeSet<VertexId>> = Vec::new();
                for k in 1..=*count {
                    let copy = prism_ops::duplicate(m, selection);
                    let vs = copy.all_vertices(m);
                    let k = f64::from(k);
                    match *step {
                        Motion::Translate { by } => transform::translate_only(m, &vs, by * k),
                        Motion::Rotate {
                            centre,
                            axis,
                            angle,
                        } => transform::rotate_only(m, &vs, centre, axis, angle * k),
                    }
                    placed.push(vs);
                }
                // Copies that landed on each other were welded (the lower id
                // stays), so the vertices left are the ones to settle.
                for g in &mut placed {
                    g.retain(|v| m.has_vertex(*v));
                }
                prism_ops::settle::settle_rigid(m, &placed);
            }
            Command::Offset { face, distance } => {
                prism_ops::offset::offset(m, *face, *distance);
            }
            Command::AddGuide { guide } => {
                m.add_guide(*guide);
            }
            Command::Import { model } => {
                m.append(model);
            }
            Command::IntersectFaces { faces } => {
                prism_ops::intersect::intersect_faces(m, faces.as_ref());
            }
            Command::OrientFaces { faces } => {
                prism_ops::health::orient(m, faces.as_ref());
            }
            Command::ReverseFaces { faces } => prism_ops::health::reverse(m, faces),
            Command::CleanEnvelope { faces } => {
                prism_ops::health::clean_envelope(m, faces.as_ref());
            }
            Command::RemoveStray => {
                prism_ops::health::remove_stray(m);
            }
            // These change groups: run them with `apply_in`.
            Command::In { .. }
            | Command::Batch { .. }
            | Command::MakeGroup { .. }
            | Command::Explode { .. }
            | Command::MakeComponent { .. }
            | Command::MakeUnique { .. }
            | Command::Rename { .. }
            | Command::SetHidden { .. }
            | Command::NameInstance { .. }
            | Command::AddTag { .. }
            | Command::RenameTag { .. }
            | Command::SetTagVisible { .. }
            | Command::SetTag { .. }
            | Command::Place { .. }
            | Command::CopyInstances { .. }
            | Command::EraseInstances { .. } => {}
        }
    }
}

/// A command and the geometry before and after it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Step {
    command: Command,
    before: Assembly,
    after: Assembly,
}

/// The geometry and its history.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    asm: Assembly,
    done: Vec<Step>,
    undone: Vec<Step>,
    /// Counts changes (runs, undos and redos), so the app can tell whether
    /// the document changed since it was saved.
    #[serde(default)]
    revision: u64,
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start from an existing model (with an empty history).
    pub fn with_model(model: Model) -> Self {
        Self::with_assembly(Assembly::with_model(model))
    }

    /// Start from existing geometry and groups (with an empty history).
    pub fn with_assembly(asm: Assembly) -> Self {
        Self {
            asm,
            ..Self::default()
        }
    }

    /// The root model: the geometry outside every group.
    pub fn model(&self) -> &Model {
        &self.asm.root().model
    }

    /// The geometry and its groups.
    pub fn assembly(&self) -> &Assembly {
        &self.asm
    }

    /// Run a command (wrapped in [`Command::In`] to run it inside a group).
    /// A command that changes nothing is not recorded. Returns whether it
    /// changed anything.
    pub fn run(&mut self, command: Command) -> bool {
        let before = self.asm.clone();
        command.apply_in(&mut self.asm, ROOT);
        if self.asm == before {
            // Put it back exactly: a command can change a value without
            // changing it (0.0 to -0.0 compare equal), and the geometry must
            // stay what the history says it is.
            self.asm = before;
            return false;
        }
        self.done.push(Step {
            command,
            before,
            after: self.asm.clone(),
        });
        self.undone.clear();
        self.revision += 1;
        true
    }

    /// Run a command inside the group at `context` (the root for none).
    pub fn run_in(&mut self, context: &[InstanceId], command: Command) -> bool {
        if context.is_empty() {
            self.run(command)
        } else {
            self.run(Command::In {
                context: context.to_vec(),
                command: Box::new(command),
            })
        }
    }

    /// The group the last step was in (Undo reopens it), and the next.
    pub fn undo_context(&self) -> Vec<InstanceId> {
        self.done
            .last()
            .map(|s| context_of(&s.command))
            .unwrap_or_default()
    }

    pub fn redo_context(&self) -> Vec<InstanceId> {
        self.undone
            .last()
            .map(|s| context_of(&s.command))
            .unwrap_or_default()
    }

    /// Goes up with every change, undo and redo.
    pub fn revision(&self) -> u64 {
        self.revision
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
        let now = std::mem::replace(&mut self.asm, step.before.clone());
        self.asm.keep_counters_from(&now);
        self.undone.push(step);
        self.revision += 1;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(step) = self.undone.pop() else {
            return false;
        };
        let now = std::mem::replace(&mut self.asm, step.after.clone());
        self.asm.keep_counters_from(&now);
        self.done.push(step);
        self.revision += 1;
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

fn context_of(c: &Command) -> Vec<InstanceId> {
    match c {
        Command::In { context, .. } => context.clone(),
        _ => Vec::new(),
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
