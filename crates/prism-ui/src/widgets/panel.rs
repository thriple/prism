use egui::{Area, Context, Frame, Id, InnerResponse, Order, Pos2, RichText, Shape, Ui};

use crate::emboss;
use crate::theme::{self, color, radius, size};

/// A floating, movable panel: a raised card with a title (kit `DockPanel`
/// look, free-floating for now). Drag it by any empty part.
pub struct SoftPanel {
    title: String,
    default_pos: Pos2,
    width: f32,
}

impl SoftPanel {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            default_pos: Pos2::ZERO,
            width: size::PANEL_WIDTH,
        }
    }

    pub fn default_pos(mut self, pos: impl Into<Pos2>) -> Self {
        self.default_pos = pos.into();
        self
    }

    /// The width of the content (the panel adds its padding around it).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn show<R>(
        self,
        ctx: &Context,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> InnerResponse<R> {
        Area::new(Id::unique(("soft-panel", &self.title)))
            .default_pos(self.default_pos)
            .movable(true)
            .constrain(true)
            .order(Order::Middle)
            .show(ctx, |ui| {
                // Reserve the background's place, then paint it once the size is known.
                let background = ui.painter().add(Shape::Noop);
                let inner = Frame::new().inner_margin(size::PANEL_PAD).show(ui, |ui| {
                    ui.set_width(self.width);
                    ui.label(
                        RichText::new(&self.title)
                            .font(theme::title())
                            .color(color::INK),
                    );
                    ui.add_space(size::PANEL_TITLE_GAP - ui.spacing().item_spacing.y);
                    add_contents(ui)
                });
                let mut shapes = Vec::new();
                emboss::floating(
                    &mut shapes,
                    inner.response.rect,
                    radius::CARD,
                    color::SURFACE,
                    1.0,
                );
                ui.painter().set(background, Shape::Vec(shapes));
                inner.inner
            })
    }
}
