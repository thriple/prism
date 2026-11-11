//! The Paint Bucket (Basic Materials 1b, brief 28): a click paints the
//! side of the face you click (a smooth surface as a whole), Ctrl fills the
//! connected faces of the same material, Shift replaces that material
//! everywhere and Ctrl+Shift in the open group only, and Alt (tap, or hold
//! while clicking) samples the material under the cursor. A group or
//! component inside the open one is painted as a whole: its Default faces
//! show the material. Hovering previews the paint.

use std::collections::BTreeSet;

use prism_doc::material::{self, PALETTE};
use prism_doc::{Command, FaceSide, InstanceId, Material};
use prism_math::{DVec2, DVec3};
use prism_topo::{FaceId, MaterialId};
use prism_view::overlay::{Label, Mark, Overlay, Shape};
use prism_view::style::Rgba;
use prism_view::{Hit, View};

use super::{Owner, Session};

pub const PAINT: &str = "tool.paint";

/// What the Paint Bucket paints with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Brush {
    /// Back to Default (the style's front and back colours).
    Default,
    /// A material in the document.
    Material(MaterialId),
    /// A built-in colour (`PALETTE`), added to the document when first used.
    Palette(usize),
}

impl Default for Brush {
    fn default() -> Self {
        Brush::Palette(0)
    }
}

/// What is under the Paint Bucket.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PaintAt {
    /// The face (the scene's id).
    face: FaceId,
    /// The side seen.
    front: bool,
    /// A group or component inside the open one: painted as a whole.
    instance: Option<InstanceId>,
    point: DVec3,
}

fn side(front: bool) -> FaceSide {
    if front {
        FaceSide::Front
    } else {
        FaceSide::Back
    }
}

impl Session {
    /// What the Paint Bucket paints with.
    pub fn brush(&self) -> Brush {
        self.brush
    }

