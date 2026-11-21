//! The Array window (Shapes & Placement part 2): linear or polar, how many,
//! the step or the angle, and the axis. In the side column while the Array
//! tool is active; a ghost shows the result and Make Array does it.

use egui::{RichText, Ui};
use prism_tools::session::{ArrayKind, ArraySet};
use prism_ui::theme::{color, size};
use prism_ui::widgets::{SideCard, SoftButton, field};

use super::panels::{PanelData, PanelOp, Panels, after_card};

const TITLE: &str = "Array";

pub(super) fn card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let Some(a) = data.array.as_ref().filter(|_| p.array) else {
        return;
    };
    let s = &a.settings;
    let resp = SideCard::new(TITLE)
        .subtitle(match s.kind {
            ArrayKind::Linear => "In a row",
            ArrayKind::Polar => "Round an axis",
        })
        .show(ui, p.folded.contains(TITLE), |ui| {
            ui.horizontal(|ui| {
                for (label, kind) in [("Linear", ArrayKind::Linear), ("Polar", ArrayKind::Polar)] {
                    if ui
                        .add(SoftButton::new(label).small().selected(s.kind == kind))
                        .clicked()
                    {
                        ops.push(PanelOp::Array(ArraySet::Kind(kind)));
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            typed(
                ui,
                "Count",
                &a.count,
                &mut p.array_count,
                "How many there are in all, the original included",
                |t| ops.push(PanelOp::Array(ArraySet::Count(t))),
            );
            ui.add_space(size::MENU_PAD);
            match s.kind {
                ArrayKind::Linear => typed(
                    ui,
                    "Spacing",
                    &a.spacing,
                    &mut p.array_spacing,
                    "The step from one to the next, along the axis",
                    |t| ops.push(PanelOp::Array(ArraySet::Spacing(t))),
                ),
                ArrayKind::Polar => typed(
                    ui,
                    "Angle",
                    &a.angle,
                    &mut p.array_angle,
                    "Degrees all the copies spread over; 360 is a full circle",
                    |t| ops.push(PanelOp::Array(ArraySet::Angle(t))),
                ),
            }
            ui.add_space(size::MENU_PAD);
            ui.horizontal(|ui| {
                ui.label(match s.kind {
                    ArrayKind::Linear => "Along",
                    ArrayKind::Polar => "About",
                });
                for (label, axis) in [("Red", 0), ("Green", 1), ("Blue", 2)] {
                    if ui
                        .add(SoftButton::new(label).small().selected(s.axis == axis))
                        .clicked()
                    {
                        ops.push(PanelOp::Array(ArraySet::Axis(axis)));
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            if ui.add(SoftButton::new("Make Array").small()).clicked() {
                ops.push(PanelOp::Array(ArraySet::Make));
            }
            ui.add_space(size::MENU_PAD);
            ui.label(RichText::new(a.how).color(color::MUTED));
        });
    if after_card(ui, &mut p.folded, TITLE, resp) {
        p.array = false;
    }
}

/// A labelled field: what is typed is kept while it has the keys and sent
/// when it loses them.
fn typed(
    ui: &mut Ui,
    label: &str,
    shown: &str,
    typing: &mut Option<String>,
    tip: &str,
    mut send: impl FnMut(String),
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut text = typing.clone().unwrap_or_else(|| shown.to_owned());
        let f = field(ui, &mut text, shown, 120.0).on_hover_text(tip);
        if f.has_focus() {
            *typing = Some(text.clone());
        }
        if f.lost_focus() {
            *typing = None;
            if text.trim() != shown {
                send(text);
            }
        }
    });
}
