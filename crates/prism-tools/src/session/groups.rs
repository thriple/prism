//! Groups in the session (Groups & Components part 1).
//!
//! - The open group (the root when none is open) is worked on as a model in
//!   world coordinates, with its own ids, so every tool works unchanged.
//!   Commands are put into the group's own coordinates when they run.
//! - Everything else is merged into the scene with ids of its own
//!   ([`Scene::merge`]): a group inside the open one is picked as a whole
//!   and can be selected; geometry outside the open group is drawn faded,
//!   cannot be picked for editing, and a click on it closes the group.
//! - Inference sees all of it, so drawing snaps to other groups too.

use std::collections::BTreeSet;

use prism_doc::{Change, Command, InstanceId, ROOT};
use prism_math::{DAffine3, DVec3};
use prism_ops::Selection;
use prism_topo::Model;
use prism_view::{Hit, Scene};

use super::Session;

/// Who owns something merged into the scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    /// A group inside the open one (or at the root): picked as a whole.
    Child(InstanceId),
    /// Outside the open group: drawn faded, not editable.
    Outside,
}

/// A scene seen from the open group, and what it was built from.
struct Built {
    scene: Scene,
    owners: Vec<Owner>,
    pieces: Vec<Piece>,
    work: Option<Model>,
    to_world: DAffine3,
}

/// A merged piece of the scene, kept for previews.
#[derive(Clone, Debug)]
pub(super) struct Piece {
    scene: Scene,
    owner: u32,
    faded: bool,
}

impl Session {
    /// The open group, as a path of instances from the root (empty at the root).
    pub fn open_path(&self) -> &[InstanceId] {
        &self.ctx
    }

    /// The selected groups (inside the open one).
    pub fn selected_groups(&self) -> &BTreeSet<InstanceId> {
        &self.groups
    }

    /// The model the tools work on: the open group's, in world coordinates.
    pub fn work_model(&self) -> &Model {
        self.work.as_ref().unwrap_or_else(|| self.doc.model())
    }

    /// From the open group's coordinates to the world.
    pub fn open_transform(&self) -> DAffine3 {
        self.to_world
    }

    /// Who owns what a pick hit, if it is not the open group's own.
    pub fn owner(&self, hit: Hit) -> Option<Owner> {
        self.scene
            .owner_of(hit)
            .and_then(|k| self.owners.get(k as usize).copied())
    }

    /// Rebuild the scene (and the work model) from the document.
    pub(super) fn build_scene(&mut self) {
        // An open group that no longer exists (after undo) closes.
        while self.doc.assembly().context_def(&self.ctx).is_none() {
            self.ctx.pop();
        }
        let asm = self.doc.assembly().clone();
        let b = self.built(&asm);
        let def = asm.context_def(&self.ctx).unwrap_or(ROOT);
        self.groups
            .retain(|g| asm.def(def).is_some_and(|d| d.instances.contains_key(g)));
        self.to_world = b.to_world;
        self.work = b.work;
        self.owners = b.owners;
        self.pieces = b.pieces;
        self.scene = b.scene;
    }

    /// The scene of an assembly, seen from the open group.
    fn built(&self, asm: &prism_doc::Assembly) -> Built {
        let def = asm.context_def(&self.ctx).unwrap_or(ROOT);
        let to_world = asm.world_of(&self.ctx).unwrap_or(DAffine3::IDENTITY);
        // The work model: the open group's own, placed in the world.
        let work = if self.ctx.is_empty() {
            None
        } else {
            let mut m = asm.def(def).map(|d| d.model.clone()).unwrap_or_default();
            prism_ops::transform::affine(&mut m, &to_world);
            Some(m)
        };
        let mut owners = Vec::new();
        let mut pieces = Vec::new();
        if !self.ctx.is_empty() {
            owners.push(Owner::Outside);
            pieces.push(Piece {
                scene: Scene::build_with(&asm.root().model, false),
                owner: 0,
                faded: true,
            });
        }
        // Each definition's scene is built once, in its own coordinates,
        // and placed for each instance (rebuilt only for a stretch).
        let mut local: std::collections::BTreeMap<prism_doc::DefId, Scene> = Default::default();
        for placed in asm.placed() {
            if placed.path == self.ctx {
                continue;
            }
            let inside = placed.path.len() > self.ctx.len() && placed.path.starts_with(&self.ctx);
            // Other instances of the open component show the edit live.
            let sibling = !self.ctx.is_empty() && placed.def == def;
            let owner = if inside {
                Owner::Child(placed.path[self.ctx.len()])
            } else {
                Owner::Outside
            };
            let k = owners.iter().position(|o| *o == owner).unwrap_or_else(|| {
                owners.push(owner);
                owners.len() - 1
            });
            let Some(d) = asm.def(placed.def) else {
                continue;
            };
            if d.model.face_count() == 0 && d.model.edge_count() == 0 {
                continue;
            }
            let base = local
                .entry(placed.def)
                .or_insert_with(|| Scene::build_with(&d.model, false));
            let scene = base.placed(&placed.world).unwrap_or_else(|| {
                let mut m = d.model.clone();
                prism_ops::transform::affine(&mut m, &placed.world);
                Scene::build_with(&m, false)
            });
            pieces.push(Piece {
                scene,
                owner: k as u32,
                faded: !inside && !sibling,
            });
        }
        let mut scene =
            Scene::build_with(work.as_ref().unwrap_or(&asm.root().model), self.show_guides);
        for p in &pieces {
            scene.merge(p.scene.clone(), p.owner, p.faded);
        }
        Built {
            scene,
            owners,
            pieces,
            work,
            to_world,
        }
    }

