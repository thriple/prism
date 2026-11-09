//! The toolbars (brief 19): groups of square buttons from the registry,
//! docked on the left rail or in the top dock, or floating inside the app.
//! Where each one is lives in [`crate::toolbars::Layout`].

use egui::{Area, Context, Id, Order, Pos2, Rect, ScrollArea, Sense, Ui, Vec2, pos2, vec2};
use prism_ui::emboss;
use prism_ui::theme::{self, color, depth, radius, size};
use prism_ui::widgets::{SoftButton, ToolButton, menu, tooltip};

use super::{Chrome, MenuKind, Source};
use crate::actions::keymap::{Chord, Mods, key_label};
use crate::actions::registry::Entry;
use crate::toolbars::{Box2, DOCK_THICKNESS, Dock, Layout, float_columns, grid, snap};

// The scroll bar floats inside the side padding, so it never covers the
// buttons or needs a horizontal scroll bar. Checked when compiling.
const _: () = assert!(size::DOCK_SCROLL_BAR <= size::DOCK_PAD_X);

const B: f32 = size::DOCK_BUTTON;
const G: f32 = size::DOCK_GAP;
/// Floating toolbars are tighter than the rest of the UI (owner request).
const FLOAT_PAD: f32 = 4.0;
const FLOAT_GAP: f32 = 4.0;
/// The slim bar a floating toolbar is dragged by.
const GRIP: f32 = 10.0;
/// The title bar each toolbar gets in edit mode.
const TITLE: f32 = 22.0;
/// The top dock's padding, and the space between its toolbars.
const TOP_PAD: f32 = 8.0;
const TOP_SPACING: f32 = 16.0;
/// A dock with nothing in it, while a toolbar is dragged: a strip to drop on.
const DROP_STRIP: f32 = 18.0;

/// The toolbars' state: the layout, edit mode, and a toolbar being dragged.
#[derive(Clone, Debug)]
pub struct Toolbars {
    pub layout: Layout,
    pub editing: bool,
    pub drag: Option<Drag>,
    /// Where each toolbar was drawn last frame (for dropping between them).
    drawn: Vec<(String, Rect)>,
    /// The layout as last saved.
    saved: Layout,
    /// A right-click asked for the toolbar menu here; it opens after the
    /// menus are drawn, or the same click would close it.
    pub menu_at: Option<Pos2>,
}

/// A toolbar following the pointer.
#[derive(Clone, Debug)]
pub struct Drag {
    pub name: String,
    /// From the toolbar's corner to the pointer.
    pub grab: Vec2,
}

impl Toolbars {
    pub fn new(layout: Layout) -> Self {
        Self {
            saved: layout.clone(),
            layout,
            editing: false,
            drag: None,
            drawn: Vec::new(),
            menu_at: None,
        }
    }

    /// The layout, if it changed since last asked (to save it).
    pub fn take_changed(&mut self) -> Option<&Layout> {
        (self.layout != self.saved).then(|| {
            self.saved = self.layout.clone();
            &self.layout
        })
    }
}

/// Every toolbar's name, in the registry's order.
pub fn names(chrome: &Chrome) -> Vec<&'static str> {
    chrome.reg.dock().iter().map(|(n, _)| *n).collect()
}

fn ids_of(chrome: &Chrome, name: &str) -> &'static [&'static str] {
    chrome
        .reg
        .dock()
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(&[], |(_, ids)| ids)
}

/// How many buttons thick a docked toolbar is: the same rule as floating
/// ones (1 for five tools or fewer, 2 for six or more), never more than a
/// dock holds.
fn lines_for(n: usize) -> usize {
    float_columns(n).min(DOCK_THICKNESS)
}

/// How thick a dock is: as its thickest toolbar.
fn thickness(chrome: &Chrome, dock: Dock) -> usize {
    chrome
        .toolbars
        .layout
        .in_dock(dock)
        .iter()
        .map(|n| lines_for(ids_of(chrome, n).len()))
        .max()
        .unwrap_or(0)
}

/// The left rail's outer width for `cols` columns of buttons: the gaps
/// between them and the side padding (two columns: the prototype's 116
/// points).
pub fn width(cols: usize) -> f32 {
    let cols = cols as f32;
    2.0 * size::DOCK_PAD_X + cols * B + (cols - 1.0).max(0.0) * G
}

