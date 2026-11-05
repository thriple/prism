//! The top bar with the menu bar, the viewport's frame and the status bar.

use egui::{Align2, Area, Context, Id, Order, Rect, Sense, pos2, vec2};
use prism_ui::emboss;
use prism_ui::theme::{self, color, depth, radius, size};

use super::{Chrome, Layout, MenuKind, OpenMenu, dock};
use egui::epaint::RectShape;

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) -> Layout {
    let screen = ctx.viewport_rect();
    let g = size::GAP;
    let top = Rect::from_min_size(
        screen.min + vec2(g, g),
        vec2(screen.width() - 2.0 * g, size::TOP_BAR_HEIGHT),
    );
    let status = Rect::from_min_max(
        pos2(screen.left() + g, screen.bottom() - size::STATUS_HEIGHT),
        pos2(screen.right() - g, screen.bottom()),
    );
    let body_top = top.bottom() + g;
    let body_bottom = status.top();
    let dock_rect = Rect::from_min_max(
        pos2(screen.left() + g, body_top),
        pos2(screen.left() + g + dock::width(), body_bottom),
    );
    let viewport = Rect::from_min_max(
        pos2(dock_rect.right() + g, body_top),
        pos2(screen.right() - g, body_bottom),
    );

    top_bar(chrome, ctx, top);
    dock::show(chrome, ctx, dock_rect);

    // The viewport: a sunken well showing the renderer's picture, with the
    // well's inner shadow over its edge. The app handles the mouse over it.
    let texture = chrome.viewport_texture;
    let response = Area::new(Id::unique("viewport"))
        .order(Order::Background)
        .fixed_pos(viewport.min)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(viewport.size(), Sense::click_and_drag());
            let r = emboss::fit(rect, radius::CARD);
            let mut shapes =
                vec![RectShape::filled(rect, theme::corner(r), color::VIEWPORT).into()];
            if let Some(id) = texture {
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                shapes.push(
                    RectShape::filled(rect, theme::corner(r), color::UNTINTED)
                        .with_texture(id, uv)
                        .into(),
                );
            }
            emboss::inset(&mut shapes, rect, r, depth::WELL);
            ui.painter().extend(shapes);
            // Tool feedback text over the picture, clipped to the well.
            let painter = ui.painter().with_clip_rect(rect);
            if let Some((b, crossing, label)) = &chrome.viewport_box {
                let c = if *crossing {
                    color::BOX_CROSSING
                } else {
                    color::BOX_WINDOW
                };
                painter.rect_filled(*b, 0.0, c.gamma_multiply(color::BOX_FILL));
                let stroke = egui::Stroke::new(size::BOX_STROKE, c);
                if *crossing {
                    let pts = [
                        b.left_top(),
                        b.right_top(),
                        b.right_bottom(),
                        b.left_bottom(),
                        b.left_top(),
                    ];
                    painter.extend(egui::Shape::dashed_line(
                        &pts,
                        stroke,
                        size::BOX_DASH,
                        size::BOX_GAP,
                    ));
                } else {
                    painter.rect_stroke(*b, 0.0, stroke, egui::StrokeKind::Inside);
                }
                pill(&painter, b.left_top() + vec2(8.0, -14.0), label, c);
            }
            for (at, text, c) in &chrome.viewport_labels {
                pill(&painter, *at, text, *c);
            }
            if resp.clicked() {
                chrome.menu = None;
            }
            resp
        })
        .inner;

    // The status bar: the tool's hint, then the lock chip, the measurement
    // box and the build.
    Area::new(Id::unique("status"))
        .order(Order::Background)
        .fixed_pos(status.min)
        .interactable(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(status.size(), Sense::hover());
            let p = ui.painter();
            let build = p.text(
                pos2(rect.right() - size::TOP_BAR_PAD, rect.center().y),
                Align2::RIGHT_CENTER,
                crate::overlay::BUILD,
                theme::mono(),
                color::MUTED,
            );
            // The measurement box: a sunken well with the value in it.
            let well = Rect::from_min_size(
                pos2(
                    build.left() - size::STATUS_ITEM_GAP * 2.0 - size::MEASURE_WIDTH,
                    rect.center().y - size::MEASURE_HEIGHT * 0.5,
                ),
                vec2(size::MEASURE_WIDTH, size::MEASURE_HEIGHT),
            );
            let mut shapes = Vec::new();
            emboss::soft(
                &mut shapes,
                well,
                radius::MEASURE,
                -1.0,
                depth::FIELD_WELL,
                color::SURFACE,
            );
            p.extend(shapes);
            let (label, value, typing) = &chrome.measure;
            p.text(
                pos2(well.right() - size::MEASURE_PAD, well.center().y),
                Align2::RIGHT_CENTER,
                if *typing {
                    format!("{value}▏")
                } else {
                    value.clone()
                },
                theme::mono_value(),
                if *typing { color::ACCENT } else { color::INK },
            );
            let lab = p.text(
                pos2(well.left() - size::STATUS_ITEM_GAP, rect.center().y),
                Align2::RIGHT_CENTER,
                label,
                theme::small(),
                color::MUTED,
            );
            let mut left_end = lab.left();
            if let Some((text, c)) = &chrome.lock_chip {
                let g = p.layout_no_wrap(text.clone(), theme::small(), color::INK_BRIGHT);
                let w = g.size().x + 2.0 * size::CHIP_PAD;
                let chip = Rect::from_min_size(
                    pos2(
                        left_end - size::STATUS_ITEM_GAP * 2.0 - w,
                        rect.center().y - size::CHIP_HEIGHT * 0.5,
                    ),
                    vec2(w, size::CHIP_HEIGHT),
                );
                p.rect_filled(chip, theme::corner(radius::ROUND), *c);
                p.galley(chip.center() - g.size() * 0.5, g, color::INK_BRIGHT);
                left_end = chip.left();
            }
            let hint = p.layout(
                chrome.status.clone(),
                theme::body(),
                color::MUTED,
                (left_end - rect.left() - 3.0 * size::TOP_BAR_PAD).max(50.0),
            );
            let first = hint.rows.first().map(|r| r.row.size.y).unwrap_or(0.0);
            p.galley(
                pos2(
                    rect.left() + size::TOP_BAR_PAD,
                    rect.center().y - first * 0.5,
                ),
                hint,
                color::MUTED,
            );
        });

    Layout {
        viewport,
        response: Some(response),
    }
}

