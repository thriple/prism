//! The Hole tool (Shapes & Placement part 6): a plain, counterbored or
//! countersunk hole where you click a flat face of a solid group. The
//! window sets the kind and sizes. The hole is a live subtract (its cutter
//! stays in the group's features), so it can be taken out or baked.

use prism_doc::Command;
use prism_math::{DVec2, DVec3};
use prism_ops::hole::{HoleKind, HoleSpec};
use prism_view::View;
use prism_view::pick::Hit;

use super::Owner;
use super::Session;
use super::texting::frame_on;

pub const HOLE_TOOL: &str = "tool.hole";

/// The window's settings (metres and degrees), kept for each kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoleSettings {
    /// 0 plain, 1 counterbore, 2 countersink.
    pub kind: usize,
    pub diameter: f64,
    pub depth: f64,
    pub through: bool,
    pub bore_diameter: f64,
    pub bore_depth: f64,
    pub sink_diameter: f64,
    pub sink_angle: f64,
}

impl Default for HoleSettings {
    fn default() -> Self {
        HoleSettings {
            kind: 0,
            diameter: 0.05,
            depth: 0.1,
            through: true,
            bore_diameter: 0.1,
            bore_depth: 0.03,
            sink_diameter: 0.1,
            sink_angle: 90.0,
        }
    }
}

impl HoleSettings {
    pub fn spec(&self) -> HoleSpec {
        HoleSpec {
            diameter: self.diameter,
            depth: (!self.through).then_some(self.depth),
            kind: match self.kind {
                1 => HoleKind::Counterbore {
                    diameter: self.bore_diameter,
                    depth: self.bore_depth,
                },
                2 => HoleKind::Countersink {
                    diameter: self.sink_diameter,
                    angle: self.sink_angle,
                },
                _ => HoleKind::Plain,
            },
        }
    }
}

/// The window's content.
#[derive(Clone, Debug)]
pub struct HolePanel {
    pub kinds: Vec<&'static str>,
    pub current: usize,
    pub through: bool,
    /// Each size: its key, label and value as text.
    pub fields: Vec<(&'static str, &'static str, String)>,
    pub how: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HoleSet {
    Kind(usize),
    Through(bool),
    /// A size by its key, as typed.
    Size(&'static str, String),
}

impl Session {
    pub fn hole_settings(&self) -> HoleSettings {
        self.hole
    }

    pub fn hole_panel(&self) -> Option<HolePanel> {
        if self.tool != HOLE_TOOL {
            return None;
        }
        let h = self.hole;
        let nf = self.number;
        let mut fields = vec![("diameter", "Diameter", nf.show(h.diameter))];
        if !h.through {
            fields.push(("depth", "Depth", nf.show(h.depth)));
        }
        match h.kind {
            1 => {
                fields.push(("bore_diameter", "Bore diameter", nf.show(h.bore_diameter)));
                fields.push(("bore_depth", "Bore depth", nf.show(h.bore_depth)));
            }
            2 => {
                fields.push(("sink_diameter", "Sink diameter", nf.show(h.sink_diameter)));
                fields.push(("sink_angle", "Sink angle", format!("{}", h.sink_angle)));
            }
            _ => {}
        }
        Some(HolePanel {
            kinds: vec!["Plain", "Counterbore", "Countersink"],
            current: h.kind,
            through: h.through,
            fields,
            how: self.hole_hint(),
        })
    }

    pub fn hole_input(&mut self, op: HoleSet) {
        match op {
            HoleSet::Kind(k) if k < 3 => self.hole.kind = k,
            HoleSet::Kind(_) => {}
            HoleSet::Through(t) => self.hole.through = t,
            HoleSet::Size(key, text) => {
                let t = text.trim();
                if key == "sink_angle" {
                    match t.trim_end_matches('°').parse::<f64>() {
                        Ok(a) if (10.0..=170.0).contains(&a) => self.hole.sink_angle = a,
                        _ => self.notice = Some(prism_ops::hole::SINK_ANGLE),
                    }
                } else if let Some(v) = self.number.length(t).map(f64::abs).filter(|v| *v >= 1e-5) {
                    match key {
                        "diameter" => self.hole.diameter = v,
                        "depth" => self.hole.depth = v,
                        "bore_diameter" => self.hole.bore_diameter = v,
                        "bore_depth" => self.hole.bore_depth = v,
                        "sink_diameter" => self.hole.sink_diameter = v,
                        _ => {}
                    }
                } else {
                    self.notice = Some(prism_ops::hole::TOO_SMALL);
                }
            }
        }
        self.overlay_dirty = true;
        self.dirty = true;
    }

    /// The flat face of a group under the pointer, the group, and the point.
    fn hole_target(
        &self,
        s: DVec2,
        view: &View,
    ) -> Option<(prism_topo::FaceId, prism_doc::InstanceId, DVec3, DVec3)> {
        let (o, d) = view.ray(s);
        let (f, t) = self.scene.ray_face(o, d)?;
        let Some(Owner::Child(body)) = self.owner(Hit::Face(f)) else {
            return None;
        };
        let mut n = self.scene.face(f)?.flat_normal()?;
        if n.dot(d) > 0.0 {
            n = -n;
        }
        let mut at = o + d * t;
        // A snapped point on this face (an end, a middle, a circle's centre).
        if let Some(i) = &self.inference
            && i.face == Some(f)
        {
            at = i.point;
        }
        Some((f, body, at, n))
    }

    pub(super) fn hole_hover(&mut self, s: DVec2, view: &View) {
        let h = self
            .hole_target(s, view)
            .map(|(f, _, at, _)| prism_view::Picked {
                hit: Hit::Face(f),
                point: at,
            });
        self.set_hover_pick(h);
    }

    pub(super) fn hole_press(&mut self, s: DVec2) {
        let view = self.view();
        let Some((_, body, at, n)) = self.hole_target(s, &view) else {
            self.notice = Some("Click a flat face of a solid group (make the solid a group first)");
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        };
        let command = Command::AddHole {
            spec: self.hole.spec(),
            frame: frame_on(at, n),
            body,
        };
        // Said before anything is made: the kernel's reason, or the hole's.
        let asm = self.doc.assembly();
        let ctx = asm.context_def(&self.ctx).unwrap_or(prism_doc::ROOT);
        let local = command.to_local(&self.to_world.inverse(), &prism_topo::Model::new());
        if let Command::AddHole { spec, frame, body } = local
            && let Err(why) = asm.clone().add_hole(ctx, spec, frame, body)
        {
            self.notice = Some(why);
            self.overlay_dirty = true;
            self.dirty = true;
            return;
        }
        self.notice = None;
        self.cancel();
        self.set_selection(Default::default());
        self.set_groups(Default::default());
        self.run(command);
    }

    pub(super) fn hole_hint(&self) -> &'static str {
        "Click a flat face of a solid group to make the hole there · kind and sizes in the window"
    }
}
