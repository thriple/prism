//! The Inspector, part 1 (PLAN 1.17, brief 48): a panel that shows what is
//! selected and changes it.
//!
//! It is schema-driven: [`Session::inspector`] returns sections of fields
//! (a key, a label, a kind, the text shown), and the window only draws
//! them. [`Session::inspector_set`] takes a field's key and the typed
//! text, so the window, the MCP server and the tests all use one path.
//! Each committed edit is one undo step. With several groups selected, a
//! field they disagree on is shown as mixed ("—") and an edit applies to
//! all of them.

use std::collections::BTreeSet;

use prism_doc::{Change, Command, InstanceId};
use prism_math::DVec3;
use prism_ops::validate::validate;

use super::Session;
use super::clip::LOCKED;

/// What kind of control a field is.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldKind {
    /// Words (a name).
    Text,
    /// A length, typed with units or a simple expression.
    Length,
    /// "true" or "false".
    Toggle,
    /// One of these words.
    Choice(Vec<String>),
    /// Shown, not edited.
    Fixed,
    /// A button: the field's text is its label, and setting it (with any
    /// text) does it.
    Action,
}

/// One row of the Inspector.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    /// The value as text (empty when mixed).
    pub text: String,
    /// The selected things differ: show "—".
    pub mixed: bool,
}

/// A titled group of fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub title: &'static str,
    pub fields: Vec<Field>,
}

/// Everything the Inspector shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Inspector {
    /// What is selected ("Group", "3 groups", "2 faces, 4 edges").
    pub summary: String,
    /// The open groups the selection sits in, outermost first.
    pub path: Vec<String>,
    pub sections: Vec<Section>,
}

/// The volume last worked out, for the selection it was worked out for.
pub type VolumeCache = std::cell::RefCell<Option<(u64, BTreeSet<InstanceId>, String)>>;

const NONE: &str = "None";

/// Said when an edit of a face, edge or corner a live feature made baked
/// that feature to do it.
pub(super) const FEATURE_BAKED: &str =
    "A fillet or chamfer was baked to edit its faces. Undo puts it back.";

/// Whether the command is one that takes features off on purpose.
pub(super) fn takes_features_off(c: &Command) -> bool {
    match c {
        Command::BakeFeatures { .. } | Command::RemoveFeature { .. } => true,
        Command::Batch { commands } => commands.iter().any(takes_features_off),
        Command::In { command, .. } => takes_features_off(command),
        _ => false,
    }
}
/// The least size a group can have on an axis and still be scaled.
const FLAT: f64 = 1e-9;

fn field(key: &str, label: &str, kind: FieldKind, text: String) -> Field {
    Field {
        key: key.to_owned(),
        label: label.to_owned(),
        kind,
        text,
        mixed: false,
    }
}

/// The one value all agree on, or None when they differ.
fn agree<T: PartialEq>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}

impl Session {
    /// How many live features the document has.
    pub(super) fn live_features(&self) -> usize {
        self.doc
            .assembly()
            .defs()
            .filter_map(|(_, d)| d.stack.as_ref())
            .map(|s| s.features.len())
            .sum()
    }

    /// The Inspector for the selection.
    pub fn inspector(&self) -> Inspector {
        let asm = self.doc.assembly();
        let path = self
            .ctx
            .iter()
            .map(|i| asm.name_of(*i).filter(|n| !n.is_empty()).unwrap_or("Group"))
            .map(str::to_owned)
            .collect();
        let mut out = Inspector {
            path,
            ..Default::default()
        };
        if self.groups.is_empty() {
            self.inspect_geometry(&mut out);
        } else {
            self.inspect_groups(&mut out);
        }
        out
    }

