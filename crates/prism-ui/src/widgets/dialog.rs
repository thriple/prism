//! A dialog (prototype `.fcard`): a floating card over a dimmed app, with a
//! title, an optional line under it, and a close button.

use egui::{Align2, Area, Context, Id, Order, Rect, Sense, Shape, Ui, pos2, vec2};

use crate::anim;
use crate::emboss;
use crate::spring;
use crate::theme::{self, color, motion, radius, size, text};

pub struct SoftDialog<'a> {
    title: &'a str,
    subtitle: Option<String>,
    width: f32,
}

/// What happened to a dialog this frame.
pub struct DialogResponse<R> {
    pub inner: Option<R>,
    /// The close button or the dimmed area outside was clicked.
    pub close: bool,
}

impl<'a> SoftDialog<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            subtitle: None,
            width: size::DIALOG_WIDTH,
        }
    }

    pub fn subtitle(mut self, s: impl Into<String>) -> Self {
        self.subtitle = Some(s.into());
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// Show it while `open`; it pops in and fades out.
    pub fn show<R>(self, ctx: &Context, open: bool, add: impl FnOnce(&mut Ui) -> R) -> DialogResponse<R> {
        let id = Id::unique(("soft-dialog", self.title));
        let t = anim::spring(ctx, id.with("shown"), if open { 1.0 } else { 0.0 }, spring::SPRING_SNAPPY);
        let mut close = false;
        if t < 0.01 && !open {
            return DialogResponse { inner: None, close };
        }
        let shown = t.clamp(0.0, 1.0);
        let screen = ctx.viewport_rect();

        // The dimmed app behind: a click there closes the dialog.
        let scrim = Area::new(id.with("scrim"))
            .order(Order::Foreground)
            .fixed_pos(screen.min)
            .interactable(open)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(r, 0.0, theme::fade(color::SCRIM, shown));
                resp
            });
        if scrim.inner.clicked() {
            close = true;
        }

        let [px, py] = size::DIALOG_PAD;
        let max_h = screen.height() * size::DIALOG_MAX_HEIGHT;
        let left = screen.center().x - self.width * 0.5;
        let top = screen.top() + (screen.height() - max_h) * 0.5;
        let rise = (1.0 - shown) * motion::POP_RISE;
        let area = Area::new(id)
            .order(Order::Foreground)
            .fixed_pos(pos2(left, top + rise))
            .interactable(open)
            .show(ctx, |ui| {
                ui.multiply_opacity(shown);
                let bg = ui.painter().add(Shape::Noop);
                let inner = egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(px as i8, py as i8))
                    .show(ui, |ui| {
                        ui.set_width(self.width - 2.0 * px);
                        ui.set_max_height(max_h - 2.0 * py);
                        let head = ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(self.title)
                                        .font(egui::FontId::new(text::DIALOG_TITLE, theme::title().family))
                                        .color(color::INK),
                                );
                                if let Some(s) = &self.subtitle {
                                    ui.label(egui::RichText::new(s).font(theme::mono()).color(color::MUTED));
                                }
                            });
                        });
                        let close_rect = Rect::from_min_size(
                            pos2(head.response.rect.left() + self.width - 2.0 * px - size::CLOSE_BUTTON, head.response.rect.top()),
                            vec2(size::CLOSE_BUTTON, size::CLOSE_BUTTON),
                        );
                        let close_resp = ui.interact(close_rect, id.with("close"), Sense::click());
                        let hot = close_resp.hovered();
                        if hot {
                            let mut v = Vec::new();
                            emboss::inset(&mut v, close_rect, radius::MENU_BUTTON, theme::depth::MENU_INSET);
                            ui.painter().extend(v);
                        }
                        ui.painter().text(close_rect.center(), Align2::CENTER_CENTER, "×", theme::title(), color::INK);
                        if close_resp.clicked() {
                            close = true;
                        }
                        add(ui)
                    });
                let mut shapes = Vec::new();
                emboss::floating(&mut shapes, inner.response.rect, radius::CARD, color::SURFACE, 1.0);
                ui.painter().set(bg, Shape::Vec(shapes));
                inner.inner
            });
        DialogResponse {
            inner: Some(area.inner),
            close,
        }
    }
}
