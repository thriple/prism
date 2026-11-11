//! A colour swatch (the Materials window): a rounded square of the
//! colour, ringed in the accent when it is the one chosen.

use egui::{Color32, Response, Sense, StrokeKind, Ui, vec2};

use crate::theme::{color, radius, size};

/// A clickable swatch of `colour`; `chosen` rings it.
pub fn swatch(ui: &mut Ui, colour: Color32, chosen: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size::SWATCH, size::SWATCH), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, radius::MENU_BUTTON, colour);
    p.rect_stroke(
        rect,
        radius::MENU_BUTTON,
        color::swatch_edge(),
        StrokeKind::Inside,
    );
    if chosen || response.hovered() {
        p.rect_stroke(
            rect.expand(size::SWATCH_RING),
            radius::MENU_BUTTON + size::SWATCH_RING,
            color::swatch_ring(chosen),
            StrokeKind::Outside,
        );
    }
    response
}
