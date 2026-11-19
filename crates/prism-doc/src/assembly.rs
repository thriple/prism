//! Groups (PLAN 1.2, 1.9): the document's geometry as definitions, each a
//! model in its own coordinates, placed in its parent by instances.
//!
//! - The root is a definition too ([`ROOT`]).
//! - A group is a definition with one instance. Copying a group copies its
//!   definition, so copies are independent at once (DECISIONS #22).
//! - Every operation on a model works inside a definition unchanged: that
//!   is what keeps a group's geometry from sticking to anything outside.
//! - Definitions sit behind an `Arc` and are copied on write, so an undo
//!   step shares every definition it did not change.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use prism_math::{DAffine3, DVec3};
use prism_ops::select::Selection;
use prism_ops::transform;
use prism_topo::{EdgeId, FaceId, MaterialId, Model, VertexId};

use crate::material::Material;
use crate::stack::{EdgeRef, Feature, Stack};
use serde::{Deserialize, Serialize};

/// A definition's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DefId(pub u32);

/// An instance's id, unique in the whole document (a path of them names an
/// open group).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InstanceId(pub u32);

/// A tag's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TagId(pub u32);

/// A section plane's id (Annotation & Sections part 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SectionId(pub u32);

/// A section plane: a plane in the world that can cut the model away on
/// the side its normal points to. It changes no geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub origin: DVec3,
    /// Unit length. The side it points to is the side that is cut away.
    pub normal: DVec3,
}

/// A construction plane's id (Annotation & Sections part 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkPlaneId(pub u32);

/// A construction plane: a plane in the world that the drawing tools use
/// as their working plane when it is the active one. It changes no
/// geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkPlane {
    pub name: String,
    pub origin: DVec3,
    /// Unit length.
    pub normal: DVec3,
}

/// A dimension's id (Annotation & Sections part 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DimId(pub u32);

/// A linear dimension: the distance between two points, drawn offset to
/// one side. In world coordinates; it changes no geometry.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    pub a: DVec3,
    pub b: DVec3,
    /// From the measured line to the dimension line (square to it).
    pub offset: DVec3,
}

/// Where a dimension's end is attached: a vertex of the model in the group
/// at `path` (the instances from the top; empty for the top model). The
/// end follows that vertex when it moves (geometry moved, or a group moved).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Anchor {
    pub path: Vec<InstanceId>,
    pub vertex: VertexId,
}

impl Dimension {
    fn is_usable(&self) -> bool {
        self.a.is_finite() && self.b.is_finite() && self.offset.is_finite() && self.a != self.b
    }

    /// The distance measured.
    pub fn length(&self) -> f64 {
        self.a.distance(self.b)
    }
}

/// A tag: a name, and whether what is on it is shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub visible: bool,
}

/// Which boolean the Solid menu runs on two groups (Solids & CAD part 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolidOp {
    Union,
    Intersect,
    /// The second less the first (the first is the cutter); both go.
    Subtract,
    /// As Subtract, keeping the cutter.
    Trim,
    /// Three groups: each less the other, and what is in both.
    Split,
}

/// Said when a boolean has fewer than two groups.
pub const TWO_SOLIDS: &str = "Pick two different solid groups or components.";
/// Said for a group with groups inside.
pub const NESTED: &str =
    "Refused: a group with groups inside cannot be used yet. Explode them first.";

/// The root definition: the model outside every group.
pub const ROOT: DefId = DefId(0);

/// A definition placed in its parent.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub def: DefId,
    /// From the definition's coordinates to its parent's.
    pub transform: DAffine3,
}

/// Geometry in its own coordinates, and the instances placed in it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Definition {
    pub model: Model,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub instances: BTreeMap<InstanceId, Instance>,
    /// A component's name; a group has none. Copies of a component's
    /// instance share the definition; copies of a group get their own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Live features: when set, `model` is `stack.base` with them applied
    /// (Inspector part 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<Stack>,
}

impl Definition {
    pub fn is_component(&self) -> bool {
        self.name.is_some()
    }

    /// A direct edit of the body as drawn: to its base when it has live
    /// features (see `Stack::edit`).
    pub fn edit_direct(&mut self, targets: Option<crate::stack::Targets>, f: impl Fn(&mut Model)) {
        match self.stack.take() {
            Some(stack) => {
                let (s, m, _) = stack.edit(&self.model, targets, f);
                self.model = m;
                self.stack = (!s.features.is_empty()).then_some(s);
            }
            None => f(&mut self.model),
        }
    }

    /// Add geometry to the body: to its base when it has live features,
    /// and the features run again.
    fn add_geometry(&mut self, add: impl FnOnce(&mut Model)) {
        match &mut self.stack {
            Some(stack) => {
                add(&mut stack.base);
                self.model = stack.evaluate();
            }
            None => add(&mut self.model),
        }
    }
}

/// An instance as placed in the world, for drawing and picking.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// The instances from the root down to this one.
    pub path: Vec<InstanceId>,
    pub def: DefId,
    /// From the definition's coordinates to the world.
    pub world: DAffine3,
}

/// Every definition, and the counters that hand out new ids.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assembly {
    defs: BTreeMap<DefId, Arc<Definition>>,
    next_def: u32,
    next_instance: u32,
    /// Instances hidden on purpose (PLAN 1.14).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    hidden: BTreeSet<InstanceId>,
    /// Instances' own names.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    names: BTreeMap<InstanceId, String>,
    /// Which tag each instance is on.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    on_tag: BTreeMap<InstanceId, TagId>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    tags: BTreeMap<TagId, Tag>,
    #[serde(default)]
    next_tag: u32,
    /// The document's materials (Basic Materials), shared by every model.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    materials: BTreeMap<MaterialId, Material>,
    /// Texture images by their content (part 2): each stored once.
    images: BTreeMap<crate::image::ImageId, crate::image::Image>,
    #[serde(default)]
    next_material: u32,
    /// Instances painted as a whole: drawn on their Default faces.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    painted: BTreeMap<InstanceId, MaterialId>,
    /// Instances locked against edits (Everyday Commands part 1).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    locked: BTreeSet<InstanceId>,
    /// Section planes (Annotation & Sections part 1).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    sections: BTreeMap<SectionId, Section>,
    #[serde(default)]
    next_section: u32,
    /// The plane that cuts, if any: only one does at a time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_section: Option<SectionId>,
    /// Dimensions (Annotation & Sections part 2).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    dimensions: BTreeMap<DimId, Dimension>,
    #[serde(default)]
    next_dim: u32,
    /// What each end of a dimension is attached to, if anything.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    dim_anchors: BTreeMap<DimId, [Option<Anchor>; 2]>,
    /// Construction planes (Annotation & Sections part 4).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    work_planes: BTreeMap<WorkPlaneId, WorkPlane>,
    #[serde(default)]
    next_work_plane: u32,
    /// The working plane the drawing tools use, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_work_plane: Option<WorkPlaneId>,
}

/// An instance's name, tag and whether it is hidden (for files).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attributes {
    pub hidden: bool,
    pub locked: bool,
    pub name: Option<String>,
    pub tag: Option<TagId>,
}

impl Default for Assembly {
    fn default() -> Self {
        Self::with_model(Model::new())
    }
}

impl Assembly {
    /// Only a root, holding `model`.
    pub fn with_model(model: Model) -> Self {
        Self {
            defs: [(
                ROOT,
                Arc::new(Definition {
                    model,
                    instances: BTreeMap::new(),
                    name: None,
                    stack: None,
                }),
            )]
            .into(),
            next_def: 1,
            next_instance: 0,
            hidden: BTreeSet::new(),
            names: BTreeMap::new(),
            on_tag: BTreeMap::new(),
            tags: BTreeMap::new(),
            next_tag: 0,
            materials: BTreeMap::new(),
            images: BTreeMap::new(),
            next_material: 0,
            painted: BTreeMap::new(),
            locked: BTreeSet::new(),
            sections: BTreeMap::new(),
            next_section: 0,
            active_section: None,
            work_planes: BTreeMap::new(),
            next_work_plane: 0,
            active_work_plane: None,
            dimensions: BTreeMap::new(),
            next_dim: 0,
            dim_anchors: BTreeMap::new(),
        }
    }

    /// The counters for new definitions and instances (for files).
    pub fn counters(&self) -> (u32, u32) {
        (self.next_def, self.next_instance)
    }

