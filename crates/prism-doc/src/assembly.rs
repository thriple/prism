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
use prism_topo::{FaceId, MaterialId, Model, VertexId};

use crate::material::Material;
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

/// A tag: a name, and whether what is on it is shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub visible: bool,
}

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
}

impl Definition {
    pub fn is_component(&self) -> bool {
        self.name.is_some()
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
}

/// An instance's name, tag and whether it is hidden (for files).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attributes {
    pub hidden: bool,
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
    pub fn def_mut(&mut self, id: DefId) -> Option<&mut Definition> {
        self.defs.get_mut(&id).map(Arc::make_mut)
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

    /// What is shown as one model in world coordinates (mesh export).
    pub fn flattened_visible(&self) -> Model {
        let mut out = self.root().model.clone();
        for p in self.placed_visible() {
            if let Some(d) = self.def(p.def) {
                let mut m = d.model.clone();
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
        self.def_mut(def)?.name = Some(name);
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
            if let Some(d) = self.def_mut(new) {
                d.name = Some(name);
            }
            if let Some(slot) = self.def_mut(ctx).and_then(|d| d.instances.get_mut(i)) {
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
        match self.def_mut(def) {
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
        let parent = self.def_mut(ctx)?;
        let mut geo = prism_ops::select::take(&mut parent.model, sel);
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
        });
        let id = self.new_instance();
        self.def_mut(ctx)?.instances.insert(
            id,
            Instance {
                def,
                transform: DAffine3::from_translation(origin),
            },
        );
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
        let Some(parent) = self.def_mut(ctx) else {
            return false;
        };
        let before: BTreeSet<VertexId> = parent.model.vertices().map(|(v, _)| v).collect();
        parent.model.append(&geo);
        let new: BTreeSet<VertexId> = parent
            .model
            .vertices()
            .map(|(v, _)| v)
            .filter(|v| !before.contains(v))
            .collect();
        let at: Vec<DVec3> = new.iter().map(|v| parent.model.vertex(*v)).collect();
        // Join: points on points weld, then faces on one plane are rebuilt
        // as one set of regions (#30) and repeated faces become one.
        let tol = parent.model.tol();
        transform::weld_new(&mut parent.model, &new, &tol);
        let touched: BTreeSet<VertexId> = parent
            .model
            .vertices()
            .filter(|(_, p)| at.iter().any(|q| tol.same_point(*p, *q)))
            .map(|(v, _)| v)
            .collect();
        prism_ops::settle::settle(&mut parent.model, &touched);
        prism_ops::settle::dedupe_faces(&mut parent.model);
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

    /// Change the placement of instances in `ctx` by `m` (in `ctx`'s
    /// coordinates).
    pub fn place(&mut self, ctx: DefId, instances: &BTreeSet<InstanceId>, m: &DAffine3) {
        if let Some(d) = self.def_mut(ctx) {
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
                if let Some(d) = self.def_mut(ctx) {
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

    /// Erase instances from `ctx`.
    pub fn erase(&mut self, ctx: DefId, instances: &BTreeSet<InstanceId>) {
        if let Some(d) = self.def_mut(ctx) {
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
            if let Some(d) = self.def_mut(id) {
                crate::material::paint(&mut d.model, &sides, to);
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
            if let Some(d) = self.def_mut(id) {
                let faces: Vec<FaceId> = d.model.faces().map(|(f, _)| f).collect();
                for f in faces {
                    d.model.edit_face(f, |x| {
                        if x.paint.front.is_some_and(|m| !have.contains(&m)) {
                            x.paint.front = None;
                        }
                        if x.paint.back.is_some_and(|m| !have.contains(&m)) {
                            x.paint.back = None;
                        }
                    });
                }
            }
        }
    }
}