/// The left rail's width: as its thickest toolbar; a thin strip while a
/// toolbar is dragged and the rail is empty; none otherwise.
pub fn left_width(chrome: &Chrome) -> f32 {
    match thickness(chrome, Dock::Left) {
        0 if chrome.toolbars.drag.is_some() => DROP_STRIP,
        0 => 0.0,
        t => width(t),
    }
}

/// A toolbar's size: its buttons `lines` thick, running down or across.
fn block(n: usize, lines: usize, vertical: bool, gap: f32) -> Vec2 {
    let (_, (c, r)) = grid(n, lines, vertical);
    vec2(
        c as f32 * B + (c.max(1) - 1) as f32 * gap,
        r as f32 * B + (r.max(1) - 1) as f32 * gap,
    )
}

/// Where each toolbar in the top dock goes, from its top-left inside the
/// dock: they run across and wrap onto another line when the dock is full.
fn top_flow(chrome: &Chrome, width: f32) -> (Vec<(String, Rect)>, f32) {
    let t = &chrome.toolbars;
    let extra = if t.editing { TITLE } else { 0.0 };
    let rows = thickness(chrome, Dock::Top).max(1);
    let line_h = block(rows, rows, false, G).y + extra;
    let mut out = Vec::new();
    let (mut x, mut y) = (0.0, 0.0);
    for name in t.layout.in_dock(Dock::Top) {
        let n = ids_of(chrome, name).len();
        let s = block(n, lines_for(n), false, G);
        if x > 0.0 && x + s.x > width {
            x = 0.0;
            y += line_h + TOP_SPACING * 0.5;
        }
        out.push((
            name.to_owned(),
            Rect::from_min_size(pos2(x, y), vec2(s.x, s.y + extra)),
        ));
        x += s.x + TOP_SPACING;
    }
    let h = if out.is_empty() { 0.0 } else { y + line_h };
    (out, h)
}

/// The top dock's outer height for an app `width` points wide: none when
/// it is empty (a thin strip while dragging).
pub fn top_height(chrome: &Chrome, width: f32) -> f32 {
    let (bars, h) = top_flow(chrome, width - 2.0 * TOP_PAD);
    if !bars.is_empty() {
        h + 2.0 * TOP_PAD
    } else if chrome.toolbars.drag.is_some() {
        DROP_STRIP
    } else {
        0.0
    }
}

