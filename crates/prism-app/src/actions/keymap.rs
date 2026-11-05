//! The keymap engine. Keys are bound by physical position (`KeyboardEvent.code`
//! names, such as `KeyL`), so tools stay put on AZERTY and other layouts.
//! "Mod" is Ctrl on Windows, Linux and the web on a PC, and Cmd on a Mac.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use super::registry::{Mode, Registry};

/// Modifier keys in a binding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mods {
    /// Ctrl, or Cmd on a Mac.
    pub primary: bool,
    pub shift: bool,
    /// Alt, or Option on a Mac.
    pub alt: bool,
}

/// One key combination: a physical key and modifiers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Chord {
    pub mods: Mods,
    /// The physical key, as `KeyboardEvent.code` names it ("KeyL", "Delete").
    pub code: String,
}

/// Which platform's names to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Mac,
    Linux,
    /// The web build, on a Mac or not.
    Web {
        mac: bool,
    },
}

impl Platform {
    pub fn is_mac(self) -> bool {
        matches!(self, Platform::Mac | Platform::Web { mac: true })
    }
}

impl Chord {
    pub fn new(code: impl Into<String>, mods: Mods) -> Self {
        Self {
            mods,
            code: code.into(),
        }
    }

    /// Parse the prototype's notation ("Ctrl+Shift+S", "Del", "Space", "G").
    pub fn parse(text: &str) -> Option<Chord> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let mut mods = Mods::default();
        let parts: Vec<&str> = text.split('+').collect();
        let (key, rest) = parts.split_last()?;
        for m in rest {
            match m.to_ascii_lowercase().as_str() {
                "ctrl" | "cmd" | "mod" => mods.primary = true,
                "shift" => mods.shift = true,
                "alt" | "option" => mods.alt = true,
                _ => return None,
            }
        }
        Some(Chord::new(code_for(key)?, mods))
    }

    /// The stored form: "Mod+Shift+KeyS". Platform-neutral.
    pub fn stored(&self) -> String {
        let mut s = String::new();
        if self.mods.primary {
            s.push_str("Mod+");
        }
        if self.mods.shift {
            s.push_str("Shift+");
        }
        if self.mods.alt {
            s.push_str("Alt+");
        }
        s.push_str(&self.code);
        s
    }

    /// Read the stored form back.
    pub fn from_stored(text: &str) -> Option<Chord> {
        let parts: Vec<&str> = text.split('+').collect();
        let (code, rest) = parts.split_last()?;
        let mut mods = Mods::default();
        for m in rest {
            match *m {
                "Mod" => mods.primary = true,
                "Shift" => mods.shift = true,
                "Alt" => mods.alt = true,
                _ => return None,
            }
        }
        (!code.is_empty()).then(|| Chord::new(*code, mods))
    }

    /// How menus and tooltips show it on `platform`: "Ctrl+Shift+S" or "Cmd+Shift+S".
    pub fn label(&self, platform: Platform) -> String {
        let mac = platform.is_mac();
        let mut s = String::new();
        if self.mods.primary {
            s.push_str(if mac { "Cmd+" } else { "Ctrl+" });
        }
        if self.mods.shift {
            s.push_str("Shift+");
        }
        if self.mods.alt {
            s.push_str(if mac { "Option+" } else { "Alt+" });
        }
        s.push_str(&key_label(&self.code));
        s
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.stored())
    }
}

impl Serialize for Chord {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.stored())
    }
}

impl<'de> Deserialize<'de> for Chord {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Chord::from_stored(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("bad key {text:?}")))
    }
}

/// The physical key code for a key name in the prototype's notation.
fn code_for(key: &str) -> Option<String> {
    let k = key.trim();
    let mut chars = k.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphabetic() {
            return Some(format!("Key{}", c.to_ascii_uppercase()));
        }
        if c.is_ascii_digit() {
            return Some(format!("Digit{c}"));
        }
    }
    let named = match k.to_ascii_lowercase().as_str() {
        "del" | "delete" => "Delete",
        "space" | "spc" => "Space",
        "esc" | "escape" => "Escape",
        "enter" | "return" => "Enter",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        _ => {
            let f = k.strip_prefix(['F', 'f'])?;
            let n: u8 = f.parse().ok()?;
            return (1..=24).contains(&n).then(|| format!("F{n}"));
        }
    };
    Some(named.to_owned())
}

