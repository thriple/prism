use egui::{Response, Sense, Ui, vec2};

use crate::emboss;
use crate::theme::{color, depth, radius, size};

/// A divider: a short inset line across the available width.
pub fn groove(ui: &mut Ui) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), size::GROOVE_HEIGHT),
        Sense::hover(),
    );
    if ui.is_rect_visible(rect) {
        let mut shapes = Vec::new();
        emboss::soft(
            &mut shapes,
            rect,
            radius::ROUND,
            -1.0,
            depth::GROOVE,
            color::SURFACE,
        );
        super::paint(ui.painter(), shapes);
    }
    response
}
