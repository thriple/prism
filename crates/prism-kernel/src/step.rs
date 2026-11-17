//! STEP AP214 in and out (Solids & CAD part 5, brief 44), through the
//! kernel's own reader and writer (arris-io).
//!
//! Nothing outside this crate sees the kernel's types: shapes go in and
//! come out as [`Region`]s, and a file's assemblies as a tree of [`Node`]s.
//! Prism's model is in metres; the file is written in the unit asked for,
//! and read back in metres.

use arris_io::step::{self, FaceColour, Occurrence, ProductTree, ReadOptions, Rgb, WriteOptions};
use arris_math::nalgebra::{Quaternion, UnitQuaternion};
use arris_math::{Control, Isometry, Vec3};
use arris_topo::builder::Builder;
use arris_topo::entity::BodyKind;
use arris_topo::{Body, Model};
use prism_math::{DAffine3, DQuat, DVec3, LengthUnit};

use crate::solid::{self, FAULTY, NOT_CLOSED, Region, read_region};

/// Said when there is nothing the writer can write.
pub const NOTHING: &str = "Nothing to export: no solid or sheet could go to STEP.";
/// Said when the kernel's writer refuses the whole file.
pub const NOT_WRITTEN: &str = "Refused: the STEP writer could not write these shapes.";
/// Said for a sheet whose faces do not join into an open surface.
pub const NOT_A_SHEET: &str = "Refused: these faces do not join into one surface.";

/// One solid, or one open sheet, and its faces' own colours (by index into
/// the region's faces, red, green and blue from 0 to 1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shape {
    pub region: Region,
    pub open: bool,
    pub colours: Vec<(usize, [f64; 3])>,
}

/// One place a product stands: its name, placement in its parent (a
/// rigid motion), colour, the shapes it holds itself (indices into the
/// shapes) and what is placed in it. Nodes that carry the same
/// `Some(product)` are one product placed many times, and must agree in
/// all but their placement. A root's placement is not written.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub product: Option<u64>,
    pub place: DAffine3,
    pub colour: Option<[f64; 3]>,
    pub shapes: Vec<usize>,
    pub children: Vec<Node>,
}

/// A written file, and the shapes left out with why.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    pub text: String,
    pub left_out: Vec<(usize, &'static str)>,
}

/// A read file: one entry per solid or sheet placed in it (an assembly's
/// part placed twice is two entries, each already at its place), or why
/// it was refused; and the file's assemblies over those entries.
#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    pub shapes: Vec<Result<Shape, String>>,
    pub roots: Vec<Node>,
}

/// The kernel's unit for one of Prism's (feet and inches write inches).
fn unit_of(u: LengthUnit) -> step::LengthUnit {
    match u {
        LengthUnit::Millimetres => step::LengthUnit::Millimetre,
        LengthUnit::Centimetres => step::LengthUnit::Centimetre,
        LengthUnit::Metres => step::LengthUnit::Metre,
        LengthUnit::Inches | LengthUnit::FeetInches => step::LengthUnit::Inch,
    }
}

/// Metres in one of the file's units.
fn metres(u: step::LengthUnit) -> f64 {
    match u {
        step::LengthUnit::Micrometre => 1e-6,
        step::LengthUnit::Millimetre => 1e-3,
        step::LengthUnit::Centimetre => 1e-2,
        step::LengthUnit::Metre => 1.0,
        step::LengthUnit::Inch => 0.0254,
        step::LengthUnit::Foot => 0.3048,
    }
}

/// `region` added to `model` as a solid (or a sheet when `open`), checked
/// by the kernel's checker; its body and the kernel's face for each of
/// the region's faces. The model is left as it was when it is refused.
fn add_to(
    model: &mut Model,
    region: &Region,
    open: bool,
) -> Result<(Body, Vec<arris_topo::FaceId>), &'static str> {
    let prec = model.precision();
    let t = prec.default_tolerance;
    model.transaction(|m| {
        let assembly = solid::specs(m, region, prec)?;
        let (builder, slots) = if open {
            Builder::assemble_sheet(m, t, assembly).map_err(|_| NOT_A_SHEET)?
        } else {
            Builder::assemble(m, t, assembly).map_err(|_| NOT_CLOSED)?
        };
        let kind = if open {
            BodyKind::Sheet
        } else {
            BodyKind::Solid
        };
        let built = builder.finish(m, kind).map_err(|_| NOT_CLOSED)?;
        if !arris_check::check(m, built.body, arris_check::Level::Full).is_ok() {
            return Err(FAULTY);
        }
        let faces = slots
            .faces
            .first()
            .ok_or(NOT_CLOSED)?
            .iter()
            .map(|s| built.faces.get(s).copied())
            .collect::<Option<Vec<_>>>()
            .ok_or(NOT_CLOSED)?;
        Ok((built.body, faces))
    })
}

/// A rigid placement as the kernel's, its move scaled by `k`.
fn isometry(a: &DAffine3, k: f64) -> Isometry {
    let q = DQuat::from_mat3(&a.matrix3).normalize();
    let t = a.translation * k;
    Isometry::new(
        UnitQuaternion::from_quaternion(Quaternion::new(q.w, q.x, q.y, q.z)),
        Vec3::new(t.x, t.y, t.z),
    )
}

