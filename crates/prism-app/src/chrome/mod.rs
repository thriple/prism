//! The app's frame around the viewport, all generated from the action
//! registry: top bar with menus, tool dock, right-click menu, status bar,
//! command search, the hotkeys window and the Planned Features window.

mod dock;
mod features;
mod frame;
mod hotkeys;
mod menus;
mod palette;

use std::collections::{HashMap, HashSet};

use egui::{Context, Pos2, Rect};
use prism_ui::icon::Icon;

use crate::actions::esc::{EscAction, EscState, on_esc};
#[cfg(test)]
use crate::actions::keymap::Chord;
use crate::actions::keymap::{KeyPress, Keymap, Platform, Preset};
use crate::actions::registry::{Check, Entry, Run, Selection};
use crate::actions::{self, Registry};
use crate::storage;

pub use hotkeys::HotkeysWindow;

/// Where an action was started from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Menu,
    Dock,
    Context,
    Key,
    Search,
}

/// An action that ran, for later chunks to handle (tools, files, edits).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionEvent {
    pub id: &'static str,
    pub source: Source,
}

/// An open menu: from the menu bar, or the right-click menu.
#[derive(Clone, Debug)]
pub(crate) enum MenuKind {
    Bar(usize),
    Context {
        at: Pos2,
        heading: String,
        entries: Vec<Entry>,
    },
}

/// Which rows are highlighted and which submenus are open, per level.
#[derive(Clone, Debug, Default)]
pub(crate) struct Level {
    pub highlight: Option<usize>,
    pub open_sub: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct OpenMenu {
    pub kind: MenuKind,
    pub levels: Vec<Level>,
    /// Counts menu openings, so each one pops in afresh.
    pub serial: u64,
}

/// The layout this frame: where the viewport is, and the mouse over it.
#[derive(Clone, Debug)]
pub struct Layout {
    pub viewport: Rect,
    pub response: Option<egui::Response>,
}

pub struct Chrome {
    pub(crate) reg: Registry,
    pub keymap: Keymap,
    pub(crate) platform: Platform,
    pub(crate) icons: HashMap<&'static str, Icon>,
    /// The picked tool (its behaviour comes in later chunks).
    pub active_tool: &'static str,
    toggles: HashSet<&'static str>,
    pub(crate) menu: Option<OpenMenu>,
    menu_serial: u64,
    pub(crate) palette: Option<palette::Palette>,
    pub(crate) features_open: bool,
    pub(crate) hotkeys: Option<HotkeysWindow>,
    pub(crate) status: String,
    /// Actions that ran and are waiting for a chunk that handles them.
    pub events: Vec<ActionEvent>,
    pub rules: actions::Context,
    /// Save keymap changes to disk (off in tests).
    persist: bool,
    /// The renderer's picture of the model, shown in the viewport well.
    pub viewport_texture: Option<egui::TextureId>,
    /// Drawn over the picture: label pills (screen position, text, pill colour).
    pub viewport_labels: Vec<(Pos2, String, egui::Color32)>,
    /// A box-select rectangle (crossing when true) and its label.
    pub viewport_box: Option<(Rect, bool, String)>,
    /// The status bar's right side: the lock chip and the measurement box.
    pub lock_chip: Option<(String, egui::Color32)>,
    pub measure: (String, String, bool),
    /// A message that replaces the tool hint until the tool changes.
    note: Option<(String, &'static str)>,
}

impl Chrome {
    /// The chrome with the saved keymap, or the SketchUp keys if none is saved.
    pub fn new(platform: Platform) -> Self {
        let reg = Registry::new();
        let keymap = storage::load_keymap()
            .and_then(|text| Keymap::from_json(&text).ok())
            .unwrap_or_else(|| Keymap::from_preset(Preset::SketchUp, &reg));
        let mut c = Self::with_keymap(platform, keymap);
        c.persist = true;
        c
    }

    /// The chrome with a given keymap, saving nothing.
    pub fn with_keymap(platform: Platform, keymap: Keymap) -> Self {
        let reg = Registry::new();
        let icons = reg
            .icons()
            .iter()
            .filter_map(|(id, svg)| match Icon::parse(svg) {
                Ok(icon) => Some((*id, icon)),
                Err(e) => {
                    log::warn!("icon {id}: {e}");
                    None
                }
            })
            .collect();
        Self {
            reg,
            keymap,
            platform,
            icons,
            active_tool: "tool.select",
            // The ground shadow starts on, as in the prototype.
            toggles: HashSet::from(["view.shadow", "view.guides"]),
            menu: None,
            menu_serial: 0,
            palette: None,
            features_open: false,
            hotkeys: None,
            status: "Middle-drag orbits, Shift+middle-drag pans, the wheel zooms. Greyed items are not built yet.".to_owned(),
            events: Vec::new(),
            rules: actions::Context::default(),
            persist: false,
            viewport_texture: None,
            viewport_labels: Vec::new(),
            viewport_box: None,
            lock_chip: None,
            measure: (String::new(), String::new(), false),
            note: None,
        }
    }