/// A short name for a physical key: "KeyS" is "S", "Digit1" is "1".
pub fn key_label(code: &str) -> String {
    if let Some(c) = code.strip_prefix("Key") {
        return c.to_owned();
    }
    if let Some(d) = code.strip_prefix("Digit") {
        return d.to_owned();
    }
    match code {
        "Delete" => "Del",
        "Escape" => "Esc",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        "Backquote" => "`",
        "Minus" => "-",
        "Equal" => "=",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Semicolon" => ";",
        "Quote" => "'",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "Backslash" => "\\",
        other => other,
    }
    .to_owned()
}

/// Keymap presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preset {
    SketchUp,
    Blender,
    Fusion,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::SketchUp, Preset::Blender, Preset::Fusion];

    pub fn label(self) -> &'static str {
        match self {
            Preset::SketchUp => "SketchUp",
            Preset::Blender => "Blender",
            Preset::Fusion => "Fusion",
        }
    }
}

/// Second keys in the SketchUp preset, from PLAN 1.6.
const SKETCHUP_ALTERNATES: [(&str, &str); 2] = [
    ("edit.redo", "Ctrl+Shift+Z"),
    ("edit.desel", "Ctrl+Shift+A"),
];

/// The two key slots of one action.
pub type Slots = [Option<Chord>; 2];

/// The keys of every action in a preset. Blender and Fusion are empty for now,
/// to be filled in later (brief 03).
pub fn preset_bindings(preset: Preset, reg: &Registry) -> BTreeMap<String, Slots> {
    let mut map = BTreeMap::new();
    if preset != Preset::SketchUp {
        return map;
    }
    for a in reg.all() {
        let primary = Chord::parse(a.default_key);
        let alternate = SKETCHUP_ALTERNATES
            .iter()
            .find(|(id, _)| *id == a.id)
            .and_then(|(_, k)| Chord::parse(k));
        if primary.is_some() || alternate.is_some() {
            map.insert(a.id.to_owned(), [primary, alternate]);
        }
    }
    map
}

/// A modifier key, for the tool modifier meanings (PLAN 1.5 and 1.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModKey {
    Shift,
    Ctrl,
    Alt,
}

impl ModKey {
    pub const ALL: [ModKey; 3] = [ModKey::Shift, ModKey::Ctrl, ModKey::Alt];

    /// On a Mac, tools use Option where a PC uses Ctrl, and Cmd for Alt (PLAN 1.5).
    pub fn label(self, platform: Platform) -> &'static str {
        match (self, platform.is_mac()) {
            (ModKey::Shift, _) => "Shift",
            (ModKey::Ctrl, false) => "Ctrl",
            (ModKey::Ctrl, true) => "Option",
            (ModKey::Alt, false) => "Alt",
            (ModKey::Alt, true) => "Cmd",
        }
    }
}

/// What each modifier means inside tools. The same key means the same kind
/// of thing in every tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModifierRoles {
    /// Lock or constrain (Shift).
    pub lock: ModKey,
    /// Make something new instead of changing the original (Ctrl).
    pub make_new: ModKey,
    /// Alternate mode (Alt).
    pub alternate: ModKey,
}

impl Default for ModifierRoles {
    fn default() -> Self {
        Self {
            lock: ModKey::Shift,
            make_new: ModKey::Ctrl,
            alternate: ModKey::Alt,
        }
    }
}

impl ModifierRoles {
    /// Two meanings on one key break the "same key, same meaning" rule.
    pub fn clash(&self) -> bool {
        self.lock == self.make_new || self.lock == self.alternate || self.make_new == self.alternate
    }
}

/// The current bindings: a preset plus the user's changes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keymap {
    /// File format version, for later changes.
    pub version: u32,
    pub preset: Preset,
    /// Every action that has a key, by id.
    pub bindings: BTreeMap<String, Slots>,
    /// What Shift, Ctrl and Alt mean inside tools.
    #[serde(default)]
    pub modifiers: ModifierRoles,
}