/// The key hint on a dock button: a single key with no modifiers, as a
/// letter, or "Spc" for Space. Longer bindings are left to the tooltip.
pub fn hint(chord: Option<&Chord>) -> String {
    match chord {
        Some(c) if c.mods == Mods::default() => {
            let k = key_label(&c.code);
            if k == "Space" {
                "Spc".to_owned()
            } else if k.chars().count() == 1 {
                k
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

/// What the pointer did to the toolbars this frame.
#[derive(Default)]
struct Hits {
    hovered: Option<(Rect, &'static str, Rect)>,
    clicked: Option<&'static str>,
    /// A toolbar's title or grip was pressed: (name, its corner).
    grabbed: Option<(String, Pos2)>,
    closed: Option<String>,
    /// Right-click on empty space.
    menu_at: Option<Pos2>,
}

/// Draw one toolbar's buttons in `rect`, `lines` thick.
#[allow(clippy::too_many_arguments)]
fn buttons(
    ui: &mut Ui,
    chrome: &Chrome,
    ids: &'static [&'static str],
    rect: Rect,
    lines: usize,
    vertical: bool,
    gap: f32,
    tip_beside: Rect,
    hits: &mut Hits,
) {
    let (cells, _) = grid(ids.len(), lines, vertical);
    for (id, (c, r)) in ids.iter().zip(cells) {
        let at = rect.min + vec2(c as f32 * (B + gap), r as f32 * (B + gap));
        let label = hint(chrome.keymap.primary(id).as_ref());
        let resp = ui.put(
            Rect::from_min_size(at, vec2(B, B)),
            ToolButton::new(chrome.icons.get(id))
                .hint(&label)
                .active(chrome.checked(id))
                .enabled(chrome.enabled(id) && !chrome.toolbars.editing),
        );
        if resp.hovered() {
            hits.hovered = Some((resp.rect, id, tip_beside));
        }
        if resp.clicked() {
            hits.clicked = Some(id);
        }
    }
}

/// In edit mode: a toolbar's title bar (drag it by this) with an X.
fn title_bar(ui: &mut Ui, name: &str, rect: Rect, corner: Pos2, hits: &mut Hits) {
    let id = Id::unique(("toolbar-title", name));
    let x = Rect::from_min_size(pos2(rect.right() - TITLE, rect.top()), vec2(TITLE, TITLE));
    let grip = Rect::from_min_max(rect.min, pos2(x.left(), rect.top() + TITLE));
    if ui.interact(grip, id, Sense::drag()).drag_started() {
        hits.grabbed = Some((name.to_owned(), corner));
    }
    let close = ui.interact(x, id.with("x"), Sense::click());
    if close.clicked() {
        hits.closed = Some(name.to_owned());
    }
    ui.painter().text(
        grip.left_center() + vec2(4.0, 0.0),
        egui::Align2::LEFT_CENTER,
        name,
        theme::small(),
        color::INK,
    );
    ui.painter().text(
        x.center(),
        egui::Align2::CENTER_CENTER,
        "×",
        theme::small(),
        if close.hovered() {
            color::INK
        } else {
            color::MUTED
        },
    );
}

/// The rounded box around a toolbar in edit mode.
fn edit_box(ui: &Ui, rect: Rect) {
    ui.painter().rect_stroke(
        rect.expand(3.0),
        theme::corner(radius::MENU_BUTTON),
        egui::Stroke::new(size::EDGE_WIDTH, color::MUTED.gamma_multiply(0.6)),
        egui::StrokeKind::Outside,
    );
}

pub(super) fn show(chrome: &mut Chrome, ctx: &Context, left: Rect, top: Rect, area: Rect) {
    let mut hits = Hits::default();
    chrome.toolbars.drawn.clear();
    if left.width() > 0.0 {
        show_left(chrome, ctx, left, &mut hits);
    }
    if top.height() > 0.0 {
        show_top(chrome, ctx, top, &mut hits);
    }
    show_floating(chrome, ctx, &mut hits);
    if chrome.toolbars.editing {
        done_button(chrome, ctx, area);
    }

    // A right-click on a toolbar's empty space (not a button) offers the
    // toolbar menu. Checked here, as scroll areas take the click before a
    // dock's background would see it.
    if hits.menu_at.is_none()
        && hits.hovered.is_none()
        && let Some(p) = ctx.input(|i| {
            i.pointer
                .button_clicked(egui::PointerButton::Secondary)
                .then(|| i.pointer.interact_pos())
                .flatten()
        })
        && (left.contains(p)
            || top.contains(p)
            || chrome.toolbars.drawn.iter().any(|(_, r)| r.contains(p)))
    {
        hits.menu_at = Some(p);
    }
    if let Some(id) = hits.clicked {
        chrome.run(id, Source::Dock);
    }
    if let Some(name) = hits.closed {
        chrome.toolbars.layout.close(&name);
    }
    if let Some((name, corner)) = hits.grabbed {
        let p = ctx.pointer_interact_pos().unwrap_or(corner);
        chrome.toolbars.drag = Some(Drag {
            name,
            grab: p - corner,
        });
    }
    if hits.menu_at.is_some() {
        ctx.request_repaint();
        chrome.toolbars.menu_at = hits.menu_at;
    }
    drag(chrome, ctx, left, top, area);

    // No tooltips while arranging: the buttons do not run then.
    if chrome.toolbars.editing {
        hits.hovered = None;
    }
    let text = hits
        .hovered
        .map(|(_, id, _)| chrome.tip(id))
        .unwrap_or_default();
    // Beside the toolbar, level with the button, so it never covers the
    // other buttons.
    let anchor = hits.hovered.map(|(r, _, beside)| {
        Rect::from_min_max(
            pos2(beside.left(), r.top()),
            pos2(beside.right(), r.bottom()),
        )
    });
    tooltip::shared(ctx, Id::unique("dock-tip"), anchor, &text);
}

fn card(ui: &Ui, rect: Rect) {
    let mut shapes = Vec::new();
    emboss::soft(
        &mut shapes,
        rect,
        radius::CARD,
        1.0,
        depth::CARD,
        color::SURFACE,
    );
    ui.painter().extend(shapes);
}

fn show_left(chrome: &mut Chrome, ctx: &Context, rect: Rect, hits: &mut Hits) {
    let names: Vec<String> = chrome
        .toolbars
        .layout
        .in_dock(Dock::Left)
        .into_iter()
        .map(str::to_owned)
        .collect();
    let mut drawn = Vec::new();
    Area::new(Id::unique("dock"))
        .order(Order::Background)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (card_rect, bg) = ui.allocate_exact_size(rect.size(), Sense::click());
            card(ui, card_rect);
            if bg.secondary_clicked() {
                hits.menu_at = bg.interact_pointer_pos();
            }
            if names.is_empty() {
                return;
            }
            // The scroll area reaches into the right padding, where its thin
            // floating bar sits; the buttons keep their own width.
            let inner = card_rect.shrink2(vec2(size::DOCK_PAD_X, size::DOCK_PAD_Y));
            let scroll_rect = Rect::from_min_max(
                inner.min,
                pos2(
                    card_rect.right() - size::DOCK_SCROLL_BAR * 0.5,
                    inner.bottom(),
                ),
            );
            let mut content = ui.new_child(egui::UiBuilder::new().max_rect(scroll_rect));
            let scroll = &mut content.style_mut().spacing.scroll;
            *scroll = egui::style::ScrollStyle::floating();
            scroll.bar_width = size::DOCK_SCROLL_BAR;
            scroll.bar_outer_margin = 0.0;
            scroll.bar_inner_margin = 0.0;
            // Hidden until the pointer is over the dock.
            scroll.dormant_handle_opacity = 0.0;
            scroll.dormant_background_opacity = 0.0;
            let editing = chrome.toolbars.editing;
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(&mut content, |ui| {
                    ui.spacing_mut().item_spacing = vec2(G, G);
                    for (i, name) in names.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(size::DOCK_GROUP_GAP);
                        }
                        let ids = ids_of(chrome, name);
                        let lines = lines_for(ids.len());
                        let s = block(ids.len(), lines, true, G);
                        let w = inner.width();
                        if editing {
                            let (r, _) = ui.allocate_exact_size(vec2(w, TITLE), Sense::hover());
                            title_bar(ui, name, r, r.min, hits);
                        } else {
                            ui.label(menu::caps(name, color::MUTED));
                        }
                        let (r, _) = ui.allocate_exact_size(vec2(w, s.y), Sense::hover());
                        buttons(ui, chrome, ids, r, lines, true, G, rect, hits);
                        let whole = if editing {
                            Rect::from_min_max(r.min - vec2(0.0, TITLE + G), r.max)
                        } else {
                            r
                        };
                        if editing {
                            edit_box(ui, whole);
                        }
                        drawn.push((name.clone(), whole));
                    }
                });
        });
    chrome.toolbars.drawn.extend(drawn);
}