    fn inspect_geometry(&self, out: &mut Inspector) {
        let sel = &self.selection;
        if sel.is_empty() {
            out.summary = "Nothing selected".into();
            return;
        }
        let m = self.work_model();
        let line = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
        out.summary = [
            (sel.faces.len(), "face"),
            (sel.edges.len(), "edge"),
            (sel.vertices.len(), "point"),
        ]
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, w)| line(*n, w))
        .collect::<Vec<_>>()
        .join(", ");
        let mut fields = Vec::new();
        if !sel.faces.is_empty() {
            let area: f64 = sel.faces.iter().map(|f| m.face_area(*f)).sum();
            fields.push(field(
                "area",
                "Area",
                FieldKind::Fixed,
                self.area_text(area),
            ));
            if let Some((front, back)) = self.selection_materials() {
                fields.push(field("front", "Front", FieldKind::Fixed, front));
                fields.push(field("back", "Back", FieldKind::Fixed, back));
            }
        }
        if !fields.is_empty() {
            out.sections.push(Section {
                title: "Geometry",
                fields,
            });
        }
    }

    fn inspect_groups(&self, out: &mut Inspector) {
        let asm = self.doc.assembly();
        let ids: Vec<InstanceId> = self.groups.iter().copied().collect();
        let n = ids.len();
        out.summary = if n == 1 {
            let def = asm
                .context_def(&self.path_of(ids[0]))
                .and_then(|d| asm.def(d));
            def.and_then(|d| d.name.clone())
                .map_or("Group".to_owned(), |name| format!("Component \"{name}\""))
        } else {
            format!("{n} groups")
        };

        let mut general = Vec::new();
        if n == 1 {
            general.push(field(
                "name",
                "Name",
                FieldKind::Text,
                asm.name_of(ids[0]).unwrap_or_default().to_owned(),
            ));
        }
        let tags = self.tag_rows();
        let mut choices = vec![NONE.to_owned()];
        choices.extend(tags.iter().map(|t| t.1.clone()));
        let tag_name = |i: &InstanceId| {
            asm.tag_of(*i)
                .and_then(|t| tags.iter().find(|r| r.0 == t))
                .map_or(NONE.to_owned(), |r| r.1.clone())
        };
        general.push(self.mixed_field(
            "tag",
            "Tag",
            FieldKind::Choice(choices),
            agree(ids.iter().map(tag_name)),
        ));
        general.push(self.mixed_field(
            "material",
            "Material",
            FieldKind::Fixed,
            agree(ids.iter().map(|i| self.material_name(asm.painted_with(*i)))),
        ));
        let flag = |on: Option<bool>| on.map(|b| b.to_string());
        general.push(self.mixed_field(
            "hidden",
            "Hidden",
            FieldKind::Toggle,
            flag(agree(ids.iter().map(|i| asm.is_hidden(*i)))),
        ));
        general.push(self.mixed_field(
            "locked",
            "Locked",
            FieldKind::Toggle,
            flag(agree(ids.iter().map(|i| asm.is_locked(*i)))),
        ));
        out.sections.push(Section {
            title: "General",
            fields: general,
        });
        if n == 1
            && let Some(stack) = asm
                .context_def(&self.path_of(ids[0]))
                .and_then(|d| asm.def(d))
                .and_then(|d| d.stack.as_ref())
        {
            out.sections.push(self.features_section(stack));
        }

        let Some((lo, hi)) = self.groups_bounds() else {
            return;
        };
        let nf = self.number;
        let axes = [("x", "X"), ("y", "Y"), ("z", "Z")];
        out.sections.push(Section {
            title: "Position",
            fields: axes
                .iter()
                .enumerate()
                .map(|(a, (k, l))| field(k, l, FieldKind::Length, nf.show(lo[a])))
                .collect(),
        });
        let sizes = [("w", "Width X"), ("d", "Depth Y"), ("h", "Height Z")];
        let mut size: Vec<Field> = sizes
            .iter()
            .enumerate()
            .map(|(a, (k, l))| field(k, l, FieldKind::Length, nf.show(hi[a] - lo[a])))
            .collect();
        let (mut faces, mut edges) = (0, 0);
        for i in &ids {
            if let Some(d) = asm.context_def(&self.path_of(*i)).and_then(|d| asm.def(d)) {
                faces += d.model.face_count();
                edges += d.model.edge_count();
            }
        }
        size.push(field("faces", "Faces", FieldKind::Fixed, faces.to_string()));
        size.push(field("edges", "Edges", FieldKind::Fixed, edges.to_string()));
        size.push(field(
            "volume",
            "Volume",
            FieldKind::Fixed,
            self.volume_of(),
        ));
        out.sections.push(Section {
            title: "Size",
            fields: size,
        });
    }

    /// The live features of the one selected body, each with its size, and
    /// a way to take it off or bake them all.
    fn features_section(&self, stack: &prism_doc::Stack) -> Section {
        let mut fields = Vec::new();
        for (i, f) in stack.features.iter().enumerate() {
            let prism_doc::Feature::Round { size, fillet, .. } = f;
            let label = if *fillet {
                "Fillet radius"
            } else {
                "Chamfer size"
            };
            let key = format!("feature.{i}.size");
            if f.is_broken() {
                fields.push(field(
                    &key,
                    f.name(),
                    FieldKind::Fixed,
                    "Broken: its edges are gone. Remove it or bake.".into(),
                ));
            } else {
                fields.push(field(
                    &key,
                    label,
                    FieldKind::Length,
                    self.number.show(*size),
                ));
            }
            fields.push(field(
                &format!("feature.{i}.remove"),
                "",
                FieldKind::Action,
                format!("Remove {}", f.name()),
            ));
        }
        fields.push(field("bake", "", FieldKind::Action, "Bake".into()));
        Section {
            title: "Features",
            fields,
        }
    }

    fn mixed_field(&self, key: &str, label: &str, kind: FieldKind, value: Option<String>) -> Field {
        Field {
            key: key.to_owned(),
            label: label.to_owned(),
            kind,
            mixed: value.is_none(),
            text: value.unwrap_or_default(),
        }
    }

    /// The path of a selected group (in the open group).
    pub(super) fn path_of(&self, id: InstanceId) -> Vec<InstanceId> {
        let mut p = self.ctx.clone();
        p.push(id);
        p
    }

    /// The box of all the selected groups together, in the world.
    fn groups_bounds(&self) -> Option<(DVec3, DVec3)> {
        let mut b: Option<(DVec3, DVec3)> = None;
        for g in &self.groups {
            for p in self.group_box(*g)? {
                b = Some(b.map_or((p, p), |(l, h)| (l.min(p), h.max(p))));
            }
        }
        b
    }

    /// An area in the document's unit squared.
    fn area_text(&self, a: f64) -> String {
        let u = self.number.units;
        let unit = match u.length {
            prism_math::LengthUnit::Millimetres => "mm²",
            prism_math::LengthUnit::Centimetres => "cm²",
            prism_math::LengthUnit::Metres => "m²",
            _ => "in²",
        };
        let n = format!("{:.*}", u.decimals as usize, a / u.length.metres().powi(2));
        format!(
            "{} {unit}",
            n.replace('.', &self.number.decimal.to_string())
        )
    }

    /// The selected groups' volume, or "—" when one is not a closed solid.
    /// Worked out once for each selection and model.
    fn volume_of(&self) -> String {
        if let Some((v, ids, text)) = &*self.inspector_volume.borrow()
            && *v == self.version
            && *ids == self.groups
        {
            return text.clone();
        }
        let asm = self.doc.assembly();
        let mut total = Some(0.0);
        for g in &self.groups {
            let path = self.path_of(*g);
            let (Some(def), Some(world)) = (
                asm.context_def(&path).and_then(|d| asm.def(d)),
                asm.world_of(&path),
            ) else {
                total = None;
                break;
            };
            let checks = validate(&def.model);
            let one = (!checks.is_empty())
                .then(|| checks.iter().map(|c| c.volume).sum::<Option<f64>>())
                .flatten();
            total = total
                .zip(one)
                .map(|(t, v)| t + v * world.matrix3.determinant().abs());
        }
        let text = total.map_or("—".to_owned(), |v| self.volume_text(v));
        *self.inspector_volume.borrow_mut() =
            Some((self.version, self.groups.clone(), text.clone()));
        text
    }

    /// Change a field. `key` is the field's key and `text` what was typed
    /// (lengths take units and expressions). One undo step.
    pub fn inspector_set(&mut self, key: &str, text: &str) -> Result<(), String> {
        if self.groups.is_empty() {
            return Err("Select a group or component first".into());
        }
        let asm = self.doc.assembly();
        let locked = self.groups.iter().any(|g| asm.is_locked(*g));
        if locked && key != "locked" {
            return Err(LOCKED.into());
        }
        let ids = self.groups.clone();
        match key {
            "name" => {
                if ids.len() != 1 {
                    return Err("Name one group at a time".into());
                }
                self.name_instance(*ids.iter().next().expect("one"), text.trim());
            }
            "tag" => {
                let want = text.trim();
                let tag = if want.is_empty() || want == NONE {
                    None
                } else {
                    let found = self.tag_rows().into_iter().find(|t| t.1 == want);
                    Some(found.ok_or(format!("No tag named {want:?}"))?.0)
                };
                self.cancel();
                self.run(Command::SetTag {
                    instances: ids,
                    tag,
                });
            }
            "hidden" | "locked" => {
                let on = match text.trim() {
                    "true" => true,
                    "false" => false,
                    _ => return Err("Use true or false".into()),
                };
                if key == "locked" {
                    self.set_locked(on);
                } else {
                    self.cancel();
                    self.run(Command::SetHidden {
                        instances: ids,
                        hidden: on,
                    });
                }
            }
            "x" | "y" | "z" | "w" | "d" | "h" => {
                let v = self
                    .number
                    .expression(text)
                    .ok_or(format!("{text:?} is not a length"))?;
                let (lo, hi) = self.groups_bounds().ok_or("The selection has no box")?;
                let change = if let Some(a) = "xyz".find(key) {
                    let mut by = DVec3::ZERO;
                    by[a] = v - lo[a];
                    Change::Translate { by }
                } else {
                    let a = "wdh".find(key).expect("w, d or h");
                    let now = hi[a] - lo[a];
                    if now < FLAT {
                        return Err("It has no size along that axis to scale".into());
                    }
                    if v <= 0.0 {
                        return Err("A size must be more than zero".into());
                    }
                    let mut factor = DVec3::ONE;
                    factor[a] = v / now;
                    Change::Scale { centre: lo, factor }
                };
                self.cancel();
                self.run(Command::Place {
                    instances: ids,
                    change,
                });
            }
            "bake" | "feature" => return self.feature_set(key, text),
            k if k.starts_with("feature.") => return self.feature_set(k, text),
            _ => return Err(format!("{key:?} is not an Inspector field")),
        }
        Ok(())
    }

    /// `bake`, `feature.N.size` and `feature.N.remove`, for the one
    /// selected body.
    fn feature_set(&mut self, key: &str, text: &str) -> Result<(), String> {
        let [id] = self.groups.iter().copied().collect::<Vec<_>>()[..] else {
            return Err("Select one group to edit its features".into());
        };
        let asm = self.doc.assembly();
        let count = asm
            .context_def(&self.path_of(id))
            .and_then(|d| asm.def(d))
            .and_then(|d| d.stack.as_ref())
            .map_or(0, |s| s.features.len());
        if count == 0 {
            return Err("It has no live features".into());
        }
        let command = if key == "bake" {
            Command::BakeFeatures { instance: id }
        } else {
            let mut parts = key.split('.').skip(1);
            let index: usize = parts
                .next()
                .and_then(|n| n.parse().ok())
                .filter(|n| *n < count)
                .ok_or(format!("{key:?} is not an Inspector field"))?;
            match parts.next() {
                Some("remove") => Command::RemoveFeature {
                    instance: id,
                    index,
                },
                Some("size") => {
                    let size = self
                        .number
                        .expression(text)
                        .ok_or(format!("{text:?} is not a length"))?;
                    if size <= 0.0 {
                        return Err("A size must be more than zero".into());
                    }
                    Command::SetFeatureSize {
                        instance: id,
                        index,
                        size,
                    }
                }
                _ => return Err(format!("{key:?} is not an Inspector field")),
            }
        };
        self.cancel();
        if self.run(command) {
            Ok(())
        } else {
            Err("Nothing changed: the size is the same, or it does not fit these edges".into())
        }
    }
}
