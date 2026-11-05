//! Command search (Shift+S, kit `CommandPalette`): type to find any action by
//! name; arrows move, Enter runs, Esc closes.

use egui::{Align2, Area, Context, Id, Key, Order, Sense, Shape, pos2, vec2};
use prism_ui::emboss;
use prism_ui::theme::{self, color, depth, radius, size};
use prism_ui::widgets::{field, keycap};

use super::{Chrome, Source};
use crate::actions::search;

/// How many results show at once.
const ROWS: usize = 9;

#[derive(Default)]
pub(crate) struct Palette {
    query: String,
    highlight: usize,
    focused: bool,
}

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let Some(mut p) = chrome.palette.take() else {
        return;
    };
    let hits: Vec<&'static str> = search::find(&chrome.reg, &p.query)
        .into_iter()
        .take(ROWS)
        .collect();
    let (down, up, enter) = ctx.input(|i| {
        (
            i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::Enter),
        )
    });
    if !hits.is_empty() {
        if down {
            p.highlight = (p.highlight + 1) % hits.len();
        }
        if up {
            p.highlight = (p.highlight + hits.len() - 1) % hits.len();
        }
        p.highlight = p.highlight.min(hits.len() - 1);
    }
    let mut run = None;
    if enter && let Some(id) = hits.get(p.highlight) {
        run = Some(*id);
    }

    let screen = ctx.viewport_rect();
    let width = size::DIALOG_WIDTH;
    let pos = pos2(
        screen.center().x - width * 0.5,
        screen.top() + screen.height() * 0.18,
    );
    let area = Area::new(Id::unique("palette"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let bg = ui.painter().add(Shape::Noop);
            let inner = egui::Frame::new()
                .inner_margin(size::MENU_PAD * 2.0)
                .show(ui, |ui| {
                    let w = width - 4.0 * size::MENU_PAD;
                    ui.set_width(w);
                    ui.spacing_mut().item_spacing = vec2(0.0, size::MENU_PAD);
                    let before = p.query.clone();
                    let f = field(ui, &mut p.query, "Search every command…", w);
                    if !p.focused {
                        f.request_focus();
                        p.focused = true;
                    }
                    if p.query != before {
                        p.highlight = 0;
                    }
                    if hits.is_empty() {
                        ui.label(egui::RichText::new("Nothing matches").color(color::MUTED));
                    }
                    for (i, id) in hits.iter().enumerate() {
                        let Some(a) = chrome.reg.get(id) else {
                            continue;
                        };
                        let enabled = chrome.enabled(id);
                        let (rect, resp) =
                            ui.allocate_exact_size(vec2(w, size::MENU_ROW_HEIGHT), Sense::click());
                        if resp.hovered() {
                            p.highlight = i;
                        }
                        let mut painter = ui.painter().clone();
                        if !enabled {
                            painter.multiply_opacity(color::GREYED_OPACITY);
                        }
                        if i == p.highlight {
                            let mut v = Vec::new();
                            emboss::inset(&mut v, rect, radius::MENU_ROW, depth::MENU_INSET);
                            painter.extend(v);
                        }
                        let x = rect.left() + size::MENU_ROW_PAD[1];
                        let name = painter.text(
                            pos2(x, rect.center().y),
                            Align2::LEFT_CENTER,
                            a.name,
                            theme::body(),
                            color::INK,
                        );
                        let path = a.menu_path.join(" › ");
                        let note = if enabled {
                            path
                        } else {
                            format!("{path}  ·  not built yet")
                        };
                        painter.text(
                            pos2(name.right() + size::MENU_ROW_PAD[1], rect.center().y),
                            Align2::LEFT_CENTER,
                            note,
                            theme::mono(),
                            color::MUTED,
                        );
                        let key = chrome.key_label(id);
                        if !key.is_empty() {
                            let mut child = ui.new_child(
                                egui::UiBuilder::new()
                                    .max_rect(rect.shrink2(vec2(size::MENU_ROW_PAD[1], 0.0)))
                                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
                            );
                            keycap(&mut child, &key);
                        }
                        if resp.clicked() {
                            run = Some(*id);
                        }
                    }
                });
            let mut shapes = Vec::new();
            emboss::floating(
                &mut shapes,
                inner.response.rect,
                radius::CARD,
                color::SURFACE,
                1.0,
            );
            ui.painter().set(bg, Shape::Vec(shapes));
            inner.response.rect
        });

    let outside = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|q| !area.inner.contains(q))
    });
    if let Some(id) = run {
        if chrome.enabled(id) {
            chrome.run(id, Source::Search);
        } else {
            chrome.status = format!(
                "{}: {}",
                chrome.reg.get(id).map_or("", |a| a.name),
                chrome.reg.why_disabled(id, &chrome.rules).unwrap_or("")
            );
            chrome.palette = Some(p);
        }
        return;
    }
    if !outside {
        chrome.palette = Some(p);
    }
}
