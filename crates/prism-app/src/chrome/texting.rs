//! The Text window (Annotation & Sections part 3): the words, the font
//! (one of three bundled), the capitals' height, and what it is made of.
//! In the side column while the Text tool is active; a click in the
//! viewport sets the text down.

use egui::{RichText, Ui};
use prism_text::Font;
use prism_tools::session::{TextKind, TextSet};
use prism_ui::theme::{color, size};
use prism_ui::widgets::{SideCard, SoftButton, field};

use super::panels::{PanelData, PanelOp, Panels, after_card};

const TITLE: &str = "Text";

pub(super) fn card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let Some(t) = data.text.as_ref().filter(|_| p.text) else {
        return;
    };
    let s = &t.settings;
    let resp =
        SideCard::new(TITLE)
            .subtitle(s.font.name())
            .show(ui, p.folded.contains(TITLE), |ui| {
                // The words: what is typed is kept while the box has the keys.
                let mut words = p.text_words.clone().unwrap_or_else(|| s.text.clone());
                let r = ui.add(
                    egui::TextEdit::multiline(&mut words)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text("The text"),
                );
                if r.changed() {
                    p.text_words = Some(words.clone());
                    ops.push(PanelOp::Text(TextSet::Text(words)));
                }
                if r.lost_focus() {
                    p.text_words = None;
                }
                ui.add_space(size::MENU_PAD);
                ui.horizontal(|ui| {
                    for font in Font::ALL {
                        if ui
                            .add(
                                SoftButton::new(font.word())
                                    .small()
                                    .selected(s.font == font),
                            )
                            .on_hover_text(font.name())
                            .clicked()
                        {
                            ops.push(PanelOp::Text(TextSet::Font(font)));
                        }
                    }
                });
                ui.add_space(size::MENU_PAD);
                length(
                    ui,
                    "Height",
                    &t.height,
                    &mut p.text_height,
                    "Capital letters' height",
                    |x| {
                        ops.push(PanelOp::Text(TextSet::Height(x)));
                    },
                );
                ui.add_space(size::MENU_PAD);
                ui.horizontal(|ui| {
                    for (label, kind, tip) in [
                        ("Outline", TextKind::Outline, "Edges only"),
                        ("Face", TextKind::Face, "Flat faces"),
                        ("Solid", TextKind::Solid, "Pulled out into solids"),
                    ] {
                        if ui
                            .add(SoftButton::new(label).small().selected(s.kind == kind))
                            .on_hover_text(tip)
                            .clicked()
                        {
                            ops.push(PanelOp::Text(TextSet::Kind(kind)));
                        }
                    }
                });
                if s.kind == TextKind::Solid {
                    ui.add_space(size::MENU_PAD);
                    length(
                        ui,
                        "Depth",
                        &t.depth,
                        &mut p.text_depth,
                        "How far the letters stand out",
                        |x| {
                            ops.push(PanelOp::Text(TextSet::Depth(x)));
                        },
                    );
                }
                ui.add_space(size::MENU_PAD);
                if t.missing > 0 {
                    ui.label(
                        RichText::new(match t.missing {
                            1 => "1 character is not in this font".to_owned(),
                            n => format!("{n} characters are not in this font"),
                        })
                        .color(color::INK),
                    );
                }
                for line in t.how.split(" · ") {
                    ui.label(RichText::new(line).color(color::MUTED));
                }
            });
    if after_card(ui, &mut p.folded, TITLE, resp) {
        p.text = false;
    }
}

/// A labelled length field: what is typed is kept while it has the keys
/// and sent when it loses them.
fn length(
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