/// The current file format version.
const VERSION: u32 = 1;

impl Keymap {
    pub fn from_preset(preset: Preset, reg: &Registry) -> Self {
        Self {
            version: VERSION,
            preset,
            bindings: preset_bindings(preset, reg),
            modifiers: ModifierRoles::default(),
        }
    }

    pub fn slots(&self, id: &str) -> Slots {
        self.bindings.get(id).cloned().unwrap_or_default()
    }

    /// The first key of an action, if any.
    pub fn primary(&self, id: &str) -> Option<Chord> {
        let [a, b] = self.slots(id);
        a.or(b)
    }

    /// Which action a key press runs. Modes are not built, so all apply.
    pub fn lookup(&self, chord: &Chord, _mode: Mode) -> Option<&str> {
        self.bindings
            .iter()
            .find(|(_, slots)| slots.iter().flatten().any(|c| c == chord))
            .map(|(id, _)| id.as_str())
    }

    /// Other actions that already use `chord` (in the same mode).
    pub fn conflicts(&self, chord: &Chord, except: &str) -> Vec<String> {
        self.bindings
            .iter()
            .filter(|(id, slots)| *id != except && slots.iter().flatten().any(|c| c == chord))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Set one slot of an action (None clears it).
    pub fn set(&mut self, id: &str, slot: usize, chord: Option<Chord>) {
        let mut slots = self.slots(id);
        slots[slot.min(1)] = chord;
        if slots.iter().all(Option::is_none) {
            self.bindings.remove(id);
        } else {
            self.bindings.insert(id.to_owned(), slots);
        }
    }

    /// Remove `chord` from every action except `keep` (the "Replace" choice).
    pub fn take_from_others(&mut self, chord: &Chord, keep: &str) {
        for id in self.conflicts(chord, keep) {
            let slots = self.slots(&id).map(|c| c.filter(|c| c != chord));
            self.bindings.insert(id.clone(), slots);
            if self.slots(&id).iter().all(Option::is_none) {
                self.bindings.remove(&id);
            }
        }
    }

    /// Whether an action's keys differ from its preset.
    pub fn differs(&self, id: &str, reg: &Registry) -> bool {
        let base = preset_bindings(self.preset, reg);
        base.get(id).cloned().unwrap_or_default() != self.slots(id)
    }

    /// Put one action back to its preset keys.
    pub fn reset(&mut self, id: &str, reg: &Registry) {
        match preset_bindings(self.preset, reg).remove(id) {
            Some(slots) => {
                self.bindings.insert(id.to_owned(), slots);
            }
            None => {
                self.bindings.remove(id);
            }
        }
    }

    /// Every chord bound to more than one action: (chord, actions).
    pub fn duplicates(&self) -> Vec<(Chord, Vec<String>)> {
        let mut by_chord: BTreeMap<Chord, Vec<String>> = BTreeMap::new();
        for (id, slots) in &self.bindings {
            for c in slots.iter().flatten() {
                by_chord.entry(c.clone()).or_default().push(id.clone());
            }
        }
        by_chord
            .into_iter()
            .filter(|(_, ids)| ids.len() > 1)
            .collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let k: Keymap = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if k.version > VERSION {
            return Err(format!("made by a newer Prism (version {})", k.version));
        }
        Ok(k)
    }
}

/// Why a key is a poor choice on this platform, if it is.
pub fn reserved(chord: &Chord, platform: Platform) -> Option<&'static str> {
    let m = chord.mods;
    let only_primary = m.primary && !m.shift && !m.alt;
    let code = chord.code.as_str();
    match platform {
        Platform::Web { .. } => {
            if only_primary && matches!(code, "KeyT" | "KeyN" | "KeyW") {
                return Some("The browser takes this key (new tab, new window or close tab)");
            }
            if m.primary && code == "Tab" {
                return Some("The browser takes this key (switch tabs)");
            }
            if code.starts_with('F')
                && code.len() > 1
                && code[1..].chars().all(|c| c.is_ascii_digit())
            {
                return Some("Browsers use the F-keys; pick a letter key instead");
            }
            None
        }
        Platform::Mac if only_primary && matches!(code, "KeyQ" | "KeyH" | "KeyM") => {
            Some("macOS uses this key (quit, hide or minimise)")
        }
        Platform::Windows | Platform::Linux if m.alt && code == "F4" => {
            Some("The system closes the window with this key")
        }
        _ => None,
    }
}