    /// An assembly from its parts (a file). Refused if they do not fit: no
    /// root, an instance of a missing definition, a definition inside
    /// itself, an id at or above its counter, or an instance id used twice.
    pub fn from_parts(
        defs: BTreeMap<DefId, Definition>,
        next_def: u32,
        next_instance: u32,
    ) -> Result<Assembly, String> {
        if !defs.contains_key(&ROOT) {
            return Err("no root definition".into());
        }
        let mut seen: BTreeSet<InstanceId> = BTreeSet::new();
        for (id, d) in &defs {
            if id.0 >= next_def {
                return Err(format!("definition {} is not below its counter", id.0));
            }
            for (i, inst) in &d.instances {
                if i.0 >= next_instance || !seen.insert(*i) {
                    return Err(format!(
                        "instance {} is repeated or not below its counter",
                        i.0
                    ));
                }
                if !defs.contains_key(&inst.def) || inst.def == ROOT {
                    return Err(format!("instance {} places a missing definition", i.0));
                }
                if !inst.transform.is_finite() {
                    return Err(format!("instance {} has no finite placement", i.0));
                }
            }
        }
        // No definition may contain itself, however deep.
        fn reaches(
            defs: &BTreeMap<DefId, Definition>,
            from: DefId,
            to: DefId,
            depth: usize,
        ) -> bool {
            if depth > defs.len() {
                return true;
            }
            defs.get(&from).is_some_and(|d| {
                d.instances
                    .values()
                    .any(|i| i.def == to || reaches(defs, i.def, to, depth + 1))
            })
        }
        if let Some(id) = defs.keys().find(|id| reaches(&defs, **id, **id, 0)) {
            return Err(format!("definition {} contains itself", id.0));
        }
        Ok(Assembly {
            defs: defs.into_iter().map(|(k, v)| (k, Arc::new(v))).collect(),
            next_def,
            next_instance,
            hidden: BTreeSet::new(),
            names: BTreeMap::new(),
            on_tag: BTreeMap::new(),
            tags: BTreeMap::new(),
            next_tag: 0,
            materials: BTreeMap::new(),
            images: BTreeMap::new(),
            next_material: 0,
            painted: BTreeMap::new(),
            locked: BTreeSet::new(),
            sections: BTreeMap::new(),
            next_section: 0,
            active_section: None,
            work_planes: BTreeMap::new(),
            next_work_plane: 0,
            active_work_plane: None,
            dimensions: BTreeMap::new(),
            next_dim: 0,
            dim_anchors: BTreeMap::new(),
        })
    }

    /// Set the tags and each instance's name, tag and hidden flag (a file).
    /// Entries for instances or tags that do not exist are dropped.
    pub fn with_attributes(
        mut self,
        tags: BTreeMap<TagId, Tag>,
        next_tag: u32,
        attrs: BTreeMap<InstanceId, Attributes>,
    ) -> Assembly {
        self.next_tag = next_tag.max(tags.keys().next_back().map_or(0, |t| t.0 + 1));
        self.tags = tags;
        for (i, a) in attrs {
            if a.hidden {
                self.hidden.insert(i);
            }
            if a.locked {
                self.locked.insert(i);
            }
            if let Some(n) = a.name {
                self.names.insert(i, n);
            }
            if let Some(t) = a.tag.filter(|t| self.tags.contains_key(t)) {
                self.on_tag.insert(i, t);
            }
        }
        self.prune();
        self
    }

    /// Each instance's name, tag and hidden flag, where any is set.
    pub fn attributes(&self) -> BTreeMap<InstanceId, Attributes> {
        let mut out: BTreeMap<InstanceId, Attributes> = BTreeMap::new();
        for i in &self.hidden {
            out.entry(*i).or_default().hidden = true;
        }
        for i in &self.locked {
            out.entry(*i).or_default().locked = true;
        }
        for (i, n) in &self.names {
            out.entry(*i).or_default().name = Some(n.clone());
        }
        for (i, t) in &self.on_tag {
            out.entry(*i).or_default().tag = Some(*t);
        }
        out
    }

    /// The tags, and the counter for new ones (for files).
    pub fn tags(&self) -> (&BTreeMap<TagId, Tag>, u32) {
        (&self.tags, self.next_tag)
    }

    // ---------- hiding, names and tags (part 3) ----------

    /// Whether an instance is hidden on purpose.
    pub fn is_hidden(&self, i: InstanceId) -> bool {
        self.hidden.contains(&i)
    }

    /// Whether an instance is shown: not hidden, and not on a hidden tag.
    pub fn is_shown(&self, i: InstanceId) -> bool {
        !self.hidden.contains(&i)
            && self
                .on_tag
                .get(&i)
                .is_none_or(|t| self.tags.get(t).is_none_or(|t| t.visible))
    }

    pub fn name_of(&self, i: InstanceId) -> Option<&str> {
        self.names.get(&i).map(String::as_str)
    }

    pub fn tag_of(&self, i: InstanceId) -> Option<TagId> {
        self.on_tag.get(&i).copied()
    }

    pub fn tag(&self, t: TagId) -> Option<&Tag> {
        self.tags.get(&t)
    }

    /// Hide or show instances.
    pub fn set_hidden(&mut self, instances: &BTreeSet<InstanceId>, hidden: bool) {
        for i in instances {
            if hidden {
                self.hidden.insert(*i);
            } else {
                self.hidden.remove(i);
            }
        }
        self.prune();
    }

    /// Name an instance (an empty name clears it).
    /// Whether an instance is locked against edits.
    pub fn is_locked(&self, i: InstanceId) -> bool {
        self.locked.contains(&i)
    }

    /// Whether a path of instances (an open group) goes through a locked one.
    pub fn path_locked(&self, path: &[InstanceId]) -> bool {
        path.iter().any(|i| self.is_locked(*i))
    }

    // ---------- dimensions (Annotation & Sections part 2) ----------

    /// The dimensions, and the counter for new ones (for files).
    pub fn dimensions(&self) -> (&BTreeMap<DimId, Dimension>, u32) {
        (&self.dimensions, self.next_dim)
    }

    pub fn dimension(&self, id: DimId) -> Option<&Dimension> {
        self.dimensions.get(&id)
    }

    /// Set the dimensions from a file; unusable ones (not finite, or both
    /// points the same) are dropped.
    pub fn with_dimensions(mut self, dims: BTreeMap<DimId, Dimension>, next: u32) -> Self {
        self.dimensions = dims.into_iter().filter(|(_, d)| d.is_usable()).collect();
        self.next_dim = next.max(self.dimensions.keys().next_back().map_or(0, |k| k.0 + 1));
        self
    }

    /// Add a dimension. None if the points are not finite or are the same.
    pub fn add_dimension(&mut self, d: Dimension) -> Option<DimId> {
        if !d.is_usable() {
            return None;
        }
        let id = DimId(self.next_dim);
        self.next_dim += 1;
        self.dimensions.insert(id, d);
        Some(id)
    }

    /// Move a dimension's line to a new offset.
    pub fn move_dimension(&mut self, id: DimId, offset: DVec3) {
        if offset.is_finite()
            && let Some(d) = self.dimensions.get_mut(&id)
        {
            d.offset = offset;
        }
    }

    pub fn delete_dimension(&mut self, id: DimId) {
        self.dimensions.remove(&id);
        self.dim_anchors.remove(&id);
    }

    /// What the dimensions' ends are attached to (for files).
    pub fn dim_anchors(&self) -> &BTreeMap<DimId, [Option<Anchor>; 2]> {
        &self.dim_anchors
    }

    /// Set the anchors from a file.
    pub fn with_dim_anchors(mut self, anchors: BTreeMap<DimId, [Option<Anchor>; 2]>) -> Self {
        self.dim_anchors = anchors
            .into_iter()
            .filter(|(id, _)| self.dimensions.contains_key(id))
            .collect();
        self
    }

    /// Attach a dimension's ends (None leaves an end free).
    pub fn anchor_dimension(&mut self, id: DimId, a: Option<Anchor>, b: Option<Anchor>) {
        if !self.dimensions.contains_key(&id) {
            return;
        }
        if a.is_none() && b.is_none() {
            self.dim_anchors.remove(&id);
        } else {
            self.dim_anchors.insert(id, [a, b]);
        }
    }

    /// Where an anchor is now, in the world; None if its group or vertex
    /// is gone.
    fn anchor_at(&self, an: &Anchor) -> Option<DVec3> {
        let mut def = ROOT;
        let mut m = DAffine3::IDENTITY;
        for id in &an.path {
            let inst = self.def(def)?.instances.get(id)?;
            m *= inst.transform;
            def = inst.def;
        }
        let model = &self.def(def)?.model;
        model
            .has_vertex(an.vertex)
            .then(|| m.transform_point3(model.vertex(an.vertex)))
    }

    /// Move each attached dimension end to where its vertex is now (after
    /// an edit); an end whose vertex or group is gone is let go. The
    /// dimension line keeps its side and distance.
    pub fn follow_dimensions(&mut self) {
        let ids: Vec<DimId> = self.dim_anchors.keys().copied().collect();
        for id in ids {
            let (Some(mut d), Some(anchors)) = (
                self.dimensions.get(&id).copied(),
                self.dim_anchors.get(&id).cloned(),
            ) else {
                self.dim_anchors.remove(&id);
                continue;
            };
            let mut kept = anchors.clone();
            let mut ends = [d.a, d.b];
            for k in 0..2 {
                if let Some(an) = &anchors[k] {
                    match self.anchor_at(an) {
                        Some(p) => ends[k] = p,
                        None => kept[k] = None,
                    }
                }
            }
            if [d.a, d.b] != ends {
                let line = (ends[1] - ends[0]).normalize_or_zero();
                let offset = d.offset - line * d.offset.dot(line);
                if ends[0] != ends[1] && line != DVec3::ZERO && offset.is_finite() {
                    d.a = ends[0];
                    d.b = ends[1];
                    if offset.length() > 0.0 {
                        d.offset = offset;
                    }
                    self.dimensions.insert(id, d);
                }
            }
            if kept.iter().all(Option::is_none) {
                self.dim_anchors.remove(&id);
            } else {
                self.dim_anchors.insert(id, kept);
            }
        }
    }