    /// The scene a command would make, groups and all (previews of moves
    /// that take groups along).
    pub(super) fn preview_of(&self, c: &Command) -> Scene {
        let mut asm = self.doc.assembly().clone();
        let def = asm.context_def(&self.ctx).unwrap_or(ROOT);
        let local = asm
            .def(def)
            .map(|d| c.to_local(&self.to_world.inverse(), &d.model))
            .unwrap_or_else(|| c.clone());
        local.apply_in(&mut asm, def);
        self.built(&asm).scene
    }

    /// The open group's scene with everything else merged in.
    pub(super) fn compose(&self, mut own: Scene) -> Scene {
        for p in &self.pieces {
            own.merge(p.scene.clone(), p.owner, p.faded);
        }
        own
    }

    /// Run a command in the open group, in its own coordinates.
    pub(super) fn run_here(&mut self, c: Command) -> bool {
        if self.ctx.is_empty() {
            return self.doc.run(c);
        }
        let asm = self.doc.assembly();
        let Some(def) = asm.context_def(&self.ctx).and_then(|d| asm.def(d)) else {
            return false;
        };
        let local = c.to_local(&self.to_world.inverse(), &def.model);
        let ctx = self.ctx.clone();
        self.doc.run_in(&ctx, local)
    }

    /// Open a group inside the open one (double-click, Enter).
    pub fn open_group(&mut self, id: InstanceId) {
        self.cancel();
        self.ctx.push(id);
        self.groups.clear();
        self.selection = Selection::default();
        self.model_changed();
        self.refresh();
    }

    /// Close the open group, one level (Esc, a click outside). Returns
    /// whether one was open.
    pub(super) fn close_group(&mut self) -> bool {
        let Some(id) = self.ctx.pop() else {
            return false;
        };
        self.cancel();
        self.selection = Selection::default();
        self.groups = [id].into();
        self.model_changed();
        self.refresh();
        true
    }

    /// Set the selected groups.
    pub(super) fn set_groups(&mut self, g: BTreeSet<InstanceId>) {
        if g != self.groups {
            self.groups = g;
            self.highlight_dirty = true;
            self.overlay_dirty = true;
            self.dirty = true;
        }
    }

    /// Edit > Make Group (G): the selection and the selected groups.
    pub(super) fn make_group(&mut self) {
        if self.selection.is_empty() && self.groups.is_empty() {
            return;
        }
        self.cancel();
        let id = self.doc.assembly().next_instance();
        let c = Command::MakeGroup {
            selection: self.selection.clone(),
            instances: self.groups.clone(),
        };
        if self.run(c) {
            self.selection = Selection::default();
            self.set_groups([id].into());
        }
    }

    /// Edit > Make Component: as Make Group, as a named component.
    pub(super) fn make_component(&mut self) {
        if self.selection.is_empty() && self.groups.is_empty() {
            return;
        }
        self.cancel();
        let id = self.doc.assembly().next_instance();
        let c = Command::MakeComponent {
            selection: self.selection.clone(),
            instances: self.groups.clone(),
            name: "Component".into(),
        };
        if self.run(c) {
            self.selection = Selection::default();
            self.set_groups([id].into());
        }
    }

    /// The components (Window > Components): id, name and how many
    /// instances place each in the whole model.
    pub fn component_rows(&self) -> Vec<(prism_doc::DefId, String, usize)> {
        let asm = self.doc.assembly();
        asm.components()
            .into_iter()
            .map(|(id, name)| (id, name.to_owned(), asm.uses(id)))
            .collect()
    }

