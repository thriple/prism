//! STEP export (Solids & CAD part 5b, brief 44): the document's solids
//! and sheets as one STEP AP214 file, groups and components kept as
//! products, through the kernel's writer (`prism_kernel::step`).
//!
//! What is shown is exported (hidden groups are left out, PLAN 1.14).
//! Each closed body is a solid and each open one a sheet; a component is
//! one product placed at each of its instances. An instance that is not
//! a rigid placement (scaled or mirrored) has its geometry written where
//! it stands instead.

use std::collections::{BTreeMap, BTreeSet};

use prism_doc::{Assembly, DefId, InstanceId, ROOT};
use prism_kernel::step::{self, Node, Shape};
use prism_math::{DAffine3, LengthUnit};
use prism_ops::bridge::region;
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