    // ---------- section planes (Annotation & Sections part 1) ----------

    /// The section planes, and the counter for new ones (for files).
    pub fn sections(&self) -> (&BTreeMap<SectionId, Section>, u32) {
        (&self.sections, self.next_section)
    }

    pub fn section(&self, id: SectionId) -> Option<&Section> {
        self.sections.get(&id)
    }

    /// The plane that cuts now.
    pub fn active_section(&self) -> Option<(SectionId, &Section)> {
        let id = self.active_section?;
        self.sections.get(&id).map(|s| (id, s))
    }

    /// Set the planes from a file. A plane that is not a plane (a zero or
    /// unusable normal) is dropped; `active` must name one that is left.
    pub fn with_sections(
        mut self,
        sections: BTreeMap<SectionId, Section>,
        next: u32,
        active: Option<SectionId>,
    ) -> Self {
        self.sections = sections
            .into_iter()
            .filter_map(|(id, mut s)| {
                let n = s.normal.normalize_or_zero();
                (n != DVec3::ZERO && s.origin.is_finite()).then(|| {
                    // A unit normal is kept as it is (normalising it again
                    // would change its last digits and break exact round trips).
                    if (s.normal.length() - 1.0).abs() > 1e-12 {
                        s.normal = n;
                    }
                    (id, s)
                })
            })
            .collect();
        self.next_section = next.max(self.sections.keys().next_back().map_or(0, |k| k.0 + 1));
        self.active_section = active.filter(|a| self.sections.contains_key(a));
        self
    }

    /// Add a plane through `origin` whose normal points to the side that is
    /// cut away. It becomes the active one. None for an unusable normal.
    pub fn add_section(&mut self, origin: DVec3, normal: DVec3) -> Option<SectionId> {
        let n = normal.normalize_or_zero();
        if n == DVec3::ZERO || !origin.is_finite() {
            return None;
        }
        let id = SectionId(self.next_section);
        self.next_section += 1;
        let name = format!("Section {}", id.0 + 1);
        self.sections.insert(
            id,
            Section {
                name,
                origin,
                normal: n,
            },
        );
        self.active_section = Some(id);
        Some(id)
    }

    /// Move a plane's origin (it keeps its direction).
    pub fn move_section(&mut self, id: SectionId, origin: DVec3) {
        if origin.is_finite()
            && let Some(s) = self.sections.get_mut(&id)
        {
            s.origin = origin;
        }
    }

    /// Cut away the other side instead.
    pub fn flip_section(&mut self, id: SectionId) {
        if let Some(s) = self.sections.get_mut(&id) {
            s.normal = -s.normal;
        }
    }

    /// Make a plane the one that cuts, or stop any plane cutting.
    pub fn set_active_section(&mut self, id: SectionId, active: bool) {
        if !self.sections.contains_key(&id) {
            return;
        }
        if active {
            self.active_section = Some(id);
        } else if self.active_section == Some(id) {
            self.active_section = None;
        }
    }

    pub fn rename_section(&mut self, id: SectionId, name: &str) {
        let name = name.trim();
        if !name.is_empty()
            && let Some(s) = self.sections.get_mut(&id)
        {
            s.name = name.to_owned();
        }
    }

    pub fn delete_section(&mut self, id: SectionId) {
        self.sections.remove(&id);
        if self.active_section == Some(id) {
            self.active_section = None;
        }
    }

    // ---------- construction planes (Annotation & Sections part 4) ----------

    /// The construction planes, and the counter for new ones (for files).
    pub fn work_planes(&self) -> (&BTreeMap<WorkPlaneId, WorkPlane>, u32) {
        (&self.work_planes, self.next_work_plane)
    }

    pub fn work_plane(&self, id: WorkPlaneId) -> Option<&WorkPlane> {
        self.work_planes.get(&id)
    }

    /// The working plane the drawing tools use now.
    pub fn active_work_plane(&self) -> Option<(WorkPlaneId, &WorkPlane)> {
        let id = self.active_work_plane?;
        self.work_planes.get(&id).map(|p| (id, p))
    }

    /// Set the planes from a file. One with an unusable normal is dropped;
    /// `active` must name one that is left.
    pub fn with_work_planes(
        mut self,
        planes: BTreeMap<WorkPlaneId, WorkPlane>,
        next: u32,
        active: Option<WorkPlaneId>,
    ) -> Self {
        self.work_planes = planes
            .into_iter()
            .filter_map(|(id, mut p)| {
                let n = p.normal.normalize_or_zero();
                (n != DVec3::ZERO && p.origin.is_finite()).then(|| {
                    // A unit normal is kept as it is, so files round trip.
                    if (p.normal.length() - 1.0).abs() > 1e-12 {
                        p.normal = n;
                    }
                    (id, p)
                })
            })
            .collect();
        self.next_work_plane = next.max(self.work_planes.keys().next_back().map_or(0, |k| k.0 + 1));
        self.active_work_plane = active.filter(|a| self.work_planes.contains_key(a));
        self
    }

    /// Add a plane and make it the working plane. None if the normal is
    /// unusable.
    pub fn add_work_plane(&mut self, origin: DVec3, normal: DVec3) -> Option<WorkPlaneId> {
        let n = normal.normalize_or_zero();
        if n == DVec3::ZERO || !origin.is_finite() {
            return None;
        }
        let id = WorkPlaneId(self.next_work_plane);
        self.next_work_plane += 1;
        self.work_planes.insert(
            id,
            WorkPlane {
                name: format!("Plane {}", id.0 + 1),
                origin,
                normal: n,
            },
        );
        self.active_work_plane = Some(id);
        Some(id)
    }

    /// Move a plane's origin (it keeps its direction).
    pub fn move_work_plane(&mut self, id: WorkPlaneId, origin: DVec3) {
        if origin.is_finite()
            && let Some(p) = self.work_planes.get_mut(&id)
        {
            p.origin = origin;
        }
    }

    /// Make a plane the working plane, or stop it being one.
    pub fn set_active_work_plane(&mut self, id: WorkPlaneId, active: bool) {
        if !self.work_planes.contains_key(&id) {
            return;
        }
        if active {
            self.active_work_plane = Some(id);
        } else if self.active_work_plane == Some(id) {
            self.active_work_plane = None;
        }
    }

    pub fn rename_work_plane(&mut self, id: WorkPlaneId, name: &str) {
        let name = name.trim();
        if !name.is_empty()
            && let Some(p) = self.work_planes.get_mut(&id)
        {
            p.name = name.to_owned();
        }
    }

    pub fn delete_work_plane(&mut self, id: WorkPlaneId) {
        self.work_planes.remove(&id);
        if self.active_work_plane == Some(id) {
            self.active_work_plane = None;
        }
    }

    /// Lock or unlock instances (anywhere: instance ids are unique).
    pub fn set_locked(&mut self, instances: &BTreeSet<InstanceId>, locked: bool) {
        let alive: BTreeSet<InstanceId> = self
            .defs
            .values()
            .flat_map(|d| d.instances.keys().copied())
            .collect();
        for i in instances.iter().filter(|i| alive.contains(i)) {
            if locked {
                self.locked.insert(*i);
            } else {
                self.locked.remove(i);
            }
        }
    }

