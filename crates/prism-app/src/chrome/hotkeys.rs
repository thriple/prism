//! The Keyboard Shortcuts window (PLAN 1.12): every action grouped like the
//! menus, searchable; click a key to record a new one (Esc cancels,
//! Backspace clears); conflicts offer Replace, Keep both or Cancel; keys the
//! browser or system takes are warned about. A second tab sets what the
//! modifier keys mean inside tools.

use egui::{Context, RichText, ScrollArea, vec2};
use prism_ui::theme::{self, color, size};
use prism_ui::widgets::{SoftButton, SoftDialog, SoftSelect, field, groove, menu};

use super::Chrome;
use crate::actions::Registry;
use crate::actions::keymap::{Chord, KeyPress, Keymap, ModKey, Platform, Preset, reserved};
use crate::storage;

/// Column widths: action, primary key, alternate key.
const COLUMNS: [f32; 3] = [280.0, 150.0, 150.0];
/// The width of a meaning's description on the Modifiers tab.
const MEANING_WIDTH: f32 = 380.0;

/// A recorded key waiting for a choice.
#[derive(Clone, Debug)]
struct Pending {
    id: &'static str,
    slot: usize,
    chord: Chord,
    /// Actions already using the key.
    others: Vec<String>,
    /// Why the key is a poor choice here, if it is.
    warning: Option<&'static str>,
}

#[derive(Default)]
pub struct HotkeysWindow {
    tab: usize,
    query: String,
    recording: Option<(&'static str, usize)>,
    pending: Option<Pending>,
    message: String,
}

impl HotkeysWindow {
    /// Waiting for a key press for some cell.
    pub fn recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Esc: stop recording or drop a pending choice. True if there was one.
    pub fn cancel_pending(&mut self) -> bool {
        let had = self.recording.is_some() || self.pending.is_some();
        self.recording = None;
        self.pending = None;
        had
    }

    /// Start recording a new key for one slot of an action.
    pub fn start_recording(&mut self, id: &'static str, slot: usize) {
        self.recording = Some((id, slot.min(1)));
        self.pending = None;
    }

    /// The actions a recorded key clashes with, while the choice is open.
    pub fn pending_others(&self) -> Option<Vec<String>> {
        self.pending.as_ref().map(|p| p.others.clone())
    }

    /// Take a key press for the cell being recorded.
    pub fn record(
        &mut self,
        press: &KeyPress,
        keymap: &mut Keymap,
        reg: &Registry,
        platform: Platform,
    ) {
        let Some((id, slot)) = self.recording else {
            return;
        };
        match press.code.as_str() {
            "Escape" => {
                self.recording = None;
                self.message = "Cancelled.".to_owned();
            }
            "Backspace" | "Delete" if !press.ctrl && !press.cmd && !press.shift && !press.alt => {
                keymap.set(id, slot, None);
                self.recording = None;
                self.message = format!("Cleared a key of {}.", name(reg, id));
            }
            _ if press.is_modifier() => {} // wait for the key itself
            _ => {
                let chord = press.chord(platform);
                self.recording = None;
                let others = keymap.conflicts(&chord, id);
                let warning = reserved(&chord, platform);
                if others.is_empty() && warning.is_none() {
                    keymap.set(id, slot, Some(chord.clone()));
                    self.message = format!("{} is now {}.", name(reg, id), chord.label(platform));
                } else {
                    self.pending = Some(Pending {
                        id,
                        slot,
                        chord,
                        others,
                        warning,
                    });
                }
            }
        }
    }
}

fn name<'a>(reg: &'a Registry, id: &str) -> &'a str {
    reg.get(id).map_or("?", |a| a.name)
}

pub(super) fn show(chrome: &mut Chrome, ctx: &Context) {
    let open = chrome.hotkeys.is_some();
    let Some(mut w) = chrome.hotkeys.take() else {
        // Let it fade out.
        let _ = SoftDialog::new("Keyboard shortcuts")
            .width(size::DIALOG_WIDE)
            .show(ctx, false, |_| ());
        return;
    };
    let mut changed = false;
    let r = SoftDialog::new("Keyboard shortcuts")
        .subtitle(format!("Preset: {}", chrome.keymap.preset.label()))
        .width(size::DIALOG_WIDE)
        .show(ctx, open, |ui| {
            ui.horizontal(|ui| {
                for (i, tab) in ["Keyboard", "Modifiers"].iter().enumerate() {
                    if ui
                        .add(SoftButton::new(*tab).small().selected(w.tab == i))
                        .clicked()
                    {
                        w.tab = i;
                    }
                }
            });
            ui.add_space(size::MENU_PAD);
            if w.tab == 0 {
                changed |= keyboard_tab(ui, chrome, &mut w);
            } else {
                changed |= modifiers_tab(ui, chrome);
            }
        });
    if changed {
        chrome.save_keymap();
    }
    if r.close {
        chrome.hotkeys = None;
    } else {
        chrome.hotkeys = Some(w);
    }
}

