use egui::emath::TSTransform;
use egui::{
    Align2, Area, Key, LayerId, Order, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2,
    Widget, pos2, vec2,
};

use crate::anim;
use crate::emboss;
use crate::glass;
use crate::spring;
use crate::theme::{self, color, depth, motion, radius, size};

/// Dropdown (kit `SoftSelect`). The trigger is a raised button that sinks while
/// the list is open. The list's background is a glass cutout; the highlighted
/// item is an opaque pill that lifts out of the glass and slides between items.
/// Up and down, Home and End move; Enter or Space picks; Esc closes.
pub struct SoftSelect<'a> {
    selected: &'a mut usize,
    options: &'a [&'a str],
    label: String,
}

impl<'a> SoftSelect<'a> {
    pub fn new(selected: &'a mut usize, options: &'a [&'a str], label: impl Into<String>) -> Self {
        Self {
            selected,
            options,
            label: label.into(),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct State {
    open: bool,
    active: usize,
}

impl Widget for SoftSelect<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let n = self.options.len();
        let (rect, mut response) = ui.allocate_exact_size(
            vec2(size::SELECT_WIDTH, size::BUTTON_HEIGHT),
            Sense::click(),
        );
        let id = response.id;
        let ctx = ui.ctx().clone();
        let mut state: State = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
        let current = (*self.selected).min(n.saturating_sub(1));
        let value = self.options.get(current).copied().unwrap_or("");
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::ComboBox,
                true,
                format!("{}: {value}", self.label),
            )
        });

