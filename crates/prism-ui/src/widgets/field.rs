//! A text field sunk into the surface.

use egui::{Response, TextEdit, Ui};

use crate::emboss;
use crate::theme::{self, color, depth, radius, size};

/// A single-line field. `hint` shows while it is empty.
pub fn field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let edit = TextEdit::singleline(text)
        .hint_text(egui::RichText::new(hint).color(color::MUTED))
        .font(theme::body())
        .text_color(color::INK)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::symmetric(size::FIELD_PAD as i8, 0))
        .align(egui::Align2::LEFT_CENTER)
        .min_size(egui::vec2(width, size::FIELD_HEIGHT))
        .desired_width(width);
    let bg = ui.painter().add(egui::Shape::Noop);
    let response = ui.add(edit);
    let mut shapes = Vec::new();
    emboss::soft(&mut shapes, response.rect, radius::MENU_BUTTON, -1.0, depth::FIELD_WELL, color::FROST);
    if response.has_focus() {
        emboss::focus_ring(&mut shapes, response.rect, radius::MENU_BUTTON);
    }
    ui.painter().set(bg, egui::Shape::Vec(shapes));
    response
}
