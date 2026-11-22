//! The Primitives window (Shapes & Placement part 4): the six kinds as
//! buttons and the sizes of the chosen one as fields. In the side column
//! while the Primitives tool is active; click in the model to set one down,
//! or press and drag.

use egui::{RichText, Ui};
use prism_tools::session::PrimSet;
use prism_ui::theme::{color, size};
use prism_ui::widgets::{SideCard, SoftButton, field};

use super::panels::{PanelData, PanelOp, Panels, after_card};

const TITLE: &str = "Primitives";

pub(super) fn card(ui: &mut Ui, p: &mut Panels, data: &PanelData, ops: &mut Vec<PanelOp>) {
    let Some(a) = data.prim.as_ref().filter(|_| p.prim) else {
        return;
    };
    let resp = SideCard::new(TITLE)
        .subtitle(a.kinds.get(a.current).copied().unwrap_or(""))
        .show(ui, p.folded.contains(TITLE), |ui| {
            for row in a.kinds.chunks(3).enumerate() {
                ui.horizontal(|ui| {
                    for (k, name) in row.1.iter().enumerate() {
                        let i = row.0 * 3 + k;
                        if ui
                            .add(SoftButton::new(*name).small().selected(a.current == i))
                            .clicked()
                        {
                            p.prim_typing = None;
                            ops.push(PanelOp::Prim(PrimSet::Kind(i)));
                        }
                    }
                });
            }
            ui.add_space(size::MENU_PAD);
            for (key, label, shown) in &a.fields {
                ui.horizontal(|ui| {
                    ui.label(*label);
                    let mine = p.prim_typing.as_ref().filter(|(k, _)| k == key);
                    let mut text = mine.map_or_else(|| shown.clone(), |(_, t)| t.clone());
                    let f = field(ui, &mut text, shown, 120.0)
                        .on_hover_text("A size: type it with units if you like, then Enter");
                    if f.has_focus() {
                        p.prim_typing = Some((*key, text.clone()));
                    }
                    if f.lost_focus() {
                        p.prim_typing = None;
                        if text.trim() != shown {
                            ops.push(PanelOp::Prim(PrimSet::Size(key, text)));
                        }
                    }
                });
                ui.add_space(size::MENU_PAD / 2.0);
            }
            ui.add_space(size::MENU_PAD / 2.0);
            ui.label(RichText::new(a.how).color(color::MUTED));
        });
    if after_card(ui, &mut p.folded, TITLE, resp) {
        p.prim = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Registry;
    use crate::actions::keymap::{Keymap, Platform, Preset};
    use crate::chrome::{Chrome, Source};
    use egui::{Pos2, Rect, vec2};

    #[test]
    fn a_kind_from_the_menu_is_the_tool_and_the_window_draws_its_sizes() {
        let reg = Registry::new();
        let mut c = Chrome::with_keymap(
            Platform::Windows,
            Keymap::from_preset(Preset::SketchUp, &reg),
        );
        c.run("prim.cyl", Source::Menu);
        assert_eq!(c.active_tool, prism_tools::session::PRIM_TOOL);
        assert!(
            c.panels.prim && c.side_open(),
            "docked, so the view is not dimmed"
        );
        assert_eq!(c.events.last().map(|e| e.id), Some("prim.cyl"));
        // The window draws for a panel with fields, and does nothing alone.
        let mut session = prism_tools::Session::new(prism_doc::Document::new());
        session.handle(&prism_tools::session::Event {
            t: 0.0,
            input: prism_tools::session::Input::Tool {
                id: prism_tools::session::PRIM_TOOL.into(),
            },
        });
        let data = PanelData {
            prim: session.prim_panel(),
            ..Default::default()
        };
        assert!(data.prim.as_ref().is_some_and(|p| p.fields.len() == 3));
        let ctx = egui::Context::default();
        let mut p = c.panels.clone();
        let mut ops = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 500.0))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| card(ui, &mut p, &data, &mut ops));
        out.textures_delta.clear();
        assert!(ops.is_empty() && p.prim);
    }
}
