//! The Hole window (Shapes & Placement part 6): the kind (plain,
//! counterbore, countersink), whether it goes through, and its sizes. In
//! the side column while the Hole tool is active.

use egui::{RichText, Ui};
use prism_tools::session::HoleSet;
use prism_ui::theme::{color, size};
use prism_ui::widgets::{SideCard, SoftButton, field};

use super::panels::{PanelData, PanelOp, Panels, after_card};

const TITLE: &str = "Hole";

pub(super) fn card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let Some(a) = data.hole.as_ref().filter(|_| p.hole) else {
        return;
    };
    let resp = SideCard::new(TITLE)
        .subtitle(a.kinds.get(a.current).copied().unwrap_or(""))
        .show(ui, p.folded.contains(TITLE), |ui| {
            ui.horizontal(|ui| {
                for (i, name) in a.kinds.iter().enumerate() {
                    if ui
                        .add(SoftButton::new(*name).small().selected(a.current == i))
                        .clicked()
                    {
                        p.hole_typing = None;
                        ops.push(PanelOp::Hole(HoleSet::Kind(i)));
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            if ui
                .add(SoftButton::new("Through").small().selected(a.through))
                .on_hover_text("Goes all the way through the solid")
                .clicked()
            {
                ops.push(PanelOp::Hole(HoleSet::Through(!a.through)));
            }
            ui.add_space(size::MENU_PAD);
            for (key, label, shown) in &a.fields {
                ui.horizontal(|ui| {
                    ui.label(*label);
                    let mine = p.hole_typing.as_ref().filter(|(k, _)| k == key);
                    let mut text = mine.map_or_else(|| shown.clone(), |(_, t)| t.clone());
                    let f = field(ui, &mut text, shown, 110.0)
                        .on_hover_text("A size: type it with units if you like, then Enter");
                    if f.has_focus() {
                        p.hole_typing = Some((*key, text.clone()));
                    }
                    if f.lost_focus() {
                        p.hole_typing = None;
                        if text.trim() != shown {
                            ops.push(PanelOp::Hole(HoleSet::Size(key, text)));
                        }
                    }
                });
                ui.add_space(size::MENU_PAD / 2.0);
            }
            ui.add_space(size::MENU_PAD / 2.0);
            ui.label(RichText::new(a.how).color(color::MUTED));
        });
    if after_card(ui, &mut p.folded, TITLE, resp) {
        p.hole = false;
    }
}
