//! The Shell / Thicken window (brief 42, part 3d): the thickness a click
//! makes (empty sizes by the pointer), inside or outside, and the faces
//! left open. In the side column while the tool is active.

use egui::{RichText, Ui};
use prism_tools::session::ShellSet;
use prism_ui::theme::{color, size};
use prism_ui::widgets::{SideCard, SoftButton, field};

use super::panels::{PanelData, PanelOp, Panels, after_card};

pub(super) fn card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let Some(r) = data.shell.as_ref().filter(|_| p.shell) else {
        return;
    };
    let resp = SideCard::new("Shell / Thicken")
        .subtitle(if r.size.is_empty() {
            "Sized by the pointer"
        } else if r.outside {
            "New wall outside"
        } else {
            "New wall inside"
        })
        .show(ui, p.folded.contains("Shell / Thicken"), |ui| {
            ui.horizontal(|ui| {
                ui.label("Thickness");
                // The field keeps what is typed while it has the keys, and
                // shows the tool's thickness otherwise.
                let mut text = p.shell_size.clone().unwrap_or_else(|| r.size.clone());
                let f = field(ui, &mut text, "By the pointer", 120.0);
                if f.has_focus() {
                    p.shell_size = Some(text.clone());
                }
                if f.lost_focus() {
                    p.shell_size = None;
                    if text.trim() != r.size {
                        ops.push(PanelOp::ShellSize(text));
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            ui.horizontal(|ui| {
                let set = !r.size.is_empty();
                for (label, out, tip) in [
                    ("Inside", false, "The new wall goes inside the faces"),
                    ("Outside", true, "The new wall grows outside the faces"),
                ] {
                    let tip = if set {
                        tip
                    } else {
                        "Type a thickness first; without one, move the pointer out or in"
                    };
                    if ui
                        .add_enabled(
                            set,
                            SoftButton::new(label)
                                .small()
                                .selected(set && r.outside == out),
                        )
                        .on_hover_text(tip)
                        .on_disabled_hover_text(tip)
                        .clicked()
                    {
                        ops.push(PanelOp::Shell(ShellSet::Outside(out)));
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            ui.horizontal(|ui| {
                muted(
                    ui,
                    &match r.open {
                        0 => "No faces left open".to_owned(),
                        1 => "1 face left open".to_owned(),
                        n => format!("{n} faces left open"),
                    },
                );
                if r.open > 0
                    && ui
                        .add(SoftButton::new("Close all").small())
                        .on_hover_text("Close every face left open")
                        .clicked()
                {
                    ops.push(PanelOp::Shell(ShellSet::CloseAll));
                }
            });
            ui.add_space(size::MENU_PAD);
            if let Some(why) = r.refused {
                ui.label(RichText::new(why).color(color::INK));
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
    if after_card(ui, &mut p.folded, "Shell / Thicken", resp) {
        p.shell = false;
    }
}

fn muted(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(color::MUTED));
}
