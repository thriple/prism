//! Drop-down and right-click menus, with submenus and keyboard control
//! (kit `ContextMenu`): arrows move, Right opens a submenu, Left closes one
//! (or moves to the next menu in the bar), Enter runs, Esc closes.

use egui::emath::TSTransform;
use egui::{Area, Context, Id, Key, LayerId, Order, Pos2, Rect, Shape, pos2, vec2};
use prism_ui::anim;
use prism_ui::spring;
use prism_ui::theme::{motion, size};
use prism_ui::widgets::menu::{self, MenuRow, RowEnd};

use super::frame::BarTitle;
use super::{Chrome, Level, MenuKind, OpenMenu, Source};
use crate::actions::registry::Entry;

/// What the menus did this frame.
#[derive(Default)]
struct Outcome {
    run: Option<&'static str>,
    rects: Vec<Rect>,
}

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let Some(mut open) = chrome.menu.clone() else {
        return;
    };
    let titles: Vec<BarTitle> = ctx
        .data(|d| d.get_temp(Id::unique("menu-bar-titles")))
        .unwrap_or_default();
    let (entries, at, heading, source) = match &open.kind {
        MenuKind::Bar(i) => {
            let items = chrome.reg.menubar()[*i].1;
            let at = titles.get(*i).map_or(Pos2::ZERO, |t| {
                t.rect.left_bottom() + vec2(0.0, size::MENU_GAP)
            });
            (Entry::from_items(items), at, None, Source::Menu)
        }
        MenuKind::Context {
            at,
            heading,
            entries,
        } => (entries.clone(), *at, Some(heading.clone()), Source::Context),
    };

    let mut out = Outcome::default();
    if keyboard(chrome, ctx, &mut open, &entries, &mut out) {
        return;
    }
    level(
        chrome,
        ctx,
        &mut open,
        0,
        &entries,
        at,
        None,
        heading.as_deref(),
        &mut out,
    );

    if let Some(id) = out.run {
        chrome.menu = None;
        chrome.run(id, source);
        return;
    }
    // A press anywhere outside the menus and the menu bar closes them.
    let pressed = ctx.input(|i| {
        i.pointer
            .any_pressed()
            .then(|| i.pointer.interact_pos())
            .flatten()
    });
    if let Some(p) = pressed
        && !out.rects.iter().any(|r| r.contains(p))
        && !titles.iter().any(|t| t.rect.contains(p))
    {
        chrome.menu = None;
        return;
    }
    // Keep our state unless the top bar replaced the menu this frame.
    if chrome
        .menu
        .as_ref()
        .is_some_and(|m| m.serial == open.serial)
    {
        chrome.menu = Some(open);
    }
}

/// The entries shown at `depth`, following the open submenus.
fn entries_at(top: &[Entry], open: &OpenMenu, depth: usize) -> Vec<Entry> {
    let mut list = top.to_vec();
    for l in open.levels.iter().take(depth) {
        match l.open_sub.and_then(|i| list.get(i).cloned()) {
            Some(Entry::Sub(_, sub)) => list = sub,
            _ => return Vec::new(),
        }
    }
    list
}

fn selectable(chrome: &Chrome, e: &Entry) -> bool {
    match e {
        Entry::Action(id) => chrome.enabled(id),
        Entry::Sub(_, sub) => sub.iter().any(|s| selectable(chrome, s)),
        Entry::Separator => false,
    }
}

