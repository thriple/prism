//! The tool dock: groups of square buttons in two columns, from the registry.

use egui::{Area, Context, Id, Order, Rect, ScrollArea, Sense, vec2};
use prism_ui::emboss;
use prism_ui::theme::{color, depth, radius, size};
use prism_ui::widgets::{ToolButton, menu, tooltip};

use super::{Chrome, Source};
use crate::actions::keymap::{Chord, Mods, key_label};

// The scroll bar floats inside the side padding, so it never covers the
// buttons or needs a horizontal scroll bar. Checked when compiling.
const _: () = assert!(size::DOCK_SCROLL_BAR <= size::DOCK_PAD_X);

/// The dock's outer width: two columns of buttons, the gap between them and
/// the side padding. The scroll bar floats inside the padding, so the dock
/// never needs a horizontal scroll bar (the prototype's dock did).
pub fn width() -> f32 {
    let cols = size::DOCK_COLUMNS as f32;
    2.0 * size::DOCK_PAD_X + cols * size::DOCK_BUTTON + (cols - 1.0) * size::DOCK_GAP
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

pub(super) fn show(chrome: &mut Chrome, ctx: &Context, rect: Rect) {
    let mut hovered: Option<(Rect, &'static str)> = None;
    let mut clicked: Option<&'static str> = None;
    Area::new(Id::unique("dock"))
        .order(Order::Background)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            let (card, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
            let mut shapes = Vec::new();
            emboss::soft(
                &mut shapes,
                card,
                radius::CARD,
                1.0,
                depth::CARD,
                color::SURFACE,
            );
            ui.painter().extend(shapes);

            // The scroll area reaches into the right padding, where its thin
            // floating bar sits; the buttons keep their own width.
            let inner = card.shrink2(vec2(size::DOCK_PAD_X, size::DOCK_PAD_Y));
            let area = Rect::from_min_max(
                inner.min,
                egui::pos2(card.right() - size::DOCK_SCROLL_BAR * 0.5, inner.bottom()),
            );
            let mut content = ui.new_child(egui::UiBuilder::new().max_rect(area));
            let scroll = &mut content.style_mut().spacing.scroll;
            *scroll = egui::style::ScrollStyle::floating();
            scroll.bar_width = size::DOCK_SCROLL_BAR;
            scroll.bar_outer_margin = 0.0;
            scroll.bar_inner_margin = 0.0;
            // Hidden until the pointer is over the dock.
            scroll.dormant_handle_opacity = 0.0;
            scroll.dormant_background_opacity = 0.0;
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(&mut content, |ui| {
                    ui.spacing_mut().item_spacing = vec2(size::DOCK_GAP, size::DOCK_GAP);
                    for (gi, (group, ids)) in chrome.reg.dock().iter().enumerate() {
                        if gi > 0 {
                            ui.add_space(size::DOCK_GROUP_GAP);
                        }
                        let job = menu::caps(group, color::MUTED);
                        ui.label(job);
                        for row in ids.chunks(size::DOCK_COLUMNS) {
                            ui.horizontal(|ui| {
                                for id in row {
                                    let label = hint(chrome.keymap.primary(id).as_ref());
                                    let r = ui.add(
                                        ToolButton::new(chrome.icons.get(id))
                                            .hint(&label)
                                            .active(chrome.checked(id))
                                            .enabled(chrome.enabled(id)),
                                    );
                                    if r.hovered() {
                                        hovered = Some((r.rect, id));
                                    }
                                    if r.clicked() {
                                        clicked = Some(id);
                                    }
                                }
                            });
                        }
                    }
                });
        });
    if let Some(id) = clicked {
        chrome.run(id, Source::Dock);
    }
    let text = hovered.map(|(_, id)| chrome.tip(id)).unwrap_or_default();
    // Beside the dock, level with the button, so it never covers the other column.
    let anchor = hovered.map(|(r, _)| {
        Rect::from_min_max(
            egui::pos2(rect.left(), r.top()),
            egui::pos2(rect.right(), r.bottom()),
        )
    });
    tooltip::shared(ctx, Id::unique("dock-tip"), anchor, &text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_columns_fit_with_room_for_the_scroll_bar() {
        // 10 + 44 + 8 + 44 + 10, as the prototype's 116 px dock.
        assert_eq!(width(), 116.0);
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
}
