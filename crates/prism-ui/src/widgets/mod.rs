//! Soft controls, re-implemented from the kit's `components/soft`.

mod button;
mod dialog;
mod field;
mod groove;
mod kbd;
pub mod menu;
mod panel;
mod select;
mod slider;
mod toggle;
mod tool_button;
pub mod tooltip;

pub use button::SoftButton;
pub use dialog::{DialogResponse, SoftDialog};
pub use field::field;
pub use kbd::keycap;
pub use tool_button::ToolButton;
pub use groove::groove;
pub use panel::SoftPanel;
pub use select::SoftSelect;
pub use slider::SoftSlider;
pub use toggle::SoftToggle;

use egui::{Painter, Rect, Shape};

/// Paint shapes built by the emboss helpers.
pub(crate) fn paint(painter: &Painter, shapes: Vec<Shape>) {
    painter.extend(shapes);
}

/// Dim a whole control when it cannot be used.
pub(crate) fn opacity(enabled: bool) -> f32 {
    if enabled {
        1.0
    } else {
        crate::theme::color::DISABLED_OPACITY
    }
}

/// The focus ring, when the control has keyboard focus.
pub(crate) fn focus(painter: &Painter, response: &egui::Response, rect: Rect, radius: f32) {
    if response.has_focus() {
        let mut v = Vec::new();
        crate::emboss::focus_ring(&mut v, rect, radius);
        paint(painter, v);
    }
}
