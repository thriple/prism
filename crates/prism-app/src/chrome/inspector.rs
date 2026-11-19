//! The Inspector window (PLAN 1.17, brief 48): what is selected, and its
//! fields. The session describes the fields ([`Inspector`]) and takes the
//! typed text back; this only draws them.

use egui::{RichText, Ui};
use prism_tools::session::{Field, FieldKind};
use prism_ui::theme::{color, size};
use prism_ui::widgets::{SideCard, SoftButton, field};

use super::panels::{PanelData, PanelOp, Panels, after_card};

pub(super) fn card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    if !p.inspector {
        return;
    }
    let i = &data.inspector;
    let r = SideCard::new("Inspector").show(ui, p.folded.contains("Inspector"), |ui| {
        ui.label(RichText::new(&i.summary).color(color::INK));
        if !i.path.is_empty() {
            ui.label(RichText::new(i.path.join(" › ")).color(color::MUTED));
        }
        for section in &i.sections {
            ui.add_space(size::MENU_PAD);
            ui.label(RichText::new(section.title).color(color::MUTED));
            for f in &section.fields {
                row(ui, p, f, ops);
            }
        }
        if let Some(why) = &p.inspector_error {
            ui.add_space(size::MENU_PAD);
            ui.label(RichText::new(why).color(color::INK));
        }
    });
    if after_card(ui, &mut p.folded, "Inspector", r) {
        p.inspector = false;
    }
}

fn row(ui: &mut Ui, p: &mut Panels, f: &Field, ops: &mut Vec<PanelOp>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(&f.label);
        match &f.kind {
            FieldKind::Text | FieldKind::Length => {
                // What is being typed is kept while the field has the keys.
                let mut text = match &p.inspector_edit {
                    Some((k, t)) if *k == f.key => t.clone(),
                    _ => f.text.clone(),
                };
                let hint = if f.mixed { "—" } else { "" };
                let w = field(ui, &mut text, hint, 140.0);
                if w.has_focus() {
                    let changed = w.changed() && text.trim() != f.text;
                    p.inspector_edit = Some((f.key.clone(), text.clone()));
                    // A feature's size shows on the body as it is typed.
                    if changed && f.key.starts_with("feature.") {
                        ops.push(PanelOp::InspectPreview(f.key.clone(), text.clone()));
                    }
                }
                if w.lost_focus() {
                    p.inspector_edit = None;
                    if f.key.starts_with("feature.") {
                        ops.push(PanelOp::InspectPreviewEnd);
                    }
                    if text.trim() != f.text && !text.trim().is_empty() {
                        ops.push(PanelOp::Inspect(f.key.clone(), text));
                    }
                }
            }
            FieldKind::Toggle => {
                let on = !f.mixed && f.text == "true";
                let label = if f.mixed {
                    "—"
                } else if on {
                    "Yes"
                } else {
                    "No"
                };
                if ui
                    .add(SoftButton::new(label).small().selected(on))
                    .clicked()
                {
                    ops.push(PanelOp::Inspect(f.key.clone(), (!on).to_string()));
                }
            }
            FieldKind::Choice(options) => {
                for o in options {
                    if ui
                        .add(
                            SoftButton::new(o)
                                .small()
                                .selected(!f.mixed && *o == f.text),
                        )
                        .clicked()
                    {
                        ops.push(PanelOp::Inspect(f.key.clone(), o.clone()));
                    }
                }
            }
            FieldKind::Action => {
                if ui.add(SoftButton::new(&f.text).small()).clicked() {
                    ops.push(PanelOp::Inspect(f.key.clone(), String::new()));
                }
            }
            FieldKind::Fixed => {
                let text = if f.mixed { "—" } else { &f.text };
                ui.label(RichText::new(text).color(color::MUTED));
            }
        }
    });
}
