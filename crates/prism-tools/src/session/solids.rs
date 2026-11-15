//! Solid > Validate and Heal in the session (Solids & CAD part 1, brief
//! 37): one row per solid in the open group, selecting one, and Heal.

use std::collections::BTreeSet;

use prism_doc::{Command, InstanceId};
use prism_math::{DVec3, LengthUnit};
use prism_ops::Selection;
use prism_ops::validate::{SolidCheck, heal, validate};
use prism_topo::FaceId;
use prism_view::camera::EASE_EXTENTS;

use super::Session;

/// What a row of the Solids window is.
#[derive(Clone, Debug, PartialEq)]
pub enum SolidOf {
    /// Loose faces in the open group.
    Faces(BTreeSet<FaceId>),
    /// A group or component in it, checked from its own geometry.
    Group(InstanceId),
}

/// One row of the Solids window.
#[derive(Clone, Debug, PartialEq)]
pub struct SolidRow {
    pub name: String,
    pub valid: bool,
    /// "24.000 m³", when it is closed.
    pub volume: Option<String>,
    pub reasons: Vec<String>,
    /// Why the kernel did not check it, if it did not.
    pub note: Option<&'static str>,
    pub of: SolidOf,
}

impl Session {
    /// Every solid in the open group: its loose pieces, then its groups
    /// and components that hold faces of their own.
    pub fn solid_rows(&self) -> Vec<SolidRow> {
        let mut rows: Vec<SolidRow> = validate(self.work_model())
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                self.solid_row(
                    format!("Solid {}", i + 1),
                    &c,
                    1.0,
                    SolidOf::Faces(c.faces.clone()),
                )
            })
            .collect();
        let asm = self.doc.assembly();
        let Some(here) = asm.context_def(&self.ctx).and_then(|d| asm.def(d)) else {
            return rows;
        };
        for (id, inst) in &here.instances {
            let Some(def) = asm.def(inst.def) else {
                continue;
            };
            let checks = validate(&def.model);
            if checks.is_empty() {
                continue;
            }
            let name = asm
                .name_of(*id)
                .map(str::to_owned)
                .or_else(|| def.name.clone())
                .unwrap_or_else(|| "Group".to_owned());
            // One row for the group: valid if all its pieces are.
            let all = SolidCheck {
                faces: BTreeSet::new(),
                valid: checks.iter().all(|c| c.valid),
                volume: checks.iter().map(|c| c.volume).sum(),
                reasons: checks.iter().flat_map(|c| c.reasons.clone()).collect(),
                kernel_skipped: checks.iter().find_map(|c| c.kernel_skipped),
            };
            let k = inst.transform.matrix3.determinant().abs();
            rows.push(self.solid_row(name, &all, k, SolidOf::Group(*id)));
        }
        rows
    }

    fn solid_row(&self, name: String, c: &SolidCheck, scale: f64, of: SolidOf) -> SolidRow {
        SolidRow {
            name,
            valid: c.valid,
            volume: c.volume.map(|v| self.volume_text(v * scale)),
            reasons: c.reasons.clone(),
            note: c.kernel_skipped,
            of,
        }
    }

    /// A volume in the document's unit cubed.
    fn volume_text(&self, v: f64) -> String {
        let u = self.number.units;
        let unit = match u.length {
            LengthUnit::Millimetres => "mm³",
            LengthUnit::Centimetres => "cm³",
            LengthUnit::Metres => "m³",
            LengthUnit::Inches | LengthUnit::FeetInches => "in³",
        };
        let n = format!(
            "{:.*}",
            u.decimals as usize,
            v.abs() / u.length.metres().powi(3)
        );
        format!(
            "{} {unit}",
            n.replace('.', &self.number.decimal.to_string())
        )
    }

    /// Select a solid and zoom to it.
    pub fn select_solid(&mut self, of: &SolidOf) {
        let pts: Vec<DVec3> = match of {
            SolidOf::Faces(faces) => {
                let m = self.work_model();
                let sel = Selection {
                    faces: faces.clone(),
                    ..Default::default()
                };
                let pts = sel
                    .all_vertices(m)
                    .into_iter()
                    .map(|v| m.vertex(v))
                    .collect();
                self.set_groups(Default::default());
                self.set_selection(sel);
                pts
            }
            SolidOf::Group(id) => {
                let asm = self.doc.assembly();
                let here = asm.context_def(&self.ctx).and_then(|d| asm.def(d));
                let pts = here
                    .and_then(|h| h.instances.get(id))
                    .and_then(|inst| {
                        let (lo, hi) = asm.def(inst.def)?.model.bbox()?;
                        let to = self.to_world * inst.transform;
                        Some(
                            (0..8)
                                .map(|i| {
                                    let pick = |k: usize, a: f64, b: f64| {
                                        if i >> k & 1 == 0 { a } else { b }
                                    };
                                    to.transform_point3(DVec3::new(
                                        pick(0, lo.x, hi.x),
                                        pick(1, lo.y, hi.y),
                                        pick(2, lo.z, hi.z),
                                    ))
                                })
                                .collect(),
                        )
                    })
                    .unwrap_or_default();
                self.set_selection(Selection::default());
                self.set_groups([*id].into());
                pts
            }
        };
        if let Some(first) = pts.first() {
            let (lo, hi) = pts
                .iter()
                .fold((*first, *first), |(l, h), p| (l.min(*p), h.max(*p)));
            let goal = self.camera.extents(lo, hi, self.view().aspect());
            self.ease_to(goal, EASE_EXTENTS);
        }
    }

    /// Heal the open group's loose geometry (the selected faces' pieces,
    /// if faces are selected), in one undo step. Says what it did; nothing
    /// changes, and no undo step is added, if nothing needed it.
    pub fn heal_solids(&mut self) -> String {
        let faces = (!self.selection.faces.is_empty()).then(|| self.selection.faces.clone());
        let mut trial = self.work_model().clone();
        let healed = heal(&mut trial, faces.as_ref());
        if !healed.is_empty() {
            self.cancel();
            self.set_selection(Selection::default());
            self.run(Command::Heal { faces });
            self.refresh();
        }
        healed.words()
    }
}
