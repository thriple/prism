//! STEP export and import (Solids & CAD parts 5b and 5c, brief 44): the
//! document's solids and sheets as one STEP AP214 file, groups and
//! components kept as products, through the kernel's writer and reader
//! (`prism_kernel::step`); and a STEP file's products back as groups and
//! components.
//!
//! What is shown is exported (hidden groups are left out, PLAN 1.14).
//! Each closed body is a solid and each open one a sheet; a component is
//! one product placed at each of its instances. An instance that is not
//! a rigid placement (scaled or mirrored) has its geometry written where
//! it stands instead.

use std::collections::{BTreeMap, BTreeSet};

use prism_doc::{Assembly, DefId, Definition, Instance, InstanceId, ROOT};
use prism_kernel::step::{self, Node, Shape};
use prism_math::{DAffine3, LengthUnit};
use prism_ops::bridge::{put_back, region};
use prism_ops::validate::validate;
use prism_topo::{FaceId, MaterialId, Model};

/// Said for a closed body that is not a valid solid.
pub const INVALID: &str = "not a valid solid (Solid > Validate says why)";

/// A STEP file made from a document, and what it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Exported {
    pub text: String,
    pub solids: usize,
    pub sheets: usize,
    /// Why each body left out was left out.
    pub left_out: Vec<&'static str>,
}

impl Exported {
    /// What was exported and what was left out, in a sentence or two.
    pub fn summary(&self) -> String {
        let count = |n: usize, one: &str, many: &str| match n {
            1 => format!("1 {one}"),
            n => format!("{n} {many}"),
        };
        let mut s = match (self.solids, self.sheets) {
            (n, 0) => count(n, "solid", "solids"),
            (0, k) => count(k, "sheet", "sheets"),
            (n, k) => format!(
                "{} and {}",
                count(n, "solid", "solids"),
                count(k, "sheet", "sheets")
            ),
        };
        if !self.left_out.is_empty() {
            let mut why: BTreeMap<&str, usize> = BTreeMap::new();
            for w in &self.left_out {
                *why.entry(plain(w)).or_default() += 1;
            }
            let reasons: Vec<String> = why
                .into_iter()
                .map(|(w, n)| {
                    if n == 1 {
                        w.to_owned()
                    } else {
                        format!("{w} ({n})")
                    }
                })
                .collect();
            s.push_str(&format!(
                ". Left out {}: {}",
                self.left_out.len(),
                reasons.join("; ")
            ));
        }
        s
    }
}

/// A refusal's words without "Refused: " and its full stop, to follow
/// "Left out 1: ".
fn plain(why: &str) -> &str {
    let w = why.strip_prefix("Refused: ").unwrap_or(why);
    w.strip_suffix('.').unwrap_or(w)
}

/// The document `asm` as STEP in `unit`. `name` names the top product
/// (the document's name). With `faces`, only those loose faces go out.
pub fn export(
    asm: &Assembly,
    name: &str,
    unit: LengthUnit,
    faces: Option<&BTreeSet<FaceId>>,
) -> Result<Exported, &'static str> {
    let mut w = Walk {
        asm,
        shapes: Vec::new(),
        open: Vec::new(),
        left_out: Vec::new(),
        made: BTreeMap::new(),
    };
    let root_model = &asm.def(ROOT).ok_or(step::NOTHING)?.model;
    let mut root = Node {
        name: name.to_owned(),
        product: None,
        place: DAffine3::IDENTITY,
        colour: None,
        shapes: w.bodies(root_model, faces, &DAffine3::IDENTITY),
        children: Vec::new(),
    };
    if faces.is_none() {
        root.children = w.children(ROOT, None);
    }
    let tol = root_model.tol().dist;
    let written = step::write(&w.shapes, &[root], unit, tol)?;
    let mut left_out = w.left_out;
    left_out.extend(written.left_out.iter().map(|(_, why)| *why));
    let gone: BTreeSet<usize> = written.left_out.iter().map(|(i, _)| *i).collect();
    let kept = |open: bool| {
        (0..w.shapes.len())
            .filter(|i| w.open[*i] == open && !gone.contains(i))
            .count()
    };
    Ok(Exported {
        text: written.text,
        solids: kept(false),
        sheets: kept(true),
        left_out,
    })
}