        // Keyboard while open (handled before clicks, since Enter and Space also click).
        let mut picked = None;
        let mut keyboard_used = false;
        if state.open && n > 0 {
            ui.input(|i| {
                if i.key_pressed(Key::ArrowDown) {
                    state.active = (state.active + 1) % n;
                }
                if i.key_pressed(Key::ArrowUp) {
                    state.active = (state.active + n - 1) % n;
                }
                if i.key_pressed(Key::Home) {
                    state.active = 0;
                }
                if i.key_pressed(Key::End) {
                    state.active = n - 1;
                }
                if i.key_pressed(Key::Enter) || i.key_pressed(Key::Space) {
                    picked = Some(state.active);
                    keyboard_used = true;
                }
                if i.key_pressed(Key::Escape) {
                    state.open = false;
                    keyboard_used = true;
                }
            });
        } else if response.has_focus()
            && ui.input(|i| i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowUp))
        {
            state.open = true;
            state.active = current;
        }
        if response.clicked() && !keyboard_used {
            state.open = !state.open;
            state.active = current;
        }

        let open_t = anim::spring(
            &ctx,
            id.with("shown"),
            if state.open { 1.0 } else { 0.0 },
            spring::SPRING_SNAPPY,
        );
        let d = anim::spring(
            &ctx,
            id.with("depth"),
            if state.open { depth::OPEN } else { depth::REST },
            if state.open {
                spring::OPEN
            } else {
                spring::CLOSE
            },
        );

        // The trigger.
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let mut shapes = Vec::new();
            emboss::soft(
                &mut shapes,
                rect,
                radius::BUTTON,
                d,
                depth::BUTTON,
                color::SURFACE,
            );
            super::paint(painter, shapes);
            let left = pos2(
                rect.left() + size::ITEM_PAD_X + size::LIST_PAD,
                rect.center().y,
            );
            let label =
                painter.layout_no_wrap(format!("{} ", self.label), theme::body(), color::MUTED);
            let label_w = label.size().x;
            painter.galley(left - vec2(0.0, label.size().y * 0.5), label, color::MUTED);
            painter.text(
                left + vec2(label_w, 0.0),
                Align2::LEFT_CENTER,
                value,
                theme::body_strong(),
                color::INK,
            );
            let chevron = pos2(
                rect.right() - size::ITEM_PAD_X - size::CHEVRON * 0.5,
                rect.center().y,
            );
            painter.add(chevron_shape(chevron, open_t * std::f32::consts::PI));
            super::focus(painter, &response, rect, radius::BUTTON);
        }

        // The list, drawn while open or still closing.
        if open_t > 0.01 && n > 0 {
            let area_id = id.with("list");
            let list_h = n as f32 * size::ITEM_HEIGHT
                + (n - 1) as f32 * size::ITEM_GAP
                + 2.0 * size::LIST_PAD;
            let panel_size = vec2(rect.width(), list_h + 2.0 * size::LIST_PAD);
            let pos = rect.left_bottom() + vec2(0.0, size::LIST_GAP);
            let shown = open_t.clamp(0.0, 1.0);
            let area = Area::new(area_id)
                .order(Order::Foreground)
                .fixed_pos(pos)
                .interactable(state.open)
                .show(&ctx, |ui| {
                    ui.multiply_opacity(shown);
                    let (panel, _) = ui.allocate_exact_size(panel_size, Sense::hover());
                    let painter = ui.painter();
                    let mut shapes = Vec::new();
                    emboss::soft(
                        &mut shapes,
                        panel,
                        radius::LIST,
                        1.0,
                        depth::CARD,
                        color::SURFACE,
                    );
                    super::paint(painter, shapes);
                    let well = panel.shrink(size::LIST_PAD);
                    glass::well(
                        painter,
                        well,
                        radius::LIST_WELL,
                        color::FROST_LIST,
                        depth::WELL,
                    );

                    let row = |i: usize| {
                        let top = well.top()
                            + size::LIST_PAD
                            + i as f32 * (size::ITEM_HEIGHT + size::ITEM_GAP);
                        Rect::from_min_size(
                            pos2(well.left() + size::LIST_PAD, top),
                            vec2(well.width() - 2.0 * size::LIST_PAD, size::ITEM_HEIGHT),
                        )
                    };
                    for i in 0..n {
                        let r = ui.interact(row(i), area_id.with(i), Sense::click());
                        if r.hovered() {
                            state.active = i;
                        }
                        if r.clicked() {
                            picked = Some(i);
                        }
                    }

                    // The pill slides to the active row.
                    let y = anim::spring(
                        &ctx,
                        id.with("pill"),
                        row(state.active).top(),
                        spring::SPRING_SNAPPY,
                    );
                    let base = row(0);
                    let pill = Rect::from_min_size(pos2(base.left(), y), base.size());
                    let pill =
                        Rect::from_center_size(pill.center(), pill.size() * motion::PILL_SCALE);
                    let mut shapes = Vec::new();
                    emboss::soft(
                        &mut shapes,
                        pill,
                        radius::ITEM,
                        1.0,
                        depth::PILL,
                        color::SURFACE,
                    );
                    super::paint(painter, shapes);

                    for (i, option) in self.options.iter().enumerate() {
                        let r = row(i);
                        let nudge = anim::spring(
                            &ctx,
                            id.with(("nudge", i)),
                            if i == state.active {
                                motion::ITEM_NUDGE
                            } else {
                                0.0
                            },
                            spring::SPRING_SNAPPY,
                        );
                        let font = if i == state.active {
                            theme::body_strong()
                        } else {
                            theme::body()
                        };
                        painter.text(
                            pos2(r.left() + size::ITEM_PAD_X + nudge, r.center().y),
                            Align2::LEFT_CENTER,
                            *option,
                            font,
                            color::INK,
                        );
                        if i == current {
                            painter.add(check_shape(pos2(
                                r.right() - size::ITEM_PAD_X,
                                r.center().y,
                            )));
                        }
                    }
                    panel
                });

            // Pop in from the trigger: rise and grow about the top centre.
            let pivot = pos + vec2(panel_size.x * 0.5, 0.0);
            let scale = motion::POP_SCALE + (1.0 - motion::POP_SCALE) * shown;
            let rise = (1.0 - shown) * motion::POP_RISE;
            let transform = TSTransform::from_translation(pivot.to_vec2() + vec2(0.0, rise))
                * TSTransform::from_scaling(scale)
                * TSTransform::from_translation(-pivot.to_vec2());
            ctx.set_transform_layer(LayerId::new(Order::Foreground, area_id), transform);

            // A press anywhere else closes it.
            let panel_rect = area.inner;
            if state.open
                && ctx.input(|i| i.pointer.any_pressed())
                && let Some(p) = ctx.input(|i| i.pointer.interact_pos())
                && !panel_rect.contains(p)
                && !rect.contains(p)
            {
                state.open = false;
            }
        }

        if let Some(i) = picked {
            if *self.selected != i {
                *self.selected = i;
                response.mark_changed();
            }
            state.open = false;
            response.request_focus();
        }
        ctx.data_mut(|d| d.insert_temp(id, state));
        response
    }
}

fn path(centre: Pos2, points: &[Vec2], angle: f32, stroke: Stroke) -> Shape {
    let (s, c) = angle.sin_cos();
    let pts = points
        .iter()
        .map(|p| centre + vec2(p.x * c - p.y * s, p.x * s + p.y * c))
        .collect();
    Shape::line(pts, stroke)
}

/// The dropdown chevron, rotated by `angle` (half a turn when open).
pub(crate) fn chevron_shape(centre: Pos2, angle: f32) -> Shape {
    let h = size::CHEVRON * 0.5;
    path(
        centre,
        &[vec2(-h, -h * 0.5), vec2(0.0, h * 0.5), vec2(h, -h * 0.5)],
        angle,
        Stroke::new(size::CHEVRON_STROKE, color::INK),
    )
}

/// The tick beside the chosen item.
fn check_shape(centre: Pos2) -> Shape {
    let h = size::CHEVRON * 0.5;
    path(
        centre,
        &[
            vec2(-h * 1.1, 0.1 * h),
            vec2(-0.4 * h, h * 0.85),
            vec2(h * 1.1, -h * 0.9),
        ],
        0.0,
        Stroke::new(size::CHECK_STROKE, color::ACCENT),
    )
}