    /// Select every instance of a component in the open group.
    pub fn select_component(&mut self, def: prism_doc::DefId) {
        let asm = self.doc.assembly();
        let here = asm
            .context_def(&self.ctx)
            .and_then(|d| asm.def(d))
            .map(|d| {
                d.instances
                    .iter()
                    .filter(|(_, i)| i.def == def)
                    .map(|(id, _)| *id)
                    .collect()
            })
            .unwrap_or_default();
        self.set_selection(Selection::default());
        self.set_groups(here);
    }

    /// Rename a component. Returns false if the name is empty or taken.
    pub fn rename_component(&mut self, def: prism_doc::DefId, name: &str) -> bool {
        self.run(Command::Rename {
            def,
            name: name.to_owned(),
        })
    }

    /// Edit > Make Unique: the selected instances get their own copies.
    pub(super) fn make_unique(&mut self) {
        if !self.groups.is_empty() {
            self.cancel();
            self.run(Command::MakeUnique {
                instances: self.groups.clone(),
            });
        }
    }

    /// Edit > Flip Along: mirror each selected group about its own centre
    /// along a world axis (0 red, 1 green, 2 blue).
    pub(super) fn flip(&mut self, axis: usize) {
        if self.groups.is_empty() {
            return;
        }
        self.cancel();
        let mut factor = DVec3::ONE;
        factor[axis] = -1.0;
        let commands = self
            .groups
            .iter()
            .filter_map(|g| {
                let c = self.group_box(*g)?;
                let centre = c.iter().copied().sum::<DVec3>() / 8.0;
                Some(Command::Place {
                    instances: [*g].into(),
                    change: Change::Scale { centre, factor },
                })
            })
            .collect();
        self.run(Command::Batch { commands });
    }

    /// Edit > Explode: the selected groups join the open one.
    pub(super) fn explode(&mut self) {
        if self.groups.is_empty() {
            return;
        }
        self.cancel();
        let commands = self
            .groups
            .iter()
            .map(|g| Command::Explode { instance: *g })
            .collect();
        self.run(Command::Batch { commands });
    }

    /// Delete: the selection and the selected groups, in one step.
    pub(super) fn delete(&mut self) {
        let mut commands = Vec::new();
        if !self.selection.is_empty() {
            commands.push(Command::Erase {
                selection: std::mem::take(&mut self.selection),
            });
        }
        if !self.groups.is_empty() {
            commands.push(Command::EraseInstances {
                instances: std::mem::take(&mut self.groups),
            });
        }
        if !commands.is_empty() {
            self.run(Command::Batch { commands });
        }
    }

    /// The groups in the open one, all of them (Select All).
    pub(super) fn all_groups(&self) -> BTreeSet<InstanceId> {
        let asm = self.doc.assembly();
        asm.context_def(&self.ctx)
            .and_then(|d| asm.def(d))
            .map(|d| d.instances.keys().copied().collect())
            .unwrap_or_default()
    }

    /// A group's box in the world: its corners, for the blue box (PLAN 1.2).
    pub fn group_box(&self, id: InstanceId) -> Option<[DVec3; 8]> {
        let mut path = self.ctx.clone();
        path.push(id);
        self.path_box(&path)
    }

    /// The open group's box in the world (the dashed box).
    pub fn open_box(&self) -> Option<[DVec3; 8]> {
        if self.ctx.is_empty() {
            return None;
        }
        self.path_box(&self.ctx)
    }

    fn path_box(&self, path: &[InstanceId]) -> Option<[DVec3; 8]> {
        let asm = self.doc.assembly();
        let def = asm.context_def(path)?;
        let world = asm.world_of(path)?;
        let (lo, hi) = def_bounds(asm, def)?;
        Some(corners(lo, hi).map(|p| world.transform_point3(p)))
    }

