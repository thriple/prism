//! A keycap (kit `Kbd`): a small raised key with its name.

use egui::{Response, Sense, Ui, vec2};

use crate::emboss;
use crate::theme::{color, depth, radius, size};

pub fn keycap(ui: &mut Ui, key: &str) -> Response {
    let font = super::menu::key_font();
    let galley = ui
        .painter()
        .layout_no_wrap(key.to_owned(), font, color::INK);
    let [px, py] = size::KEYCAP_PAD;
    let want = galley.size() + vec2(2.0 * px, 2.0 * py);
    let (rect, response) = ui.allocate_exact_size(want, Sense::hover());
    if ui.is_rect_visible(rect) {
        let mut shapes = Vec::new();
        emboss::soft(
            &mut shapes,
            rect,
            radius::KEYCAP,
            1.0,
            depth::KNOB * 0.5,
            color::SURFACE,
        );
        ui.painter().extend(shapes);
        ui.painter()
            .galley(rect.min + vec2(px, py), galley, color::INK);
    }
    response
}