struct Walk<'a> {
    asm: &'a Assembly,
    shapes: Vec<Shape>,
    /// Whether each shape is a sheet.
    open: Vec<bool>,
    left_out: Vec<&'static str>,
    /// A component's shapes, made once.
    made: BTreeMap<DefId, Vec<usize>>,
}

/// Whether `m` turns and moves only (no scale, no mirror).
fn rigid(m: &DAffine3) -> bool {
    let a = m.matrix3;
    let near = |x: f64, y: f64| (x - y).abs() < 1e-9;
    near(a.determinant(), 1.0)
        && (a.transpose() * a)
            .to_cols_array()
            .iter()
            .zip(prism_math::DMat3::IDENTITY.to_cols_array())
            .all(|(x, y)| near(*x, y))
}

impl Walk<'_> {
    /// The shapes of `model`'s bodies (only `only`'s faces, when given),
    /// moved by `at`: their indices.
    fn bodies(
        &mut self,
        model: &Model,
        only: Option<&BTreeSet<FaceId>>,
        at: &DAffine3,
    ) -> Vec<usize> {
        let mut out = Vec::new();
        for c in validate(model) {
            if only.is_some_and(|o| !c.faces.is_subset(o)) {
                continue;
            }
            let closed = c.volume.is_some();
            if closed && !c.valid {
                self.left_out.push(INVALID);
                continue;
            }
            let r = region(model, &c.faces).and_then(|r| {
                if *at == DAffine3::IDENTITY {
                    Ok(r)
                } else {
                    r.transformed(at).ok_or(prism_kernel::solid::CURVE_NOT_YET)
                }
            });
            match r {
                Ok(region) => {
                    let colours = c
                        .faces
                        .iter()
                        .enumerate()
                        .filter_map(|(i, f)| {
                            let paint = model.face(*f)?.paint.front?;
                            Some((i, self.colour(paint)?))
                        })
                        .collect();
                    self.shapes.push(Shape {
                        region,
                        open: !closed,
                        colours,
                    });
                    self.open.push(!closed);
                    out.push(self.shapes.len() - 1);
                }
                Err(why) => self.left_out.push(why),
            }
        }
        out
    }

    /// A material's colour, red, green and blue from 0 to 1.
    fn colour(&self, m: MaterialId) -> Option<[f64; 3]> {
        let c = self.asm.material(m)?.srgba();
        Some([c[0], c[1], c[2]].map(|x| f64::from(x) / 255.0))
    }

    /// The shown instances placed in `def`, as nodes.
    fn children(&mut self, def: DefId, paint: Option<MaterialId>) -> Vec<Node> {
        let Some(d) = self.asm.def(def) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (id, inst) in &d.instances {
            if self.asm.is_shown(*id) {
                out.push(self.instance(*id, inst.def, inst.transform, paint));
            }
        }
        out
    }

    fn instance(
        &mut self,
        id: InstanceId,
        def: DefId,
        place: DAffine3,
        paint: Option<MaterialId>,
    ) -> Node {
        let d = self.asm.def(def);
        let component = d.and_then(|d| d.name.clone());
        let name = component
            .clone()
            .unwrap_or_else(|| self.asm.name_of(id).unwrap_or("Group").to_owned());
        // A component is one product, the same at every instance, so its
        // instances' paint is not carried (its faces' own paint is).
        let paint = if component.is_some() {
            None
        } else {
            self.asm.painted_with(id).or(paint)
        };
        let colour = paint.and_then(|m| self.colour(m));
        if !rigid(&place) {
            // Written where it stands, all its insides with it.
            let shapes = self.flat(def, &place);
            return Node {
                name,
                product: None,
                place: DAffine3::IDENTITY,
                colour,
                shapes,
                children: Vec::new(),
            };
        }
        let shapes = match (component.is_some(), self.made.get(&def)) {
            (true, Some(s)) => s.clone(),
            _ => {
                let s = match d {
                    Some(d) => self.bodies(&d.model, None, &DAffine3::IDENTITY),
                    None => Vec::new(),
                };
                if component.is_some() {
                    self.made.insert(def, s.clone());
                }
                s
            }
        };
        Node {
            name,
            // A component is one product; a group is its own.
            product: component.map(|_| u64::from(def.0)),
            place,
            colour,
            shapes,
            children: self.children(def, paint),
        }
    }

    /// Every shown body in `def` and the instances inside it, moved by
    /// `at`, as shapes of one node.
    fn flat(&mut self, def: DefId, at: &DAffine3) -> Vec<usize> {
        let Some(d) = self.asm.def(def) else {
            return Vec::new();
        };
        let mut out = self.bodies(&d.model, None, at);
        for (id, inst) in &d.instances {
            if self.asm.is_shown(*id) {
                out.extend(self.flat(inst.def, &(*at * inst.transform)));
            }
        }
        out
    }
}