/// The kernel's placement as Prism's.
fn affine(i: &Isometry) -> DAffine3 {
    let q = i.rotation();
    let t = i.translation();
    DAffine3::from_rotation_translation(
        DQuat::from_xyzw(q.i, q.j, q.k, q.w).normalize(),
        DVec3::new(t.x, t.y, t.z),
    )
}

/// The STEP text of `shapes` placed by the trees `roots`, in `unit`.
/// `tol` is the model's tolerance in metres. A shape the kernel refuses
/// is left out (and named in `left_out`), and so is a node left holding
/// nothing.
pub fn write(
    shapes: &[Shape],
    roots: &[Node],
    unit: LengthUnit,
    tol: f64,
) -> Result<Written, &'static str> {
    let file = unit_of(unit);
    let k = 1.0 / metres(file);
    let to_file = DAffine3::from_scale(DVec3::splat(k));
    let mut model = Model::new(solid::precision(tol * k)).map_err(|_| NOT_WRITTEN)?;
    let mut bodies = Vec::new();
    let mut faces = Vec::new();
    // Each shape's body, by its index.
    let mut index: Vec<Option<usize>> = vec![None; shapes.len()];
    let mut left_out = Vec::new();
    for (i, s) in shapes.iter().enumerate() {
        let built = s
            .region
            .transformed(&to_file)
            .ok_or(solid::CURVE_NOT_YET)
            .and_then(|r| add_to(&mut model, &r, s.open));
        match built {
            Ok((body, fs)) => {
                index[i] = Some(bodies.len());
                for &(f, c) in &s.colours {
                    if let Some(face) = fs.get(f) {
                        faces.push(FaceColour {
                            solid: bodies.len(),
                            face: *face,
                            colour: Rgb(c),
                        });
                    }
                }
                bodies.push(body);
            }
            Err(why) => left_out.push((i, why)),
        }
    }
    fn occurrence(n: &Node, index: &[Option<usize>], k: f64) -> Option<Occurrence> {
        let solids: Vec<usize> = n
            .shapes
            .iter()
            .filter_map(|i| index.get(*i).copied().flatten())
            .collect();
        let children: Vec<Occurrence> = n
            .children
            .iter()
            .filter_map(|c| occurrence(c, index, k))
            .collect();
        if solids.is_empty() && children.is_empty() {
            return None;
        }
        Some(Occurrence {
            product: n.product,
            name: n.name.clone(),
            placement: Ok(isometry(&n.place, k)),
            colour: n.colour.filter(|_| !solids.is_empty()).map(Rgb),
            solids,
            children,
        })
    }
    let tree = ProductTree {
        roots: roots
            .iter()
            .filter_map(|r| occurrence(r, &index, k))
            .collect(),
        faces,
    };
    if tree.roots.is_empty() {
        return Err(NOTHING);
    }
    let options = WriteOptions { length_unit: file };
    let text = step::write_products_with(&model, &bodies, &tree, &options, &Control::NONE)
        .map_err(|_| NOT_WRITTEN)?;
    Ok(Written { text, left_out })
}

/// The solids and sheets of the STEP text `text`, in metres; `tol` is the
/// model's tolerance in metres. Errs only when the text is not a STEP
/// file.
pub fn read(text: &str, tol: f64) -> Result<Imported, String> {
    let mut model = Model::new(solid::precision(tol)).map_err(|e| e.to_string())?;
    let options = ReadOptions {
        length_unit: step::LengthUnit::Metre,
        ..ReadOptions::default()
    };
    let read = step::read(&mut model, text, &options, &Control::NONE).map_err(|e| e.to_string())?;
    let mut shapes: Vec<Result<Shape, String>> = read
        .solids
        .iter()
        .map(|s| {
            let body = s.result.as_ref().map_err(|r| r.kind().to_string())?;
            let region = read_region(&model, body.body, &|_, _| None).map_err(str::to_owned)?;
            let open = model
                .body(body.body.id)
                .is_ok_and(|b| b.kind() == BodyKind::Sheet);
            Ok(Shape {
                region,
                open,
                colours: Vec::new(),
            })
        })
        .collect();
    // Faces coloured apart from their solid: by their place in the body,
    // which is the region's order.
    for fc in &read.products.faces {
        let Some(Ok(shape)) = shapes.get_mut(fc.solid) else {
            continue;
        };
        let Some(Ok(body)) = read.solids.get(fc.solid).map(|s| s.result.as_ref()) else {
            continue;
        };
        if let Some(i) = model
            .faces(body.body)
            .ok()
            .and_then(|fs| fs.iter().position(|f| f.id == fc.face))
        {
            shape.colours.push((i, fc.colour.0));
        }
    }
    fn node(o: &Occurrence) -> Node {
        Node {
            name: o.name.clone(),
            product: o.product,
            place: o.placement.as_ref().map_or(DAffine3::IDENTITY, affine),
            colour: o.colour.map(|c| c.0),
            shapes: o.solids.clone(),
            children: o.children.iter().map(node).collect(),
        }
    }
    let roots = read.products.roots.iter().map(node).collect();
    Ok(Imported { shapes, roots })
}
