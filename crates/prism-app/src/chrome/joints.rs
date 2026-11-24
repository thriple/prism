//! The bottom-centre joint island (brief 53's diegetic design, owner
//! 2026-11-24): while a Hinge/Slide tool is active, or an existing joint
//! is selected, a small Motion Kit `DynamicIsland`-style pill shows each
//! bound that is set, as a typed field to correct exactly, and Remove
//! Joint. Nothing else lives here — no part picker, axis field or
//! Stopped toggle; those are the gizmo's own job in the viewport.

use egui::{Align2, Area, Context, Frame, Id, Margin, Order, Rect, Shape, Ui, vec2};
use prism_tools::session::JointIslandSet;
use prism_ui::emboss;
use prism_ui::theme::{color, radius, size};
use prism_ui::widgets::{SoftButton, field};

use super::panels::{PanelData, PanelOp, Panels};

/// How far above the viewport's own bottom edge the island floats.
const PAD: f32 = 16.0;
const FIELD_WIDTH: f32 = 64.0;

pub(super) fn show(
    ctx: &Context,
    viewport: Rect,
    data: &PanelData,
    p: &mut Panels,
    ops: &mut Vec<PanelOp>,
) {
    let Some(j) = data.joint_island.clone() else {
        p.joint_typing = None;
        return;
    };
    let screen = ctx.viewport_rect();
    Area::new(Id::unique("joint-island"))
        .order(Order::Foreground)
        .movable(false)
        .anchor(
            Align2::CENTER_BOTTOM,
            vec2(
                viewport.center().x - screen.center().x,
                viewport.bottom() - screen.bottom() - PAD,
            ),
        )
        .show(ctx, |ui| {
            let bg = ui.painter().add(Shape::Noop);
            let pad = (size::PANEL_PAD * 0.6) as i8;
            let inner = Frame::new()
                .inner_margin(Margin::symmetric(size::PANEL_PAD as i8, pad))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (max, value) in [(false, &j.min), (true, &j.max)] {
                            if let Some(committed) = value {
                                bound_field(ui, p, ops, max, committed);
                            }
                        }
                        if ui.add(SoftButton::new("Remove Joint").small()).clicked() {
                            ops.push(PanelOp::JointRemove);
                        }
                    });
                });
            let mut shapes = Vec::new();
            emboss::floating(
                &mut shapes,
                inner.response.rect,
                radius::ROUND,
                color::CURSOR_INK,
                1.0,
            );
            ui.painter().set(bg, Shape::Vec(shapes));
        });
}

fn bound_field(ui: &mut Ui, p: &mut Panels, ops: &mut Vec<PanelOp>, max: bool, committed: &str) {
    ui.label(egui::RichText::new(if max { "Max" } else { "Min" }).color(color::INK));
    let mut text = p
        .joint_typing
        .clone()
        .filter(|(m, _)| *m == max)
        .map(|(_, t)| t)
        .unwrap_or_else(|| committed.to_owned());
    let f = field(ui, &mut text, committed, FIELD_WIDTH);
    if f.has_focus() {
        p.joint_typing = Some((max, text.clone()));
    }
    if f.lost_focus() {
        p.joint_typing = None;
        if text.trim() != committed {
            ops.push(PanelOp::JointBound(if max {
                JointIslandSet::Max(text)
            } else {
                JointIslandSet::Min(text)
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, vec2};
    use prism_doc::{JointId, JointKind};
    use prism_tools::session::JointIsland;

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))
    }

    fn input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(screen()),
            ..Default::default()
        }
    }

    #[test]
    fn nothing_is_drawn_or_done_with_no_joint_in_context() {
        let ctx = Context::default();
        let data = PanelData::default();
        let mut p = Panels::default();
        let mut ops = Vec::new();
        let mut out = ctx.run_ui(input(), |_ui| show(&ctx, screen(), &data, &mut p, &mut ops));
        out.textures_delta.clear();
        assert!(ops.is_empty());
        assert!(p.joint_typing.is_none());
    }

    #[test]
    fn draws_the_set_bound_and_remove_button_without_interaction_doing_anything() {
        let ctx = Context::default();
        let data = PanelData {
            joint_island: Some(JointIsland {
                id: JointId(0),
                kind: JointKind::Slide,
                min: None,
                max: Some("0.600 m".to_owned()),
            }),
            ..Default::default()
        };
        let mut p = Panels::default();
        let mut ops = Vec::new();
        let mut out = ctx.run_ui(input(), |_ui| show(&ctx, screen(), &data, &mut p, &mut ops));
        out.textures_delta.clear();
        // No click, no typing: showing it alone does nothing.
        assert!(ops.is_empty());
    }
}