fn show_top(chrome: &mut Chrome, ctx: &Context, rect: Rect, hits: &mut Hits) {
    let inner = rect.shrink(TOP_PAD);
    let (bars, _) = top_flow(chrome, inner.width());
    let editing = chrome.toolbars.editing;
    let mut drawn = Vec::new();
    Area::new(Id::unique("top-dock"))
        .order(Order::Background)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (card_rect, bg) = ui.allocate_exact_size(rect.size(), Sense::click());
            card(ui, card_rect);
            if bg.secondary_clicked() {
                hits.menu_at = bg.interact_pointer_pos();
            }
            for (name, r) in &bars {
                let r = r.translate(inner.min.to_vec2());
                let ids = ids_of(chrome, name);
                let buttons_rect = if editing {
                    title_bar(ui, name, r, r.min, hits);
                    edit_box(ui, r);
                    Rect::from_min_max(r.min + vec2(0.0, TITLE), r.max)
                } else {
                    r
                };
                buttons(
                    ui,
                    chrome,
                    ids,
                    buttons_rect,
                    lines_for(ids.len()),
                    false,
                    G,
                    r,
                    hits,
                );
                drawn.push((name.clone(), r));
            }
        });
    chrome.toolbars.drawn.extend(drawn);
}

/// A floating toolbar's outer size.
fn float_size(n: usize) -> Vec2 {
    block(n, float_columns(n), true, FLOAT_GAP) + vec2(2.0 * FLOAT_PAD, 2.0 * FLOAT_PAD + GRIP)
}

