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
            FieldKind::Text | FieldKind::Length | FieldKind::Suggest(_) => {
                // What is being typed is kept while the field has the keys.
                let mut text = match &p.inspector_edit {
                    Some((k, t)) if *k == f.key => t.clone(),
                    _ => f.text.clone(),
                };
                let hint = if f.mixed { "—" } else { "" };
                let w = field(ui, &mut text, hint, 140.0);
                // Asked for the keys (a new component's name): in the
                // field with its text selected, once.
                if p.inspector_focus.as_deref() == Some(f.key.as_str()) {
                    p.inspector_focus = None;
                    w.request_focus();
                    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), w.id) {
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(text.chars().count()),
                            )));
                        state.store(ui.ctx(), w.id);
                    }
                }
                if w.has_focus() {
                    let changed = w.changed() && text.trim() != f.text;
                    p.inspector_edit = Some((f.key.clone(), text.clone()));
                    // A feature's size shows on the body as it is typed.
                    if changed && f.key.starts_with("feature.") {
                        ops.push(PanelOp::InspectPreview(f.key.clone(), text.clone()));
                    }
                }
                // Esc leaves the field and drops what was typed (a new
                // component keeps its default name).
                if w.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    p.inspector_edit = None;
                    if f.key == "name" {
                        ops.push(PanelOp::NameCancelled);
                    }
                    if f.key.starts_with("feature.") {
                        ops.push(PanelOp::InspectPreviewEnd);
                    }
                } else if w.lost_focus() {
                    p.inspector_edit = None;
                    if f.key.starts_with("feature.") {
                        ops.push(PanelOp::InspectPreviewEnd);
                    }
                    // Empty takes a tag off; for any other field it is no change.
                    if text.trim() != f.text && (!text.trim().is_empty() || f.key == "tag") {
                        ops.push(PanelOp::Inspect(f.key.clone(), text));
                    }
                }
                if let FieldKind::Suggest(options) = &f.kind {
                    // Tags: a click puts the tag on or takes it off, leaving
                    // the others; None takes them all off.
                    let tags = f.key == "tag";
                    for o in options {
                        let on = !f.mixed
                            && if tags {
                                f.text.split(", ").any(|t| t == o)
                            } else {
                                *o == f.text
                            };
                        if ui.add(SoftButton::new(o).small().selected(on)).clicked() {
                            p.inspector_edit = None;
                            let key = if tags && o != "None" {
                                "tag.toggle"
                            } else {
                                &f.key
                            };
                            ops.push(PanelOp::Inspect(key.to_owned(), o.clone()));
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn name_field() -> Field {
        Field {
            key: "name".into(),
            label: "Name".into(),
            kind: FieldKind::Text,
            text: "Component".into(),
            mixed: false,
        }
    }

    fn frame(
        ctx: &egui::Context,
        p: &mut Panels,
        ops: &mut Vec<PanelOp>,
        events: Vec<egui::Event>,
    ) {
        let input = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(600.0, 400.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| row(ui, p, &name_field(), ops));
        out.textures_delta.clear();
    }

    fn key(key: egui::Key) -> Vec<egui::Event> {
        [true, false]
            .map(|pressed| egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            })
            .to_vec()
    }

    #[test]
    fn a_new_components_name_field_takes_the_keys_with_its_text_selected() {
        let ctx = egui::Context::default();
        let mut p = Panels {
            inspector_focus: Some("name".into()),
            ..Default::default()
        };
        let mut ops = Vec::new();
        frame(&ctx, &mut p, &mut ops, vec![]);
        frame(&ctx, &mut p, &mut ops, vec![]);
        assert!(p.inspector_focus.is_none(), "asked once");
        assert!(
            ctx.memory(|m| m.focused()).is_some(),
            "the field has the keys"
        );
        // Typing replaces the selected default, and Enter finishes.
        frame(
            &ctx,
            &mut p,
            &mut ops,
            vec![egui::Event::Text("Wall".into())],
        );
        frame(&ctx, &mut p, &mut ops, key(egui::Key::Enter));
        frame(&ctx, &mut p, &mut ops, vec![]);
        assert!(
            ops.iter()
                .any(|o| matches!(o, PanelOp::Inspect(k, t) if k == "name" && t == "Wall")),
            "{}",
            ops.len()
        );
    }

    #[test]
    fn esc_in_the_name_field_keeps_the_default_name() {
        let ctx = egui::Context::default();
        let mut p = Panels {
            inspector_focus: Some("name".into()),
            ..Default::default()
        };
        let mut ops = Vec::new();
        frame(&ctx, &mut p, &mut ops, vec![]);
        frame(&ctx, &mut p, &mut ops, vec![]);
        frame(
            &ctx,
            &mut p,
            &mut ops,
            vec![egui::Event::Text("Wall".into())],
        );
        frame(&ctx, &mut p, &mut ops, key(egui::Key::Escape));
        frame(&ctx, &mut p, &mut ops, vec![]);
        assert!(ops.iter().any(|o| matches!(o, PanelOp::NameCancelled)));
        assert!(!ops.iter().any(|o| matches!(o, PanelOp::Inspect(..))));
    }
}
