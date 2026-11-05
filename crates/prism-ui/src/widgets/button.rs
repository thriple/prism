use egui::{Align2, Response, Sense, Ui, Widget, vec2};

use crate::anim;
use crate::emboss;
use crate::spring;
use crate::theme::{self, color, depth, motion, radius, size};

/// Raised soft button (kit `SoftButton`). Hover lifts it, press sinks it into
/// the surface, release springs it back up past rest. The label rides the
/// press a beat behind the surface.
pub struct SoftButton {
    text: String,
    enabled: bool,
}

impl SoftButton {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            enabled: true,
        }
    }

    /// Greyed out: shown dimmed and cannot be pressed.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for SoftButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let font = theme::body_strong();
        let galley = ui
            .painter()
            .layout_no_wrap(self.text.clone(), font, color::INK);
        let want = vec2(
            galley.size().x + 2.0 * size::BUTTON_PAD_X,
            size::BUTTON_HEIGHT,
        );
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(want, sense);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, self.enabled, &self.text)
        });

        let key_down = response.has_focus()
            && ui.input(|i| i.key_down(egui::Key::Space) || i.key_down(egui::Key::Enter));
        let down = self.enabled && (response.is_pointer_button_down_on() || key_down);
        let target = if down {
            depth::PRESSED
        } else if self.enabled && response.hovered() {
            depth::HOVER
        } else {
            depth::REST
        };
        let feel = if down { spring::PRESS } else { spring::RELEASE };
        let d = anim::spring(ui.ctx(), response.id.with("depth"), target, feel);
        let sink = anim::spring(
            ui.ctx(),
            response.id.with("label"),
            if down { 1.0 } else { 0.0 },
            spring::LABEL,
        );

        if ui.is_rect_visible(rect) {
            let mut painter = ui.painter().clone();
            painter.multiply_opacity(super::opacity(self.enabled));
            let mut shapes = Vec::new();
            emboss::soft(
                &mut shapes,
                rect,
                radius::BUTTON,
                d,
                depth::BUTTON,
                color::SURFACE,
            );
            super::paint(&painter, shapes);
            let scale = 1.0 - (1.0 - motion::LABEL_SCALE) * sink;
            let centre = rect.center() + vec2(0.0, motion::LABEL_SINK * sink);
            let font = egui::FontId::new(theme::text::BODY * scale, theme::body_strong().family);
            painter.text(centre, Align2::CENTER_CENTER, &self.text, font, color::INK);
            super::focus(&painter, &response, rect, radius::BUTTON);
        }
        response
    }
}
