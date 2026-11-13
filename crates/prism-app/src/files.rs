//! The open document's file (brief 08): New, Open, Save, Save As, Recent,
//! Document Settings, "Save changes?", the window title, and autosave with
//! recovery after a crash.

use egui::Context;
use prism_doc::Document;
use prism_file::{CameraData, Contents, Journal, Settings};
use prism_math::LengthUnit;
use prism_tools::Session;
use prism_ui::widgets::{SoftButton, SoftDialog, SoftSelect, SoftSlider};
use prism_view::Camera;
use serde_json::{Map, Value};

use crate::chrome::Chrome;
use crate::doc_files::{self, Opened};
use crate::recovery;

/// Autosave this often while there are unsaved changes (seconds).
pub const AUTOSAVE_EVERY: f64 = 10.0;

/// What to do once the unsaved changes are dealt with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Next {
    /// Close this tab.
    Close,
}

/// What a tab asks the app to do (brief 11): files open in tabs, which the
/// app owns.
pub enum Request {
    NewTab,
    Open(Opened),
}

/// Where the recovery journal starts from.
struct Base {
    /// The base model as a `.prism` file.
    bytes: Vec<u8>,
    /// How many commands of the document's log the base already has.
    log_len: usize,
    /// Whether the base has been written to the recovery store yet.
    written: bool,
}

pub struct Files {
    /// The file's name, and where it lives (natively).
    name: Option<String>,
    path: Option<String>,
    /// The document revision when it was last saved or opened.
    saved_rev: Option<u64>,
    /// Document Settings changed since then (they are not commands).
    settings_changed: bool,
    /// The view style as last saved (a change to it needs saving).
    saved_view: prism_view::ViewStyle,
    /// Fields a newer Prism wrote, kept for the next save.
    extra: Map<String, Value>,
    /// This tab's recovery slot.
    slot: u32,
    /// For the app: new tabs and files to open.
    pub requests: Vec<Request>,
    /// "Save changes?" was cancelled (stops closing the window).
    cancelled: bool,
    /// "Save changes?" is showing, and what comes after it.
    asking: Option<Next>,
    /// A web save is running; this comes after it.
    after_save: Option<Option<Next>>,
    settings_open: bool,
    /// The app should close now.
    quit: bool,
    /// Recovery.
    base: Base,
    last_autosave: f64,
    /// A journal left behind by a crash, offered at start.
    offer: Option<crate::recovery::Found>,
    title: String,
    /// Import and Export.
    transfer: crate::transfer::Transfer,
}

/// The key the view style is saved under in the file's settings (View
/// Settings, PLAN 1.14).
const VIEW_KEY: &str = "view";

fn contents(s: &Session, extra: &Map<String, Value>) -> Contents {
    let c = s.camera();
    let mut extra = extra.clone();
    if let Ok(v) = serde_json::to_value(s.view_style()) {
        extra.insert(VIEW_KEY.into(), v);
    }
    Contents {
        assembly: s.document().assembly().clone(),
        settings: Settings {
            units: s.number.units,
            camera: Some(CameraData {
                eye: c.eye.to_array(),
                target: c.target.to_array(),
                parallel: s.lens().parallel,
                fov: Some(s.lens().fov),
            }),
            extra,
        },
    }
}

/// The projection saved in a file (perspective at the default field of
/// view when none is).
fn lens_of(c: Option<&CameraData>) -> prism_view::Lens {
    let d = prism_view::Lens::default();
    c.map_or(d, |c| prism_view::Lens {
        parallel: c.parallel,
        fov: c.fov.unwrap_or(d.fov),
    })
}