    /// Show a message in the status bar until the tool changes.
    pub fn status_note(&mut self, text: String) {
        self.note = Some((text, self.active_tool));
    }

    /// Show the tool's hint, unless a message is up for this tool.
    pub fn set_hint(&mut self, hint: &str) {
        match &self.note {
            Some((text, tool)) if *tool == self.active_tool => self.status = text.clone(),
            _ => {
                self.note = None;
                self.status = hint.to_owned();
            }
        }
    }

    /// The registry's id for a tool name (so a `String` from the session can
    /// become the chrome's `&'static str`).
    pub fn tool_id(&self, id: &str) -> Option<&'static str> {
        self.reg.get(id).map(|a| a.id)
    }

    pub fn menu_close(&mut self) {
        self.menu = None;
    }

    /// Whether a menu, the search or a window has the keys.
    pub fn ui_open_pub(&self) -> bool {
        self.ui_open()
    }

    pub fn is_on(&self, id: &str) -> bool {
        self.toggles.contains(id)
    }

    pub(crate) fn checked(&self, id: &str) -> bool {
        match self.reg.get(id).map(|a| a.check) {
            Some(Check::ActiveTool) => self.active_tool == id,
            Some(Check::Toggle) => self.is_on(id),
            _ => false,
        }
    }

    pub(crate) fn enabled(&self, id: &str) -> bool {
        self.reg.enabled(id, &self.rules)
    }

    /// The key an action shows in menus and tooltips.
    pub(crate) fn key_label(&self, id: &str) -> String {
        self.keymap
            .primary(id)
            .map(|c| c.label(self.platform))
            .unwrap_or_default()
    }

    /// Tooltip text: name, key, and why it is greyed.
    pub(crate) fn tip(&self, id: &str) -> String {
        let Some(a) = self.reg.get(id) else {
            return String::new();
        };
        let mut t = a.name.to_owned();
        let key = self.key_label(id);
        if !key.is_empty() {
            t.push_str("   ");
            t.push_str(&key);
        }
        if let Some(why) = self.reg.why_disabled(id, &self.rules) {
            t.push_str("   ·   ");
            t.push_str(why);
        }
        t
    }

    /// Run an action, if it can run now.
    pub fn run(&mut self, id: &'static str, source: Source) {
        let Some(a) = self.reg.get(id).cloned() else {
            return;
        };
        if !self.enabled(id) {
            return;
        }
        match a.run {
            Run::NotBuilt => return,
            Run::Tool => {
                self.active_tool = id;
                self.note = None;
            }
            Run::Event => {
                if a.check == Check::Toggle && !self.toggles.remove(id) {
                    self.toggles.insert(id);
                }
                self.status = a.name.to_owned();
            }
            Run::Command => {
                match id {
                    "help.features" => self.features_open = true,
                    "help.search" => self.palette = Some(palette::Palette::default()),
                    "help.keys" | "win.hotkeys" => {
                        self.hotkeys.get_or_insert_with(HotkeysWindow::default);
                    }
                    _ => {
                        if a.check == Check::Toggle && !self.toggles.remove(id) {
                            self.toggles.insert(id);
                        }
                    }
                }
                self.status = a.name.trim_end_matches('…').to_owned();
            }
        }
        self.menu = None;
        self.events.push(ActionEvent { id, source });
    }

    fn ui_open(&self) -> bool {
        self.menu.is_some()
            || self.palette.is_some()
            || self.features_open
            || self.hotkeys.is_some()
    }

    /// Close the topmost open thing (Esc rule, step 2).
    fn close_top(&mut self) {
        if self.menu.is_some() {
            self.menu = None;
        } else if self.palette.is_some() {
            self.palette = None;
        } else if let Some(h) = &mut self.hotkeys {
            if !h.cancel_pending() {
                self.hotkeys = None;
            }
        } else {
            self.features_open = false;
        }
    }

    /// A key was pressed. Returns true if the chrome used it.
    /// `typing` is true while a text field has the keyboard.
    pub fn key(&mut self, press: &KeyPress, typing: bool) -> bool {
        if let Some(h) = &mut self.hotkeys
            && h.recording()
        {
            h.record(press, &mut self.keymap, &self.reg, self.platform);
            self.save_keymap();
            return true;
        }
        if press.code == "Escape" {
            let state = EscState {
                operation: false,
                ui_open: self.ui_open(),
                in_group: false,
            };
            return match on_esc(state) {
                EscAction::CloseUi => {
                    self.close_top();
                    true
                }
                _ => false,
            };
        }
        // Menus and search use the arrows and Enter themselves.
        if typing || self.menu.is_some() || self.palette.is_some() || press.is_modifier() {
            return false;
        }
        let chord = press.chord(self.platform);
        let found = self
            .keymap
            .lookup(&chord, actions::registry::Mode::All)
            .and_then(|id| self.reg.get(id))
            .map(|a| a.id);
        match found {
            Some(id) => {
                self.run(id, Source::Key);
                true
            }
            None => false,
        }
    }