/// Where each menu bar title is, for opening its menu below it.
#[derive(Clone, Copy)]
pub(super) struct BarTitle {
    pub rect: Rect,
}

fn top_bar(chrome: &mut Chrome, ctx: &Context, rect: Rect) {
    Area::new(Id::unique("top-bar"))
        .order(Order::Background)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (bar, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
            let mut shapes = Vec::new();
            emboss::soft(
                &mut shapes,
                bar,
                radius::CARD,
                1.0,
                depth::CARD,
                color::SURFACE,
            );
            ui.painter().extend(shapes);

            let mut x = bar.left() + size::TOP_BAR_PAD;
            let title = ui.painter().text(
                pos2(x, bar.center().y),
                Align2::LEFT_CENTER,
                "Prism",
                theme::title(),
                color::INK,
            );
            x = title.right() + size::TOP_BAR_PAD;
            ui.painter().text(
                pos2(bar.right() - size::TOP_BAR_PAD, bar.center().y),
                Align2::RIGHT_CENTER,
                "Greyed items are not built yet",
                theme::body(),
                color::MUTED,
            );

            let open_bar = match &chrome.menu {
                Some(OpenMenu {
                    kind: MenuKind::Bar(i),
                    ..
                }) => Some(*i),
                _ => None,
            };
            let mut titles = Vec::new();
            for (i, (name, _)) in chrome.reg.menubar().iter().enumerate() {
                let galley = ui.painter().layout_no_wrap(
                    (*name).to_owned(),
                    theme::body_medium(),
                    color::INK,
                );
                let w = galley.size().x + 2.0 * size::MENU_BAR_BUTTON_PAD;
                let r = Rect::from_center_size(
                    pos2(x + w * 0.5, bar.center().y),
                    vec2(w, size::MENU_BAR_BUTTON_HEIGHT),
                );
                x += w + size::MENU_BAR_GAP;
                let resp = ui.interact(r, Id::unique(("menu-bar", i)), Sense::click());
                let open = open_bar == Some(i);
                let sink = prism_ui::anim::spring(
                    ctx,
                    resp.id.with("sink"),
                    if open { 1.0 } else { 0.0 },
                    prism_ui::spring::SPRING_SNAPPY,
                );
                if sink > 0.01 {
                    let mut v = Vec::new();
                    emboss::inset(&mut v, r, radius::MENU_BUTTON, depth::MENU_INSET * sink);
                    ui.painter().extend(v);
                }
                let ink = if resp.hovered() || open {
                    color::INK_BRIGHT
                } else {
                    color::INK
                };
                ui.painter()
                    .galley(r.center() - galley.size() * 0.5, galley, ink);
                if resp.clicked() {
                    if open {
                        chrome.menu = None;
                    } else {
                        chrome.open_menu(MenuKind::Bar(i));
                    }
                } else if resp.hovered() && open_bar.is_some() && !open {
                    // While a menu is open, hovering another title switches to it.
                    chrome.open_menu(MenuKind::Bar(i));
                }
                titles.push(BarTitle { rect: r });
            }
            ctx.data_mut(|d| d.insert_temp(Id::unique("menu-bar-titles"), titles));
        });
}

/// A dark pill with white text (feel test `pill(`): its left middle at `at`.
fn pill(p: &egui::Painter, at: egui::Pos2, text: &str, fill: egui::Color32) {
    let g = p.layout_no_wrap(text.to_owned(), theme::mono_small(), color::INK_BRIGHT);
    let r = Rect::from_min_size(
        pos2(at.x, at.y - size::PILL_HEIGHT * 0.5),
        vec2(g.size().x + 2.0 * size::PILL_PAD, size::PILL_HEIGHT),
    );
    p.rect_filled(r, theme::corner(radius::PILL), fill);
    p.galley(
        pos2(r.left() + size::PILL_PAD, r.center().y - g.size().y * 0.5),
        g,
        color::INK_BRIGHT,
    );
}
