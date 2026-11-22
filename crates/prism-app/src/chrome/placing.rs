//! The Align and Distribute windows (Shapes & Placement part 3): side
//! cards opened from the dock or the Window menu. Align is a 3 x 3 grid
//! like an anchor picker: right-angle wedges for the corners, short bars
//! for the edges and a hollow square for the middle. Distribute has one
//! button for each of red, green and blue.

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};
use prism_ui::theme::{color, size};
use prism_ui::widgets::{SideCard, SoftButton};

use super::panels::{PanelData, PanelOp, Panels, after_card};

const ALIGN: &str = "Align";
const DISTRIBUTE: &str = "Distribute";

const CELL: f32 = 46.0;
const GAP: f32 = 8.0;
const GLYPH: Color32 = Color32::from_rgb(0xc8, 0xc8, 0xc8);
const FRAME: Color32 = Color32::from_rgb(0x56, 0x58, 0x60);

pub(super) fn align_card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    if !p.align {
        return;
    }
    let ready = data.groups >= 2;
    let resp = SideCard::new(ALIGN)
        .subtitle("Selected groups, by their boxes")
        .show(ui, p.folded.contains(ALIGN), |ui| {
            ui.horizontal(|ui| {
                ui.label("Seen from");
                for (k, label) in ["Front", "Side", "Top"].into_iter().enumerate() {
                    if ui
                        .add(SoftButton::new(label).small().selected(p.align_plane == k))
                        .on_hover_text(match k {
                            0 => "Red across, blue up",
                            1 => "Green across, blue up",
                            _ => "Red across, green up (the back is up)",
                        })
                        .clicked()
                    {
                        p.align_plane = k;
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            let (across, up) = match p.align_plane {
                0 => ("red", "blue"),
                1 => ("green", "blue"),
                _ => ("red", "green"),
            };
            ui.horizontal(|ui| {
                // Centred in the column.
                let w = 3.0 * CELL + 2.0 * GAP;
                ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = vec2(GAP, GAP);
                    for row in 0..3 {
                        ui.horizontal(|ui| {
                            for col in 0..3 {
                                let tip = cell_tip(col, row, across, up);
                                if cell(ui, glyph_of(col, row), ready, &tip) {
                                    ops.push(PanelOp::Align {
                                        plane: p.align_plane,
                                        col,
                                        row,
                                    });
                                }
                            }
                        });
                    }
                });
            });
            ui.add_space(size::MENU_PAD);
            ui.label(
                RichText::new(if ready {
                    "Corners line up two ways, edges one way, the middle centres both."
                } else {
                    "Select two or more groups or components."
                })
                .color(color::MUTED),
            );
        });
    if after_card(ui, &mut p.folded, ALIGN, resp) {
        p.align = false;
    }
}

pub(super) fn distribute_card(
    ui: &mut Ui,
    p: &mut Panels,
    data: &PanelData,
    ops: &mut Vec<PanelOp>,
) {
    if !p.distribute {
        return;
    }
    let ready = data.groups >= 3;
    let resp = SideCard::new(DISTRIBUTE)
        .subtitle("Equal gaps between the boxes")
        .show(ui, p.folded.contains(DISTRIBUTE), |ui| {
            ui.horizontal(|ui| {
                let w = 3.0 * CELL + 2.0 * GAP;
                ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                ui.spacing_mut().item_spacing = vec2(GAP, GAP);
                for (axis, name) in ["red", "green", "blue"].into_iter().enumerate() {
                    let tip = format!("Space them evenly along {name}; the two outermost stay");
                    if cell(ui, Glyph::Spread(axis), ready, &tip) {
                        ops.push(PanelOp::Distribute(axis));
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            ui.horizontal(|ui| {
                let w = 3.0 * CELL + 2.0 * GAP;
                ui.add_space(((ui.available_width() - w) / 2.0).max(0.0));
                ui.spacing_mut().item_spacing = vec2(GAP, GAP);
                for name in ["Red", "Green", "Blue"] {
                    ui.add_sized(
                        [CELL, 16.0],
                        egui::Label::new(RichText::new(name).color(color::MUTED)),
                    );
                }
            });
            ui.add_space(size::MENU_PAD);
            ui.label(
                RichText::new(if ready {
                    "Each one keeps its place across; the gaps along the axis are made equal."
                } else {
                    "Select three or more groups or components."
                })
                .color(color::MUTED),
            );
        });
    if after_card(ui, &mut p.folded, DISTRIBUTE, resp) {
        p.distribute = false;
    }
}

/// What a cell shows.
#[derive(Clone, Copy)]
enum Glyph {
    /// A right-angle wedge in a corner: 0 top left, 1 top right, 2 bottom
    /// left, 3 bottom right.
    Wedge(usize),
    /// A short vertical bar (top and bottom edges).
    Upright,
    /// A short horizontal bar (left and right edges).
    Flat,
    /// The hollow square.
    Middle,
    /// Three bars spread along red (0), green (1) or blue (2).
    Spread(usize),
}

fn glyph_of(col: usize, row: usize) -> Glyph {
    match (col, row) {
        (0, 0) => Glyph::Wedge(0),
        (2, 0) => Glyph::Wedge(1),
        (0, 2) => Glyph::Wedge(2),
        (2, 2) => Glyph::Wedge(3),
        (1, 1) => Glyph::Middle,
        (1, _) => Glyph::Upright,
        _ => Glyph::Flat,
    }
}

fn cell_tip(col: usize, row: usize, across: &str, up: &str) -> String {
    let h = ["left", "centre", "right"][col];
    let v = ["top", "middle", "bottom"][row];
    match (col, row) {
        (1, 1) => format!("Centre them both ways (across {across} and {up})"),
        (1, _) => format!("Line up the {v} sides ({up})"),
        (_, 1) => format!("Line up the {h} sides ({across})"),
        _ => format!("Line up the {h} and {v} sides"),
    }
}

/// One button: two nested frames and a light glyph. True if clicked.
fn cell(ui: &mut Ui, glyph: Glyph, enabled: bool, tip: &str) -> bool {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(CELL), sense);
    let resp = resp.on_hover_text(tip);
    let painter = ui.painter_at(rect.expand(2.0));
    let lit = enabled && resp.hovered();
    let (frame, ink) = if !enabled {
        (FRAME.gamma_multiply(0.5), GLYPH.gamma_multiply(0.35))
    } else if lit {
        (color::ACCENT, color::ACCENT)
    } else {
        (FRAME, GLYPH)
    };
    let stroke = Stroke::new(1.5, frame);
    painter.rect_stroke(rect, 3.0, stroke, egui::StrokeKind::Inside);
    let inner = rect.shrink(4.0);
    painter.rect_stroke(
        inner,
        2.0,
        Stroke::new(1.0, frame),
        egui::StrokeKind::Inside,
    );
    draw_glyph(&painter, inner.shrink(6.0), glyph, ink);
    enabled && resp.clicked()
}

fn draw_glyph(painter: &egui::Painter, r: Rect, glyph: Glyph, ink: Color32) {
    let t = 4.0; // the thickness of every mark
    let fill = |a: Pos2, b: Pos2| {
        painter.rect_filled(Rect::from_two_pos(a, b), 0.5, ink);
    };
    let c = r.center();
    match glyph {
        Glyph::Wedge(k) => {
            // An L in the corner: two arms of a right angle.
            let (left, top) = (k % 2 == 0, k < 2);
            let arm = r.width() * 0.75;
            let x = if left { r.left() } else { r.right() };
            let y = if top { r.top() } else { r.bottom() };
            let dx = if left { 1.0 } else { -1.0 };
            let dy = if top { 1.0 } else { -1.0 };
            fill(pos2(x, y), pos2(x + dx * arm, y + dy * t));
            fill(pos2(x, y), pos2(x + dx * t, y + dy * arm));
        }
        Glyph::Upright => fill(
            pos2(c.x - t / 2.0, c.y - r.height() * 0.38),
            pos2(c.x + t / 2.0, c.y + r.height() * 0.38),
        ),
        Glyph::Flat => fill(
            pos2(c.x - r.width() * 0.38, c.y - t / 2.0),
            pos2(c.x + r.width() * 0.38, c.y + t / 2.0),
        ),
        Glyph::Middle => {
            let h = r.width() * 0.42;
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(h * 2.0)),
                0.5,
                Stroke::new(t, ink),
                egui::StrokeKind::Inside,
            );
        }
        Glyph::Spread(axis) => {
            let reach = r.width() * 0.4;
            for k in -1..=1 {
                let k = k as f32;
                let at = match axis {
                    0 => pos2(c.x + k * reach, c.y),
                    1 => pos2(c.x + k * reach * 0.8, c.y - k * reach * 0.8),
                    _ => pos2(c.x, c.y + k * reach),
                };
                let half = match axis {
                    0 => vec2(t / 2.0, r.height() * 0.3),
                    2 => vec2(r.width() * 0.3, t / 2.0),
                    _ => vec2(t / 2.0 + 1.0, t / 2.0 + 1.0),
                };
                painter.rect_filled(Rect::from_center_size(at, half * 2.0), 0.5, ink);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Registry;
    use crate::actions::keymap::{Keymap, Platform, Preset};
    use crate::chrome::{Chrome, Source};

    fn frame(ctx: &egui::Context, mut run: impl FnMut(&mut Ui)) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 500.0))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| run(ui));
        out.textures_delta.clear();
    }

    #[test]
    fn the_grid_has_wedges_in_the_corners_bars_on_the_edges_and_a_square_in_the_middle() {
        assert!(matches!(glyph_of(0, 0), Glyph::Wedge(0)));
        assert!(matches!(glyph_of(2, 0), Glyph::Wedge(1)));
        assert!(matches!(glyph_of(0, 2), Glyph::Wedge(2)));
        assert!(matches!(glyph_of(2, 2), Glyph::Wedge(3)));
        assert!(matches!(glyph_of(1, 0), Glyph::Upright));
        assert!(matches!(glyph_of(1, 2), Glyph::Upright));
        assert!(matches!(glyph_of(0, 1), Glyph::Flat));
        assert!(matches!(glyph_of(2, 1), Glyph::Flat));
        assert!(matches!(glyph_of(1, 1), Glyph::Middle));
        assert!(cell_tip(0, 0, "red", "blue").contains("left and top"));
        assert!(cell_tip(1, 1, "red", "blue").contains("Centre"));
    }

    #[test]
    fn the_windows_open_from_their_actions_and_draw_ready_or_not() {
        let reg = Registry::new();
        let mut c = Chrome::with_keymap(
            Platform::Windows,
            Keymap::from_preset(Preset::SketchUp, &reg),
        );
        assert!(!c.panels.align && !c.panels.distribute);
        c.run("win.align", Source::Menu);
        c.run("win.distribute", Source::Menu);
        assert!(c.panels.align && c.panels.distribute);
        assert!(c.panels.any_open(), "docked, so they do not dim the view");
        let ctx = egui::Context::default();
        for groups in [0, 2, 3] {
            let data = PanelData {
                groups,
                ..Default::default()
            };
            let mut p = c.panels.clone();
            let mut ops = Vec::new();
            frame(&ctx, |ui| {
                align_card(ui, &mut p, &data, &mut ops);
                distribute_card(ui, &mut p, &data, &mut ops);
            });
            assert!(ops.is_empty(), "nothing is done without a click");
            assert!(p.align && p.distribute);
        }
    }

    #[test]
    fn every_placing_function_has_an_action_with_a_hotkey_slot_and_none_by_default() {
        let reg = Registry::new();
        let keys = Keymap::from_preset(Preset::SketchUp, &reg);
        let mut ids: Vec<String> = Vec::new();
        for ax in ["x", "y", "z"] {
            for side in ["low", "middle", "high"] {
                ids.push(format!("align.{ax}.{side}"));
            }
            ids.push(format!("dist.{ax}"));
        }
        ids.extend(["edit.drop", "edit.center", "win.align", "win.distribute"].map(String::from));
        for id in &ids {
            let a = reg
                .get(id)
                .unwrap_or_else(|| panic!("{id} is not an action"));
            assert!(
                keys.primary(a.id).is_none(),
                "{id} has no hotkey by default"
            );
        }
        // And each one can be given a key (the hotkeys window lists it).
        assert!(
            reg.all()
                .iter()
                .filter(|a| ids.iter().any(|i| i == a.id))
                .count()
                == ids.len()
        );
    }
}
