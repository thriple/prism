//! A window docked in the side column (Outliner, Model Health, ...): a
//! raised card with a title, an arrow in its corner that folds it to its
//! title, and a close button. Nothing behind it is dimmed.

use egui::{Align2, Rect, RichText, Sense, Shape, Ui, pos2, vec2};

use super::select::chevron_shape;
use crate::emboss;
use crate::theme::{self, color, depth, radius, size};

pub struct SideCard<'a> {
    title: &'a str,
    subtitle: Option<String>,
}

/// What was clicked in the card's corner this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SideCardResponse {
    /// The arrow (or the title): fold or unfold.
    pub toggle: bool,
    pub close: bool,
}

impl<'a> SideCard<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            subtitle: None,
        }
    }

    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        self.subtitle = Some(s.into());
        self
    }

    /// Show the card across the column's width; `add` draws the contents
    /// unless it is folded.
    pub fn show(self, ui: &mut Ui, folded: bool, add: impl FnOnce(&mut Ui)) -> SideCardResponse {
        let id = egui::Id::unique(("side-card", self.title));
        let mut out = SideCardResponse::default();
        let width = ui.available_width();
        let pad = size::PANEL_PAD;
        let bg = ui.painter().add(Shape::Noop);
        let inner = egui::Frame::new()
            .inner_margin(egui::Margin::same(pad as i8))
            .show(ui, |ui| {
                ui.set_width(width - 2.0 * pad);
                let head = ui.vertical(|ui| {
                    ui.label(
                        RichText::new(self.title)
                            .font(theme::title())
                            .color(color::INK),
                    );
                    if let Some(s) = &self.subtitle {
                        ui.label(RichText::new(s).font(theme::mono()).color(color::MUTED));
                    }
                });
                // The corner: the fold arrow, then the close button.
                let b = size::CLOSE_BUTTON;
                let top = head.response.rect.top() - (b - theme::title().size) * 0.5;
                let right = ui.max_rect().right();
                let close_rect = Rect::from_min_size(pos2(right - b, top), vec2(b, b));
                let fold_rect = close_rect.translate(vec2(-b, 0.0));
                let title_resp = ui.interact(head.response.rect, id.with("title"), Sense::click());
                let fold = ui.interact(fold_rect, id.with("fold"), Sense::click());
                let close = ui.interact(close_rect, id.with("close"), Sense::click());
                for (r, resp) in [(fold_rect, &fold), (close_rect, &close)] {
                    if resp.hovered() {
                        let mut v = Vec::new();
                        emboss::inset(&mut v, r, radius::MENU_BUTTON, depth::MENU_INSET);
                        ui.painter().extend(v);
                    }
                }
                let angle = if folded {
                    -std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                };
                ui.painter().add(chevron_shape(fold_rect.center(), angle));
                ui.painter().text(
                    close_rect.center(),
                    Align2::CENTER_CENTER,
                    "×",
                    theme::title(),
                    color::INK,
                );
                out.toggle = fold
                    .on_hover_text(if folded { "Unfold" } else { "Fold" })
                    .clicked()
                    || title_resp.clicked();
                out.close = close.on_hover_text("Close").clicked();
                if !folded {
                    ui.add_space(size::PANEL_TITLE_GAP - ui.spacing().item_spacing.y);
                    add(ui);
                }
            });
        let mut shapes = Vec::new();
        emboss::floating(
            &mut shapes,
            inner.response.rect,
            radius::CARD,
            color::SURFACE,
            1.0,
        );
        ui.painter().set(bg, Shape::Vec(shapes));
        out
    }
}
