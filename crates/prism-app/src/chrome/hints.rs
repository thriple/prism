//! Help > Tool Hint Panel (brief 33): a side window that says what the
//! active tool does, the steps to use it, and what it wants now.

use egui::{RichText, Ui};
use prism_ui::theme::{self, color, size};
use prism_ui::widgets::SideCard;

use super::Chrome;
use super::panels::{Panels, after_card};

pub(super) const TITLE: &str = "Tool Hints";

pub(super) fn card(chrome: &Chrome, ui: &mut Ui, p: &mut Panels) {
    if !p.hints {
        return;
    }
    let tool = chrome.active_tool;
    let name = chrome.reg.get(tool).map_or(tool, |a| a.name);
    let guide = prism_tools::guides::guide(tool);
    let resp = SideCard::new(TITLE)
        .subtitle(name)
        .show(ui, p.folded.contains(TITLE), |ui| {
            let Some(g) = guide else {
                ui.label(RichText::new("Pick a tool to see how to use it.").color(color::MUTED));
                return;
            };
            ui.add(egui::Label::new(RichText::new(g.what).color(color::INK)).wrap());
            ui.add_space(size::MENU_PAD);
            for (k, step) in g.steps.iter().enumerate() {
                ui.horizontal_top(|ui| {
                    ui.label(
                        RichText::new(format!("{}.", k + 1))
                            .font(theme::body_strong())
                            .color(color::MUTED),
                    );
                    ui.add(egui::Label::new(RichText::new(*step).color(color::INK)).wrap());
                });
            }
            ui.add_space(size::MENU_PAD);
            ui.label(
                RichText::new("Now")
                    .font(theme::small())
                    .color(color::MUTED),
            );
            ui.add(egui::Label::new(RichText::new(&chrome.status).color(color::INK)).wrap());
        });
    if after_card(ui, &mut p.folded, TITLE, resp) {
        p.hints = false;
    }
}
