//! The app's frame around the viewport, all generated from the action
//! registry: top bar with menus, tool dock, right-click menu, status bar,
//! command search, the hotkeys window and the Planned Features window.

mod dock;
mod frame;
mod hotkeys;
mod menus;
mod palette;
mod features;

use std::collections::{HashMap, HashSet};

use egui::{Context, Pos2, Rect};
use prism_ui::icon::Icon;

use crate::actions::esc::{EscAction, EscState, on_esc};
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
    Context { at: Pos2, heading: String, entries: Vec<Entry> },
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

/// The layout this frame: where the viewport is.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub viewport: Rect,
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
    pub(crate) rules: actions::Context,
}

impl Chrome {
    pub fn new(platform: Platform) -> Self {
        let reg = Registry::new();
        let keymap = storage::load_keymap()
            .and_then(|text| Keymap::from_json(&text).ok())
            .unwrap_or_else(|| Keymap::from_preset(Preset::SketchUp, &reg));
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
            toggles: HashSet::new(),
            menu: None,
            menu_serial: 0,
            palette: None,
            features_open: false,
            hotkeys: None,
            status: "Greyed items are not built yet. Tools can be picked; they are built in later chunks.".to_owned(),
            events: Vec::new(),
            rules: actions::Context::default(),
        }
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
                self.status = format!("{}: picked. The tool itself is built in a later chunk.", a.name);
            }
            Run::Event => {
                if a.check == Check::Toggle && !self.toggles.remove(id) {
                    self.toggles.insert(id);
                }
                self.status = format!("{}: sent. Nothing handles it until a later chunk.", a.name);
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
                self.status = format!("{}.", a.name);
            }
        }
        self.menu = None;
        self.events.push(ActionEvent { id, source });
    }

    fn ui_open(&self) -> bool {
        self.menu.is_some() || self.palette.is_some() || self.features_open || self.hotkeys.is_some()
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
        storage::save_keymap(&self.keymap.to_json());
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
