//! The Materials window's side of the session (Basic Materials 1c): the
//! document's materials as rows, the window's requests (pick, new,
//! duplicate, delete, purge, edit), what Entity Info says about materials,
//! and Select > All with the Same Material.

use prism_doc::material::PALETTE;
use prism_doc::{Command, Material};
use prism_ops::Selection;
use prism_topo::MaterialId;

use super::Session;
use super::paint::{Brush, PAINT};

/// One material in the window.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialRow {
    pub id: MaterialId,
    pub name: String,
    /// Its colour as the screen shows it (sRGB, alpha is opacity).
    pub srgba: [u8; 4],
    pub roughness: f32,
    pub metalness: f32,
    /// Painted on a face or a group somewhere.
    pub used: bool,
    /// The material itself (to edit a copy).
    pub material: Material,
}

/// What the Materials window asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum MaterialOp {
    /// Paint with it: the Paint Bucket is chosen.
    Pick(Brush),
    /// A new plain material (a colour from the palette), picked.
    New,
    Duplicate(MaterialId),
    /// Faces using it go back to Default.
    Delete(MaterialId),
    Purge,
    Edit(MaterialId, Material),
}

/// What to paint with from outside the window (MCP's `paint`).
#[derive(Clone, Debug, PartialEq)]
pub enum PaintWith {
    Default,
    Existing(MaterialId),
    /// A new material, added to the document in the same step.
    New(Material),
}

impl Session {
    /// Paint face sides of the open group and groups in it, in one undo
    /// step (MCP's `paint`). Returns the material painted with, or why not.
    pub fn paint_with(
        &mut self,
        sides: std::collections::BTreeSet<(prism_topo::FaceId, prism_doc::FaceSide)>,
        groups: std::collections::BTreeSet<prism_doc::InstanceId>,
        with: PaintWith,
    ) -> Result<Option<MaterialId>, &'static str> {
        let m = self.work_model();
        if sides.iter().any(|(f, _)| m.face(*f).is_none()) {
            return Err("A face id is not in the open group (get_geometry lists them).");
        }
        if sides.is_empty() && groups.is_empty() {
            return Err("Nothing to paint: give faces, or select some.");
        }
        let next = MaterialId(self.doc.assembly().materials().1);
        let (add, material) = match with {
            PaintWith::Default => (None, None),
            PaintWith::Existing(id) => {
                if self.doc.assembly().material(id).is_none() {
                    return Err("No material with that id or name (get_model lists them).");
                }
                (None, Some(id))
            }
            PaintWith::New(mat) => (Some(Command::AddMaterial { material: mat }), Some(next)),
        };
        let mut commands: Vec<Command> = add.into_iter().collect();
        if !sides.is_empty() {
            commands.push(Command::Paint { sides, material });
        }
        if !groups.is_empty() {
            commands.push(Command::PaintInstances {
                instances: groups,
                material,
            });
        }
        self.cancel();
        self.run(Command::Batch { commands });
        self.mark_all_dirty();
        Ok(material)
    }

    /// A material by name (the first with it).
    pub fn material_named(&self, name: &str) -> Option<MaterialId> {
        self.doc
            .assembly()
            .materials()
            .0
            .iter()
            .find(|(_, m)| m.name.eq_ignore_ascii_case(name))
            .map(|(id, _)| *id)
    }

    /// The document's materials, used ones first, then by name.
    pub fn material_rows(&self) -> Vec<MaterialRow> {
        let asm = self.doc.assembly();
        let unused = asm.unused_materials();
        let mut rows: Vec<MaterialRow> = asm
            .materials()
            .0
            .iter()
            .map(|(id, m)| {
                let s = m.surface();
                MaterialRow {
                    id: *id,
                    name: m.name.clone(),
                    srgba: m.srgba(),
                    roughness: s.roughness,
                    metalness: s.metalness,
                    used: !unused.contains(id),
                    material: m.clone(),
                }
            })
            .collect();
        rows.sort_by(|a, b| b.used.cmp(&a.used).then(a.name.cmp(&b.name)));
        rows
    }

    /// Run a request from the Materials window. Returns the tool to choose,
    /// if it picks the Paint Bucket.
    pub fn material_op(&mut self, op: MaterialOp) -> Option<&'static str> {
        let next = MaterialId(self.doc.assembly().materials().1);
        match op {
            MaterialOp::Pick(b) => {
                self.set_brush(b);
                return Some(PAINT);
            }
            MaterialOp::New => {
                // The palette colour after the ones already used.
                let n = self.doc.assembly().materials().0.len();
                let (name, c) = PALETTE[n % PALETTE.len()];
                if self.run(Command::AddMaterial {
                    material: Material::colour(name, c),
                }) {
                    self.set_brush(Brush::Material(next));
                    return Some(PAINT);
                }
            }
            MaterialOp::Duplicate(id) => {
                let m = self.doc.assembly().material(id).cloned()?;
                if self.run(Command::AddMaterial { material: m }) {
                    self.set_brush(Brush::Material(next));
                    return Some(PAINT);
                }
            }
            MaterialOp::Delete(id) => {
                self.run(Command::RemoveMaterial { id });
                if self.brush() == Brush::Material(id) {
                    self.set_brush(Brush::default());
                }
            }
            MaterialOp::Purge => {
                self.run(Command::PurgeMaterials);
            }
            MaterialOp::Edit(id, material) => {
                // A colour dragged in the picker changes every frame: an edit
                // right after an edit of the same material replaces it, so
                // the drag is one undo step.
                let last = self.doc.log().last().map(|c| match c {
                    Command::In { command, .. } => command.as_ref().clone(),
                    c => c.clone(),
                });
                if matches!(last, Some(Command::EditMaterial { id: was, .. }) if was == id)
                    && self.doc.undo()
                {
                    self.model_changed();
                }
                self.run(Command::EditMaterial { id, material });
            }
        }
        self.mark_all_dirty();
        None
    }

    /// The material named by a face side, for Entity Info and MCP.
    pub fn material_name(&self, m: Option<MaterialId>) -> String {
        m.and_then(|m| self.doc.assembly().material(m))
            .map_or("Default".to_owned(), |x| x.name.clone())
    }

    /// The selected faces' materials, front and back: one name, "Several",
    /// or None with no face selected.
    pub fn selection_materials(&self) -> Option<(String, String)> {
        let m = self.work_model();
        let faces: Vec<_> = self
            .selection
            .faces
            .iter()
            .filter_map(|f| m.face(*f))
            .collect();
        if faces.is_empty() {
            return None;
        }
        let one = |side: fn(&prism_topo::Face) -> Option<MaterialId>| {
            let first = side(faces[0]);
            if faces.iter().all(|f| side(f) == first) {
                self.material_name(first)
            } else {
                "Several".to_owned()
            }
        };
        Some((one(|f| f.paint.front), one(|f| f.paint.back)))
    }

    /// Select > All with the Same Material: every face in the open group
    /// painted, on either side, with the material on the front of the
    /// first selected face (or Default faces, front side).
    pub(super) fn select_same_material(&mut self) {
        let m = self.work_model();
        let Some(first) = self.selection.faces.iter().next().and_then(|f| m.face(*f)) else {
            return;
        };
        let want = first.paint.front;
        let faces = m
            .faces()
            .filter(|(_, f)| match want {
                Some(_) => f.paint.front == want || f.paint.back == want,
                None => f.paint.front.is_none(),
            })
            .map(|(id, _)| id)
            .collect();
        self.set_groups(Default::default());
        self.set_selection(Selection {
            faces,
            ..Default::default()
        });
    }
}