/// A STEP file read as an assembly to bring in, and what it held.
#[derive(Clone, Debug)]
pub struct Imported {
    pub assembly: Assembly,
    pub solids: usize,
    pub sheets: usize,
    /// Why each solid left out was refused (the kernel's words, or
    /// Prism's).
    pub refused: Vec<String>,
}

impl Imported {
    /// What came in and what did not, in a sentence or two.
    pub fn summary(&self) -> String {
        let count = |n: usize, one: &str, many: &str| match n {
            1 => format!("1 {one}"),
            n => format!("{n} {many}"),
        };
        let mut s = match (self.solids, self.sheets) {
            (n, 0) => count(n, "solid", "solids"),
            (0, k) => count(k, "sheet", "sheets"),
            (n, k) => format!(
                "{} and {}",
                count(n, "solid", "solids"),
                count(k, "sheet", "sheets")
            ),
        };
        if !self.refused.is_empty() {
            let mut why: BTreeMap<&str, usize> = BTreeMap::new();
            for w in &self.refused {
                *why.entry(plain(w)).or_default() += 1;
            }
            let reasons: Vec<String> = why
                .into_iter()
                .map(|(w, n)| {
                    if n == 1 {
                        w.to_owned()
                    } else {
                        format!("{w} ({n})")
                    }
                })
                .collect();
            s.push_str(&format!(
                ". Left out {}: {}",
                self.refused.len(),
                reasons.join("; ")
            ));
        }
        s
    }
}

/// The STEP text `text` as an assembly: each product placed in the file a
/// group named after it, nested as in the file; a product placed more
/// than once one component, placed at each; colours as materials. `tol`
/// is the tolerance (in metres) of the model it comes into. Errs when the
/// text is not STEP, or holds nothing Prism can take.
pub fn import(text: &str, tol: f64) -> Result<Imported, String> {
    let read = step::read(text, tol)?;
    let mut b = Builder {
        read: &read,
        defs: BTreeMap::from([(ROOT, Definition::default())]),
        next_def: 1,
        next_instance: 0,
        names: Vec::new(),
        shared: BTreeMap::new(),
        colours: Vec::new(),
        solids: 0,
        sheets: 0,
    };
    let mut refused: Vec<String> = read
        .shapes
        .iter()
        .filter_map(|s| s.as_ref().err().cloned())
        .collect();
    for r in &read.roots {
        if let Some((id, inst)) = b.node(r, &DAffine3::IDENTITY, &mut refused) {
            b.defs
                .get_mut(&ROOT)
                .expect("a root")
                .instances
                .insert(id, inst);
        }
    }
    if b.solids + b.sheets == 0 {
        return Err(match refused.first() {
            Some(why) => format!("Nothing Prism can take: {why}"),
            None => "Nothing to import: the file holds no solids or surfaces.".to_owned(),
        });
    }
    let mut asm = Assembly::from_parts(b.defs, b.next_def, b.next_instance)?;
    for (id, name) in &b.names {
        asm.set_name(*id, name);
    }
    // Each colour once, as a material, on the faces it colours.
    let mut made: BTreeMap<[u8; 3], MaterialId> = BTreeMap::new();
    for (def, face, c) in b.colours {
        let srgb = c.map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u8);
        let m = *made.entry(srgb).or_insert_with(|| {
            let name = format!("STEP #{:02X}{:02X}{:02X}", srgb[0], srgb[1], srgb[2]);
            asm.add_material(prism_doc::Material::colour(&name, srgb))
        });
        if let Some(d) = asm.def_mut(def) {
            d.model.edit_face(face, |f| {
                f.paint.front = Some(m);
                f.paint.back = Some(m);
            });
        }
    }
    Ok(Imported {
        assembly: asm,
        solids: b.solids,
        sheets: b.sheets,
        refused,
    })
}

