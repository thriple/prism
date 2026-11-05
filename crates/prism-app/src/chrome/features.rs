//! The Planned Features window: every action, grouped like the menus, with
//! built ones ticked and crossed out (prototype `openFeatures`).

use egui::{Context, RichText, ScrollArea, Sense, vec2};
use prism_ui::emboss;
use prism_ui::theme::{self, color, depth, radius, size};
use prism_ui::widgets::{SoftDialog, menu};

use super::Chrome;
use crate::actions::registry::planned_features;

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let groups = planned_features(&chrome.reg);
    let total: usize = groups.iter().map(|(_, f)| f.len()).sum();
    let built: usize = groups
        .iter()
        .flat_map(|(_, f)| f)
        .filter(|f| f.built)
        .count();
    let r = SoftDialog::new("Planned features")
        .subtitle(format!("{built} of {total} built"))
        .show(ctx, chrome.features_open, |ui| {
            // Progress: an inset well with an accent fill.
            let w = ui.available_width();
            let (bar, _) = ui.allocate_exact_size(vec2(w, size::PROGRESS_HEIGHT), Sense::hover());
            let mut v = Vec::new();
            emboss::soft(
                &mut v,
                bar,
                radius::BAR,
                -1.0,
                depth::BAR_WELL,
                color::SURFACE,
            );
            ui.painter().extend(v);
            let fill = bar.with_max_x(bar.left() + w * built as f32 / total.max(1) as f32);
            ui.painter().rect_filled(fill, radius::BAR, color::ACCENT);
            ScrollArea::vertical()
                .max_width(w)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (group, feats) in &groups {
                        ui.add_space(size::MENU_PAD);
                        ui.horizontal(|ui| {
                            ui.label(menu::caps(group, color::MUTED));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let done = feats.iter().filter(|f| f.built).count();
                                    ui.label(
                                        RichText::new(format!("{done}/{}", feats.len()))
                                            .font(theme::mono())
                                            .color(color::MUTED),
                                    );
                                },
                            );
                        });
                        let name_w = w - 2.0 * size::MENU_CHECK;
                        for f in feats {
                            ui.horizontal_top(|ui| {
                                let tick = if f.built { "✓" } else { " " };
                                ui.add_sized(
                                    vec2(size::MENU_CHECK, size::MENU_ROW_HEIGHT * 0.6),
                                    egui::Label::new(RichText::new(tick).color(color::ACCENT)),
                                );
                                let text = RichText::new(f.name);
                                let text = if f.built {
                                    text.strikethrough().color(color::MUTED)
                                } else {
                                    text.color(color::INK)
                                };
                                // A fixed width, so long lines wrap instead of widening the window.
                                ui.allocate_ui_with_layout(
                                    vec2(name_w, 0.0),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        ui.set_width(name_w);
                                        ui.add(egui::Label::new(text).wrap());
                                    },
                                );
                            });
                        }
                    }
                });
        });
    if r.close {
        chrome.features_open = false;
    }
}
