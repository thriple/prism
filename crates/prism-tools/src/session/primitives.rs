//! The Primitives tool (Shapes & Placement part 4): Box, Cylinder, Cone,
//! Sphere, Torus and Capsule as exact solids, each made into a group that
//! remembers what it is. Click to set one down with the window's sizes
//! (it stands on the clicked point), or press and drag: a box takes its
//! footprint from the two corners, the round ones their radius from the
//! centre. The window's fields hold the sizes; the Inspector shows them
//! again for the group, and changing one makes the solid again in place
//! (until the group's geometry is edited, which it no longer is then).

use prism_doc::Command;
use prism_math::{DVec2, DVec3};
use prism_ops::primitives::{Primitive, SMALLEST};
use prism_view::View;

use super::Session;

pub const PRIM_TOOL: &str = "tool.prim";

/// A drag shorter than this (pixels) is a click.
const DRAG_PX: f64 = 8.0;

/// The window's content.
#[derive(Clone, Debug)]
pub struct PrimPanel {
    /// The six kinds' names, and which is chosen.
    pub kinds: Vec<&'static str>,
    pub current: usize,
    /// Each size: its key, label and value as text.
    pub fields: Vec<(&'static str, &'static str, String)>,
    pub how: &'static str,
}

/// A change from the window.
#[derive(Clone, Debug, PartialEq)]
pub enum PrimSet {
    Kind(usize),
    /// A size (by its key), as typed.
    Size(&'static str, String),
}

fn kind_index(p: &Primitive) -> usize {
    Primitive::kinds()
        .iter()
        .position(|k| k.name() == p.name())
        .unwrap_or(0)
}

impl Session {
    pub fn primitive(&self) -> Primitive {
        self.prim
    }

    pub fn prim_panel(&self) -> Option<PrimPanel> {
        (self.tool == PRIM_TOOL).then(|| PrimPanel {
            kinds: Primitive::kinds().iter().map(Primitive::name).collect(),
            current: kind_index(&self.prim),
            fields: self
                .prim
                .fields()
                .into_iter()
                .map(|(k, l, v)| (k, l, self.number.show(v)))
                .collect(),
            how: self.prim_hint().unwrap_or(""),
        })
    }

    /// A kind picked from the Primitives menu: the tool, with that kind.
    pub(super) fn prim_pick(&mut self, id: &str) {
        let k = match id {
            "prim.box" => 0,
            "prim.cyl" => 1,
            "prim.cone" => 2,
            "prim.sphere" => 3,
            "prim.torus" => 4,
            "prim.capsule" => 5,
            _ => return,
        };
        self.prim = Primitive::kinds()[k];
        if self.tool != PRIM_TOOL {
            self.set_tool(PRIM_TOOL);
        }
        self.prim_ghost();
    }

    /// A change in the window.
    pub fn prim_input(&mut self, op: PrimSet) {
        match op {
            PrimSet::Kind(k) => {
                if let Some(p) = Primitive::kinds().get(k) {
                    self.prim = *p;
                }
            }
            PrimSet::Size(key, text) => match self.number.length(text.trim()).map(f64::abs) {
                Some(v) => match self.prim.with(key, v) {
                    Ok(p) => self.prim = p,
                    Err(why) => self.notice = Some(why),
                },
                None => self.notice = Some("That is not a size"),
            },
        }
        self.prim_ghost();
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The primitive and the point it stands on, as the pointer shows it:
    /// the window's, or sized by the drag in progress.
    fn prim_now(&self) -> Option<(Primitive, DVec3)> {
        let at = self.inference.as_ref().map(|i| i.point)?;
        match self.prim_down {
            Some((a, s)) if self.cursor.is_some_and(|c| c.distance(s) >= DRAG_PX) => {
                Some(self.prim_dragged(a, at))
            }
            Some((a, _)) => Some((self.prim, a)),
            None => Some((self.prim, at)),
        }
    }

    /// The primitive a drag from `a` to `b` makes, and where it stands.
    fn prim_dragged(&self, a: DVec3, b: DVec3) -> (Primitive, DVec3) {
        let (dx, dy) = ((b.x - a.x).abs(), (b.y - a.y).abs());
        let reach = dx.hypot(dy);
        let keep = |p: Result<Primitive, &'static str>| p.unwrap_or(self.prim);
        match self.prim {
            Primitive::Box { .. } => {
                let p = keep(self.prim.with("w", dx).and_then(|p| p.with("d", dy)));
                (p, DVec3::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0, a.z))
            }
            Primitive::Torus { minor, .. } => {
                // The tube stays as thick as it was, but thinner than the ring.
                let major = reach.max(SMALLEST);
                let tube = minor.min(major * 0.4);
                let p = self
                    .prim
                    .with("major", major)
                    .and_then(|p| p.with("minor", tube));
                (keep(p), a)
            }
            _ => (keep(self.prim.with("r", reach)), a),
        }
    }

