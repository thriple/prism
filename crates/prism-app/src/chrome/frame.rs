//! The top bar with the menu bar, the viewport's frame and the status bar.

use egui::{Align2, Area, Context, Id, Order, Rect, Sense, pos2, vec2};
use prism_ui::emboss;
use prism_ui::theme::{self, color, depth, radius, size};

use super::{Chrome, Layout, MenuKind, OpenMenu, dock};
use crate::actions::registry::Selection;

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

    // The viewport: a well the renderer will draw into. Right-click opens
    // the context menu (for empty space until there is a model).
    Area::new(Id::new("viewport"))
        .order(Order::Background)
        .fixed_pos(viewport.min)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(viewport.size(), Sense::click());
            let mut shapes = Vec::new();
            emboss::soft(&mut shapes, rect, radius::CARD, -1.0, depth::WELL, color::VIEWPORT);
            ui.painter().extend(shapes);
            if resp.secondary_clicked()
                && let Some(p) = resp.interact_pointer_pos()
            {
                chrome.open_context(p, Selection::default());
            }
            if resp.clicked() {
                chrome.menu = None;
            }
        });

    // The status bar: a one-line hint.
    Area::new(Id::new("status"))
        .order(Order::Background)
        .fixed_pos(status.min)
        .interactable(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(status.size(), Sense::hover());
            ui.painter().text(
                pos2(rect.left() + size::TOP_BAR_PAD, rect.center().y),
                Align2::LEFT_CENTER,
                &chrome.status,
                theme::body(),
                color::MUTED,
            );
            ui.painter().text(
                pos2(rect.right() - size::TOP_BAR_PAD, rect.center().y),
                Align2::RIGHT_CENTER,
                crate::overlay::BUILD,
                theme::mono(),
                color::MUTED,
            );
        });

    Layout { viewport }
}

/// Where each menu bar title is, for opening its menu below it.
#[derive(Clone, Copy, Default)]
pub(super) struct BarTitle {
    pub rect: Rect,
}

fn top_bar(chrome: &mut Chrome, ctx: &Context, rect: Rect) {
    Area::new(Id::new("top-bar"))
        .order(Order::Background)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (bar, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
            let mut shapes = Vec::new();
            emboss::soft(&mut shapes, bar, radius::CARD, 1.0, depth::CARD, color::SURFACE);
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
                Some(OpenMenu { kind: MenuKind::Bar(i), .. }) => Some(*i),
                _ => None,
            };
            let mut titles = Vec::new();
            for (i, (name, _)) in chrome.reg.menubar().iter().enumerate() {
                let galley = ui.painter().layout_no_wrap((*name).to_owned(), theme::body_medium(), color::INK);
                let w = galley.size().x + 2.0 * size::MENU_BAR_BUTTON_PAD;
                let r = Rect::from_center_size(
                    pos2(x + w * 0.5, bar.center().y),
                    vec2(w, size::MENU_BAR_BUTTON_HEIGHT),
                );
                x += w + size::MENU_BAR_GAP;
                let resp = ui.interact(r, Id::new(("menu-bar", i)), Sense::click());
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
                let ink = if resp.hovered() || open { color::INK_BRIGHT } else { color::INK };
                ui.painter().galley(r.center() - galley.size() * 0.5, galley, ink);
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
            ctx.data_mut(|d| d.insert_temp(Id::new("menu-bar-titles"), titles));
        });
}