    pub fn set_name(&mut self, i: InstanceId, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.names.remove(&i);
        } else {
            self.names.insert(i, name.to_owned());
        }
        self.prune();
    }

    /// A new, shown tag with a free name. Returns it.
    pub fn add_tag(&mut self, name: &str) -> TagId {
        let base = name.trim();
        let base = if base.is_empty() { "Tag" } else { base };
        let taken = |n: &str| self.tags.values().any(|t| t.name == n);
        let name = if taken(base) {
            (2..)
                .map(|k| format!("{base} {k}"))
                .find(|n| !taken(n))
                .unwrap_or_default()
        } else {
            base.to_owned()
        };
        let id = TagId(self.next_tag);
        self.next_tag += 1;
        self.tags.insert(
            id,
            Tag {
                name,
                visible: true,
            },
        );
        id
    }

    /// Rename a tag (refused if empty or taken).
    pub fn rename_tag(&mut self, t: TagId, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || self.tags.iter().any(|(id, x)| *id != t && x.name == name) {
            return false;
        }
        match self.tags.get_mut(&t) {
            Some(x) => {
                x.name = name.to_owned();
                true
            }
            None => false,
        }
    }

    /// Show or hide a tag.
    pub fn set_tag_visible(&mut self, t: TagId, visible: bool) {
        if let Some(x) = self.tags.get_mut(&t) {
            x.visible = visible;
        }
    }

    /// Put instances on a tag (None: on no tag).
    pub fn set_tag(&mut self, instances: &BTreeSet<InstanceId>, tag: Option<TagId>) {
        for i in instances {
            match tag.filter(|t| self.tags.contains_key(t)) {
                Some(t) => {
                    self.on_tag.insert(*i, t);
                }
                None => {
                    self.on_tag.remove(i);
                }
            }
        }
        self.prune();
    }

    /// How many instances are on a tag.
    pub fn on_tag_count(&self, t: TagId) -> usize {
        self.on_tag.values().filter(|x| **x == t).count()
    }

    pub fn root(&self) -> &Definition {
        &self.defs[&ROOT]
    }

    pub fn def(&self, id: DefId) -> Option<&Definition> {
        self.defs.get(&id).map(|d| &**d)
    }

    pub fn defs(&self) -> impl Iterator<Item = (DefId, &Definition)> + '_ {
        self.defs.iter().map(|(id, d)| (*id, &**d))
    }

    /// The definition, copied first if an undo step shares it.
    /// For a change to the definition's geometry the live features do not
    /// know about: they are baked into it first, so nothing is lost or put
    /// out of step. Use `def_mut_live` for what leaves the geometry alone.
    pub fn def_mut(&mut self, id: DefId) -> Option<&mut Definition> {
        let d = self.def_mut_live(id)?;
        d.stack = None;
        Some(d)
    }

    /// The definition, copied first if an undo step shares it, with its
    /// live features kept: for changes to its instances or name, or to
    /// geometry the caller also applies to the stack's base.
    pub fn def_mut_live(&mut self, id: DefId) -> Option<&mut Definition> {
        self.defs.get_mut(&id).map(Arc::make_mut)
    }

    // ---------- live features (Inspector part 2) ----------

    /// Round `edges` of the body in `ctx` as a live feature. The edges are
    /// those of the model as drawn, which is the base with the features so
    /// far applied. Changes nothing when the kernel refuses.
    pub fn round_live(
        &mut self,
        ctx: DefId,
        edges: &BTreeSet<EdgeId>,
        size: f64,
        fillet: bool,
    ) -> bool {
        let Some(d) = self.def_mut_live(ctx) else {
            return false;
        };
        let refs: Vec<EdgeRef> = edges
            .iter()
            .filter_map(|e| EdgeRef::of(&d.model, *e))
            .collect();
        if refs.len() != edges.len() {
            return false;
        }
        let mut stack = d
            .stack
            .clone()
            .unwrap_or_else(|| Stack::new(d.model.clone()));
        stack.features.push(Feature::Round {
            edges: refs,
            size,
            fillet,
            broken: false,
        });
        let model = stack.evaluate();
        if stack.features.last().is_some_and(Feature::is_broken) {
            return false;
        }
        d.model = model;
        d.stack = Some(stack);
        true
    }

    /// Change a live feature's size and run the features again.
    pub fn set_feature_size(&mut self, def: DefId, index: usize, new: f64) -> bool {
        let Some(d) = self.def_mut_live(def) else {
            return false;
        };
        let Some(stack) = &mut d.stack else {
            return false;
        };
        let mut next = stack.clone();
        match next.features.get_mut(index) {
            Some(Feature::Round { size, .. }) => *size = new,
            None => return false,
        }
        // A size the kernel refuses is not taken.
        let model = next.evaluate();
        if next.features[index].is_broken() {
            return false;
        }
        d.model = model;
        d.stack = Some(next);
        true
    }

    /// Take a live feature off and run the rest again.
    pub fn remove_feature(&mut self, def: DefId, index: usize) -> bool {
        let Some(d) = self.def_mut_live(def) else {
            return false;
        };
        let Some(stack) = &mut d.stack else {
            return false;
        };
        if index >= stack.features.len() {
            return false;
        }
        stack.features.remove(index);
        d.model = stack.evaluate();
        if stack.features.is_empty() {
            d.stack = None;
        }
        true
    }

    /// Turn the live features into plain geometry.
    pub fn bake(&mut self, def: DefId) -> bool {
        match self.def_mut_live(def) {
            Some(d) if d.stack.is_some() => {
                d.stack = None;
                true
            }
            _ => false,
        }
    }

    /// Whether two assemblies share this definition (not a copy of it).
    pub fn shares(&self, other: &Assembly, id: DefId) -> bool {
        match (self.defs.get(&id), other.defs.get(&id)) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// The definition a path of instances leads to (the root for none).
    pub fn context_def(&self, path: &[InstanceId]) -> Option<DefId> {
        let mut def = ROOT;
        for id in path {
            def = self.def(def)?.instances.get(id)?.def;
        }
        Some(def)
    }

    /// From the coordinates inside a path to the world.
    pub fn world_of(&self, path: &[InstanceId]) -> Option<DAffine3> {
        let mut def = ROOT;
        let mut m = DAffine3::IDENTITY;
        for id in path {
            let inst = self.def(def)?.instances.get(id)?;
            m *= inst.transform;
            def = inst.def;
        }
        Some(m)
    }

    /// Every instance in the world, parents before children.
    pub fn placed(&self) -> Vec<Placed> {
        self.placed_where(|_| true)
    }

    /// The shown instances (a hidden instance hides what is inside it).
    pub fn placed_visible(&self) -> Vec<Placed> {
        self.placed_where(|i| self.is_shown(i))
    }

    fn placed_where(&self, keep: impl Fn(InstanceId) -> bool) -> Vec<Placed> {
        let mut out = Vec::new();
        let mut stack = vec![(Vec::new(), ROOT, DAffine3::IDENTITY)];
        while let Some((path, def, world)) = stack.pop() {
            let Some(d) = self.def(def) else { continue };
            for (id, inst) in d.instances.iter().rev() {
                if !keep(*id) {
                    continue;
                }
                let mut p = path.clone();
                p.push(*id);
                stack.push((p, inst.def, world * inst.transform));
            }
            if !path.is_empty() {
                out.push(Placed { path, def, world });
            }
        }
        out
    }

    /// Everything as one model in world coordinates, groups placed and
    /// added beside the root's geometry, not joined (mesh export).
    pub fn flattened(&self) -> Model {
        let mut out = self.root().model.clone();
        for p in self.placed() {
            if let Some(d) = self.def(p.def) {
                let mut m = d.model.clone();
                transform::affine(&mut m, &p.world);
                out.append(&m);
            }
        }
        out
    }

    /// What is shown as one model in world coordinates (mesh export),
    /// groups' materials on their Default faces.
    pub fn flattened_visible(&self) -> Model {
        let mut out = self.root().model.clone();
        for p in self.placed_visible() {
            if let Some(d) = self.def(p.def) {
                let mut m = d.model.clone();
                // As shown: the group's material on Default sides, and
                // textures placed in the group's own coordinates (so they
                // travel with it).
                let group = p.path.iter().rev().find_map(|i| self.painted_with(*i));
                let ids: Vec<prism_topo::FaceId> = m.faces().map(|(id, _)| id).collect();
                for f in ids {
                    m.edit_face(f, |x| {
                        x.paint.front = x.paint.front.or(group);
                        x.paint.back = x.paint.back.or(group);
                        if x.paint.front.is_some() || x.paint.back.is_some() {
                            x.paint.uv = x.texture_map();
                        }
                    });
                }
                transform::affine(&mut m, &p.world);
                out.append(&m);
            }
        }
        out
    }

    /// Whether there are any groups.
    pub fn has_instances(&self) -> bool {
        !self.root().instances.is_empty()
    }

    fn new_def(&mut self, def: Definition) -> DefId {
        let id = DefId(self.next_def);
        self.next_def += 1;
        self.defs.insert(id, Arc::new(def));
        id
    }

    fn new_instance(&mut self) -> InstanceId {
        let id = InstanceId(self.next_instance);
        self.next_instance += 1;
        id
    }

    /// The id the next new instance gets.
    pub fn next_instance(&self) -> InstanceId {
        InstanceId(self.next_instance)
    }

    /// Keep id counters from going back (after undo): no id is handed out
    /// twice. Definitions are only copied where a counter is behind.
    pub fn keep_counters_from(&mut self, newer: &Assembly) {
        self.next_material = self.next_material.max(newer.next_material);
        self.next_section = self.next_section.max(newer.next_section);
        self.next_work_plane = self.next_work_plane.max(newer.next_work_plane);
        self.next_dim = self.next_dim.max(newer.next_dim);
        self.next_def = self.next_def.max(newer.next_def);
        self.next_instance = self.next_instance.max(newer.next_instance);
        for (id, d) in &newer.defs {
            let c = d.model.counters();
            if let Some(mine) = self.defs.get_mut(id)
                && mine.model.counters() != c
            {
                Arc::make_mut(mine).model.keep_counters_at_least(c);
            }
        }
    }

    /// A deep copy of a definition, nested groups included, as new
    /// definitions (DECISIONS #22).
    fn copy_def(&mut self, id: DefId) -> Option<DefId> {
        let d = self.def(id)?.clone();
        let mut instances = BTreeMap::new();
        for inst in d.instances.values() {
            // A component inside stays the same component.
            let def = if self.def(inst.def)?.is_component() {
                inst.def
            } else {
                self.copy_def(inst.def)?
            };
            let new = self.new_instance();
            instances.insert(
                new,
                Instance {
                    def,
                    transform: inst.transform,
                },
            );
        }
        Some(self.new_def(Definition {
            model: d.model,
            instances,
            name: d.name,
            stack: d.stack,
        }))
    }

    /// The first free component name from `base` ("Box", "Box#2", ...).
    pub fn free_name(&self, base: &str) -> String {
        let base = base.split('#').next().unwrap_or(base).trim();
        let base = if base.is_empty() { "Component" } else { base };
        let taken = |n: &str| self.defs().any(|(_, d)| d.name.as_deref() == Some(n));
        if !taken(base) {
            return base.to_owned();
        }
        (2..)
            .map(|k| format!("{base}#{k}"))
            .find(|n| !taken(n))
            .unwrap_or_default()
    }

    /// The components, by name.
    pub fn components(&self) -> Vec<(DefId, &str)> {
        let mut out: Vec<(DefId, &str)> = self
            .defs()
            .filter_map(|(id, d)| d.name.as_deref().map(|n| (id, n)))
            .collect();
        out.sort_by(|a, b| a.1.cmp(b.1));
        out
    }

    /// How many instances place a definition, in the whole document.
    pub fn uses(&self, def: DefId) -> usize {
        self.placed().iter().filter(|p| p.def == def).count()
    }

    /// Make Component: as Make Group, but the definition is a component
    /// named `base` (or the next free name).
    pub fn make_component(
        &mut self,
        ctx: DefId,
        sel: &Selection,
        instances: &BTreeSet<InstanceId>,
        base: &str,
    ) -> Option<InstanceId> {
        let id = self.make_group(ctx, sel, instances)?;
        let name = self.free_name(base);
        let def = self.def(ctx)?.instances.get(&id)?.def;
        self.def_mut_live(def)?.name = Some(name);
        Some(id)
    }

    /// Make Unique: each instance gets its own copy of its definition, as
    /// a component with a new name.
    pub fn make_unique(&mut self, ctx: DefId, instances: &BTreeSet<InstanceId>) {
        for i in instances {
            let Some(inst) = self.def(ctx).and_then(|d| d.instances.get(i)).copied() else {
                continue;
            };
            let Some(old) = self.def(inst.def).cloned() else {
                continue;
            };
            if old.name.is_none() || self.uses(inst.def) < 2 {
                continue;
            }
            let Some(new) = self.copy_def(inst.def) else {
                continue;
            };
            let name = self.free_name(old.name.as_deref().unwrap_or("Component"));
            if let Some(d) = self.def_mut_live(new) {
                d.name = Some(name);
            }
            if let Some(slot) = self.def_mut_live(ctx).and_then(|d| d.instances.get_mut(i)) {
                slot.def = new;
            }
        }
    }

    /// Rename a component (refused if the name is empty or taken).
    pub fn rename(&mut self, def: DefId, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty()
            || self
                .defs()
                .any(|(id, d)| id != def && d.name.as_deref() == Some(name))
        {
            return false;
        }
        match self.def_mut_live(def) {
            Some(d) if d.name.is_some() => {
                d.name = Some(name.to_owned());
                true
            }
            _ => false,
        }
    }

    /// Drop definitions no instance reaches.
    fn prune(&mut self) {
        let mut reached: BTreeSet<DefId> = [ROOT].into();
        let mut stack = vec![ROOT];
        while let Some(d) = stack.pop() {
            if let Some(def) = self.def(d) {
                for inst in def.instances.values() {
                    if reached.insert(inst.def) {
                        stack.push(inst.def);
                    }
                }
            }
        }
        self.defs.retain(|id, _| reached.contains(id));
        let alive: BTreeSet<InstanceId> = self
            .defs
            .values()
            .flat_map(|d| d.instances.keys().copied())
            .collect();
        self.hidden.retain(|i| alive.contains(i));
        self.names.retain(|i, _| alive.contains(i));
        self.on_tag.retain(|i, _| alive.contains(i));
        self.painted.retain(|i, _| alive.contains(i));
        self.locked.retain(|i| alive.contains(i));
    }

    /// Make Group: the selection (and the selected groups) in `ctx` move
    /// into a new group, placed at the corner of their box. Returns it.
    pub fn make_group(
        &mut self,
        ctx: DefId,
        sel: &Selection,
        instances: &BTreeSet<InstanceId>,
    ) -> Option<InstanceId> {
        let parent = self.def(ctx)?;
        let instances: BTreeSet<InstanceId> = instances
            .iter()
            .copied()
            .filter(|i| parent.instances.contains_key(i))
            .collect();
        if sel.is_empty() && instances.is_empty() {
            return None;
        }
        let parent = self.def_mut_live(ctx)?;
        // A body with live features gives its solids' features to the group.
        let mut carried: Option<Stack> = None;
        let mut geo = match parent.stack.take() {
            Some(mut stack) if !sel.is_empty() => {
                let (taken, c) = stack.take(&parent.model, sel);
                carried = c;
                if stack.features.is_empty() {
                    parent.model = stack.base;
                } else {
                    parent.model = stack.evaluate();
                    parent.stack = Some(stack);
                }
                taken
            }
            other => {
                parent.stack = other;
                prism_ops::select::take(&mut parent.model, sel)
            }
        };
        let moved: Vec<(InstanceId, Instance)> = instances
            .iter()
            .filter_map(|i| parent.instances.remove(i).map(|inst| (*i, inst)))
            .collect();
        // The group's origin: the low corner of what goes in.
        let mut lo = geo.bbox().map(|(lo, _)| lo);
        for (_, inst) in &moved {
            let at = inst.transform.translation;
            lo = Some(lo.map_or(at, |l: DVec3| l.min(at)));
        }
        let origin = lo.unwrap_or(DVec3::ZERO);
        let back = DAffine3::from_translation(-origin);
        transform::affine(&mut geo, &back);
        if let Some(c) = &mut carried {
            c.moved(&back);
            geo = c.evaluate();
        }
        let inner = moved
            .into_iter()
            .map(|(i, inst)| {
                (
                    i,
                    Instance {
                        def: inst.def,
                        transform: back * inst.transform,
                    },
                )
            })
            .collect();
        let def = self.new_def(Definition {
            model: geo,
            instances: inner,
            name: None,
            stack: carried,
        });
        let id = self.new_instance();
        self.def_mut_live(ctx)?.instances.insert(
            id,
            Instance {
                def,
                transform: DAffine3::from_translation(origin),
            },
        );
        Some(id)
    }

    /// Put a finished model in a new group in `ctx`, named `name`, placed
    /// by `at` (the model is in the group's own coordinates). Returns the
    /// group; None if the model is empty.
    pub fn add_group_model(
        &mut self,
        ctx: DefId,
        model: Model,
        name: &str,
        at: DAffine3,
    ) -> Option<InstanceId> {
        if model.face_count() == 0 && model.edge_count() == 0 {
            return None;
        }
        self.def(ctx)?;
        let def = self.new_def(Definition {
            model,
            instances: BTreeMap::new(),
            name: None,
            stack: None,
        });
        let id = self.new_instance();
        self.def_mut_live(ctx)?
            .instances
            .insert(id, Instance { def, transform: at });
        self.set_name(id, name);
        Some(id)
    }

    /// Explode: the group's geometry, placed, joins `ctx`'s model, where
    /// the overlap rules run (PLAN 1.9); its own groups become `ctx`'s.
    pub fn explode(&mut self, ctx: DefId, id: InstanceId) -> bool {
        let Some(inst) = self.def(ctx).and_then(|d| d.instances.get(&id)).copied() else {
            return false;
        };
        let Some(child) = self.def(inst.def).cloned() else {
            return false;
        };
        let mut geo = child.model;
        transform::affine(&mut geo, &inst.transform);
        let Some(parent) = self.def_mut_live(ctx) else {
            return false;
        };
        match child.stack {
            // The body's live features go with it, on the base.
            Some(mut carried) => {
                carried.moved(&inst.transform);
                let stack = parent
                    .stack
                    .get_or_insert_with(|| Stack::new(parent.model.clone()));
                join(&mut stack.base, &carried.base);
                stack.adopt(carried.features);
                parent.model = stack.evaluate();
            }
            None => parent.add_geometry(|m| join(m, &geo)),
        }
        parent.instances.remove(&id);
        for (cid, ci) in child.instances {
            parent.instances.insert(
                cid,
                Instance {
                    def: ci.def,
                    transform: inst.transform * ci.transform,
                },
            );
        }
        self.prune();
        true
    }

    // ---------- the clipboard (Everyday Commands part 1) ----------

    /// Copy: a selection in `ctx` as an assembly of its own, in world
    /// coordinates (`to_world` maps `ctx` there). Its root holds the loose
    /// geometry (`geo`, already in world coordinates) and the instances,
    /// with everything they use: definitions, materials, images, tags.
    /// Copies are not locked.
    pub fn clip(
        &self,
        ctx: DefId,
        to_world: &DAffine3,
        geo: Model,
        instances: &BTreeSet<InstanceId>,
    ) -> Assembly {
        let placed: BTreeMap<InstanceId, Instance> = self
            .def(ctx)
            .map(|d| {
                instances
                    .iter()
                    .filter_map(|i| {
                        d.instances.get(i).map(|x| {
                            (
                                *i,
                                Instance {
                                    def: x.def,
                                    transform: *to_world * x.transform,
                                },
                            )
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut out = self.clone();
        out.defs.insert(
            ROOT,
            Arc::new(Definition {
                model: geo,
                instances: placed,
                name: None,
                stack: None,
            }),
        );
        out.prune();
        out.purge_materials();
        let used: BTreeSet<TagId> = out.on_tag.values().copied().collect();
        out.tags.retain(|t, _| used.contains(t));
        out.locked.clear();
        out
    }

    /// Whether a clip holds nothing.
    pub fn is_empty(&self) -> bool {
        let r = self.root();
        r.instances.is_empty() && r.model.vertices().next().is_none()
    }

    /// Paste: a clip's contents into `ctx`, moved by `at` (from the clip's
    /// coordinates to `ctx`'s). Loose geometry joins `ctx`'s by the overlap
    /// rules; groups stay groups. A material, image or tag that is already
    /// here (the same name and value) is reused, as is a component whose
    /// definition is the same; a different component with a name in use
    /// gets a new name.
    pub fn paste(&mut self, ctx: DefId, clip: &Assembly, at: &DAffine3) -> bool {
        if self.def(ctx).is_none() || clip.is_empty() {
            return false;
        }
        let mut map = Remap::default();
        for img in clip.images.values() {
            self.add_image(img.clone());
        }
        for (id, m) in &clip.materials {
            let here = self
                .materials
                .iter()
                .find(|(_, x)| *x == m)
                .map(|(k, _)| *k);
            let to = here.unwrap_or_else(|| self.add_material(m.clone()));
            map.materials.insert(*id, to);
        }
        for (id, t) in &clip.tags {
            let here = self
                .tags
                .iter()
                .find(|(_, x)| x.name == t.name)
                .map(|(k, _)| *k);
            let to = here.unwrap_or_else(|| self.add_tag(&t.name));
            map.tags.insert(*id, to);
        }
        let root = clip.root().clone();
        for (ci, inst) in &root.instances {
            let Some(def) = self.bring(clip, inst.def, &mut map) else {
                continue;
            };
            let id = self.new_instance();
            if let Some(d) = self.def_mut_live(ctx) {
                d.instances.insert(
                    id,
                    Instance {
                        def,
                        transform: *at * inst.transform,
                    },
                );
            }
            self.copy_attributes(clip, *ci, id, &map);
        }
        let mut geo = root.model;
        crate::material::remap(&mut geo, &map.materials);
        transform::affine(&mut geo, at);
        if geo.vertices().next().is_some()
            && let Some(d) = self.def_mut_live(ctx)
        {
            d.add_geometry(|m| join(m, &geo));
        }
        self.prune();
        true
    }

    /// A clip's definition brought into this assembly (its own instances
    /// first). Returns its id here.
    fn bring(&mut self, clip: &Assembly, d: DefId, map: &mut Remap) -> Option<DefId> {
        if let Some(x) = map.defs.get(&d) {
            return Some(*x);
        }
        let src = clip.def(d)?.clone();
        let mut model = src.model.clone();
        crate::material::remap(&mut model, &map.materials);
        let mut instances = BTreeMap::new();
        let mut inner = Vec::new();
        for (ci, inst) in &src.instances {
            let def = self.bring(clip, inst.def, map)?;
            inner.push((*ci, def, inst.transform));
        }
        // A component already here, the same, is reused.
        if src.is_component() {
            let same = self.defs.iter().find(|(k, x)| {
                **k != ROOT
                    && x.name == src.name
                    && x.model == model
                    && x.instances.len() == inner.len()
                    && x.instances
                        .values()
                        .zip(&inner)
                        .all(|(a, (_, def, t))| a.def == *def && a.transform == *t)
            });
            if let Some((k, _)) = same {
                let k = *k;
                map.defs.insert(d, k);
                return Some(k);
            }
        }
        for (ci, def, transform) in inner {
            let id = self.new_instance();
            instances.insert(id, Instance { def, transform });
            self.copy_attributes(clip, ci, id, map);
        }
        let name = src.name.as_deref().map(|n| {
            if self.defs().any(|(_, x)| x.name.as_deref() == Some(n)) {
                self.free_name(n)
            } else {
                n.to_owned()
            }
        });
        let k = self.new_def(Definition {
            model,
            instances,
            name,
            stack: None,
        });
        map.defs.insert(d, k);
        Some(k)
    }

    /// A clip instance's name, tag, paint and hidden flag, onto `to`.
    fn copy_attributes(&mut self, clip: &Assembly, from: InstanceId, to: InstanceId, map: &Remap) {
        if let Some(n) = clip.names.get(&from) {
            self.names.insert(to, n.clone());
        }
        if let Some(t) = clip.on_tag.get(&from).and_then(|t| map.tags.get(t)) {
            self.on_tag.insert(to, *t);
        }
        if let Some(m) = clip.painted.get(&from).and_then(|m| map.materials.get(m)) {
            self.painted.insert(to, *m);
        }
        if clip.hidden.contains(&from) {
            self.hidden.insert(to);
        }
    }

    /// Where an instance is placed: the definition it is in, and itself
    /// (instance ids are unique in the document).
    pub fn find_instance(&self, i: InstanceId) -> Option<(DefId, Instance)> {
        self.defs
            .iter()
            .find_map(|(d, def)| def.instances.get(&i).map(|inst| (*d, *inst)))
    }

    /// Every point of a definition's contents in its own coordinates: its
    /// vertices and those of its groups, placed (Group Axes).
    pub fn content_points(&self, def: DefId) -> Vec<DVec3> {
        let Some(d) = self.def(def) else {
            return Vec::new();
        };
        let mut out: Vec<DVec3> = d.model.vertices().map(|(_, p)| p).collect();
        for inst in d.instances.values() {
            out.extend(
                self.content_points(inst.def)
                    .into_iter()
                    .map(|p| inst.transform.transform_point3(p)),
            );
        }
        out
    }

    /// Change Axes (brief 36): `axes` (in the definition's own coordinates)
    /// become its origin and directions. Its contents move by the inverse
    /// and every instance of it by `axes`, so nothing moves in the world,
    /// every copy of a component included. False if nothing changed.
    pub fn change_axes(&mut self, def: DefId, axes: &DAffine3) -> bool {
        let det = axes.matrix3.determinant();
        if def == ROOT || !det.is_finite() || det.abs() < 1e-12 || !self.defs.contains_key(&def) {
            return false;
        }
        if *axes == DAffine3::IDENTITY {
            return false;
        }
        let back = axes.inverse();
        if let Some(d) = self.def_mut(def) {
            transform::affine(&mut d.model, &back);
            for inst in d.instances.values_mut() {
                inst.transform = back * inst.transform;
            }
        }
        let parents: Vec<DefId> = self
            .defs
            .iter()
            .filter(|(_, d)| d.instances.values().any(|i| i.def == def))
            .map(|(id, _)| *id)
            .collect();
        for p in parents {
            if let Some(d) = self.def_mut(p) {
                for inst in d.instances.values_mut().filter(|i| i.def == def) {
                    inst.transform *= *axes;
                }
            }
        }
        true
    }

    /// The model's own axes (Annotation & Sections part 4): `axes` (a
    /// rigid frame in world coordinates) become the world's origin and
    /// directions. Everything moves by the inverse: the loose geometry,
    /// guides, the groups and components placed at the top level, section
    /// planes, construction planes and dimensions. What is inside a group keeps its own
    /// coordinates. False if nothing changed (not rigid, or the same).
    pub fn change_model_axes(&mut self, axes: &DAffine3) -> bool {
        let m = axes.matrix3;
        let rigid = m.determinant() > 0.0
            && m.x_axis.is_finite()
            && m.y_axis.is_finite()
            && m.z_axis.is_finite()
            && axes.translation.is_finite()
            && (m.x_axis.length() - 1.0).abs() < 1e-9
            && (m.y_axis.length() - 1.0).abs() < 1e-9
            && (m.z_axis.length() - 1.0).abs() < 1e-9
            && m.x_axis.dot(m.y_axis).abs() < 1e-9
            && m.x_axis.dot(m.z_axis).abs() < 1e-9
            && m.y_axis.dot(m.z_axis).abs() < 1e-9;
        if !rigid || *axes == DAffine3::IDENTITY {
            return false;
        }
        let back = axes.inverse();
        if let Some(d) = self.def_mut(ROOT) {
            transform::affine(&mut d.model, &back);
            for inst in d.instances.values_mut() {
                inst.transform = back * inst.transform;
            }
        }
        for s in self.sections.values_mut() {
            s.origin = back.transform_point3(s.origin);
            s.normal = back.transform_vector3(s.normal).normalize();
        }
        for p in self.work_planes.values_mut() {
            p.origin = back.transform_point3(p.origin);
            p.normal = back.transform_vector3(p.normal).normalize();
        }
        for d in self.dimensions.values_mut() {
            d.a = back.transform_point3(d.a);
            d.b = back.transform_point3(d.b);
            d.offset = back.transform_vector3(d.offset);
        }
        true
    }

    /// Reset Axes (brief 36): the axes for instance `i` that are square
    /// with its parent's again, origin at the low corner of the contents.
    /// A group scaled unevenly keeps its directions (only the origin
    /// moves), so curved geometry stays exact.
    pub fn reset_axes(&self, i: InstanceId) -> Option<DAffine3> {
        let (_, inst) = self.find_instance(i)?;
        let m = inst.transform.matrix3;
        let c = m.determinant().cbrt();
        let mut turn = m.inverse() * prism_math::DMat3::from_diagonal(DVec3::splat(c));
        if !c.is_finite() || prism_geom::even_scale(&DAffine3::from_mat3(turn)).is_none() {
            turn = prism_math::DMat3::IDENTITY;
        }
        let back = turn.inverse();
        let pts = self.content_points(inst.def);
        let lo = pts
            .iter()
            .map(|p| back * *p)
            .reduce(DVec3::min)
            .unwrap_or(DVec3::ZERO);
        Some(DAffine3::from_mat3_translation(turn, turn * lo))
    }

    /// Axes to Centre (brief 36): the axes for definition `def` with the
    /// origin at the centre of its contents' box, directions kept.
    pub fn centre_axes(&self, def: DefId) -> Option<DAffine3> {
        let pts = self.content_points(def);
        let lo = pts.iter().copied().reduce(DVec3::min)?;
        let hi = pts.iter().copied().reduce(DVec3::max)?;
        Some(DAffine3::from_translation((lo + hi) * 0.5))
    }

    /// Change the placement of instances in `ctx` by `m` (in `ctx`'s
    /// coordinates).
    pub fn place(&mut self, ctx: DefId, instances: &BTreeSet<InstanceId>, m: &DAffine3) {
        if let Some(d) = self.def_mut_live(ctx) {
            for i in instances {
                if let Some(inst) = d.instances.get_mut(i) {
                    inst.transform = *m * inst.transform;
                }
            }
        }
    }

    /// Copies of instances in `ctx`, each `steps[k]` from the original.
    /// Each copy of a group is a group of its own (DECISIONS #22).
    pub fn copy(&mut self, ctx: DefId, instances: &BTreeSet<InstanceId>, steps: &[DAffine3]) {
        let originals: Vec<(InstanceId, Instance)> = self
            .def(ctx)
            .map(|d| {
                instances
                    .iter()
                    .filter_map(|i| d.instances.get(i).map(|x| (*i, *x)))
                    .collect()
            })
            .unwrap_or_default();
        for m in steps {
            for (from, inst) in &originals {
                // A component's copies share it; a group's copies do not.
                let shared = self.def(inst.def).is_some_and(Definition::is_component);
                let Some(def) = (if shared {
                    Some(inst.def)
                } else {
                    self.copy_def(inst.def)
                }) else {
                    continue;
                };
                let id = self.new_instance();
                if let Some(d) = self.def_mut_live(ctx) {
                    d.instances.insert(
                        id,
                        Instance {
                            def,
                            transform: *m * inst.transform,
                        },
                    );
                }
                // A copy keeps its original's name and tag.
                if let Some(n) = self.names.get(from).cloned() {
                    self.names.insert(id, n);
                }
                if let Some(t) = self.on_tag.get(from).copied() {
                    self.on_tag.insert(id, t);
                }
                if let Some(mat) = self.painted.get(from).copied() {
                    self.painted.insert(id, mat);
                }
            }
        }
    }

    // ---------- booleans (Solids & CAD part 2, brief 39) ----------

    /// A group's own geometry, placed in `ctx`. Refused for a group with
    /// groups inside (explode them first).
    fn solid_in(&self, ctx: DefId, id: InstanceId) -> Result<(Instance, Model), &'static str> {
        let inst = *self
            .def(ctx)
            .and_then(|d| d.instances.get(&id))
            .ok_or(prism_ops::boolean::NOT_SOLID)?;
        let def = self.def(inst.def).ok_or(prism_ops::boolean::NOT_SOLID)?;
        if !def.instances.is_empty() {
            return Err(NESTED);
        }
        let mut m = def.model.clone();
        transform::affine(&mut m, &inst.transform);
        Ok((inst, m))
    }

    /// The results of `op` on `first` and `second` (groups in `ctx`), each
    /// with the instance whose axes it takes, without changing anything.
    pub fn boolean_results(
        &self,
        ctx: DefId,
        op: SolidOp,
        first: InstanceId,
        second: InstanceId,
    ) -> Result<Vec<(Instance, Model)>, &'static str> {
        use prism_ops::boolean::{Op, boolean};
        if first == second {
            return Err(TWO_SOLIDS);
        }
        let (fi, fm) = self.solid_in(ctx, first)?;
        let (si, sm) = self.solid_in(ctx, second)?;
        Ok(match op {
            SolidOp::Union => vec![(si, boolean(&fm, &sm, Op::Union)?)],
            SolidOp::Intersect => vec![(si, boolean(&fm, &sm, Op::Intersect)?)],
            // The first is the cutter; the second is cut.
            SolidOp::Subtract | SolidOp::Trim => vec![(si, boolean(&sm, &fm, Op::Subtract)?)],
            SolidOp::Split => vec![
                (si, boolean(&sm, &fm, Op::Subtract)?),
                (fi, boolean(&fm, &sm, Op::Subtract)?),
                (si, boolean(&fm, &sm, Op::Intersect)?),
            ],
        })
    }

    /// Run a boolean: the results become groups in `ctx` at their axes, and
    /// the inputs go (Trim keeps the cutter). Returns the new groups.
    pub fn boolean(
        &mut self,
        ctx: DefId,
        op: SolidOp,
        first: InstanceId,
        second: InstanceId,
    ) -> Result<Vec<InstanceId>, &'static str> {
        let results = self.boolean_results(ctx, op, first, second)?;
        let gone: BTreeSet<InstanceId> = if op == SolidOp::Trim {
            [second].into()
        } else {
            [first, second].into()
        };
        Ok(self.replace_with(ctx, &gone, results))
    }

    /// Outer Shell: the union of all of `instances` (in id order), at the
    /// first one's axes, without changing anything.
    pub fn outer_shell_result(
        &self,
        ctx: DefId,
        instances: &BTreeSet<InstanceId>,
    ) -> Result<(Instance, Model), &'static str> {
        let mut it = instances.iter();
        let first = it.next().ok_or(TWO_SOLIDS)?;
        let (fi, mut acc) = self.solid_in(ctx, *first)?;
        let mut n = 1;
        for id in it {
            let (_, m) = self.solid_in(ctx, *id)?;
            acc = prism_ops::boolean::boolean(&acc, &m, prism_ops::boolean::Op::Union)?;
            n += 1;
        }
        if n < 2 {
            return Err(TWO_SOLIDS);
        }
        Ok((fi, acc))
    }

    /// Outer Shell, run. Returns the new group.
    pub fn outer_shell(
        &mut self,
        ctx: DefId,
        instances: &BTreeSet<InstanceId>,
    ) -> Result<Vec<InstanceId>, &'static str> {
        let r = self.outer_shell_result(ctx, instances)?;
        Ok(self.replace_with(ctx, instances, vec![r]))
    }

    /// Erase `gone` from `ctx` and add each result with faces as a group
    /// placed like its instance.
    fn replace_with(
        &mut self,
        ctx: DefId,
        gone: &BTreeSet<InstanceId>,
        results: Vec<(Instance, Model)>,
    ) -> Vec<InstanceId> {
        self.erase(ctx, gone);
        let mut made = Vec::new();
        for (at, mut m) in results {
            if m.faces().next().is_none() {
                continue;
            }
            transform::affine(&mut m, &at.transform.inverse());
            let def = self.new_def(Definition {
                model: m,
                instances: BTreeMap::new(),
                name: None,
                stack: None,
            });
            let id = self.new_instance();
            if let Some(d) = self.def_mut_live(ctx) {
                d.instances.insert(
                    id,
                    Instance {
                        def,
                        transform: at.transform,
                    },
                );
                made.push(id);
            }
        }
        made
    }

    /// Erase instances from `ctx`.
    pub fn erase(&mut self, ctx: DefId, instances: &BTreeSet<InstanceId>) {
        if let Some(d) = self.def_mut_live(ctx) {
            for i in instances {
                d.instances.remove(i);
            }
        }
        self.prune();
    }

    // ---------- materials (Basic Materials) ----------

    /// The materials, and the counter for new ones (for files).
    pub fn materials(&self) -> (&BTreeMap<MaterialId, Material>, u32) {
        (&self.materials, self.next_material)
    }

    pub fn material(&self, id: MaterialId) -> Option<&Material> {
        self.materials.get(&id)
    }

    /// Instances painted as a whole (for files).
    pub fn painted(&self) -> &BTreeMap<InstanceId, MaterialId> {
        &self.painted
    }

    /// The material an instance is painted with, if any.
    pub fn painted_with(&self, i: InstanceId) -> Option<MaterialId> {
        self.painted.get(&i).copied()
    }

    /// Materials, counter and painted instances read from a file. Faces and
    /// instances naming a material that is missing go back to Default.
    pub fn with_materials(
        mut self,
        materials: BTreeMap<MaterialId, Material>,
        next: u32,
        painted: BTreeMap<InstanceId, MaterialId>,
    ) -> Assembly {
        self.next_material = next.max(materials.keys().next_back().map_or(0, |m| m.0 + 1));
        self.materials = materials;
        self.painted = painted;
        self.drop_missing_materials();
        self.prune();
        self
    }

    /// Add a material (its name made unique). Returns its id.
    pub fn add_material(&mut self, mut m: Material) -> MaterialId {
        m.name = crate::material::free_name(&self.materials, &m.name);
        let id = MaterialId(self.next_material);
        self.next_material += 1;
        self.materials.insert(id, m);
        id
    }

    /// Change a material (its name kept unique among the others).
    pub fn edit_material(&mut self, id: MaterialId, mut m: Material) {
        if !self.materials.contains_key(&id) {
            return;
        }
        let others: BTreeMap<MaterialId, Material> = self
            .materials
            .iter()
            .filter(|(k, _)| **k != id)
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        m.name = crate::material::free_name(&others, &m.name);
        self.materials.insert(id, m);
    }

    /// Remove a material: what used it goes back to Default.
    pub fn remove_material(&mut self, id: MaterialId) {
        if self.materials.remove(&id).is_some() {
            self.drop_missing_materials();
        }
    }

    /// Every material nothing uses.
    pub fn unused_materials(&self) -> BTreeSet<MaterialId> {
        let used: BTreeSet<MaterialId> = self
            .defs
            .values()
            .flat_map(|d| crate::material::used_in(&d.model))
            .chain(self.painted.values().copied())
            .collect();
        self.materials
            .keys()
            .filter(|m| !used.contains(m))
            .copied()
            .collect()
    }

    /// Remove every material nothing uses, then every image no material
    /// uses.
    pub fn purge_materials(&mut self) {
        for m in self.unused_materials() {
            self.materials.remove(&m);
        }
        self.purge_images();
    }

    // ---------- images (Basic Materials part 2) ----------

    /// The texture images (for files).
    pub fn images(&self) -> &BTreeMap<crate::image::ImageId, crate::image::Image> {
        &self.images
    }

    pub fn image(&self, id: &crate::image::ImageId) -> Option<&crate::image::Image> {
        self.images.get(id)
    }

    /// Keep an image (once: the same bytes have the same id).
    pub fn add_image(&mut self, image: crate::image::Image) {
        self.images.entry(image.id()).or_insert(image);
    }

    /// Images read from a file. A texture naming a missing image is
    /// dropped (the material keeps its colour).
    pub fn with_images(
        mut self,
        images: BTreeMap<crate::image::ImageId, crate::image::Image>,
    ) -> Assembly {
        self.images = images;
        for m in self.materials.values_mut() {
            if m.texture
                .as_ref()
                .is_some_and(|t| !self.images.contains_key(&t.image))
            {
                m.texture = None;
            }
        }
        self
    }

    /// Images no material uses.
    pub fn unused_images(&self) -> BTreeSet<crate::image::ImageId> {
        let used: BTreeSet<&crate::image::ImageId> = self
            .materials
            .values()
            .filter_map(|m| m.texture.as_ref().map(|t| &t.image))
            .collect();
        self.images
            .keys()
            .filter(|i| !used.contains(i))
            .cloned()
            .collect()
    }

    /// Remove every image no material uses.
    pub fn purge_images(&mut self) {
        for i in self.unused_images() {
            self.images.remove(&i);
        }
    }

    /// Paint instances as a whole (None: back to Default).
    pub fn paint_instances(&mut self, instances: &BTreeSet<InstanceId>, m: Option<MaterialId>) {
        for i in instances {
            match m.filter(|m| self.materials.contains_key(m)) {
                Some(m) => {
                    self.painted.insert(*i, m);
                }
                None => {
                    self.painted.remove(i);
                }
            }
        }
        self.prune();
    }

    /// Replace one material by another on faces (and painted instances),
    /// in `ctx` only or everywhere.
    pub fn replace_material(
        &mut self,
        ctx: Option<DefId>,
        from: Option<MaterialId>,
        to: Option<MaterialId>,
    ) {
        let ids: Vec<DefId> = match ctx {
            Some(c) => vec![c],
            None => self.defs.keys().copied().collect(),
        };
        for id in ids {
            let sides = self
                .def(id)
                .map(|d| crate::material::sides_with(&d.model, from))
                .unwrap_or_default();
            if sides.is_empty() {
                continue;
            }
            if let Some(d) = self.def_mut_live(id) {
                crate::material::paint(&mut d.model, &sides, to);
                // The live features' base too, or the next run loses it.
                if let Some(s) = &mut d.stack {
                    let base = crate::material::sides_with(&s.base, from);
                    crate::material::paint(&mut s.base, &base, to);
                }
            }
        }
        if from.is_some() {
            let insts: BTreeSet<InstanceId> = match ctx {
                Some(c) => self
                    .def(c)
                    .map(|d| d.instances.keys().copied().collect())
                    .unwrap_or_default(),
                None => self.painted.keys().copied().collect(),
            };
            for i in insts {
                if self.painted.get(&i).copied() == from {
                    match to {
                        Some(t) => {
                            self.painted.insert(i, t);
                        }
                        None => {
                            self.painted.remove(&i);
                        }
                    }
                }
            }
        }
    }

    /// Faces and instances naming a material not in the table go back to
    /// Default.
    fn drop_missing_materials(&mut self) {
        let have: BTreeSet<MaterialId> = self.materials.keys().copied().collect();
        self.painted.retain(|_, m| have.contains(m));
        let ids: Vec<DefId> = self.defs.keys().copied().collect();
        for id in ids {
            let stale = self.def(id).is_some_and(|d| {
                crate::material::used_in(&d.model)
                    .iter()
                    .any(|m| !have.contains(m))
            });
            if !stale {
                continue;
            }
            if let Some(d) = self.def_mut_live(id) {
                let clear = |m: &mut Model| {
                    let faces: Vec<FaceId> = m.faces().map(|(f, _)| f).collect();
                    for f in faces {
                        m.edit_face(f, |x| {
                            if x.paint.front.is_some_and(|m| !have.contains(&m)) {
                                x.paint.front = None;
                            }
                            if x.paint.back.is_some_and(|m| !have.contains(&m)) {
                                x.paint.back = None;
                            }
                        });
                    }
                };
                clear(&mut d.model);
                if let Some(s) = &mut d.stack {
                    clear(&mut s.base);
                }
            }
        }
    }
}

/// How a clip's ids map to an assembly's, while pasting.
#[derive(Default)]
struct Remap {
    materials: BTreeMap<MaterialId, MaterialId>,
    tags: BTreeMap<TagId, TagId>,
    defs: BTreeMap<DefId, DefId>,
}

/// Join `geo` to `model`: points on points weld, then faces on one plane
/// are rebuilt as one set of regions (#30) and repeated faces become one
/// (Explode, Paste).
fn join(model: &mut Model, geo: &Model) {
    let before: BTreeSet<VertexId> = model.vertices().map(|(v, _)| v).collect();
    model.append(geo);
    let new: BTreeSet<VertexId> = model
        .vertices()
        .map(|(v, _)| v)
        .filter(|v| !before.contains(v))
        .collect();
    let at: Vec<DVec3> = new.iter().map(|v| model.vertex(*v)).collect();
    let tol = model.tol();
    transform::weld_new(model, &new, &tol);
    let touched: BTreeSet<VertexId> = model
        .vertices()
        .filter(|(_, p)| at.iter().any(|q| tol.same_point(*p, *q)))
        .map(|(v, _)| v)
        .collect();
    prism_ops::settle::settle(model, &touched);
    prism_ops::settle::dedupe_faces(model);
}
