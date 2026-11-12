//! The Fillet window (brief 29): the Fillet / Chamfer tool's kind and size,
//! in the side column while the tool is active. The edge under the cursor
//! shows the result before the click.

use egui::{RichText, Ui};
use prism_tools::session::RoundOp;
use prism_ui::theme::{color, size};
use prism_ui::widgets::{SideCard, SoftButton, field};

use super::panels::{PanelData, PanelOp, Panels, after_card};

pub(super) fn card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let Some(r) = data.round.as_ref().filter(|_| p.fillet) else {
        return;
    };
    let resp = SideCard::new("Fillet")
        .subtitle(if r.chamfer {
            "Cut edges off flat"
        } else {
            "Round edges"
        })
        .show(ui, p.folded.contains("Fillet"), |ui| {
            ui.horizontal(|ui| {
                for (label, fillet, tip) in [
                    ("Fillet", true, "Round the edge"),
                    ("Chamfer", false, "Cut the edge off flat"),
                ] {
                    if ui
                        .add(SoftButton::new(label).small().selected(r.chamfer != fillet))
                        .on_hover_text(tip)
                        .clicked()
                    {
                        ops.push(PanelOp::Round(RoundOp::Fillet(fillet)));
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            ui.horizontal(|ui| {
                ui.label(if r.chamfer { "Distance" } else { "Radius" });
                // The field keeps what is typed while it has the keys, and
                // shows the tool's size otherwise.
                let mut text = p.fillet_size.clone().unwrap_or_else(|| r.size.clone());
                let hint = match &r.fits {
                    Some(f) => format!("Fit: {f}"),
                    None => "Fit each edge".to_owned(),
                };
                let f = field(ui, &mut text, &hint, 120.0);
                if f.has_focus() {
                    p.fillet_size = Some(text.clone());
                }
                if f.lost_focus() {
                    p.fillet_size = None;
                    if text.trim() != r.size {
                        ops.push(PanelOp::RoundSize(text));
                    }
                }
                if !r.size.is_empty()
                    && ui
                        .add(SoftButton::new("Fit").small())
                        .on_hover_text("Fit each edge (empty size)")
                        .clicked()
                {
                    p.fillet_size = None;
                    ops.push(PanelOp::RoundSize(String::new()));
                }
            });
            ui.add_space(size::MENU_PAD);
            if let Some(why) = r.refused {
                ui.label(RichText::new(why).color(color::INK));
            }
            if r.changes_last {
                muted(ui, "A new kind or size changes the last one.");
            }
            for line in r.how.split(" · ") {
                let mut c = line.chars();
                let line: String = c
                    .next()
                    .map(|f| f.to_uppercase().chain(c).collect())
                    .unwrap_or_default();
                muted(ui, &line);
            }
        });
    if after_card(ui, &mut p.folded, "Fillet", resp) {
        p.fillet = false;
    }
}

fn muted(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(color::MUTED));
}