struct Builder<'a> {
    read: &'a step::Imported,
    defs: BTreeMap<DefId, Definition>,
    next_def: u32,
    next_instance: u32,
    names: Vec<(InstanceId, String)>,
    /// A product placed more than once: its component's definition.
    shared: BTreeMap<u64, DefId>,
    /// Faces to colour: the definition, the face, red, green and blue.
    colours: Vec<(DefId, FaceId, [f64; 3])>,
    solids: usize,
    sheets: usize,
}

/// How many times each product is placed in `nodes` and below.
fn uses(nodes: &[Node], out: &mut BTreeMap<u64, usize>) {
    for n in nodes {
        if let Some(p) = n.product {
            *out.entry(p).or_default() += 1;
        }
        uses(&n.children, out);
    }
}

impl Builder<'_> {
    /// Node `n`, whose parent stands at `world`, as an instance placed in
    /// its parent: a group, or the component its product is. None when it
    /// holds nothing Prism could take.
    fn node(
        &mut self,
        n: &Node,
        world: &DAffine3,
        refused: &mut Vec<String>,
    ) -> Option<(InstanceId, Instance)> {
        let at = *world * n.place;
        let mut counts = BTreeMap::new();
        uses(&self.read.roots, &mut counts);
        let component = n
            .product
            .filter(|p| counts.get(p).copied().unwrap_or(0) > 1);
        let def = match component.and_then(|p| self.shared.get(&p).copied()) {
            Some(d) => d,
            None => {
                let d = self.definition(n, &at, refused)?;
                if let Some(p) = component {
                    self.shared.insert(p, d);
                    if let Some(x) = self.defs.get_mut(&d) {
                        x.name = Some(if n.name.is_empty() {
                            "Part".to_owned()
                        } else {
                            n.name.clone()
                        });
                    }
                }
                d
            }
        };
        let id = InstanceId(self.next_instance);
        self.next_instance += 1;
        if component.is_none() && !n.name.is_empty() {
            self.names.push((id, n.name.clone()));
        }
        Some((
            id,
            Instance {
                def,
                transform: n.place,
            },
        ))
    }

    /// Node `n`'s own shapes (read where it stands, at `at`, and moved back
    /// into its own coordinates) and its children, as a new definition.
    fn definition(&mut self, n: &Node, at: &DAffine3, refused: &mut Vec<String>) -> Option<DefId> {
        let back = at.inverse();
        let mut model = Model::new();
        let mut colours = Vec::new();
        for &i in &n.shapes {
            let Some(Ok(shape)) = self.read.shapes.get(i) else {
                continue;
            };
            let Some(r) = shape.region.transformed(&back) else {
                refused.push(prism_kernel::solid::CURVE_NOT_YET.to_owned());
                continue;
            };
            match put_back(&mut model, &BTreeSet::new(), &r) {
                Ok(faces) => {
                    if shape.open {
                        self.sheets += 1;
                    } else {
                        self.solids += 1;
                    }
                    let own: BTreeMap<usize, [f64; 3]> = shape.colours.iter().copied().collect();
                    for (k, f) in faces.iter().enumerate() {
                        if let Some(c) = own.get(&k).copied().or(n.colour) {
                            colours.push((*f, c));
                        }
                    }
                }
                Err(why) => refused.push(why.to_owned()),
            }
        }
        let def = DefId(self.next_def);
        self.next_def += 1;
        let mut d = Definition {
            model,
            ..Definition::default()
        };
        for c in &n.children {
            if let Some((id, inst)) = self.node(c, at, refused) {
                d.instances.insert(id, inst);
            }
        }
        if d.model.face_count() == 0 && d.instances.is_empty() {
            return None;
        }
        self.colours
            .extend(colours.into_iter().map(|(f, c)| (def, f, c)));
        self.defs.insert(def, d);
        Some(def)
    }
}