    /// The groups a box selects, taking their pieces out of `hits`: in a
    /// window, a group whose every edge is inside; in a crossing, any it
    /// touches. Shift toggles and Ctrl adds, as for geometry.
    pub(super) fn box_groups(
        &self,
        hits: &mut crate::select::BoxHits,
        crossing: bool,
        b: &super::BoxOp,
    ) -> BTreeSet<InstanceId> {
        use std::collections::BTreeMap;
        let mut touched: BTreeMap<u32, usize> = BTreeMap::new();
        for e in &hits.edges {
            if let Some(k) = self.scene.edge_owner.get(e) {
                *touched.entry(*k).or_default() += 1;
            }
        }
        let mut total: BTreeMap<u32, usize> = BTreeMap::new();
        for k in self.scene.edge_owner.values() {
            *total.entry(*k).or_default() += 1;
        }
        hits.edges
            .retain(|e| !self.scene.edge_owner.contains_key(e));
        hits.faces
            .retain(|f| !self.scene.face_owner.contains_key(f));
        hits.vertices
            .retain(|v| !self.scene.vertex_owner.contains_key(v));
        let mut out = if b.shift || b.ctrl {
            b.groups.clone()
        } else {
            BTreeSet::new()
        };
        for (k, n) in touched {
            let Some(Owner::Child(g)) = self.owners.get(k as usize).copied() else {
                continue;
            };
            if crossing || total.get(&k) == Some(&n) {
                if b.shift && out.contains(&g) {
                    out.remove(&g);
                } else {
                    out.insert(g);
                }
            }
        }
        out
    }

    /// The blue box of each selected group, and the dashed box of the open
    /// group (PLAN 1.2). Faces in front hide them, as they hide edges.
    pub(super) fn group_boxes(&self, o: &mut prism_view::overlay::Overlay) {
        let box_lines = |c: [DVec3; 8], color, width, dash| {
            (0..8usize).flat_map(move |i| {
                [1usize, 2, 4]
                    .into_iter()
                    .filter(move |b| i & b == 0)
                    .map(move |b| prism_view::overlay::Line {
                        a: c[i],
                        b: c[i | b],
                        color,
                        width,
                        dash,
                    })
            })
        };
        if let Some(c) = self.open_box() {
            o.tested.extend(box_lines(
                c,
                self.style.open_box,
                self.style.edge_width,
                self.style.open_box_dash,
            ));
        }
        for g in &self.groups {
            if let Some(c) = self.group_box(*g) {
                o.tested.extend(box_lines(
                    c,
                    self.style.selected,
                    self.style.highlight_width,
                    [0.0, 0.0],
                ));
            }
        }
    }

    /// With nothing selected, Move, Rotate and Scale take the group under
    /// the cursor (and select it). Returns whether there are groups to move.
    pub(super) fn groups_for_tool(&mut self, s: prism_math::DVec2) -> bool {
        if self.selection.is_empty() && self.groups.is_empty() {
            let hit = prism_view::pick::pick(&self.scene, &self.view(), s, self.settings.aperture);
            if let Some(Owner::Child(g)) = hit.and_then(|h| self.owner(h.hit)) {
                self.set_groups([g].into());
            }
        }
        !self.groups.is_empty()
    }

    /// The command for the selected groups' part of a move, rotation or
    /// scale (`copy`: copies, each group its own), joined to `geometry`.
    pub(super) fn with_groups(
        &self,
        geometry: Option<Command>,
        change: Change,
        copy: bool,
    ) -> Option<Command> {
        let groups = (!self.groups.is_empty()).then(|| {
            if copy {
                Command::CopyInstances {
                    instances: self.groups.clone(),
                    step: change,
                    count: 1,
                }
            } else {
                Command::Place {
                    instances: self.groups.clone(),
                    change,
                }
            }
        });
        match (geometry, groups) {
            (Some(g), Some(i)) => Some(Command::Batch {
                commands: vec![g, i],
            }),
            (g, i) => g.or(i),
        }
    }

    /// The selected groups' corners in the world (Scale's box).
    pub(super) fn groups_corners(&self) -> Vec<DVec3> {
        self.groups
            .iter()
            .filter_map(|g| self.group_box(*g))
            .flatten()
            .collect()
    }
}

/// The box of a definition's geometry and everything nested in it.
fn def_bounds(asm: &prism_doc::Assembly, def: prism_doc::DefId) -> Option<(DVec3, DVec3)> {
    let d = asm.def(def)?;
    let mut b = d.model.bbox();
    for inst in d.instances.values() {
        if let Some((lo, hi)) = def_bounds(asm, inst.def) {
            for p in corners(lo, hi) {
                let p = inst.transform.transform_point3(p);
                b = Some(b.map_or((p, p), |(l, h)| (l.min(p), h.max(p))));
            }
        }
    }
    b
}

/// A box's eight corners (bit 0 picks x, bit 1 y, bit 2 z).
pub(crate) fn corners(lo: DVec3, hi: DVec3) -> [DVec3; 8] {
    std::array::from_fn(|i| {
        DVec3::new(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        )
    })
}