/// A key press as the window reports it.
#[derive(Clone, Debug)]
pub struct KeyPress {
    pub code: String,
    /// Ctrl, Cmd, Shift and Alt as held, by name.
    pub ctrl: bool,
    pub cmd: bool,
    pub shift: bool,
    pub alt: bool,
}

impl KeyPress {
    /// The chord this press makes on `platform` (Cmd is the primary key on a Mac).
    pub fn chord(&self, platform: Platform) -> Chord {
        let primary = if platform.is_mac() {
            self.cmd
        } else {
            self.ctrl
        };
        Chord::new(
            self.code.clone(),
            Mods {
                primary,
                shift: self.shift,
                alt: self.alt,
            },
        )
    }

    /// Modifier keys alone never make a chord.
    pub fn is_modifier(&self) -> bool {
        matches!(
            self.code.as_str(),
            "ShiftLeft"
                | "ShiftRight"
                | "ControlLeft"
                | "ControlRight"
                | "AltLeft"
                | "AltRight"
                | "SuperLeft"
                | "SuperRight"
                | "MetaLeft"
                | "MetaRight"
                | "CapsLock"
                | "Fn"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_sketchup_preset_has_no_key_used_twice() {
        let reg = Registry::new();
        let k = Keymap::from_preset(Preset::SketchUp, &reg);
        assert!(k.duplicates().is_empty(), "{:?}", k.duplicates());
        assert!(k.bindings.len() > 30);
    }

    #[test]
    fn parsing_the_prototype_notation() {
        let c = Chord::parse("Ctrl+Shift+S").expect("parses");
        assert_eq!(c.stored(), "Mod+Shift+KeyS");
        assert_eq!(Chord::parse("Del").map(|c| c.code), Some("Delete".into()));
        assert_eq!(Chord::parse("Space").map(|c| c.code), Some("Space".into()));
        assert_eq!(Chord::parse("F12").map(|c| c.code), Some("F12".into()));
        assert_eq!(Chord::parse(""), None);
        assert_eq!(Chord::parse("Hyper+X"), None);
        // Every default key in the registry parses.
        let reg = Registry::new();
        for a in reg.all() {
            if !a.default_key.is_empty() {
                assert!(
                    Chord::parse(a.default_key).is_some(),
                    "{} {:?}",
                    a.id,
                    a.default_key
                );
            }
        }
    }

    #[test]
    fn labels_follow_the_platform() {
        let c = Chord::parse("Ctrl+Shift+S").expect("parses");
        assert_eq!(c.label(Platform::Windows), "Ctrl+Shift+S");
        assert_eq!(c.label(Platform::Mac), "Cmd+Shift+S");
        assert_eq!(c.label(Platform::Web { mac: true }), "Cmd+Shift+S");
        let alt = Chord::parse("Alt+Del").expect("parses");
        assert_eq!(alt.label(Platform::Mac), "Option+Del");
    }

    #[test]
    fn the_same_physical_key_fires_on_qwerty_and_azerty() {
        let reg = Registry::new();
        let k = Keymap::from_preset(Preset::SketchUp, &reg);
        // On AZERTY the key in QWERTY's Q position types "a", but its code is
        // still KeyQ, so it runs Rotate, not Arc.
        let azerty_q = press("KeyQ");
        assert_eq!(
            k.lookup(&azerty_q.chord(Platform::Windows), Mode::All),
            Some("tool.rotate")
        );
        // And the key that types "q" on AZERTY (code KeyA) runs Arc.
        assert_eq!(
            k.lookup(&press("KeyA").chord(Platform::Windows), Mode::All),
            Some("tool.arc")
        );
    }

    #[test]
    fn cmd_is_the_primary_key_on_a_mac() {
        let reg = Registry::new();
        let k = Keymap::from_preset(Preset::SketchUp, &reg);
        let mut p = press("KeyZ");
        p.cmd = true;
        assert_eq!(
            k.lookup(&p.chord(Platform::Mac), Mode::All),
            Some("edit.undo")
        );
        assert_eq!(
            k.lookup(&p.chord(Platform::Windows), Mode::All),
            Some("cam.zoom"),
            "Cmd is not Ctrl on Windows"
        );
        p.cmd = false;
        p.ctrl = true;
        assert_eq!(
            k.lookup(&p.chord(Platform::Windows), Mode::All),
            Some("edit.undo")
        );
    }

    #[test]
    fn conflicts_replace_and_reset() {
        let reg = Registry::new();
        let mut k = Keymap::from_preset(Preset::SketchUp, &reg);
        let l = Chord::parse("L").expect("parses");
        assert_eq!(k.conflicts(&l, "tool.rect"), vec!["tool.line".to_owned()]);
        // Replace: Rectangle takes L from Line.
        k.take_from_others(&l, "tool.rect");
        k.set("tool.rect", 0, Some(l.clone()));
        assert_eq!(k.lookup(&l, Mode::All), Some("tool.rect"));
        assert!(k.primary("tool.line").is_none());
        assert!(k.differs("tool.line", &reg) && k.differs("tool.rect", &reg));
        k.reset("tool.line", &reg);
        k.reset("tool.rect", &reg);
        assert!(!k.differs("tool.line", &reg) && !k.differs("tool.rect", &reg));
        assert!(k.duplicates().is_empty());
    }

    #[test]
    fn json_round_trip_and_bad_input() {
        let reg = Registry::new();
        let mut k = Keymap::from_preset(Preset::SketchUp, &reg);
        k.set("solid.union", 1, Chord::parse("Ctrl+Alt+U"));
        let back = Keymap::from_json(&k.to_json()).expect("reads back");
        assert_eq!(back, k);
        assert!(k.to_json().contains("\"Mod+Alt+KeyU\""));
        assert!(Keymap::from_json("not json").is_err());
        let future = k.to_json().replace("\"version\": 1", "\"version\": 99");
        assert!(Keymap::from_json(&future).is_err());
    }

    #[test]
    fn other_presets_start_empty() {
        let reg = Registry::new();
        assert!(
            Keymap::from_preset(Preset::Blender, &reg)
                .bindings
                .is_empty()
        );
        assert!(
            Keymap::from_preset(Preset::Fusion, &reg)
                .bindings
                .is_empty()
        );
    }

    #[test]
    fn reserved_keys_warn_per_platform() {
        let t = Chord::parse("Ctrl+T").expect("parses");
        assert!(reserved(&t, Platform::Web { mac: false }).is_some());
        assert!(reserved(&t, Platform::Windows).is_none());
        let q = Chord::parse("Cmd+Q").expect("parses");
        assert!(reserved(&q, Platform::Mac).is_some());
        assert!(reserved(&Chord::parse("Alt+F4").expect("parses"), Platform::Windows).is_some());
        assert!(
            reserved(
                &Chord::parse("F5").expect("parses"),
                Platform::Web { mac: false }
            )
            .is_some()
        );
    }

    #[test]
    fn modifier_meanings_and_their_clash_warning() {
        let mut m = ModifierRoles::default();
        assert!(!m.clash());
        m.alternate = ModKey::Shift;
        assert!(m.clash());
        assert_eq!(ModKey::Ctrl.label(Platform::Mac), "Option");
        assert_eq!(ModKey::Alt.label(Platform::Windows), "Alt");
        // Old files without the field still load.
        let reg = Registry::new();
        let json = Keymap::from_preset(Preset::SketchUp, &reg).to_json();
        let i = json.find("\"modifiers\"").expect("written");
        let trimmed = format!("{}}}", json[..i].trim_end().trim_end_matches(','));
        assert_eq!(
            Keymap::from_json(&trimmed).expect("reads").modifiers,
            ModifierRoles::default()
        );
    }

    #[test]
    fn modifier_keys_alone_are_not_chords() {
        assert!(press("ShiftLeft").is_modifier());
        assert!(!press("KeyL").is_modifier());
    }
}
