//! Mesh formats (brief 09): STL, OBJ, 3MF, glTF (GLB) and PLY, in and out.
//!
//! Pure: bytes in, bytes out. Export turns faces into triangles with the
//! viewport's triangulator (or keeps a face as one polygon where the format
//! allows and it has no holes). Import reads triangles or polygons, then
//! [`build`] welds them and merges coplanar triangles back into faces.

use std::collections::BTreeSet;

use prism_math::{DVec2, DVec3, LengthUnit};
use prism_topo::{FaceId, Model};

use crate::FileError;

mod build;
mod gltf;
mod obj;
mod ply;
mod stl;
mod threemf;

pub use build::build;

/// Triangles or polygons, as indices into `positions`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<DVec3>,
    pub polygons: Vec<Vec<u32>>,
}

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.polygons
            .iter()
            .map(|p| p.len().saturating_sub(2))
            .sum()
    }

    /// The size of the bounding box, in file units.
    pub fn size(&self) -> DVec3 {
        let mut it = self.positions.iter();
        let Some(first) = it.next() else {
            return DVec3::ZERO;
        };
        let (lo, hi) = it.fold((*first, *first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        hi - lo
    }
}

/// A mesh file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Stl,
    Obj,
    ThreeMf,
    Glb,
    Ply,
}

impl Format {
    pub const ALL: [Format; 5] = [
        Format::ThreeMf,
        Format::Stl,
        Format::Glb,
        Format::Obj,
        Format::Ply,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Format::Stl => "STL",
            Format::Obj => "OBJ",
            Format::ThreeMf => "3MF",
            Format::Glb => "glTF (GLB)",
            Format::Ply => "PLY",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Stl => "stl",
            Format::Obj => "obj",
            Format::ThreeMf => "3mf",
            Format::Glb => "glb",
            Format::Ply => "ply",
        }
    }

    /// The format of a file name, by its extension (`.gltf` reads as glTF).
    pub fn of(name: &str) -> Option<Format> {
        let ext = name.rsplit('.').next()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "stl" => Format::Stl,
            "obj" => Format::Obj,
            "3mf" => Format::ThreeMf,
            "glb" | "gltf" => Format::Glb,
            "ply" => Format::Ply,
            _ => return None,
        })
    }

    /// The unit the format itself states, if it does (3MF: millimetres,
    /// glTF: metres). The others need asking.
    pub fn own_unit(self) -> Option<LengthUnit> {
        match self {
            Format::ThreeMf => Some(LengthUnit::Millimetres),
            Format::Glb => Some(LengthUnit::Metres),
            _ => None,
        }
    }

    /// The usual up axis: Y for glTF and OBJ, Z for the rest.
    pub fn y_up(self) -> bool {
        matches!(self, Format::Glb | Format::Obj)
    }
}

/// How to export.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportOptions {
    pub format: Format,
    /// The unit the file's numbers are in.
    pub unit: LengthUnit,
    /// Turn Prism's Z up into Y up.
    pub y_up: bool,
}

impl ExportOptions {
    pub fn for_format(format: Format) -> Self {
        Self {
            format,
            unit: format.own_unit().unwrap_or(LengthUnit::Millimetres),
            y_up: format.y_up(),
        }
    }
}

/// A model point in the file's frame and unit.
fn out(p: DVec3, o: &ExportOptions) -> DVec3 {
    let p = p / o.unit.metres();
    if o.y_up {
        DVec3::new(p.x, p.z, -p.y)
    } else {
        p
    }
}

/// A file point back in metres, Z up.
pub fn into_model(p: DVec3, metres_per_unit: f64, y_up: bool) -> DVec3 {
    let p = if y_up { DVec3::new(p.x, -p.z, p.y) } else { p };
    p * metres_per_unit
}

