//! GLB that keeps the document's structure (owner request 2026-11-15,
//! brief 35: export for Beverly and other game engines): one named node per
//! group and component instance, nested as in Prism, each with its own
//! axes as its transform and its mesh in those local coordinates; a
//! component's mesh is stored once and shared by its instances.

use std::collections::BTreeMap;

use prism_doc::{Assembly, DefId, Instance, InstanceId, ROOT};
use prism_math::{DAffine3, DMat3, DVec3};
use prism_topo::{MaterialId, Model};
use serde_json::json;

use super::gltf::{Node, Scene};
use super::{ExportOptions, Mesh, MeshMaterial, to_painted_mesh};

/// The document as a GLB with its groups and components kept. `name`
/// names the node holding the loose geometry at the top (the document's
/// name); `visible_only` leaves hidden groups out.
pub fn export_structured(
    asm: &Assembly,
    o: &ExportOptions,
    name: &str,
    visible_only: bool,
) -> Vec<u8> {
    super::gltf::write_scene(&structured(asm, o, name, visible_only))
}

/// The scene [`export_structured`] writes.
pub fn structured(asm: &Assembly, o: &ExportOptions, name: &str, visible_only: bool) -> Scene {
    let mut b = Builder {
        asm,
        o,
        visible_only,
        scene: Scene::default(),
        meshes: BTreeMap::new(),
    };
    let mut roots = Vec::new();
    let mut names = Names::default();
    if let Some(k) = b.mesh(ROOT, None) {
        roots.push(b.push(Node {
            name: names.unique(name),
            mesh: Some(k),
            ..Node::default()
        }));
    }
    roots.extend(b.children(ROOT, None, &mut names));
    b.scene.roots = roots;
    b.scene
}

/// Sibling names made unique: " 2", " 3", ... after the first.
#[derive(Default)]
struct Names {
    used: std::collections::BTreeSet<String>,
    next: BTreeMap<String, usize>,
}

impl Names {
    fn unique(&mut self, base: &str) -> String {
        if self.used.insert(base.to_owned()) {
            return base.to_owned();
        }
        let mut n = self.next.get(base).copied().unwrap_or(1);
        loop {
            n += 1;
            let name = format!("{base} {n}");
            if self.used.insert(name.clone()) {
                self.next.insert(base.to_owned(), n);
                return name;
            }
        }
    }
}

struct Builder<'a> {
    asm: &'a Assembly,
    o: &'a ExportOptions,
    visible_only: bool,
    scene: Scene,
    /// A definition's mesh, per material shown on its Default faces.
    meshes: BTreeMap<(DefId, Option<MaterialId>), Option<usize>>,
}

impl Builder<'_> {
    fn push(&mut self, n: Node) -> usize {
        self.scene.nodes.push(n);
        self.scene.nodes.len() - 1
    }

    /// The nodes of `def`'s instances, in id order (stable names).
    fn children(&mut self, def: DefId, paint: Option<MaterialId>, names: &mut Names) -> Vec<usize> {
        let Some(d) = self.asm.def(def) else {
            return Vec::new();
        };
        let instances: Vec<(InstanceId, Instance)> =
            d.instances.iter().map(|(k, v)| (*k, *v)).collect();
        let mut out = Vec::new();
        for (id, inst) in instances {
            if self.visible_only && !self.asm.is_shown(id) {
                continue;
            }
            out.push(self.instance(id, inst, paint, names));
        }
        out
    }

    fn instance(
        &mut self,
        id: InstanceId,
        inst: Instance,
        paint: Option<MaterialId>,
        names: &mut Names,
    ) -> usize {
        let d = self.asm.def(inst.def);
        let component = d.and_then(|d| d.name.clone());
        let base = match &component {
            Some(n) => n.clone(),
            None => self.asm.name_of(id).unwrap_or("Group").to_owned(),
        };
        // A group's material shows on Default faces inside it.
        let paint = self.asm.painted_with(id).or(paint);
        let mesh = self.mesh(inst.def, paint);
        let extras = match &component {
            Some(n) => json!({"prism": {"kind": "component", "definition": n}}),
            None => json!({"prism": {"kind": "group"}}),
        };
        let at = self.push(Node {
            name: names.unique(&base),
            matrix: Some(matrix(&inst.transform, self.o)),
            mesh,
            children: Vec::new(),
            extras: Some(extras),
        });
        let mut inner = Names::default();
        let children = self.children(inst.def, paint, &mut inner);
        self.scene.nodes[at].children = children;
        at
    }

    /// `def`'s own geometry as a mesh in its own coordinates (None when it
    /// has no faces), made once per material shown on its Default faces.
    fn mesh(&mut self, def: DefId, paint: Option<MaterialId>) -> Option<usize> {
        if let Some(k) = self.meshes.get(&(def, paint)) {
            return *k;
        }
        let k = self.asm.def(def).and_then(|d| {
            let m = painted(&d.model, paint);
            let mesh = to_painted_mesh(&m, None, self.o, self.asm);
            (!mesh.polygons.is_empty()).then(|| {
                let mesh = self.shared(mesh);
                self.scene.meshes.push(mesh);
                self.scene.meshes.len() - 1
            })
        });
        self.meshes.insert((def, paint), k);
        k
    }

    /// A mesh's materials moved into the scene's one table.
    fn shared(&mut self, mut mesh: Mesh) -> Mesh {
        let own = std::mem::take(&mut mesh.materials);
        let map: Vec<u32> = own.into_iter().map(|m| self.material(m)).collect();
        for k in mesh.polygon_materials.iter_mut().flatten() {
            *k = map[*k as usize];
        }
        mesh
    }

    fn material(&mut self, m: MeshMaterial) -> u32 {
        let all = &mut self.scene.materials;
        let k = all.iter().position(|x| *x == m).unwrap_or_else(|| {
            all.push(m);
            all.len() - 1
        });
        k as u32
    }
}

/// `model` with `paint` on its Default faces, textures placed in its own
/// coordinates (as `Assembly::flattened_visible` shows a group).
fn painted(model: &Model, paint: Option<MaterialId>) -> Model {
    let mut m = model.clone();
    let ids: Vec<prism_topo::FaceId> = m.faces().map(|(id, _)| id).collect();
    for f in ids {
        m.edit_face(f, |x| {
            x.paint.front = x.paint.front.or(paint);
            x.paint.back = x.paint.back.or(paint);
            if x.paint.front.is_some() || x.paint.back.is_some() {
                x.paint.uv = x.texture_map();
            }
        });
    }
    m
}

/// The file's frame: Prism's Z up turned to Y up, and the unit.
fn frame(o: &ExportOptions) -> DMat3 {
    if o.y_up {
        // (x, y, z) -> (x, z, -y), as columns.
        DMat3::from_cols(DVec3::X, -DVec3::Z, DVec3::Y)
    } else {
        DMat3::IDENTITY
    }
}

/// An instance's transform in the file's frame, column-major: the same
/// turn seen in the file's axes, the move in its unit.
pub(super) fn matrix(t: &DAffine3, o: &ExportOptions) -> [f64; 16] {
    let r = frame(o);
    let l = r * t.matrix3 * r.transpose();
    let p = r * t.translation / o.unit.metres();
    [
        l.x_axis.x, l.x_axis.y, l.x_axis.z, 0.0, l.y_axis.x, l.y_axis.y, l.y_axis.z, 0.0,
        l.z_axis.x, l.z_axis.y, l.z_axis.z, 0.0, p.x, p.y, p.z, 1.0,
    ]
}
