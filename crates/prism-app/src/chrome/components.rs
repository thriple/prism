//! Window > Components (Groups & Components part 2): the document's
//! components, how many instances each has, select them all, rename.

use egui::{Context, RichText};
use prism_ui::theme::{self, color, size};
use prism_ui::widgets::{SoftButton, SoftDialog, field};

use super::Chrome;

/// One row: the definition's number, its name, its instances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentRow {
    pub def: u32,
    pub name: String,
    pub uses: usize,
}

/// What the window asks the session to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComponentOp {
    Select(u32),
    Rename(u32, String),
}

/// The window's own state: the row being renamed and its text.
#[derive(Clone, Debug, Default)]
pub struct ComponentsWindow {
    pub renaming: Option<(u32, String)>,
}

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let open = chrome.components.is_some();
    let rows = chrome.component_rows.clone();
    let mut ops = Vec::new();
    let mut state = chrome.components.clone().unwrap_or_default();
    let r = SoftDialog::new("Components")
        .subtitle(match rows.len() {
            0 => "None yet: Edit > Make Component".to_owned(),
            1 => "1 component".to_owned(),
            n => format!("{n} components"),
        })
        .show(ctx, open, |ui| {
            for row in &rows {
                ui.add_space(size::MENU_PAD);
                ui.horizontal(|ui| match &mut state.renaming {
                    Some((def, text)) if *def == row.def => {
                        let resp = field(ui, text, "Name", 180.0);
                        let done =
                            resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if ui.add(SoftButton::new("Save").small()).clicked() || done {
                            ops.push(ComponentOp::Rename(row.def, text.clone()));
                            state.renaming = None;
                        }
                    }
                    _ => {
                        ui.label(RichText::new(&row.name).color(color::INK));
                        ui.label(
                            RichText::new(match row.uses {
                                1 => "1 instance".to_owned(),
                                n => format!("{n} instances"),
                            })
                            .font(theme::mono())
                            .color(color::MUTED),
                        );
                        if ui.add(SoftButton::new("Select").small()).clicked() {
                            ops.push(ComponentOp::Select(row.def));
                        }
                        if ui.add(SoftButton::new("Rename").small()).clicked() {
                            state.renaming = Some((row.def, row.name.clone()));
                        }
                    }
                });
            }
        });
    if r.close {
        chrome.components = None;
    } else if open {
        chrome.components = Some(state);
    }
    chrome.component_ops.extend(ops);
}
