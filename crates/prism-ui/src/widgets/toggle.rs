use egui::{Align2, Rect, Response, Sense, Ui, Widget, pos2, vec2};

use crate::anim;
use crate::emboss;
use crate::glass;
use crate::spring;
use crate::theme::{self, color, depth, motion, radius, size};

/// Switch whose recessed track is a glass cutout (kit `SoftToggle`). Off, the
/// glass is heavily frosted; on, it clears and the aurora shows through. The
/// knob stretches as it travels and lights a small accent dot.
pub struct SoftToggle<'a> {
    on: &'a mut bool,
    label: String,
}

impl<'a> SoftToggle<'a> {
    pub fn new(on: &'a mut bool, label: impl Into<String>) -> Self {
        Self {
            on,
            label: label.into(),
        }
    }
}

impl Widget for SoftToggle<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let galley = ui
            .painter()
            .layout_no_wrap(self.label.clone(), theme::body(), color::INK);
        let [w, h] = size::TOGGLE;
        let gap = size::ROW_GAP * 1.6;
        let want = vec2(galley.size().x + gap + w, h);
        let (rect, mut response) = ui.allocate_exact_size(want, Sense::click());
        if response.clicked() {
            *self.on = !*self.on;
            response.mark_changed();
        }
        let on = *self.on;
        response
            .widget_info(|| egui::WidgetInfo::selected(egui::Role::Switch, true, on, &self.label));

        let ctx = ui.ctx().clone();
        let pad = size::TOGGLE_PAD;
        let knob = h - 2.0 * pad;
        let travel = w - 2.0 * pad - knob;
        let frost = anim::spring(
            &ctx,
            response.id.with("frost"),
            if on {
                color::FROST_TOGGLE_ON
            } else {
                color::FROST_TOGGLE_OFF
            },
            spring::FROST,
        );
        let (x, speed) = anim::spring_with_speed(
            &ctx,
            response.id.with("x"),
            if on { travel } else { 0.0 },
            spring::KNOB,
        );
        let dot = anim::spring(
            &ctx,
            response.id.with("dot"),
            if on { 1.0 } else { 0.0 },
            spring::DOT,
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.text(
                pos2(rect.left(), rect.center().y),
                Align2::LEFT_CENTER,
                &self.label,
                theme::body(),
                color::INK,
            );
            let track = Rect::from_min_size(pos2(rect.right() - w, rect.top()), vec2(w, h));
            glass::well(painter, track, radius::ROUND, frost, depth::WELL);

            // Squash and stretch: long while it travels, round once it lands.
            let stretch = 1.0 + (speed.abs() * motion::KNOB_STRETCH).min(motion::KNOB_STRETCH_MAX);
            let centre = pos2(track.left() + pad + knob * 0.5 + x, track.center().y);
            let knob_rect = Rect::from_center_size(centre, vec2(knob * stretch, knob));
            let mut shapes = Vec::new();
            emboss::soft(
                &mut shapes,
                knob_rect,
                radius::ROUND,
                depth::REST,
                depth::KNOB,
                color::SURFACE,
            );
            super::paint(painter, shapes);
            if dot > 0.01 {
                painter.circle_filled(
                    centre,
                    size::TOGGLE_DOT * 0.5 * dot.max(0.0),
                    theme::fade(color::ACCENT, dot),
                );
            }
            super::focus(painter, &response, track, radius::ROUND);
        }
        response
    }
}
