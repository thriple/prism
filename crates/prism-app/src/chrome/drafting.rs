//! The Draft Angle window (Shapes & Placement part 6): the angle in
//! degrees. In the side column while the Draft Angle tool is active.

use egui::{RichText, Ui};
use prism_tools::session::DraftSet;
use prism_ui::theme::{color, size};
use prism_ui::widgets::{SideCard, field};

use super::panels::{PanelData, PanelOp, Panels, after_card};

const TITLE: &str = "Draft Angle";

pub(super) fn card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let Some(a) = data.draft.as_ref().filter(|_| p.draft) else {
        return;
    };
    let resp = SideCard::new(TITLE)
        .subtitle("Lean the selected faces")
        .show(ui, p.folded.contains(TITLE), |ui| {
            ui.horizontal(|ui| {
                ui.label("Angle (degrees)");
                let mut text = p.draft_typing.clone().unwrap_or_else(|| a.angle.clone());
                let f = field(ui, &mut text, &a.angle, 90.0).on_hover_text(
                    "Positive: narrower away from the neutral face. Negative: wider. Up to 45.",
                );
                if f.has_focus() {
                    p.draft_typing = Some(text.clone());
                }
                if f.lost_focus() {
                    p.draft_typing = None;
                    if text.trim() != a.angle {
                        ops.push(PanelOp::Draft(DraftSet::Angle(text)));
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            ui.label(RichText::new(a.how).color(color::MUTED));
        });
    if after_card(ui, &mut p.folded, TITLE, resp) {
        p.draft = false;
    }
}