    /// Choose what the Paint Bucket paints with (the Materials window).
    pub fn set_brush(&mut self, b: Brush) {
        self.take_brush(b);
        self.highlight_dirty = true;
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// Whether the next click samples (Alt tapped).
    pub fn paint_sampling(&self) -> bool {
        self.sampling
    }

    /// Use `b`, keeping a copy of its material so that painting with it
    /// still works after Undo took the material away.
    fn take_brush(&mut self, b: Brush) {
        self.brush = b;
        if let Brush::Material(id) = b
            && let Some(m) = self.doc.assembly().material(id)
        {
            self.brush_copy = Some(m.clone());
        }
    }

    pub(super) fn toggle_sampling(&mut self) {
        self.sampling = !self.sampling;
        self.highlight_dirty = true;
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The brush's name and colour (sRGB) for the cursor's swatch.
    fn brush_look(&self) -> (String, Rgba) {
        match self.brush {
            Brush::Default => ("Default".to_owned(), self.style.face_front),
            Brush::Palette(i) => {
                let (name, c) = PALETTE[i % PALETTE.len()];
                (name.to_owned(), Rgba::rgb(c[0], c[1], c[2]))
            }
            Brush::Material(id) => match self
                .doc
                .assembly()
                .material(id)
                .or(self.brush_copy.as_ref())
            {
                Some(m) => (m.name.clone(), Rgba(m.srgba()).with_alpha(255)),
                None => ("Default".to_owned(), self.style.face_front),
            },
        }
    }

    pub(super) fn paint_hover(&mut self, s: DVec2, view: &View) {
        let (o, d) = view.ray(s);
        let at = self.scene.ray_face(o, d).and_then(|(f, t)| {
            let instance = match self.owner(Hit::Face(f)) {
                None => None,
                Some(Owner::Child(i)) => Some(i),
                // Outside the open group: not paintable from here.
                Some(Owner::Outside) => return None,
            };
            Some(PaintAt {
                face: f,
                front: self.scene.seen_front(f, o, d).unwrap_or(true),
                instance,
                point: o + d * t,
            })
        });
        let h = at.map(|a| prism_view::Picked {
            hit: Hit::Face(a.face),
            point: a.point,
        });
        self.set_hover_pick(h);
        if at.map(|a| (a.face, a.front)) != self.paint_at.map(|a| (a.face, a.front)) {
            self.highlight_dirty = true;
            self.dirty = true;
        }
        if at.is_some() || self.paint_at.is_some() {
            self.overlay_dirty = true;
        }
        self.paint_at = at;
    }

    /// The faces a plain click would paint, and the colour (the hover
    /// preview).
    pub fn paint_preview(&self) -> Option<(Vec<FaceId>, Rgba)> {
        if self.tool != PAINT || self.sampling {
            return None;
        }
        let at = self.paint_at?;
        let faces: Vec<FaceId> = match at.instance {
            Some(_) => {
                // The group's Default faces show its material.
                let k = self.scene.face_owner.get(&at.face)?;
                self.scene
                    .face_owner
                    .iter()
                    .filter(|(f, o)| {
                        *o == k
                            && self
                                .scene
                                .face(**f)
                                .is_some_and(|x| x.paint[usize::from(!at.front)].is_none())
                    })
                    .map(|(f, _)| *f)
                    .collect()
            }
            None => prism_ops::select::surface(self.work_model(), at.face)
                .into_iter()
                .collect(),
        };
        let colour = match self.brush {
            Brush::Default if !at.front => self.style.face_back,
            _ => self.brush_look().1,
        };
        Some((faces, colour))
    }

    /// The material to paint with, and the command that must first add it
    /// to the document (a palette colour's first use).
    fn brush_material(&self) -> (Option<Command>, Option<MaterialId>) {
        match self.brush {
            Brush::Default => (None, None),
            Brush::Material(id) if self.doc.assembly().material(id).is_some() => (None, Some(id)),
            // Gone (undone or deleted): added again from the copy.
            Brush::Material(_) => match &self.brush_copy {
                Some(m) => self.adding(m.clone()),
                None => (None, None),
            },
            Brush::Palette(i) => {
                let (name, c) = PALETTE[i % PALETTE.len()];
                self.adding(Material::colour(name, c))
            }
        }
    }

    /// A material to paint with that may not be in the document yet: the
    /// same one there is used again, else it is added.
    fn adding(&self, m: Material) -> (Option<Command>, Option<MaterialId>) {
        let (mats, next) = self.doc.assembly().materials();
        if let Some((id, _)) = mats.iter().find(|(_, x)| **x == m) {
            return (None, Some(*id));
        }
        (
            Some(Command::AddMaterial { material: m }),
            Some(MaterialId(next)),
        )
    }

    pub(super) fn paint_press(&mut self, shift: bool, ctrl: bool) {
        let Some(at) = self.paint_at else { return };
        if self.sampling || self.press_alt {
            // Sample what is drawn there (a group's material on its Default
            // faces too).
            let b = match self.scene.material_on(at.face, at.front) {
                Some(m) => Brush::Material(m),
                None => Brush::Default,
            };
            self.take_brush(b);
            self.sampling = false;
            self.highlight_dirty = true;
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        let (add, material) = self.brush_material();
        let paint = if let Some(i) = at.instance {
            Command::PaintInstances {
                instances: [i].into(),
                material,
            }
        } else {
            let m = self.work_model();
            let sd = side(at.front);
            let own = m.face(at.face).map(|f| match sd {
                FaceSide::Front => f.paint.front,
                FaceSide::Back => f.paint.back,
            });
            let Some(from) = own else { return };
            if shift {
                Command::ReplaceMaterial {
                    from,
                    to: material,
                    everywhere: !ctrl,
                }
            } else {
                let sides: BTreeSet<(FaceId, FaceSide)> = if ctrl {
                    material::connected_same(m, at.face, sd)
                } else {
                    prism_ops::select::surface(m, at.face)
                        .into_iter()
                        .map(|f| (f, sd))
                        .collect()
                };
                Command::Paint { sides, material }
            }
        };
        let c = match add {
            Some(add) => Command::Batch {
                commands: vec![add, paint],
            },
            None => paint,
        };
        let added = matches!(c, Command::Batch { .. });
        if self.run(c) && added {
            // From now on the brush is the material just added.
            if let Some(m) = material {
                self.take_brush(Brush::Material(m));
            }
        }
        self.highlight_dirty = true;
        self.overlay_dirty = true;
        self.refresh();
    }

    pub(super) fn paint_hint(&self) -> &'static str {
        if self.sampling {
            "SAMPLE: click a face to pick up its material · Alt cancels"
        } else {
            "Click a face to paint it (a group as a whole) · Ctrl fills connected · Shift replaces everywhere · Ctrl+Shift in this group · Alt samples"
        }
    }

    /// The swatch beside the cursor: the brush's colour and name.
    pub(super) fn paint_overlay(&self, o: &mut Overlay) {
        if self.tool != PAINT {
            return;
        }
        let Some(at) = self.paint_at else { return };
        let (name, colour) = self.brush_look();
        let fb = &self.style.feedback;
        o.marks.push(Mark {
            at: at.point,
            shape: Shape::Square,
            color: colour,
            size: 7.0,
        });
        o.labels.push(Label {
            at: at.point,
            offset: [14.0, -16.0],
            text: if self.sampling {
                "Sample".to_owned()
            } else {
                name
            },
            color: fb.pill,
        });
    }
}
