//! The Materials window's side of the session (Basic Materials 1c): the
//! document's materials as rows, the window's requests (pick, new,
//! duplicate, delete, purge, edit), what Entity Info says about materials,
//! and Select > All with the Same Material.

use prism_doc::material::PALETTE;
use prism_doc::{Command, Image, Material, Texture};
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
    /// Its texture, if it has one.
    pub texture: Option<TextureRow>,
}

/// A material's texture in the window.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureRow {
    /// The image's size in pixels.
    pub pixels: [u32; 2],
    /// One copy's width and height in the model, and as shown.
    pub size: [f64; 2],
    pub size_text: [String; 2],
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
    /// A texture's width (or height) as typed; `keep_shape` changes the
    /// other side with it.
    TextureSize {
        id: MaterialId,
        text: String,
        height: bool,
        keep_shape: bool,
    },
    /// The material goes back to its colour.
    RemoveTexture(MaterialId),
}

/// Said when a texture size cannot be read.
pub const NOT_A_SIZE: &str = "Type a size greater than zero, such as 1.2 or 600mm.";

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
                let image = m.texture.as_ref().and_then(|t| asm.images().get(&t.image));
                let texture = m.texture.as_ref().zip(image).map(|(t, i)| TextureRow {
                    pixels: [i.width, i.height],
                    size: t.size,
                    size_text: t.size.map(|x| self.number.show(x)),
                });
                // A textured material's swatch: its image's average colour.
                let mut srgba = m.srgba();
                if let Some(a) = image.and_then(|i| i.viewport()).map(|p| p.average) {
                    srgba = [a[0], a[1], a[2], srgba[3]];
                }
                MaterialRow {
                    id: *id,
                    name: m.name.clone(),
                    srgba,
                    texture,
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
            MaterialOp::TextureSize {
                id,
                text,
                height,
                keep_shape,
            } => {
                let mut m = self.doc.assembly().material(id).cloned()?;
                let t = m.texture.as_mut()?;
                let Some(x) = self.number.length(text.trim()).filter(|x| *x > 0.0) else {
                    self.material_refused = Some(NOT_A_SIZE);
                    return None;
                };
                let k = usize::from(height);
                let ratio = t.size[1 - k] / t.size[k];
                t.size[k] = x;
                if keep_shape && ratio.is_finite() {
                    t.size[1 - k] = x * ratio;
                }
                self.run(Command::EditMaterial { id, material: m });
            }
            MaterialOp::RemoveTexture(id) => {
                let mut m = self.doc.assembly().material(id).cloned()?;
                if m.texture.take().is_some() {
                    self.run(Command::EditMaterial { id, material: m });
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

    /// An image loaded for the Materials window (picked or dropped): onto
    /// material `target`, keeping its texture's width, or as a new
    /// material named after the file and picked for the Paint Bucket. One
    /// undo step. Returns the tool to choose, or why the image was refused.
    pub fn load_image(
        &mut self,
        target: Option<MaterialId>,
        file_name: &str,
        bytes: Vec<u8>,
    ) -> Result<Option<&'static str>, &'static str> {
        let image = Image::read(bytes)?;
        let mut texture = Texture::of(&image);
        let add = Command::AddImage {
            image: image.clone(),
        };
        let next = MaterialId(self.doc.assembly().materials().1);
        self.cancel();
        let tool = match target.and_then(|id| Some((id, self.doc.assembly().material(id)?.clone())))
        {
            Some((id, mut m)) => {
                if let Some(old) = &m.texture {
                    texture.size = [old.size[0], old.size[0] * texture.size[1]];
                }
                m.texture = Some(texture);
                self.run(Command::Batch {
                    commands: vec![add, Command::EditMaterial { id, material: m }],
                });
                None
            }
            None => {
                let stem = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
                let stem = stem.rsplit_once('.').map_or(stem, |(a, _)| a).trim();
                let a = image.average().unwrap_or([255; 4]);
                let mut m = Material::colour(
                    if stem.is_empty() { "Texture" } else { stem },
                    [a[0], a[1], a[2]],
                );
                m.texture = Some(texture);
                self.run(Command::Batch {
                    commands: vec![add, Command::AddMaterial { material: m }],
                });
                self.set_brush(Brush::Material(next));
                Some(PAINT)
            }
        };
        self.mark_all_dirty();
        Ok(tool)
    }

    /// Why the Materials window's last request was refused (taken once,
    /// for the status bar).
    pub fn take_material_refusal(&mut self) -> Option<&'static str> {
        self.material_refused.take()
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