    pub(crate) fn save_keymap(&self) {
        if self.persist {
            storage::save_keymap(&self.keymap.to_json());
        }
    }

    /// Open a menu (replacing any open one).
    pub(crate) fn open_menu(&mut self, kind: MenuKind) {
        self.menu_serial += 1;
        self.menu = Some(OpenMenu {
            kind,
            levels: vec![Level::default()],
            serial: self.menu_serial,
        });
    }

    /// Open the right-click menu at `at` for `sel`.
    pub fn open_context(&mut self, at: Pos2, sel: Selection) {
        self.open_menu(MenuKind::Context {
            at,
            heading: sel.label(),
            entries: actions::registry::context_menu(sel),
        });
    }

    /// Draw the frame and everything that floats over it.
    pub fn show(&mut self, ctx: &Context) -> Layout {
        let layout = frame::show(self, ctx);
        menus::show(self, ctx);
        palette::show(self, ctx);
        features::show(self, ctx);
        hotkeys::show(self, ctx);
        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chrome() -> Chrome {
        let reg = Registry::new();
        Chrome::with_keymap(
            Platform::Windows,
            Keymap::from_preset(Preset::SketchUp, &reg),
        )
    }

    fn press(code: &str) -> KeyPress {
        KeyPress {
            code: code.into(),
            ctrl: false,
            cmd: false,
            shift: false,
            alt: false,
        }
    }

    #[test]
    fn keys_pick_tools_and_send_events() {
        let mut c = chrome();
        assert!(c.key(&press("KeyL"), false));
        assert_eq!(c.active_tool, "tool.line");
        assert!(c.checked("tool.line") && !c.checked("tool.select"));
        assert_eq!(
            c.events.last(),
            Some(&ActionEvent {
                id: "tool.line",
                source: Source::Key
            })
        );
        // While typing in a field, letters are text, not shortcuts.
        assert!(!c.key(&press("KeyR"), true));
        assert_eq!(c.active_tool, "tool.line");
    }

    #[test]
    fn greyed_actions_do_nothing() {
        let mut c = chrome();
        c.run("solid.union", Source::Menu);
        c.run("edit.undo", Source::Menu); // nothing to undo
        assert!(c.events.is_empty());
        // Smart Guides' Ctrl+U is bound but greyed until Inference & Drawing.
        let mut ctrl_u = press("KeyU");
        ctrl_u.ctrl = true;
        assert!(c.key(&ctrl_u, false));
        assert!(c.events.is_empty());
    }

    #[test]
    fn esc_closes_the_topmost_thing_first() {
        let mut c = chrome();
        let mut shift_s = press("KeyS");
        shift_s.shift = true;
        assert!(c.key(&shift_s, false));
        assert!(c.palette.is_some(), "Shift+S opens command search");
        c.run("help.features", Source::Menu);
        c.open_context(Pos2::ZERO, Selection::default());
        assert!(c.key(&press("Escape"), false));
        assert!(c.menu.is_none() && c.palette.is_some());
        assert!(c.key(&press("Escape"), false));
        assert!(c.palette.is_none() && c.features_open);
        assert!(c.key(&press("Escape"), false));
        assert!(!c.features_open);
        assert!(!c.key(&press("Escape"), false), "nothing left to close");
    }

    #[test]
    fn while_a_menu_is_open_shortcuts_wait() {
        let mut c = chrome();
        c.open_menu(MenuKind::Bar(0));
        assert!(!c.key(&press("KeyL"), false));
        assert_eq!(c.active_tool, "tool.select");
    }

    #[test]
    fn recording_a_conflicting_key_offers_the_choices() {
        let mut c = chrome();
        c.run("win.hotkeys", Source::Menu);
        let h = c.hotkeys.as_mut().expect("open");
        h.start_recording("tool.rect", 0);
        assert!(
            c.key(&press("KeyL"), false),
            "the press goes to the recorder"
        );
        let h = c.hotkeys.as_ref().expect("open");
        assert_eq!(h.pending_others(), Some(vec!["tool.line".to_owned()]));
        // Nothing changed until a choice is made.
        assert_eq!(
            c.keymap.lookup(
                &Chord::parse("L").expect("parses"),
                crate::actions::registry::Mode::All
            ),
            Some("tool.line")
        );
        // Esc drops the choice and keeps the window open.
        assert!(c.key(&press("Escape"), false));
        assert!(
            c.hotkeys
                .as_ref()
                .is_some_and(|h| h.pending_others().is_none())
        );
    }

    #[test]
    fn toggles_check_and_uncheck() {
        let mut c = chrome();
        c.run("help.debug", Source::Menu);
        assert!(c.is_on("help.debug") && c.checked("help.debug"));
        c.run("help.debug", Source::Menu);
        assert!(!c.is_on("help.debug"));
    }
}
