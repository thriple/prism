//! GLB that keeps the document's structure (owner request 2026-11-15,
//! brief 35: export for Beverly and other game engines): one named node per
//! group and component instance, nested as in Prism, each with its own
//! axes as its transform and its mesh in those local coordinates; a
//! component's mesh is stored once and shared by its instances. Part 2
//! reads that structure back ([`assembly_of`]).

use std::collections::BTreeMap;

use prism_doc::{Assembly, DefId, Instance, InstanceId, ROOT};
use prism_math::{DAffine3, DMat3, DVec3};
use prism_topo::{MaterialId, Model};
use serde_json::json;

use super::gltf::{Node, Scene};
use super::{ExportOptions, Mesh, MeshMaterial, build_painted, to_painted_mesh};
use prism_doc::Definition;

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
            // Loose geometry, not a group: it comes back loose.
            extras: Some(json!({"prism": {"kind": "loose"}})),
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
        let mut extras = match &component {
            Some(n) => json!({"prism": {"kind": "component", "definition": n}}),
            None => json!({"prism": {"kind": "group"}}),
        };
        // The instance's tags, by name: Beverly reads the words and decides
        // what each means. Left out when there are none.
        let tags: Vec<String> = self
            .asm
            .tags_of(id)
            .into_iter()
            .filter_map(|t| self.asm.tag(t).map(|t| t.name.clone()))
            .collect();
        if !tags.is_empty() {
            extras["prism"]["tags"] = json!(tags);
        }
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

/// The file's structure, read (brief 35, part 2).
pub fn read_structured(bytes: &[u8]) -> Result<Scene, crate::FileError> {
    super::gltf::read_scene(bytes)
}

/// Whether a file's scene is worth keeping as groups: more than one node,
/// or nesting. A single object imports as plain geometry, as before.
pub fn has_structure(sc: &Scene) -> bool {
    let objects = sc
        .nodes
        .iter()
        .filter(|n| n.mesh.is_some() || !n.children.is_empty())
        .count();
    objects > 1 || sc.roots.iter().any(|r| !sc.nodes[*r].children.is_empty())
}

fn prism_kind(n: &Node) -> Option<&str> {
    n.extras.as_ref()?.get("prism")?.get("kind")?.as_str()
}

/// A file matrix (column-major, the file's frame and unit) as a placement
/// in Prism's.
fn placement(m: &[f64; 16], metres_per_unit: f64, y_up: bool) -> DAffine3 {
    let f = DAffine3::from_mat4(prism_math::DMat4::from_cols_array(m));
    let r = if y_up {
        DMat3::from_cols(DVec3::X, -DVec3::Z, DVec3::Y)
    } else {
        DMat3::IDENTITY
    };
    DAffine3 {
        matrix3: r.transpose() * f.matrix3 * r,
        translation: r.transpose() * f.translation * metres_per_unit,
    }
}

/// A file's scene as an assembly to paste: each node a group (named after
/// it), nested, its matrix its axes; nodes alike that share a mesh (or
/// that Prism marked as components) become instances of one component;
/// Prism's own loose geometry comes back loose. Materials come in as the
/// file has them.
pub fn assembly_of(sc: &Scene, metres_per_unit: f64, y_up: bool, merge: bool) -> Assembly {
    // Each mesh as faces in its own coordinates, and each material's
    // texture size from the first mesh that uses it.
    let mut models = Vec::with_capacity(sc.meshes.len());
    let mut sizes: Vec<Option<[f64; 2]>> = vec![None; sc.materials.len()];
    for m in &sc.meshes {
        let mesh = Mesh {
            materials: sc.materials.clone(),
            ..m.clone()
        };
        let (model, s) = build_painted(&mesh, metres_per_unit, y_up, merge);
        for (k, size) in s.into_iter().enumerate() {
            if sizes[k].is_none() && m.polygon_materials.contains(&Some(k as u32)) {
                sizes[k] = Some(size);
            }
        }
        models.push(model);
    }
    let mut b = Reader {
        sc,
        models,
        metres_per_unit,
        y_up,
        defs: BTreeMap::from([(ROOT, Definition::default())]),
        names: BTreeMap::new(),
        tags: BTreeMap::new(),
        keys: BTreeMap::new(),
        shared: BTreeMap::new(),
        next_instance: 0,
    };
    // How many nodes look alike (same mesh, same children placed alike).
    let mut alike: BTreeMap<String, usize> = BTreeMap::new();
    for (k, n) in sc.nodes.iter().enumerate() {
        let key = b.key(k, 0);
        if n.mesh.is_some() {
            *alike.entry(key).or_default() += 1;
        }
    }
    for r in sc.roots.clone() {
        let n = &sc.nodes[r];
        let unplaced = n
            .matrix
            .is_none_or(|m| m == prism_math::DMat4::IDENTITY.to_cols_array());
        if prism_kind(n) == Some("loose") && unplaced && n.children.is_empty() {
            if let Some(m) = n.mesh {
                let geo = b.models[m].clone();
                b.defs.get_mut(&ROOT).expect("a root").model.append(&geo);
            }
            continue;
        }
        let (id, inst) = b.instance(r, &alike, 0);
        b.defs
            .get_mut(&ROOT)
            .expect("a root")
            .instances
            .insert(id, inst);
    }
    let next_def = b.defs.keys().map(|k| k.0 + 1).max().unwrap_or(1);
    let mut asm = Assembly::from_parts(b.defs, next_def, b.next_instance)
        .unwrap_or_else(|_| Assembly::with_model(Model::new()));
    for (id, name) in b.names {
        asm.set_name(id, &name);
    }
    // Tags are found by name, or made.
    for (id, words) in b.tags {
        for name in words {
            let found = asm
                .tags()
                .0
                .iter()
                .find(|(_, t)| t.name == name)
                .map(|(k, _)| *k);
            let tag = found.unwrap_or_else(|| asm.add_tag(&name));
            asm.tag_on(&[id].into(), tag, true);
        }
    }
    // Materials in the file's order, so face k's material is the k-th.
    let mut map = BTreeMap::new();
    for (k, m) in sc.materials.iter().enumerate() {
        let (mat, image) = m.to_doc(sizes[k].unwrap_or([1.0, 1.0]));
        if let Some(i) = image {
            asm.add_image(i);
        }
        map.insert(MaterialId(k as u32), asm.add_material(mat));
    }
    let ids: Vec<DefId> = asm.defs().map(|(k, _)| k).collect();
    for k in ids {
        if let Some(d) = asm.def_mut(k) {
            prism_doc::material::remap(&mut d.model, &map);
        }
    }
    asm
}

