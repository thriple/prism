use std::ops::RangeInclusive;

use egui::{Key, Rect, Response, Sense, Ui, Widget, pos2, vec2};

use crate::anim;
use crate::emboss;
use crate::glass::{self, Tint};
use crate::spring;
use crate::theme::{self, color, depth, motion, radius, size};

/// Slider with a glass-cutout rail and an opaque handle (kit `SoftSlider`).
/// The filled side of the rail is clear glass, the rest frosted. The handle
/// follows the pointer on a spring; holding it lifts it and shows the value.
pub struct SoftSlider<'a> {
    value: &'a mut f32,
    range: RangeInclusive<f32>,
    step: f32,
    label: String,
    suffix: String,
}

impl<'a> SoftSlider<'a> {
    pub fn new(value: &'a mut f32, range: RangeInclusive<f32>, label: impl Into<String>) -> Self {
        Self {
            value,
            range,
            step: 1.0,
            label: label.into(),
            suffix: String::new(),
        }
    }

    /// The value moves in steps of this size.
    pub fn step(mut self, step: f32) -> Self {
        self.step = step;
        self
    }

    /// Text after the number in the value bubble, such as "%".
    pub fn suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }
}

/// Round `v` to the nearest step inside the range.
pub(crate) fn snap(v: f32, min: f32, max: f32, step: f32) -> f32 {
    let v = v.clamp(min, max);
    if step <= 0.0 {
        return v;
    }
    (((v - min) / step).round() * step + min).clamp(min, max)
}

impl Widget for SoftSlider<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (min, max) = (*self.range.start(), *self.range.end());
        let handle = size::SLIDER_HANDLE;
        let (rect, mut response) =
            ui.allocate_exact_size(vec2(size::SLIDER_WIDTH, handle), Sense::click_and_drag());
        let travel = (rect.width() - handle).max(1.0);
        let span = (max - min).max(f32::EPSILON);

        let before = *self.value;
        if let Some(p) = response.interact_pointer_pos()
            && (response.dragged() || response.clicked() || response.is_pointer_button_down_on())
        {
            response.request_focus();
            let v = min + (p.x - rect.left() - handle * 0.5) / travel * span;
            *self.value = snap(v, min, max, self.step);
        }
        if response.has_focus() {
            let (step, big) = (self.step, self.step * 10.0);
            ui.input(|i| {
                let mut v = *self.value;
                if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::ArrowUp) {
                    v += step;
                }
                if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowDown) {
                    v -= step;
                }
                if i.key_pressed(Key::PageUp) {
                    v += big;
                }
                if i.key_pressed(Key::PageDown) {
                    v -= big;
                }
                if i.key_pressed(Key::Home) {
                    v = min;
                }
                if i.key_pressed(Key::End) {
                    v = max;
                }
                *self.value = snap(v, min, max, step);
            });
        }
        if *self.value != before {
            response.mark_changed();
        }
        let value = *self.value;
        let text = format!("{}{}", trim(value), self.suffix);
        response.widget_info(|| egui::WidgetInfo::slider(true, f64::from(value), &self.label));

        let ctx = ui.ctx().clone();
        let held = response.is_pointer_button_down_on() || response.dragged();
        let target = handle * 0.5 + (value - min) / span * travel;
        let cx = anim::spring(&ctx, response.id.with("x"), target, spring::HANDLE);
        let lift = anim::spring(
            &ctx,
            response.id.with("lift"),
            if held { depth::LIFTED } else { depth::REST },
            spring::LIFT,
        );
        let grow = anim::spring(
            &ctx,
            response.id.with("grow"),
            if held { 1.0 } else { 0.0 },
            spring::DOT,
        );
        let tip = anim::spring(
            &ctx,
            response.id.with("tip"),
            if held || response.hovered() { 1.0 } else { 0.0 },
            spring::SPRING_SNAPPY,
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let rail = Rect::from_center_size(rect.center(), vec2(rect.width(), size::SLIDER_RAIL));
            let split = rect.left() + cx;
            let tints = [
                Tint {
                    area: Rect::from_min_max(rail.min, pos2(split, rail.max.y)),
                    frost: color::FROST_RAIL_FILLED,
                },
                Tint {
                    area: Rect::from_min_max(pos2(split, rail.min.y), rail.max),
                    frost: color::FROST_RAIL_EMPTY,
                },
            ];
            glass::well_tinted(painter, rail, radius::ROUND, &tints, depth::RAIL_WELL);

            let scale = 1.0 + (motion::HANDLE_GROW - 1.0) * grow;
            let centre = pos2(split, rect.center().y);
            let knob = Rect::from_center_size(centre, vec2(handle, handle) * scale);
            let mut shapes = Vec::new();
            emboss::soft(
                &mut shapes,
                knob,
                radius::ROUND,
                lift,
                depth::HANDLE,
                color::SURFACE,
            );
            // The dimple: a small pressed-in dot that lights up while held.
            let dimple =
                Rect::from_center_size(centre, vec2(size::SLIDER_DIMPLE, size::SLIDER_DIMPLE));
            let fill = if held { color::ACCENT } else { color::SURFACE };
            emboss::soft(
                &mut shapes,
                dimple,
                radius::ROUND,
                -1.0,
                depth::DIMPLE,
                fill,
            );
            super::paint(painter, shapes);
            super::focus(painter, &response, knob, radius::ROUND);

            if tip > 0.01 {
                value_tip(ui, centre, knob.height() * 0.5, &text, tip);
            }
        }
        response
    }
}

/// The value bubble above the handle, popping up as `t` goes from 0 to 1.
fn value_tip(ui: &Ui, anchor: egui::Pos2, half: f32, text: &str, t: f32) {
    let mut painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        ui.scope_id().with("value-tip"),
    ));
    painter.multiply_opacity(t.clamp(0.0, 1.0));
    let galley = painter.layout_no_wrap(text.to_owned(), theme::body_strong(), color::INK);
    let [px, py] = size::VALUE_TIP_PAD;
    let rise = (1.0 - t) * motion::POP_RISE;
    let size = galley.size() + vec2(2.0 * px, 2.0 * py);
    let size = vec2(size.x.max(size::SLIDER_HANDLE + px), size.y);
    let bottom = anchor.y - half - size::VALUE_TIP_GAP + rise;
    let r = Rect::from_min_size(pos2(anchor.x - size.x * 0.5, bottom - size.y), size);
    let mut shapes = Vec::new();
    emboss::soft(
        &mut shapes,
        r,
        radius::TOOLTIP,
        1.0,
        depth::TOOLTIP,
        color::SURFACE,
    );
    super::paint(&painter, shapes);
    painter.galley(r.center() - galley.size() * 0.5, galley, color::INK);
}

/// A number without a pointless ".0".
fn trim(v: f32) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapping_keeps_values_on_steps_and_in_range() {
        assert_eq!(snap(63.6, 0.0, 100.0, 1.0), 64.0);
        assert_eq!(snap(-5.0, 0.0, 100.0, 1.0), 0.0);
        assert_eq!(snap(250.0, 0.0, 100.0, 1.0), 100.0);
        assert_eq!(snap(0.37, 0.0, 1.0, 0.25), 0.25);
        assert_eq!(snap(0.37, 0.0, 1.0, 0.0), 0.37);
    }

    #[test]
    fn numbers_read_cleanly() {
        assert_eq!(trim(64.0), "64");
        assert_eq!(trim(0.25), "0.25");
        assert_eq!(trim(1.5), "1.5");
    }
}
