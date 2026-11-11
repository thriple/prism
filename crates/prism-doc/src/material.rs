//! Materials (Basic Materials, brief 28). The document keeps one table of
//! materials, shared by every group's model; a face names a material for
//! its front and its back, and a group or component instance can name one
//! too (drawn on its Default faces, DECISIONS #35).
//!
//! A material is a node graph from day one (PLAN 4). For now the graph is
//! one principled-surface node with glTF's metallic-roughness inputs, so
//! the path tracer (Rendering) will need no file change.

use std::collections::{BTreeMap, BTreeSet};

use prism_topo::{FaceId, MaterialId, Model};
use serde::{Deserialize, Serialize};

/// The principled surface's inputs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Principled {
    /// Base colour, linear RGBA; alpha is opacity.
    pub base: [f32; 4],
    pub roughness: f32,
    pub metalness: f32,
}

impl Default for Principled {
    fn default() -> Self {
        Principled {
            base: [0.8, 0.8, 0.8, 1.0],
            roughness: 0.5,
            metalness: 0.0,
        }
    }
}

/// One node of a material's graph.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Node {
    Principled(Principled),
}

/// A material: its name and its graph (the last node is the output).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub nodes: Vec<Node>,
}

impl Material {
    /// A plain colour, given in sRGB (0 to 255), as colour pickers show it.
    pub fn colour(name: &str, srgb: [u8; 3]) -> Material {
        let lin = srgb.map(|c| srgb_to_linear(f32::from(c) / 255.0));
        Material {
            name: name.to_owned(),
            nodes: vec![Node::Principled(Principled {
                base: [lin[0], lin[1], lin[2], 1.0],
                ..Principled::default()
            })],
        }
    }

    /// The surface the graph ends in.
    pub fn surface(&self) -> Principled {
        match self.nodes.last() {
            Some(Node::Principled(p)) => *p,
            None => Principled::default(),
        }
    }

    pub fn surface_mut(&mut self) -> &mut Principled {
        if self.nodes.is_empty() {
            self.nodes.push(Node::Principled(Principled::default()));
        }
        match self.nodes.last_mut() {
            Some(Node::Principled(p)) => p,
            None => unreachable!("a node was just pushed"),
        }
    }

    /// The base colour in sRGB with alpha (for the screen).
    pub fn srgba(&self) -> [u8; 4] {
        let b = self.surface().base;
        let c = |x: f32| (linear_to_srgb(x.clamp(0.0, 1.0)) * 255.0).round() as u8;
        [
            c(b[0]),
            c(b[1]),
            c(b[2]),
            (b[3].clamp(0.0, 1.0) * 255.0).round() as u8,
        ]
    }
}

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Which side of a face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FaceSide {
    Front,
    Back,
}

/// Paint face sides of a model with `material` (None: back to Default).
pub fn paint(m: &mut Model, sides: &BTreeSet<(FaceId, FaceSide)>, material: Option<MaterialId>) {
    for (f, side) in sides {
        m.edit_face(*f, |x| match side {
            FaceSide::Front => x.paint.front = material,
            FaceSide::Back => x.paint.back = material,
        });
    }
}

/// Every face side of a model painted with `from`.
pub fn sides_with(m: &Model, from: Option<MaterialId>) -> BTreeSet<(FaceId, FaceSide)> {
    let mut out = BTreeSet::new();
    for (id, f) in m.faces() {
        if f.paint.front == from {
            out.insert((id, FaceSide::Front));
        }
        if f.paint.back == from {
            out.insert((id, FaceSide::Back));
        }
    }
    out
}

/// The faces joined to `start` through shared edges whose same side has
/// the same material as `start`'s (the Paint Bucket's fill connected).
pub fn connected_same(m: &Model, start: FaceId, side: FaceSide) -> BTreeSet<(FaceId, FaceSide)> {
    let get = |f: FaceId| {
        m.face(f).map(|x| match side {
            FaceSide::Front => x.paint.front,
            FaceSide::Back => x.paint.back,
        })
    };
    let Some(want) = get(start) else {
        return BTreeSet::new();
    };
    let edge_faces = m.edge_faces();
    let mut seen: BTreeSet<FaceId> = [start].into();
    let mut todo = vec![start];
    while let Some(f) = todo.pop() {
        let Some(face) = m.face(f) else { continue };
        for e in face.edge_ids() {
            for g in edge_faces.get(&e).into_iter().flatten() {
                if !seen.contains(g) && get(*g) == Some(want) {
                    seen.insert(*g);
                    todo.push(*g);
                }
            }
        }
    }
    seen.into_iter().map(|f| (f, side)).collect()
}

/// The materials a model's faces use.
pub fn used_in(m: &Model) -> BTreeSet<MaterialId> {
    m.faces()
        .flat_map(|(_, f)| [f.paint.front, f.paint.back])
        .flatten()
        .collect()
}

/// A name not yet in `taken`: `base`, or `base #2`, `base #3`...
pub fn free_name(taken: &BTreeMap<MaterialId, Material>, base: &str) -> String {
    let used = |n: &str| taken.values().any(|m| m.name == n);
    if !used(base) {
        return base.to_owned();
    }
    (2..)
        .map(|k| format!("{base} #{k}"))
        .find(|n| !used(n))
        .unwrap_or_else(|| base.to_owned())
}