/// The faces of a model (or just `faces`) as a mesh in the file's frame.
/// With `polygons`, faces without holes stay whole.
pub fn to_mesh(
    m: &Model,
    faces: Option<&BTreeSet<FaceId>>,
    o: &ExportOptions,
    polygons: bool,
) -> Mesh {
    let mut mesh = Mesh::default();
    let mut index = std::collections::BTreeMap::new();
    for (id, f) in m.faces() {
        if faces.is_some_and(|s| !s.contains(&id)) {
            continue;
        }
        let mut idx = |v: prism_topo::VertexId, mesh: &mut Mesh| -> u32 {
            *index.entry(v).or_insert_with(|| {
                mesh.positions.push(out(m.vertex(v), o));
                (mesh.positions.len() - 1) as u32
            })
        };
        if let prism_geom::Surface::Cylinder(c) = f.surface {
            // A curved face: its drawn points, triangulated unrolled flat.
            let loops: Vec<Vec<(DVec3, DVec2)>> =
                (0..=f.holes.len()).map(|k| m.unrolled(f, k)).collect();
            let flat = |l: &Vec<(DVec3, DVec2)>| l.iter().map(|x| x.1).collect::<Vec<_>>();
            let holes: Vec<Vec<DVec2>> = loops[1..].iter().map(flat).collect();
            let base = mesh.positions.len() as u32;
            mesh.positions
                .extend(loops.iter().flatten().map(|x| out(x.0, o)));
            for t in prism_view_tess::triangulate_unrolled(&flat(&loops[0]), &holes) {
                let mut tri = [t[0], t[1], t[2]].map(|i| base + i as u32);
                if !c.outward {
                    tri.swap(1, 2);
                }
                mesh.polygons.push(tri.to_vec());
            }
            continue;
        }
        if let Some(t) = prism_view_tess::turned(m, f) {
            // A cone, sphere or torus face: its grid of points on the
            // surface.
            let base = mesh.positions.len() as u32;
            mesh.positions.extend(t.points.iter().map(|p| out(*p, o)));
            for tri in t.tris {
                mesh.polygons.push(tri.map(|i| base + i as u32).to_vec());
            }
            continue;
        }
        if polygons && f.holes.is_empty() {
            let poly: Vec<u32> = f.outer.iter().map(|v| idx(*v, &mut mesh)).collect();
            mesh.polygons.push(poly);
            continue;
        }
        let Some(&plane) = f.plane() else { continue };
        let frame = plane.frame();
        let flat = |l: &[prism_topo::VertexId]| -> Vec<DVec2> {
            l.iter().map(|v| frame.to_2d(m.vertex(*v))).collect()
        };
        let outer = flat(&f.outer);
        let holes: Vec<Vec<DVec2>> = f.holes.iter().map(|h| flat(h)).collect();
        let all: Vec<prism_topo::VertexId> = f.loops().flatten().copied().collect();
        // The frame's normal may be the face's, or the opposite.
        let flip = frame.normal.dot(plane.normal) < 0.0;
        for t in prism_view_tess::triangulate(&outer, &holes) {
            let mut tri = [all[t[0]], all[t[1]], all[t[2]]];
            if flip {
                tri.swap(1, 2);
            }
            let tri: Vec<u32> = tri.iter().map(|v| idx(*v, &mut mesh)).collect();
            mesh.polygons.push(tri);
        }
    }
    mesh
}

// The viewport's triangulator, so exports match what is drawn.
use prism_view::tess as prism_view_tess;

/// Export a model (or some of its faces) as `o.format`.
pub fn export(m: &Model, faces: Option<&BTreeSet<FaceId>>, o: &ExportOptions) -> Vec<u8> {
    match o.format {
        Format::Stl => stl::write(&to_mesh(m, faces, o, false)),
        Format::Obj => obj::write(&to_mesh(m, faces, o, true)),
        Format::Ply => ply::write(&to_mesh(m, faces, o, true)),
        Format::Glb => gltf::write(&to_mesh(m, faces, o, false)),
        Format::ThreeMf => threemf::write(&to_mesh(m, faces, o, false), o.unit),
    }
}

/// Read a mesh file. The mesh is in the file's own unit and frame.
pub fn read(bytes: &[u8], format: Format) -> Result<Mesh, FileError> {
    let mesh = match format {
        Format::Stl => stl::read(bytes)?,
        Format::Obj => obj::read(bytes)?,
        Format::Ply => ply::read(bytes)?,
        Format::Glb => gltf::read(bytes)?,
        Format::ThreeMf => threemf::read(bytes)?,
    };
    let n = mesh.positions.len() as u32;
    if mesh.polygons.iter().flatten().any(|&i| i >= n) {
        return Err(FileError::Damaged("a face uses a missing vertex".into()));
    }
    if mesh.positions.iter().any(|p| !p.is_finite()) {
        return Err(FileError::Damaged("a vertex is not a finite number".into()));
    }
    Ok(mesh)
}

/// The unit a file states (metres per unit), if it does: 3MF says its own,
/// glTF is always metres. Others must be asked.
pub fn stated_unit(bytes: &[u8], format: Format) -> Option<f64> {
    match format {
        Format::ThreeMf => threemf::unit_of(bytes),
        Format::Glb => Some(1.0),
        _ => None,
    }
}

fn damaged(why: impl std::fmt::Display) -> FileError {
    FileError::Damaged(why.to_string())
}
