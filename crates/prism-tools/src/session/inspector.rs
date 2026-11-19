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

use prism_doc::{Change, Command, DefId, InstanceId};
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
        Command::BakeFeatures { .. }
        | Command::RemoveFeature { .. }
        | Command::ExplodeFeature { .. } => true,
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
    /// The definition whose live features the Inspector shows and edits:
    /// the one selected group's, or, with no group selected, the open
    /// group's (the root for loose geometry).
    pub(super) fn features_target(&self) -> Option<DefId> {
        let asm = self.doc.assembly();
        match self.groups.len() {
            0 => asm.context_def(&self.ctx),
            1 => {
                let id = *self.groups.iter().next()?;
                asm.context_def(&self.path_of(id))
            }
            _ => None,
        }
    }

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
        let chosen = self
            .selected_feature()
            .zip(
                self.features_target()
                    .and_then(|d| asm.def(d))
                    .and_then(|d| d.stack.as_ref()),
            )
            .and_then(|(i, s)| s.features.get(i).map(|f| (i, f)));
        if let Some((i, f)) = chosen {
            // A feature picked in the viewport or the Outliner.
            out.summary = f.name().to_owned();
            out.sections.push(Section {
                title: "Feature",
                fields: self.feature_fields(i, f),
            });
        } else if self.groups.is_empty() {
            self.inspect_geometry(&mut out);
            // The features of loose geometry, or of the open group.
            if let Some(stack) = self
                .features_target()
                .and_then(|d| asm.def(d))
                .and_then(|d| d.stack.as_ref())
            {
                out.sections.push(self.features_section(stack));
            }
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
            fields.extend(self.feature_fields(i, f));
        }
        fields.push(field("bake", "", FieldKind::Action, "Bake all".into()));
        Section {
            title: "Features",
            fields,
        }
    }

    /// One feature's rows: its size (a round's), and Explode and Remove.
    fn feature_fields(&self, i: usize, f: &prism_doc::Feature) -> Vec<Field> {
        let mut fields = Vec::new();
        match f {
            prism_doc::Feature::Round { size, fillet, .. } => {
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
                        "Broken: its edges are gone. Select edges and use them here, or remove it or bake.".into(),
                    ));
                } else {
                    fields.push(field(
                        &key,
                        label,
                        FieldKind::Length,
                        self.number.show(*size),
                    ));
                }
                // Edges picked in the viewport can replace the round's.
                let picked = self.selection.edges.len();
                if picked > 0 && self.groups.is_empty() {
                    fields.push(field(
                        &format!("feature.{i}.edges"),
                        "",
                        FieldKind::Action,
                        format!(
                            "Round the {picked} selected edge{}",
                            if picked == 1 { "" } else { "s" }
                        ),
                    ));
                }
            }
            prism_doc::Feature::Shell { thickness, .. } => {
                let key = format!("feature.{i}.size");
                if f.is_broken() {
                    fields.push(field(
                        &key,
                        f.name(),
                        FieldKind::Fixed,
                        "Broken: its faces are gone or the thickness does not fit. Remove it or bake.".into(),
                    ));
                } else {
                    fields.push(field(
                        &key,
                        "Thickness (negative goes in)",
                        FieldKind::Length,
                        self.number.show(*thickness),
                    ));
                }
            }
            prism_doc::Feature::Boolean { .. } => {
                let note = if f.is_broken() {
                    "Broken: the shapes no longer make a solid. Remove it or bake."
                } else {
                    "Kept live: the second solid is stored inside."
                };
                if f.is_broken() {
                    fields.push(field(
                        &format!("feature.{i}.kind"),
                        f.name(),
                        FieldKind::Fixed,
                        note.into(),
                    ));
                } else {
                    fields.push(field(
                        &format!("feature.{i}.kind"),
                        "Boolean",
                        FieldKind::Choice(
                            ["Union", "Subtract", "Intersect"]
                                .map(String::from)
                                .to_vec(),
                        ),
                        f.name().into(),
                    ));
                }
                if let Some((lo, hi)) = f.tool_box().filter(|_| !f.is_broken()) {
                    let size = hi - lo;
                    for (axis, label, v) in [
                        ("x", "Cutter X", lo.x),
                        ("y", "Cutter Y", lo.y),
                        ("z", "Cutter Z", lo.z),
                        ("w", "Cutter width", size.x),
                        ("d", "Cutter depth", size.y),
                        ("h", "Cutter height", size.z),
                    ] {
                        fields.push(field(
                            &format!("feature.{i}.cutter.{axis}"),
                            label,
                            FieldKind::Length,
                            self.number.show(v),
                        ));
                    }
                }
            }
        }
        fields.push(field(
            &format!("feature.{i}.explode"),
            "",
            FieldKind::Action,
            format!("Explode {}", f.name()),
        ));
        fields.push(field(
            &format!("feature.{i}.remove"),
            "",
            FieldKind::Action,
            format!("Remove {}", f.name()),
        ));
        fields
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
        if key == "bake" || key.starts_with("feature.") {
            let asm = self.doc.assembly();
            if self.groups.iter().any(|g| asm.is_locked(*g)) {
                return Err(LOCKED.into());
            }
            return self.feature_set(key, text);
        }
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
            _ => return Err(format!("{key:?} is not an Inspector field")),
        }
        Ok(())
    }

    /// `bake`, `feature.N.size`, `feature.N.cutter.x` and the like, and
    /// `feature.N.remove`, for the one selected body.
    fn feature_set(&mut self, key: &str, text: &str) -> Result<(), String> {
        let command = self.feature_command(key, text)?;
        self.cancel();
        if self.run(command) {
            Ok(())
        } else {
            Err("Nothing changed: the value is the same, or it would not fit".into())
        }
    }

    /// While a feature's field is being typed, before Enter: show the body
    /// (the open one: a group seen from outside is not previewed) as it
    /// would be with that text. Text that is not valid, or a change
    /// the kernel refuses, shows the body as it is.
    pub fn inspector_preview(&mut self, key: &str, text: &str) {
        let model = (key.starts_with("feature.") && self.groups.is_empty())
            .then(|| self.feature_command(key, text).ok())
            .flatten()
            .and_then(|c| self.feature_preview_of(&c));
        self.set_preview(model);
    }

    /// The typing is over (committed or abandoned): the preview goes.
    pub fn inspector_preview_end(&mut self) {
        self.set_preview(None);
    }

    /// The body with a size or cutter change made, or None if the command
    /// is another kind or the features would break.
    fn feature_preview_of(&self, c: &Command) -> Option<prism_topo::Model> {
        let (def, index) = match c {
            Command::SetFeatureSize { def, index, .. } | Command::SetCutter { def, index, .. } => {
                (*def, *index)
            }
            _ => return None,
        };
        let mut stack = self.doc.assembly().def(def)?.stack.clone()?;
        let f = stack.features.get_mut(index)?;
        match c {
            Command::SetFeatureSize { size, .. } => match f {
                prism_doc::Feature::Round { size: s, .. } => *s = *size,
                prism_doc::Feature::Shell { thickness, .. } => *thickness = *size,
                prism_doc::Feature::Boolean { .. } => return None,
            },
            Command::SetCutter { min, max, .. } => {
                if !f.set_tool_box(*min, *max) {
                    return None;
                }
            }
            _ => return None,
        }
        let m = stack.evaluate();
        (!stack.features[index].is_broken()).then_some(m)
    }

    /// The command a feature field's text asks for.
    fn feature_command(&self, key: &str, text: &str) -> Result<Command, String> {
        let Some(def) = self.features_target() else {
            return Err("Select one group to edit its features".into());
        };
        let asm = self.doc.assembly();
        let count = asm
            .def(def)
            .and_then(|d| d.stack.as_ref())
            .map_or(0, |s| s.features.len());
        if count == 0 {
            return Err("It has no live features".into());
        }
        let command = if key == "bake" {
            Command::BakeFeatures { def }
        } else {
            let mut parts = key.split('.').skip(1);
            let index: usize = parts
                .next()
                .and_then(|n| n.parse().ok())
                .filter(|n| *n < count)
                .ok_or(format!("{key:?} is not an Inspector field"))?;
            match parts.next() {
                Some("remove") => Command::RemoveFeature { def, index },
                Some("explode") => Command::ExplodeFeature { def, index },
                Some("size") => {
                    let size = self
                        .number
                        .expression(text)
                        .ok_or(format!("{text:?} is not a length"))?;
                    // A shell's thickness may be negative (in); the rest are sizes.
                    let signed = asm
                        .def(def)
                        .and_then(|d| d.stack.as_ref())
                        .is_some_and(|s| {
                            matches!(s.features[index], prism_doc::Feature::Shell { .. })
                        });
                    if size == 0.0 || (size < 0.0 && !signed) {
                        return Err("A size must be more than zero".into());
                    }
                    Command::SetFeatureSize { def, index, size }
                }
                Some("edges") => {
                    let m = self.work_model();
                    let edges: Vec<prism_doc::EdgeRef> = self
                        .selection
                        .edges
                        .iter()
                        .filter_map(|e| prism_doc::EdgeRef::of(m, *e))
                        .collect();
                    if edges.is_empty() {
                        return Err("Select the edges to round first".into());
                    }
                    Command::SetFeatureEdges { def, index, edges }
                }
                Some("kind") => {
                    let op = match text.to_ascii_lowercase().as_str() {
                        "union" => prism_doc::BoolKind::Union,
                        "subtract" => prism_doc::BoolKind::Subtract,
                        "intersect" => prism_doc::BoolKind::Intersect,
                        _ => return Err(format!("{text:?} is not Union, Subtract or Intersect")),
                    };
                    Command::SetBoolKind { def, index, op }
                }
                Some("cutter") => {
                    let axis = parts.next().unwrap_or_default();
                    let value = self
                        .number
                        .expression(text)
                        .ok_or(format!("{text:?} is not a length"))?;
                    let (mut lo, mut hi) = asm
                        .def(def)
                        .and_then(|d| d.stack.as_ref())
                        .and_then(|s| s.features[index].tool_box())
                        .ok_or("This feature has no cutter")?;
                    let size = hi - lo;
                    let (corner, extent) = match axis {
                        "x" => (0, None),
                        "y" => (1, None),
                        "z" => (2, None),
                        "w" => (0, Some(())),
                        "d" => (1, Some(())),
                        "h" => (2, Some(())),
                        _ => return Err(format!("{key:?} is not an Inspector field")),
                    };
                    if extent.is_some() {
                        if value <= 0.0 {
                            return Err("A size must be more than zero".into());
                        }
                        hi[corner] = lo[corner] + value;
                    } else {
                        lo[corner] = value;
                        hi[corner] = value + size[corner];
                    }
                    Command::SetCutter {
                        def,
                        index,
                        min: lo,
                        max: hi,
                    }
                }
                _ => return Err(format!("{key:?} is not an Inspector field")),
            }
        };
        Ok(command)
    }
}