fn keyboard_tab(ui: &mut egui::Ui, chrome: &mut Chrome, w: &mut HotkeysWindow) -> bool {
    let mut changed = false;
    let platform = chrome.platform;
    ui.horizontal(|ui| {
        field(
            ui,
            &mut w.query,
            "Search by name or key",
            size::SELECT_WIDTH,
        );
        let mut preset = Preset::ALL
            .iter()
            .position(|p| *p == chrome.keymap.preset)
            .unwrap_or(0);
        let labels: Vec<&str> = Preset::ALL.iter().map(|p| p.label()).collect();
        if ui
            .add(SoftSelect::new(&mut preset, &labels, "Preset"))
            .changed()
        {
            chrome.keymap = Keymap::from_preset(Preset::ALL[preset], &chrome.reg);
            w.message = match Preset::ALL[preset] {
                Preset::SketchUp => "Switched to the SketchUp keys.".to_owned(),
                p => format!(
                    "Switched to {}. It is empty for now: record the keys you want.",
                    p.label()
                ),
            };
            changed = true;
        }
    });
    ui.horizontal(|ui| {
        if ui.add(SoftButton::new("Reset all").small()).clicked() {
            let modifiers = chrome.keymap.modifiers;
            chrome.keymap = Keymap::from_preset(chrome.keymap.preset, &chrome.reg);
            chrome.keymap.modifiers = modifiers;
            w.message = "Every key is back to the preset.".to_owned();
            changed = true;
        }
        if ui.add(SoftButton::new("Export…").small()).clicked() {
            w.message = match storage::export_keymap(&chrome.keymap.to_json()) {
                Ok(at) => format!("Saved \"{}\" to {at}.", storage::EXPORT_NAME),
                Err(e) => format!("Could not export: {e}."),
            };
        }
        if ui.add(SoftButton::new("Import…").small()).clicked() {
            match storage::import_keymap() {
                Ok(Some(text)) => w.message = import(chrome, &text, &mut changed),
                Ok(None) => w.message = "Pick a keymap file…".to_owned(),
                Err(e) => w.message = format!("Could not import: {e}."),
            }
        }
    });
    if let Some(text) = storage::take_imported() {
        w.message = import(chrome, &text, &mut changed);
    }

    // A recorded key that needs a choice.
    if let Some(p) = w.pending.clone() {
        let key = p.chord.label(platform);
        ui.add_space(size::MENU_PAD);
        if let Some(warning) = p.warning {
            ui.label(RichText::new(format!("{key}: {warning}.")).color(color::INK));
        }
        if !p.others.is_empty() {
            let names: Vec<&str> = p.others.iter().map(|o| name(&chrome.reg, o)).collect();
            ui.label(
                RichText::new(format!("{key} is already used by {}.", names.join(", ")))
                    .color(color::INK),
            );
        }
        ui.horizontal(|ui| {
            let label = if p.others.is_empty() {
                "Use anyway"
            } else {
                "Replace"
            };
            if ui.add(SoftButton::new(label).small()).clicked() {
                chrome.keymap.take_from_others(&p.chord, p.id);
                chrome.keymap.set(p.id, p.slot, Some(p.chord.clone()));
                w.message = format!("{} is now {key}.", name(&chrome.reg, p.id));
                w.pending = None;
                changed = true;
            }
            if !p.others.is_empty() {
                // Modes are not built yet, so every action is in every mode.
                ui.add(SoftButton::new("Keep both").small().enabled(false))
                    .on_hover_text("Only for actions in different modes; modes are not built yet");
            }
            if ui.add(SoftButton::new("Cancel").small()).clicked() {
                w.pending = None;
                w.message = "Cancelled.".to_owned();
            }
        });
    } else if w.recording.is_some() {
        ui.label(
            RichText::new("Press the new key. Esc cancels, Backspace clears.").color(color::ACCENT),
        );
    } else if !w.message.is_empty() {
        ui.label(RichText::new(&w.message).color(color::MUTED));
    }
    groove(ui);

    // The table, grouped like the menus.
    let q = w.query.to_lowercase();
    let col = COLUMNS;
    let width = ui.available_width();
    ScrollArea::vertical()
        .max_width(width)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (menu_name, _) in chrome.reg.menubar() {
                let rows: Vec<&'static str> = chrome
                    .reg
                    .all()
                    .iter()
                    .filter(|a| a.menu_path.first() == Some(menu_name) && a.id != "file.recent")
                    .filter(|a| {
                        if q.is_empty() {
                            return true;
                        }
                        let keys = chrome
                            .keymap
                            .slots(a.id)
                            .iter()
                            .flatten()
                            .map(|c| c.label(platform).to_lowercase())
                            .collect::<Vec<_>>();
                        a.name.to_lowercase().contains(&q) || keys.iter().any(|k| k.contains(&q))
                    })
                    .map(|a| a.id)
                    .collect();
                if rows.is_empty() {
                    continue;
                }
                ui.add_space(size::MENU_PAD);
                ui.label(menu::caps(menu_name, color::MUTED));
                for id in rows {
                    ui.horizontal(|ui| {
                        let differs = chrome.keymap.differs(id, &chrome.reg);
                        ui.allocate_ui_with_layout(
                            vec2(col[0], size::SMALL_BUTTON_HEIGHT),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_width(col[0]);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(name(&chrome.reg, id)).color(color::INK),
                                    )
                                    .truncate(),
                                );
                            },
                        );
                        let slots = chrome.keymap.slots(id);
                        for (slot, chord) in slots.iter().enumerate() {
                            let recording = w.recording == Some((id, slot));
                            let text = if recording {
                                "Press keys…".to_owned()
                            } else {
                                chord.as_ref().map_or("—".to_owned(), |c| c.label(platform))
                            };
                            let resp = ui.add(
                                SoftButton::new(text)
                                    .small()
                                    .selected(recording)
                                    .min_width(col[slot + 1]),
                            );
                            if resp.clicked() {
                                w.start_recording(id, slot);
                            }
                        }
                        ui.label(
                            RichText::new(chrome.reg.get(id).map_or("", |a| a.mode.label()))
                                .font(theme::mono())
                                .color(color::MUTED),
                        );
                        if differs {
                            ui.label(RichText::new("●").color(color::ACCENT))
                                .on_hover_text("Changed from the preset");
                            if ui.add(SoftButton::new("Reset").small()).clicked() {
                                chrome.keymap.reset(id, &chrome.reg);
                                changed = true;
                            }
                        }
                    });
                }
            }
        });
    changed
}