fn show_floating(chrome: &mut Chrome, ctx: &Context, hits: &mut Hits) {
    let floats: Vec<(String, f32, f32)> = chrome
        .toolbars
        .layout
        .floating()
        .into_iter()
        .map(|(n, x, y)| (n.to_owned(), x, y))
        .collect();
    let editing = chrome.toolbars.editing;
    for (name, x, y) in floats {
        if chrome
            .toolbars
            .drag
            .as_ref()
            .is_some_and(|d| d.name == name)
        {
            continue;
        }
        let ids = ids_of(chrome, &name);
        let size = float_size(ids.len());
        // From the app's corner, kept on screen if the window shrank.
        let screen = ctx.viewport_rect();
        let at = pos2(
            x.min(screen.right() - size.x).max(screen.left()),
            y.min(screen.bottom() - size.y).max(screen.top()),
        );
        let rect = Rect::from_min_size(at, size);
        Area::new(Id::unique(("float-toolbar", name.as_str())))
            .order(Order::Middle)
            .fixed_pos(at)
            .show(ctx, |ui| {
                let (r, bg) = ui.allocate_exact_size(size, Sense::click());
                let mut shapes = Vec::new();
                emboss::floating(
                    &mut shapes,
                    r,
                    radius::MENU_BUTTON + 4.0,
                    color::SURFACE,
                    1.0,
                );
                ui.painter().extend(shapes);
                if bg.secondary_clicked() {
                    hits.menu_at = bg.interact_pointer_pos();
                }
                // The grip: three dots; drag the toolbar by it.
                let grip = Rect::from_min_size(r.min, vec2(r.width(), GRIP + FLOAT_PAD));
                let g = ui.interact(
                    grip,
                    Id::unique(("float-grip", name.as_str())),
                    Sense::drag(),
                );
                if g.drag_started() {
                    hits.grabbed = Some((name.clone(), r.min));
                }
                for k in -1..=1 {
                    ui.painter().circle_filled(
                        grip.center() + vec2(k as f32 * 5.0, 0.0),
                        1.3,
                        color::MUTED,
                    );
                }
                if editing {
                    let x = Rect::from_min_size(
                        pos2(r.right() - GRIP - FLOAT_PAD, r.top()),
                        vec2(GRIP + FLOAT_PAD, GRIP + FLOAT_PAD),
                    );
                    let close =
                        ui.interact(x, Id::unique(("float-x", name.as_str())), Sense::click());
                    ui.painter().text(
                        x.center(),
                        egui::Align2::CENTER_CENTER,
                        "×",
                        theme::small(),
                        color::INK,
                    );
                    if close.clicked() {
                        hits.closed = Some(name.clone());
                    }
                }
                let inner = Rect::from_min_max(
                    r.min + vec2(FLOAT_PAD, FLOAT_PAD + GRIP),
                    r.max - vec2(FLOAT_PAD, FLOAT_PAD),
                );
                let cols = float_columns(ids.len());
                buttons(ui, chrome, ids, inner, cols, true, FLOAT_GAP, r, hits);
            });
        chrome.toolbars.drawn.push((name, rect));
    }
}

/// Open the toolbar menu asked for this frame (after the menus are drawn).
pub(super) fn open_menu(chrome: &mut Chrome) {
    if let Some(at) = chrome.toolbars.menu_at.take() {
        chrome.open_menu(MenuKind::Context {
            at,
            heading: "Toolbars".to_owned(),
            entries: vec![
                Entry::Action("view.edittoolbars"),
                Entry::Action("view.resettoolbars"),
                Entry::Action("view.toolbars"),
            ],
        });
    }
}

/// Edit mode's way out, at the top of the viewport.
fn done_button(chrome: &mut Chrome, ctx: &Context, area: Rect) {
    Area::new(Id::unique("toolbars-done"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, vec2(0.0, area.top() + 8.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Editing toolbars").color(color::INK));
                if ui.add(SoftButton::new("Done").small()).clicked() {
                    chrome.toolbars.editing = false;
                }
            });
        });
}