struct Reader<'a> {
    sc: &'a Scene,
    models: Vec<Model>,
    metres_per_unit: f64,
    y_up: bool,
    defs: BTreeMap<DefId, Definition>,
    names: BTreeMap<InstanceId, String>,
    /// Tag names read from the nodes.
    tags: BTreeMap<InstanceId, Vec<String>>,
    keys: BTreeMap<usize, String>,
    /// A component's definition, by its nodes' likeness.
    shared: BTreeMap<String, DefId>,
    next_instance: u32,
}

impl Reader<'_> {
    /// What makes nodes alike: their mesh, and their children's likeness
    /// and placement (not names).
    fn key(&mut self, k: usize, depth: usize) -> String {
        if let Some(x) = self.keys.get(&k) {
            return x.clone();
        }
        let n = &self.sc.nodes[k];
        let mut s = format!("m{:?}[", n.mesh);
        if depth < 64 {
            for c in n.children.clone() {
                let ck = self.key(c, depth + 1);
                let m = self.sc.nodes[c].matrix.unwrap_or([0.0; 16]);
                s.push_str(&format!("{ck}@{m:?};"));
            }
        }
        s.push(']');
        self.keys.insert(k, s.clone());
        s
    }

    fn instance(
        &mut self,
        k: usize,
        alike: &BTreeMap<String, usize>,
        depth: usize,
    ) -> (InstanceId, Instance) {
        let n = &self.sc.nodes[k];
        let key = self.key(k, 0);
        let component = prism_kind(n) == Some("component")
            || (prism_kind(n).is_none() && alike.get(&key).copied().unwrap_or(0) > 1);
        let def = match self.shared.get(&key).filter(|_| component) {
            Some(d) => *d,
            None => {
                let name = component.then(|| {
                    n.extras
                        .as_ref()
                        .and_then(|e| e.get("prism")?.get("definition")?.as_str())
                        .map_or_else(|| n.name.clone(), str::to_owned)
                });
                let model = n.mesh.map(|m| self.models[m].clone()).unwrap_or_default();
                let mut instances = BTreeMap::new();
                if depth < 64 {
                    for c in n.children.clone() {
                        let (id, inst) = self.instance(c, alike, depth + 1);
                        instances.insert(id, inst);
                    }
                }
                let id = DefId(self.defs.len() as u32);
                self.defs.insert(
                    id,
                    Definition {
                        model,
                        instances,
                        name,
                        stack: None,
                    },
                );
                if component {
                    self.shared.insert(key, id);
                }
                id
            }
        };
        let id = InstanceId(self.next_instance);
        self.next_instance += 1;
        // A group keeps its name (an unnamed one was written "Group"); a
        // component instance is named by its definition.
        if !component && n.name != "Group" {
            self.names.insert(id, n.name.clone());
        }
        // `tags` (a list) and, from older files, `tag`.
        let prism = n.extras.as_ref().and_then(|e| e.get("prism"));
        let words: Vec<String> = prism
            .and_then(|p| p.get("tags")?.as_array())
            .into_iter()
            .flatten()
            .chain(prism.and_then(|p| p.get("tag")))
            .filter_map(|t| t.as_str())
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect();
        if !words.is_empty() {
            self.tags.insert(id, words);
        }
        let transform = n.matrix.map_or(DAffine3::IDENTITY, |m| {
            placement(&m, self.metres_per_unit, self.y_up)
        });
        (id, Instance { def, transform })
    }
}