/// The view style saved in a file, or Default.
fn view_style(extra: &mut Map<String, Value>) -> prism_view::ViewStyle {
    extra
        .remove(VIEW_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

pub(crate) fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

impl Files {
    pub fn new(session: &Session, slot: u32) -> Self {
        let base = prism_file::write(&contents(session, &Map::new()), app_version());
        Self {
            name: None,
            path: None,
            saved_rev: Some(session.document().revision()),
            settings_changed: false,
            saved_view: session.view_style().clone(),
            extra: Map::new(),
            slot,
            requests: Vec::new(),
            cancelled: false,
            asking: None,
            after_save: None,
            settings_open: false,
            quit: false,
            base: Base {
                bytes: base,
                log_len: session.document().log().count(),
                written: false,
            },
            last_autosave: 0.0,
            offer: None,
            title: String::new(),
            transfer: crate::transfer::Transfer::default(),
        }
    }

    pub fn dirty(&self, s: &Session) -> bool {
        self.settings_changed
            || self.saved_rev != Some(s.document().revision())
            || s.view_style() != &self.saved_view
    }

    /// A dialog of ours is open (keys go to it, not to the tools).
    pub fn modal(&self) -> bool {
        self.asking.is_some() || self.settings_open || self.offer.is_some() || self.transfer.modal()
    }

    pub fn take_quit(&mut self) -> bool {
        std::mem::take(&mut self.quit)
    }

    pub fn take_cancelled(&mut self) -> bool {
        std::mem::take(&mut self.cancelled)
    }

    /// Untitled and unchanged: a file opened now can use this tab.
    pub fn pristine(&self, s: &Session) -> bool {
        self.name.is_none() && self.path.is_none() && !self.dirty(s)
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub fn name(&self) -> &str {
        self.name.as_deref().unwrap_or("Untitled")
    }

    /// Offer work a crash left behind in this tab.
    pub fn offer(&mut self, f: crate::recovery::Found) {
        // The restored work keeps writing to the slot it came from.
        self.slot = f.slot;
        self.offer = Some(f);
    }

    /// Write the recovery journal now if there are unsaved changes (when
    /// switching away from the tab, so nothing waits for its next turn).
    pub fn flush(&mut self, s: &Session) {
        // Any time counts as due; the clock restarts from zero after.
        self.last_autosave = f64::NEG_INFINITY;
        self.autosave(0.0, s);
    }

    /// The recent files' names, for the Open Recent menu.
    pub fn recent_names(&self) -> Vec<String> {
        doc_files::recent()
            .iter()
            .map(|p| doc_files::display_name(p))
            .collect()
    }

    /// The window title: "house.prism • — Prism - Build 120" while
    /// unsaved. The build shows whether an update took (owner, 2026-11-09).
    pub fn title(&self, s: &Session) -> String {
        let name = self.name.as_deref().unwrap_or("Untitled");
        let dot = if self.dirty(s) { " •" } else { "" };
        format!("{name}{dot} — {}", app_name())
    }

    /// The title, if it changed since last asked.
    pub fn new_title(&mut self, s: &Session) -> Option<String> {
        let t = self.title(s);
        (t != self.title).then(|| {
            self.title = t.clone();
            t
        })
    }

    /// Open a file named on the command line.
    pub fn open_at_start(&mut self, path: &str, s: &mut Session, chrome: &mut Chrome) {
        match doc_files::open_path(path) {
            Ok(o) => self.load(o, s, chrome),
            Err(e) => chrome.status_note(e),
        }
    }

    /// A File menu action.
    pub fn action(&mut self, id: &str, s: &mut Session, chrome: &mut Chrome) {
        doc_files::use_slot(self.slot);
        match id {
            "file.new" => self.requests.push(Request::NewTab),
            "file.open" => match doc_files::open() {
                Ok(Some(o)) => self.requests.push(Request::Open(o)),
                Ok(None) => {}
                Err(e) => chrome.status_note(format!("Could not open: {e}")),
            },
            "file.save" => self.save(false, None, s, chrome),
            "file.saveas" => self.save(true, None, s, chrome),
            "file.docsettings" => self.settings_open = true,
            "file.import" | "exp.3mf" | "exp.gltf" => self.transfer.action(id, s, chrome),
            _ => {
                if let Some(n) = id
                    .strip_prefix("file.recent")
                    .and_then(|n| n.parse::<usize>().ok())
                    && n >= 1
                    && let Some(path) = doc_files::recent().get(n - 1).cloned()
                {
                    match doc_files::open_path(&path) {
                        Ok(o) => self.requests.push(Request::Open(o)),
                        Err(e) => {
                            let mut list = doc_files::recent();
                            list.retain(|p| *p != path);
                            doc_files::set_recent(&list);
                            chrome.status_note(e);
                        }
                    }
                }
            }
        }
    }

    /// A file dropped onto the window: an image becomes a new textured
    /// material; a `.prism` opens (asking about
    /// unsaved changes first), anything else imports.
    pub fn dropped(&mut self, o: Opened, s: &mut Session, chrome: &mut Chrome) {
        if crate::mesh_files::is_image(&o.name) {
            // A PNG or JPEG becomes a new textured material.
            crate::chrome::image_loaded(chrome, s, None, o);
        } else if o.name.to_ascii_lowercase().ends_with(".prism") {
            self.requests.push(Request::Open(o));
        } else {
            self.transfer.open(o, chrome);
        }
    }

    /// Close this tab (or, from the window's close button, one of the tabs
    /// being closed in turn).
    pub fn close_requested(&mut self, s: &mut Session, chrome: &mut Chrome) {
        self.then(Next::Close, s, chrome);
    }

    /// Do `next`, asking "Save changes?" first if there are any.
    fn then(&mut self, next: Next, s: &mut Session, chrome: &mut Chrome) {
        if self.dirty(s) {
            self.asking = Some(next);
        } else {
            self.go(next, s, chrome);
        }
    }

    fn go(&mut self, next: Next, _s: &mut Session, _chrome: &mut Chrome) {
        match next {
            Next::Close => {
                recovery::clear(self.slot);
                self.quit = true;
            }
        }
    }

    /// Open a file in this tab.
    pub(crate) fn load(&mut self, o: Opened, s: &mut Session, chrome: &mut Chrome) {
        let c = match prism_file::read(&o.bytes) {
            Ok(c) => c,
            Err(e) => {
                chrome.status_note(format!("Could not open {}: {e}", o.name));
                return;
            }
        };
        let camera = c.settings.camera.map(|c| Camera {
            eye: prism_math::DVec3::from_array(c.eye),
            target: prism_math::DVec3::from_array(c.target),
        });
        s.set_lens(lens_of(c.settings.camera.as_ref()));
        s.open_document(Document::with_assembly(c.assembly), camera);
        s.set_units(c.settings.units);
        self.extra = c.settings.extra;
        s.set_view_style(view_style(&mut self.extra));
        self.name = Some(o.name.clone());
        self.path = o.path.clone();
        if let Some(p) = &o.path {
            self.remember(p);
        }
        self.mark_saved(s, Some(o.bytes));
        recovery::clear(self.slot);
        chrome.status_note(format!("Opened {}", o.name));
    }

    fn remember(&mut self, path: &str) {
        let mut list = doc_files::recent();
        list.retain(|p| p != path);
        list.insert(0, path.to_owned());
        list.truncate(doc_files::MAX_RECENT);
        doc_files::set_recent(&list);
    }

    /// The document as it is now counts as saved, and is the new base for
    /// recovery.
    fn mark_saved(&mut self, s: &Session, bytes: Option<Vec<u8>>) {
        self.saved_rev = Some(s.document().revision());
        self.settings_changed = false;
        self.saved_view = s.view_style().clone();
        self.base = Base {
            bytes: bytes
                .unwrap_or_else(|| prism_file::write(&contents(s, &self.extra), app_version())),
            log_len: s.document().log().count(),
            written: false,
        };
    }

    /// Save (or Save As, which always asks where). `next` runs after a
    /// successful save.
    fn save(&mut self, save_as: bool, next: Option<Next>, s: &mut Session, chrome: &mut Chrome) {
        let bytes = prism_file::write(&contents(s, &self.extra), app_version());
        let suggested = self
            .name
            .clone()
            .unwrap_or_else(|| format!("Untitled.{}", prism_file::EXTENSION));
        // Natively the path; on the web "save again where it was" is any Some.
        let again = if save_as {
            None
        } else if cfg!(target_arch = "wasm32") {
            self.name.as_ref().map(|_| "")
        } else {
            self.path.as_deref()
        };
        match doc_files::save(again, &suggested, &bytes) {
            Ok(Some((name, path))) => self.saved(name, path, bytes, next, s, chrome),
            Ok(None) => {}
            Err(e) if e == doc_files::PENDING => {
                // A web save in place: it finishes in `frame`.
                self.after_save = Some(next);
                self.base.bytes = bytes;
            }
            Err(e) => chrome.status_note(format!("Not saved: {e}")),
        }
    }

    fn saved(
        &mut self,
        name: String,
        path: String,
        bytes: Vec<u8>,
        next: Option<Next>,
        s: &mut Session,
        chrome: &mut Chrome,
    ) {
        chrome.status_note(format!("Saved {name}"));
        self.name = Some(name);
        if !path.is_empty() {
            self.remember(&path);
            self.path = Some(path);
        }
        self.mark_saved(s, Some(bytes));
        recovery::clear(self.slot);
        if let Some(n) = next {
            self.go(n, s, chrome);
        }
    }

    /// Every frame: finished web calls, autosave, and the dialogs.
    pub fn frame(&mut self, ctx: &Context, now: f64, s: &mut Session, chrome: &mut Chrome) {
        doc_files::use_slot(self.slot);
        if let Some(r) = doc_files::take_opened() {
            match r {
                Ok(o) => self.requests.push(Request::Open(o)),
                Err(e) => chrome.status_note(format!("Could not open: {e}")),
            }
        }
        if let Some(r) = doc_files::take_saved() {
            let next = self.after_save.take().flatten();
            match r {
                Ok(Some((name, path))) => {
                    let bytes = std::mem::take(&mut self.base.bytes);
                    self.saved(name, path, bytes, next, s, chrome);
                }
                Ok(None) => {}
                Err(e) => chrome.status_note(format!("Not saved: {e}")),
            }
        }
        if let Some(r) = crate::mesh_files::take_dropped() {
            match r {
                Ok(o) => self.dropped(o, s, chrome),
                Err(e) => chrome.status_note(format!("Could not read the dropped file: {e}")),
            }
        }
        self.transfer.frame(ctx, s, chrome);
        doc_files::set_unsaved(self.dirty(s));
        self.autosave(now, s);
        self.dialogs(ctx, s, chrome);
    }

    fn autosave(&mut self, now: f64, s: &Session) {
        let log_len = s.document().log().count();
        if log_len < self.base.log_len {
            // Undone past the base: start again from the model as it is.
            self.base = Base {
                bytes: prism_file::write(&contents(s, &self.extra), app_version()),
                log_len,
                written: false,
            };
        }
        if !self.dirty(s) || now - self.last_autosave < AUTOSAVE_EVERY {
            return;
        }
        self.last_autosave = now;
        if !self.base.written {
            recovery::write_base(self.slot, &self.base.bytes);
            self.base.written = true;
        }
        let j = Journal {
            path: self.path.clone().or_else(|| self.name.clone()),
            commands: s
                .document()
                .log()
                .skip(self.base.log_len)
                .cloned()
                .collect(),
        };
        recovery::write_journal(self.slot, &j.to_json());
    }

    fn dialogs(&mut self, ctx: &Context, s: &mut Session, chrome: &mut Chrome) {
        // Save changes?
        let name = self.name.clone().unwrap_or_else(|| "Untitled".into());
        let mut choice = None;
        let r = SoftDialog::new("Save changes?")
            .subtitle(format!("{name} has changes that are not saved."))
            .show(ctx, self.asking.is_some(), |ui| {
                ui.horizontal(|ui| {
                    if ui.add(SoftButton::new("Save")).clicked() {
                        choice = Some(0);
                    }
                    if ui.add(SoftButton::new("Don't Save")).clicked() {
                        choice = Some(1);
                    }
                    if ui.add(SoftButton::new("Cancel")).clicked() {
                        choice = Some(2);
                    }
                });
            });
        if r.close {
            choice = Some(2);
        }
        if let (Some(c), Some(next)) = (choice, self.asking) {
            self.asking = None;
            match c {
                0 => self.save(false, Some(next), s, chrome),
                1 => {
                    recovery::clear(self.slot);
                    // Don't Save: what comes next replaces the document.
                    self.saved_rev = Some(s.document().revision());
                    self.settings_changed = false;
                    self.go(next, s, chrome);
                }
                _ => self.cancelled = true,
            }
        }

        // Restore after a crash.
        let mut restore = None;
        let r = SoftDialog::new("Restore your work?")
            .subtitle("Prism closed without saving your last changes.")
            .show(ctx, self.offer.is_some(), |ui| {
                ui.horizontal(|ui| {
                    if ui.add(SoftButton::new("Restore")).clicked() {
                        restore = Some(true);
                    }
                    if ui.add(SoftButton::new("Discard")).clicked() {
                        restore = Some(false);
                    }
                });
            });
        if r.close && restore.is_none() {
            restore = Some(false);
        }
        if let Some(yes) = restore
            && let Some(found) = self.offer.take()
        {
            if yes {
                self.restore(&found.journal, found.base, s, chrome);
            } else {
                recovery::clear(self.slot);
            }
        }

        // Document Settings.
        let mut units = s.number.units;
        let names: Vec<&str> = LengthUnit::ALL.iter().map(|u| u.name()).collect();
        let r = SoftDialog::new("Document settings")
            .subtitle("How lengths are shown. You can type any unit.")
            .show(ctx, self.settings_open, |ui| {
                let mut i = LengthUnit::ALL
                    .iter()
                    .position(|u| *u == units.length)
                    .unwrap_or(2);
                ui.add(SoftSelect::new(&mut i, &names, "Units"));
                units.length = LengthUnit::ALL[i];
                let mut d = f32::from(units.decimals);
                ui.add(
                    SoftSlider::new(
                        &mut d,
                        0.0..=f32::from(prism_math::units::MAX_DECIMALS),
                        "Decimals",
                    )
                    .step(1.0),
                );
                units.decimals = d.round() as u8;
                ui.add_space(6.0);
                ui.label(format!("Example: {}", {
                    let mut nf = s.number;
                    nf.units = units;
                    nf.show(1.2345)
                }));
                if ui.add(SoftButton::new("Done")).clicked() {
                    self.settings_open = false;
                }
            });
        if r.close {
            self.settings_open = false;
        }
        if units != s.number.units {
            s.set_units(units);
            self.settings_changed = true;
        }
    }

    fn restore(&mut self, j: &Journal, base: Vec<u8>, s: &mut Session, chrome: &mut Chrome) {
        let c = match prism_file::read(&base) {
            Ok(c) => c,
            Err(e) => {
                chrome.status_note(format!("Could not restore: {e}"));
                recovery::clear(self.slot);
                return;
            }
        };
        let camera = c.settings.camera.map(|c| Camera {
            eye: prism_math::DVec3::from_array(c.eye),
            target: prism_math::DVec3::from_array(c.target),
        });
        s.set_lens(lens_of(c.settings.camera.as_ref()));
        let doc = j.restore(c.assembly);
        s.open_document(doc, camera);
        s.set_units(c.settings.units);
        self.extra = c.settings.extra;
        s.set_view_style(view_style(&mut self.extra));
        self.name = j.path.as_deref().map(doc_files::display_name);
        self.path = j
            .path
            .clone()
            .filter(|_| !cfg!(target_arch = "wasm32"))
            .filter(|p| p.contains(['/', '\\']));
        // Restored work is not saved yet; the journal stays valid against
        // the same base.
        self.saved_rev = None;
        self.base = Base {
            bytes: base,
            log_len: 0,
            written: true,
        };
        chrome.status_note("Restored your unsaved work".into());
    }
}

/// "Prism - Build 120", or "Prism - Local build" for one made by hand.
pub fn app_name() -> String {
    match crate::update::this_build() {
        Some(n) => format!("Prism - Build {n}"),
        None => "Prism - Local build".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The title names the build, so an update shows (tests are local builds).
    #[test]
    fn the_title_names_the_build() {
        let s = Session::new(Document::new());
        let f = Files::new(&s, 1);
        assert_eq!(f.title(&s), "Untitled — Prism - Local build");
        assert_eq!(app_name(), "Prism - Local build");
    }

    /// The view style is saved with the file, and a file without one opens
    /// with Default (View Settings, PLAN 1.14).
    #[test]
    fn the_view_style_is_saved_with_the_file() {
        let mut s = Session::new(Document::new());
        let technical = prism_view::ViewStyle::preset("Technical").expect("preset");
        s.set_view_style(technical.clone());
        let bytes = prism_file::write(&contents(&s, &Map::new()), app_version());
        let mut extra = prism_file::read(&bytes).expect("read").settings.extra;
        assert_eq!(view_style(&mut extra), technical);
        assert!(extra.is_empty(), "the view key is taken out");
        assert_eq!(
            view_style(&mut Map::new()),
            prism_view::ViewStyle::default()
        );
    }
}
