//! A square tool button for the dock (prototype `.tool`): raised, sinks
//! while pressed or active, a small key hint, and an accent dot when active.

use egui::{Align2, FontFamily, FontId, Rect, Response, Sense, Ui, Widget, pos2, vec2};

use crate::anim;
use crate::emboss;
use crate::icon::Icon;
use crate::spring;
use crate::theme::{color, depth, motion, radius, size, text};

pub struct ToolButton<'a> {
    icon: Option<&'a Icon>,
    hint: &'a str,
    active: bool,
    enabled: bool,
}

impl<'a> ToolButton<'a> {
    pub fn new(icon: Option<&'a Icon>) -> Self {
        Self {
            icon,
            hint: "",
            active: false,
            enabled: true,
        }
    }

    /// The key hint in the corner ("L", "Spc").
    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for ToolButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let side = size::DOCK_BUTTON;
        let sense = if self.enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(side, side), sense);
        let down = self.enabled && (response.is_pointer_button_down_on() || self.active);
        let target = if !self.enabled {
            depth::DOCK_DISABLED
        } else if down {
            depth::DOCK_PRESSED
        } else if response.hovered() {
            depth::DOCK_HOVER
        } else {
            depth::REST
        };
        let feel = if down { spring::PRESS } else { spring::RELEASE };
        let ctx = ui.ctx().clone();
        let d = anim::spring(&ctx, response.id.with("depth"), target, feel);
        let press = anim::spring(&ctx, response.id.with("press"), if down { 1.0 } else { 0.0 }, spring::LABEL);
        let dot = anim::spring(&ctx, response.id.with("dot"), if self.active { 1.0 } else { 0.0 }, spring::DOT);

        if ui.is_rect_visible(rect) {
            let mut painter = ui.painter().clone();
            if !self.enabled {
                painter.multiply_opacity(color::GREYED_OPACITY);
            }
            let mut shapes = Vec::new();
            emboss::soft(&mut shapes, rect, radius::DOCK_BUTTON, d, depth::DOCK, color::SURFACE);
            painter.extend(shapes);
            if let Some(icon) = self.icon {
                let s = size::DOCK_ICON * (1.0 - (1.0 - motion::ICON_PRESS) * press);
                icon.paint(&painter, Rect::from_center_size(rect.center(), vec2(s, s)), color::INK);
            }
            if !self.hint.is_empty() {
                let [x, y] = size::DOCK_HINT_INSET;
                painter.text(
                    pos2(rect.right() - x, rect.bottom() - y),
                    Align2::RIGHT_BOTTOM,
                    self.hint,
                    FontId::new(text::HINT, FontFamily::Name(text::MONO_MEDIUM.into())),
                    color::MUTED,
                );
            }
            if dot > 0.01 {
                let inset = size::DOCK_DOT_INSET + size::DOCK_DOT * 0.5;
                painter.circle_filled(
                    pos2(rect.right() - inset, rect.top() + inset),
                    size::DOCK_DOT * 0.5 * dot.max(0.0),
                    crate::theme::fade(color::ACCENT, dot),
                );
            }
            super::focus(&painter, &response, rect, radius::DOCK_BUTTON);
        }
        response
    }
}