fn import(chrome: &mut Chrome, text: &str, changed: &mut bool) -> String {
    match Keymap::from_json(text) {
        Ok(k) => {
            chrome.keymap = k;
            *changed = true;
            "Imported the keymap.".to_owned()
        }
        Err(e) => format!("That file is not a Prism keymap ({e})."),
    }
}

fn modifiers_tab(ui: &mut egui::Ui, chrome: &mut Chrome) -> bool {
    let platform = chrome.platform;
    let labels: Vec<&str> = ModKey::ALL.iter().map(|m| m.label(platform)).collect();
    let mut roles = chrome.keymap.modifiers;
    ui.label(RichText::new("What each modifier means inside tools. The same key should mean the same kind of thing in every tool.").color(color::MUTED));
    ui.add_space(size::MENU_PAD);
    let rows: [(&str, &mut ModKey); 3] = [
        ("Lock or constrain", &mut roles.lock),
        (
            "Make something new instead of changing the original",
            &mut roles.make_new,
        ),
        ("Alternate mode", &mut roles.alternate),
    ];
    for (label, key) in rows {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                vec2(MEANING_WIDTH, size::BUTTON_HEIGHT),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_width(MEANING_WIDTH);
                    ui.label(RichText::new(label).color(color::INK));
                },
            );
            let mut i = ModKey::ALL.iter().position(|m| m == key).unwrap_or(0);
            if ui.add(SoftSelect::new(&mut i, &labels, "Key")).changed() {
                *key = ModKey::ALL[i];
            }
        });
    }
    if roles.clash() {
        ui.label(RichText::new("Two meanings now share a key, so that key would mean different things in different tools.").color(color::ACCENT));
    }
    if ui.add(SoftButton::new("Reset modifiers").small()).clicked() {
        roles = Default::default();
    }
    let changed = roles != chrome.keymap.modifiers;
    chrome.keymap.modifiers = roles;
    changed
}