    pub(super) fn prim_press(&mut self, s: DVec2) {
        let Some(at) = self.inference.as_ref().map(|i| i.point) else {
            return;
        };
        self.prim_down = Some((at, s));
    }

    pub(super) fn prim_release(&mut self) {
        let Some(_) = self.prim_down else { return };
        if let Some((p, at)) = self.prim_now() {
            self.prim_place(p, at);
        }
        self.prim_down = None;
        self.prim_shown = None;
        self.prim_ghost();
    }

    /// Make the group, in one undo step, and select it. The window keeps
    /// the sizes it was made at.
    pub(super) fn prim_place(&mut self, p: Primitive, at: DVec3) {
        self.cancel();
        let id = self.doc.assembly().next_instance();
        if self.run(Command::AddPrimitive { primitive: p, at }) {
            self.prim = p;
            self.set_groups([id].into());
        }
    }

    /// Set a primitive down for the assistant: its group's id.
    pub fn add_primitive(&mut self, p: Primitive, at: DVec3) -> Result<u32, &'static str> {
        for (key, _, v) in p.fields() {
            p.with(key, v)?;
        }
        let id = self.doc.assembly().next_instance();
        self.prim_place(p, at);
        self.groups
            .contains(&id)
            .then_some(id.0)
            .ok_or("that primitive could not be made")
    }

    /// The Inspector's rows for a selected group that still is its
    /// primitive: its sizes, to type new ones.
    pub(super) fn prim_fields(
        &self,
        id: prism_doc::InstanceId,
    ) -> Option<Vec<super::inspector::Field>> {
        let asm = self.doc.assembly();
        let ctx = asm.context_def(&self.ctx).unwrap_or(prism_doc::ROOT);
        let p = asm.live_primitive(ctx, id)?;
        Some(
            p.fields()
                .into_iter()
                .map(|(k, l, v)| {
                    super::inspector::field(
                        &format!("prim.{k}"),
                        l,
                        super::inspector::FieldKind::Length,
                        self.number.show(v),
                    )
                })
                .collect(),
        )
    }

    /// A size typed in the Inspector: the solid is made again in place.
    pub(super) fn prim_set(&mut self, key: &str, text: &str) -> Result<(), String> {
        let ids: Vec<_> = self.groups.iter().copied().collect();
        let [id] = ids.as_slice() else {
            return Err("Resize one primitive at a time".into());
        };
        let asm = self.doc.assembly();
        let ctx = asm.context_def(&self.ctx).unwrap_or(prism_doc::ROOT);
        let p = asm
            .live_primitive(ctx, *id)
            .ok_or("This is no longer a primitive (its geometry was edited)")?;
        let v = self
            .number
            .length(text.trim())
            .map(f64::abs)
            .ok_or("That is not a size")?;
        let key = key.strip_prefix("prim.").unwrap_or(key);
        let q = p.with(key, v).map_err(str::to_owned)?;
        if q == p {
            return Ok(());
        }
        self.cancel();
        self.run(Command::SetPrimitive {
            instance: *id,
            primitive: q,
        });
        Ok(())
    }

    pub(super) fn prim_hover(&mut self, _s: DVec2, _view: &View) {
        self.prim_ghost();
    }

    /// The ghost: where the primitive would stand, while the tool waits.
    pub(super) fn prim_ghost(&mut self) {
        if self.tool != PRIM_TOOL {
            return;
        }
        let want = self.prim_now();
        if want == self.prim_shown {
            return;
        }
        self.prim_shown = want;
        let scene =
            want.map(|(primitive, at)| self.preview_of(&Command::AddPrimitive { primitive, at }));
        self.set_preview_scene(scene);
    }

    pub(super) fn prim_hint(&self) -> Option<&'static str> {
        if self.tool != PRIM_TOOL {
            return None;
        }
        Some(if self.prim_down.is_some() {
            "Drag to size it · release to make it · Esc cancels"
        } else {
            "Click to set it down, or press and drag (a box from corner to corner, the rest from the middle out) · sizes are in the window"
        })
    }
}
