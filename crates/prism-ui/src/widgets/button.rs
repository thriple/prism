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
    small: bool,
    selected: bool,
}

impl SoftButton {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            enabled: true,
            small: false,
            selected: false,
        }
    }

    /// A shorter button with less padding (table cells, dialog actions).
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    /// Stays pressed in, as the chosen one of a set (tabs, a cell being recorded).
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Greyed out: shown dimmed and cannot be pressed.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for SoftButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let font = if self.small {
            theme::body_medium()
        } else {
            theme::body_strong()
        };
        let galley = ui
            .painter()
            .layout_no_wrap(self.text.clone(), font.clone(), color::INK);
        let (pad, height, corner, shadow) = if self.small {
            (
                size::SMALL_BUTTON_PAD,
                size::SMALL_BUTTON_HEIGHT,
                radius::MENU_BUTTON,
                depth::HANDLE,
            )
        } else {
            (
                size::BUTTON_PAD_X,
                size::BUTTON_HEIGHT,
                radius::BUTTON,
                depth::BUTTON,
            )
        };
        let want = vec2(galley.size().x + 2.0 * pad, height);
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
        let target = if down || self.selected {
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
            emboss::soft(&mut shapes, rect, corner, d, shadow, color::SURFACE);
            super::paint(&painter, shapes);
            let scale = 1.0 - (1.0 - motion::LABEL_SCALE) * sink;
            let centre = rect.center() + vec2(0.0, motion::LABEL_SINK * sink);
            let font = egui::FontId::new(theme::text::BODY * scale, font.family);
            painter.text(centre, Align2::CENTER_CENTER, &self.text, font, color::INK);
            super::focus(&painter, &response, rect, corner);
        }
        response
    }
}
