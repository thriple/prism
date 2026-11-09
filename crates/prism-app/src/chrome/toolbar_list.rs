//! The Toolbars window (brief 19, part 2): every toolbar, greyed if it is
//! showing. A closed one is dragged out of the list to bring it back; it
//! drops like any dragged toolbar. Nothing behind it is dimmed.

use egui::{Align2, Context, RichText, Sense, pos2, vec2};
use prism_ui::emboss;
use prism_ui::theme::{self, color, depth, radius, size};
use prism_ui::widgets::{SoftButton, SoftDialog};

use super::{Chrome, Source, dock};

/// The icons in each row, and the space between them.
const ICON: f32 = 18.0;
const ICON_GAP: f32 = 4.0;
/// How much of the row the name takes.
const NAME_WIDTH: f32 = 120.0;

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let names = dock::names(chrome);
    let mut grabbed = None;
    let mut reset = false;
    let r = SoftDialog::new("Toolbars")
        .subtitle("Drag a closed toolbar out to show it")
        .undimmed()
        .show(ctx, chrome.toolbars.window, |ui| {
            ui.add_space(size::MENU_PAD);
            let w = ui.available_width();
            for name in &names {
                let showing = chrome.toolbars.layout.is_open(name);
                let (row, resp) = ui.allocate_exact_size(
                    vec2(w, size::MENU_ROW_HEIGHT),
                    if showing {
                        Sense::hover()
                    } else {
                        Sense::drag()
                    },
                );
                let ink = if showing { color::MUTED } else { color::INK };
                if !showing && (resp.hovered() || resp.dragged()) {
                    let mut v = Vec::new();
                    emboss::inset(&mut v, row, radius::MENU_BUTTON, depth::MENU_INSET);
                    ui.painter().extend(v);
                    ctx.set_cursor_icon(egui::CursorIcon::Grab);
                }
                if resp.drag_started() {
                    grabbed = Some(*name);
                }
                let p = ui.painter();
                p.text(
                    row.left_center() + vec2(size::MENU_PAD, 0.0),
                    Align2::LEFT_CENTER,
                    *name,
                    theme::body(),
                    ink,
                );
                // Its buttons' icons, as many as fit.
                let mut x = row.left() + NAME_WIDTH;
                for id in dock::ids_of(chrome, name) {
                    if x + ICON > row.right() - size::MENU_PAD {
                        break;
                    }
                    if let Some(icon) = chrome.icons.get(id) {
                        let r = egui::Rect::from_min_size(
                            pos2(x, row.center().y - ICON * 0.5),
                            vec2(ICON, ICON),
                        );
                        icon.paint(p, r, ink);
                    }
                    x += ICON + ICON_GAP;
                }
            }
            ui.add_space(size::MENU_PAD);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Greyed toolbars are showing.")
                        .font(theme::small())
                        .color(color::MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(SoftButton::new("Reset Toolbars").small()).clicked() {
                        reset = true;
                    }
                });
            });
        });
    if r.close {
        chrome.toolbars.window = false;
    }
    if reset {
        chrome.run("view.resettoolbars", Source::Menu);
    }
    if let Some(name) = grabbed {
        chrome.toolbars.drag = Some(dock::Drag {
            name: name.to_owned(),
            grab: dock::grab_point(chrome, name),
        });
    }
}