/// A toolbar being dragged: drawn under the pointer, and dropped on a
/// dock (before the toolbar under the pointer) or left floating there,
/// snapped to nearby edges.
fn drag(chrome: &mut Chrome, ctx: &Context, left: Rect, top: Rect, area: Rect) {
    let Some(d) = chrome.toolbars.drag.clone() else {
        return;
    };
    let Some(p) = ctx.pointer_latest_pos() else {
        return;
    };
    let n = ids_of(chrome, &d.name).len();
    let size = float_size(n);
    let corner = p - d.grab;
    // Snap against the other floating toolbars and the viewport.
    let others: Vec<Box2> = chrome
        .toolbars
        .drawn
        .iter()
        .filter(|(name, _)| {
            *name != d.name
                && chrome
                    .toolbars
                    .layout
                    .floating()
                    .iter()
                    .any(|f| f.0 == name)
        })
        .map(|(_, r)| Box2 {
            x: r.left(),
            y: r.top(),
            w: r.width(),
            h: r.height(),
        })
        .collect();
    let (sx, sy) = snap(
        Box2 {
            x: corner.x,
            y: corner.y,
            w: size.x,
            h: size.y,
        },
        &others,
        Box2 {
            x: area.left(),
            y: area.top(),
            w: area.width(),
            h: area.height(),
        },
    );
    let over_left = left.expand(4.0).contains(p);
    let over_top = top.expand(4.0).contains(p);
    // The ghost.
    Area::new(Id::unique("toolbar-drag"))
        .order(Order::Tooltip)
        .fixed_pos(pos2(sx, sy))
        .interactable(false)
        .show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(size, Sense::hover());
            let mut shapes = Vec::new();
            emboss::floating(
                &mut shapes,
                r,
                radius::MENU_BUTTON + 4.0,
                color::SURFACE,
                0.85,
            );
            ui.painter().extend(shapes);
            ui.painter().text(
                r.center_top() + vec2(0.0, GRIP),
                egui::Align2::CENTER_TOP,
                &d.name,
                theme::small(),
                color::INK,
            );
        });
    for (on, r) in [(over_left, left), (over_top, top)] {
        if on {
            ctx.layer_painter(egui::LayerId::new(Order::Tooltip, Id::unique("drop")))
                .rect_stroke(
                    r,
                    theme::corner(radius::CARD),
                    egui::Stroke::new(size::FOCUS_WIDTH, color::ACCENT),
                    egui::StrokeKind::Inside,
                );
        }
    }
    if !ctx.input(|i| i.pointer.any_down()) {
        // Dropped: before the toolbar under the pointer, else at the end.
        let t = &mut chrome.toolbars;
        let index_in = |dock: Dock, along: fn(Rect, Pos2) -> bool| {
            t.layout
                .in_dock(dock)
                .iter()
                .position(|n| {
                    t.drawn
                        .iter()
                        .find(|(m, _)| m == n)
                        .is_some_and(|(_, r)| along(*r, p))
                })
                .unwrap_or(usize::MAX)
        };
        if over_left {
            let i = index_in(Dock::Left, |r, p| p.y < r.center().y);
            t.layout.dock(&d.name, Dock::Left, i);
        } else if over_top {
            let i = index_in(Dock::Top, |r, p| p.x < r.center().x && p.y < r.bottom());
            t.layout.dock(&d.name, Dock::Top, i);
        } else {
            t.layout.float(&d.name, sx, sy);
        }
        t.drag = None;
    }
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_columns_fit_with_room_for_the_scroll_bar() {
        // 10 + 44 + 8 + 44 + 10, as the prototype's 116 px dock.
        assert_eq!(width(2), 116.0);
        assert_eq!(width(1), 64.0);
    }

    #[test]
    fn key_hints() {
        let h = |k: &str| hint(Chord::parse(k).as_ref());
        assert_eq!(h("L"), "L");
        assert_eq!(h("Space"), "Spc");
        assert_eq!(h("Shift+Z"), "");
        assert_eq!(h("Del"), "");
        assert_eq!(hint(None), "");
    }

    #[test]
    fn docked_toolbars_take_the_lines_they_need() {
        assert_eq!(lines_for(3), 1, "Principal: one line");
        assert_eq!(lines_for(8), 2, "Draw: two");
        assert_eq!(lines_for(30), DOCK_THICKNESS, "never thicker than a dock");
    }

    #[test]
    fn floating_toolbars_are_tighter_than_the_rail() {
        // Three buttons: one column, padding 4 and a grip.
        assert_eq!(
            float_size(3),
            vec2(4.0 + 44.0 + 4.0, 4.0 + 10.0 + 3.0 * 44.0 + 2.0 * 4.0 + 4.0)
        );
        // Eight buttons: two columns.
        assert_eq!(float_size(8).x, 4.0 + 2.0 * 44.0 + 4.0 + 4.0);
    }
}