/// Arrow keys and Enter. Returns true if the menu was closed or replaced.
fn keyboard(
    chrome: &mut Chrome,
    ctx: &Context,
    open: &mut OpenMenu,
    top: &[Entry],
    out: &mut Outcome,
) -> bool {
    let (down, up, right, left, enter) = ctx.input(|i| {
        (
            i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::ArrowRight),
            i.key_pressed(Key::ArrowLeft),
            i.key_pressed(Key::Enter) || i.key_pressed(Key::Space),
        )
    });
    if !(down || up || right || left || enter) {
        return false;
    }
    let depth = open.levels.len() - 1;
    let list = entries_at(top, open, depth);
    let rows: Vec<usize> = (0..list.len())
        .filter(|&i| selectable(chrome, &list[i]))
        .collect();
    let lv = &mut open.levels[depth];
    let pos = lv.highlight.and_then(|h| rows.iter().position(|&r| r == h));
    if down && !rows.is_empty() {
        lv.highlight = Some(rows[pos.map_or(0, |p| (p + 1) % rows.len())]);
    }
    if up && !rows.is_empty() {
        lv.highlight =
            Some(rows[pos.map_or(rows.len() - 1, |p| (p + rows.len() - 1) % rows.len())]);
    }
    let current = lv.highlight.and_then(|h| list.get(h).cloned());
    if right || enter {
        match current {
            Some(Entry::Sub(..)) => {
                lv.open_sub = lv.highlight;
                open.levels.push(Level {
                    highlight: None,
                    open_sub: None,
                });
                let sub = entries_at(top, open, depth + 1);
                let first = (0..sub.len()).find(|&i| selectable(chrome, &sub[i]));
                if let Some(last) = open.levels.last_mut() {
                    last.highlight = first;
                }
                return false;
            }
            Some(Entry::Action(id)) if enter => {
                out.run = Some(id);
                let source = if matches!(open.kind, MenuKind::Bar(_)) {
                    Source::Menu
                } else {
                    Source::Context
                };
                chrome.menu = None;
                chrome.run(id, source);
                return true;
            }
            _ => {}
        }
        if right && let MenuKind::Bar(i) = open.kind {
            let n = chrome.reg.menubar().len();
            chrome.open_menu(MenuKind::Bar((i + 1) % n));
            return true;
        }
    }
    if left {
        if depth > 0 {
            open.levels.pop();
            if let Some(l) = open.levels.last_mut() {
                l.open_sub = None;
            }
        } else if let MenuKind::Bar(i) = open.kind {
            let n = chrome.reg.menubar().len();
            chrome.open_menu(MenuKind::Bar((i + n - 1) % n));
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn level(
    chrome: &mut Chrome,
    ctx: &Context,
    open: &mut OpenMenu,
    depth: usize,
    entries: &[Entry],
    at: Pos2,
    parent_row: Option<Rect>,
    heading: Option<&str>,
    out: &mut Outcome,
) {
    let screen = ctx.viewport_rect();
    let pad = size::MENU_PAD;

    // Measure: as wide as the widest row, at least the minimum.
    let mut inner_w = size::MENU_MIN_WIDTH - 2.0 * pad;
    ctx.fonts_mut(|_| ());
    let probe = egui::Ui::new(
        ctx.clone(),
        Id::unique("menu-measure"),
        egui::UiBuilder::new(),
    );
    for e in entries {
        let w = match e {
            Entry::Action(id) => {
                let name = chrome.reg.get(id).map_or("", |a| a.name);
                menu::row_width(&probe, name, &chrome.key_label(id))
            }
            Entry::Sub(name, _) => menu::row_width(&probe, name, "  "),
            Entry::Separator => 0.0,
        };
        inner_w = inner_w.max(w);
    }
    let rows_h: f32 = entries
        .iter()
        .map(|e| match e {
            Entry::Separator => 2.0 * size::MENU_SEPARATOR[0] + size::EDGE_WIDTH,
            _ => size::MENU_ROW_HEIGHT,
        })
        .sum::<f32>()
        + heading.map_or(0.0, |_| size::MENU_HEAD_HEIGHT);
    let full = vec2(inner_w + 2.0 * pad, rows_h + 2.0 * pad);

    // Place it: below its title, at the pointer, or beside its parent row;
    // always fully on screen.
    let m = size::SCREEN_MARGIN;
    let mut pos = match parent_row {
        Some(row) => {
            let right = pos2(row.right() + pad + size::MENU_SUB_GAP, row.top() - pad);
            if right.x + full.x > screen.right() - m {
                pos2(
                    row.left() - pad - size::MENU_SUB_GAP - full.x,
                    row.top() - pad,
                )
            } else {
                right
            }
        }
        None => at,
    };
    pos.x = pos.x.clamp(
        screen.left() + m,
        (screen.right() - m - full.x).max(screen.left() + m),
    );
    pos.y = pos.y.clamp(
        screen.top() + m,
        (screen.bottom() - m - full.y).max(screen.top() + m),
    );

    let area_id = Id::unique(("menu", depth));
    let t = anim::spring_from(
        ctx,
        Id::unique(("menu-pop", open.serial, depth)),
        0.0,
        1.0,
        spring::SPRING_SNAPPY,
    );
    let shown = t.clamp(0.0, 1.0);
    let mut child: Option<(Rect, Vec<Entry>)> = None;

    let area = Area::new(area_id)
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            ui.multiply_opacity(shown);
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            let bg = ui.painter().add(Shape::Noop);
            let frame = egui::Frame::new().inner_margin(pad).show(ui, |ui| {
                ui.set_width(inner_w);
                if let Some(h) = heading {
                    menu::heading(ui, h, inner_w);
                }
                if open.levels.len() <= depth {
                    open.levels.push(Level::default());
                }
                for (i, e) in entries.iter().enumerate() {
                    match e {
                        Entry::Separator => menu::separator(ui, inner_w),
                        Entry::Action(id) => {
                            let Some(a) = chrome.reg.get(id) else {
                                continue;
                            };
                            let enabled = chrome.enabled(id);
                            let key = chrome.key_label(id);
                            let resp = MenuRow {
                                label: a.name,
                                end: if key.is_empty() {
                                    RowEnd::None
                                } else {
                                    RowEnd::Key(&key)
                                },
                                checked: enabled && chrome.checked(id),
                                enabled,
                                highlight: open.levels[depth].highlight == Some(i),
                                width: inner_w,
                            }
                            .show(ui);
                            if resp.hovered() {
                                open.levels[depth].highlight = Some(i);
                                open.levels[depth].open_sub = None;
                                open.levels.truncate(depth + 1);
                            }
                            if let Some(why) = chrome.reg.why_disabled(id, &chrome.rules) {
                                resp.clone().on_hover_text_at_pointer(why);
                            }
                            if resp.clicked() && enabled {
                                out.run = Some(id);
                            }
                        }
                        Entry::Sub(name, sub) => {
                            let enabled = sub.iter().any(|s| selectable(chrome, s));
                            let is_open = open.levels[depth].open_sub == Some(i);
                            let resp = MenuRow {
                                label: name,
                                end: RowEnd::Submenu,
                                checked: false,
                                enabled,
                                highlight: is_open || open.levels[depth].highlight == Some(i),
                                width: inner_w,
                            }
                            .show(ui);
                            if (resp.hovered() || resp.clicked()) && !is_open {
                                open.levels[depth].highlight = Some(i);
                                open.levels[depth].open_sub = Some(i);
                                open.levels.truncate(depth + 1);
                                open.levels.push(Level::default());
                            }
                            if open.levels[depth].open_sub == Some(i) {
                                child = Some((resp.rect, sub.clone()));
                            }
                        }
                    }
                }
            });
            let mut shapes = Vec::new();
            menu::card(&mut shapes, frame.response.rect);
            ui.painter().set(bg, Shape::Vec(shapes));
            frame.response.rect
        });

    // Pop in: rise into place as it fades in.
    let rise = (1.0 - shown) * motion::POP_RISE;
    ctx.set_transform_layer(
        LayerId::new(Order::Foreground, area_id),
        TSTransform::from_translation(vec2(0.0, rise)),
    );
    out.rects.push(area.inner);

    if let Some((row, sub)) = child {
        level(
            chrome,
            ctx,
            open,
            depth + 1,
            &sub,
            row.right_top(),
            Some(row),
            None,
            out,
        );
    }
}
