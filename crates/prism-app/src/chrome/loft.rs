//! The Loft window (brief 34): the Loft tool's threshold, its sides, Next
//! Side and Loft, and the presets, in the side column while the tool is
//! active.

use egui::{RichText, Ui};
use prism_tools::session::LoftOp;
use prism_ui::theme::{color, size};
use prism_ui::widgets::{SideCard, SoftButton, SoftSlider};

use super::panels::{PanelData, PanelOp, Panels, after_card};

pub(super) fn card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let Some(r) = data.loft.as_ref().filter(|_| p.loft) else {
        return;
    };
    let subtitle = r.makes.clone().unwrap_or_else(|| "Pick sides".to_owned());
    let resp =
        SideCard::new("Loft")
            .subtitle(&subtitle)
            .show(ui, p.folded.contains("Loft"), |ui| {
                // The kit's slider shows its value only while held: say it.
                muted(ui, &format!("Sharpest turn in a side: {:.0}°", r.turn_deg));
                let mut turn = r.turn_deg as f32;
                if ui
                    .add(
                        SoftSlider::new(&mut turn, 1.0..=90.0, "Sharpest turn in a side")
                            .step(1.0)
                            .suffix("°"),
                    )
                    .changed()
                {
                    ops.push(PanelOp::Loft(LoftOp::Turn(f64::from(turn))));
                }
                ui.add_space(size::MENU_PAD);
                for (i, n) in r.sides.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let edges = if *n == 1 { "edge" } else { "edges" };
                        ui.label(format!("Side {}: {n} {edges}", i + 1));
                        if ui
                            .add(SoftButton::new("Remove").small())
                            .on_hover_text("Take this side out")
                            .clicked()
                        {
                            ops.push(PanelOp::Loft(LoftOp::Remove(i)));
                        }
                    });
                }
                if r.current > 0 {
                    let edges = if r.current == 1 { "edge" } else { "edges" };
                    muted(
                        ui,
                        &format!(
                            "Side {}: {} {edges} (picking)",
                            r.sides.len() + 1,
                            r.current
                        ),
                    );
                }
                ui.add_space(size::MENU_PAD);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(r.can_next, SoftButton::new("Next Side").small())
                        .on_hover_text("Finish this side (Enter)")
                        .clicked()
                    {
                        ops.push(PanelOp::Loft(LoftOp::NextSide));
                    }
                    if ui
                        .add_enabled(r.can_make, SoftButton::new("Loft").small())
                        .on_hover_text("Make the surface (Enter)")
                        .clicked()
                    {
                        ops.push(PanelOp::Loft(LoftOp::Make));
                    }
                });
                ui.add_space(size::MENU_PAD);
                ui.horizontal(|ui| {
                    for (label, straight, tip) in [
                        ("Smooth", false, "Smooth through the sections"),
                        ("Straight", true, "Straight between sections"),
                    ] {
                        if ui
                            .add(
                                SoftButton::new(label)
                                    .small()
                                    .selected(r.straight == straight),
                            )
                            .on_hover_text(tip)
                            .clicked()
                        {
                            ops.push(PanelOp::Loft(LoftOp::Straight(straight)));
                        }
                    }
                });
                muted(ui, &format!("Rows: {}", r.rows));
                let mut rows = r.rows as f32;
                if ui
                    .add(SoftSlider::new(&mut rows, 1.0..=8.0, "Rows").step(1.0))
                    .changed()
                {
                    ops.push(PanelOp::Loft(LoftOp::Rows(rows.round() as usize)));
                }
                ui.add_space(size::MENU_PAD);
                if let Some(tip) = &r.tip {
                    ui.label(RichText::new(tip).color(color::INK));
                }
                if r.changes_last {
                    muted(ui, "A preset changes the last loft.");
                }
                for line in r.how.split(" · ") {
                    let mut c = line.chars();
                    let line: String = c
                        .next()
                        .map(|f| f.to_uppercase().chain(c).collect())
                        .unwrap_or_default();
                    muted(ui, &line);
                }
            });
    if after_card(ui, &mut p.folded, "Loft", resp) {
        p.loft = false;
    }
}

fn muted(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(color::MUTED));
}
