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
use prism_topo::{Model, VertexId};
use serde::{Deserialize, Serialize};

/// A definition's id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DefId(pub u32);

/// An instance's id, unique in the whole document (a path of them names an
/// open group).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InstanceId(pub u32);

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
                }),
            )]
            .into(),
            next_def: 1,
            next_instance: 0,
        }
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
        let mut out = Vec::new();
        let mut stack = vec![(Vec::new(), ROOT, DAffine3::IDENTITY)];
        while let Some((path, def, world)) = stack.pop() {
            let Some(d) = self.def(def) else { continue };
            for (id, inst) in d.instances.iter().rev() {
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
            let def = self.copy_def(inst.def)?;
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
        }))
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
        let originals: Vec<Instance> = self
            .def(ctx)
            .map(|d| {
                instances
                    .iter()
                    .filter_map(|i| d.instances.get(i).copied())
                    .collect()
            })
            .unwrap_or_default();
        for m in steps {
            for inst in &originals {
                let Some(def) = self.copy_def(inst.def) else {
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
}
