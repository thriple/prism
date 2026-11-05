//! The open document's file (brief 08): New, Open, Save, Save As, Recent,
//! Document Settings, "Save changes?", the window title, and autosave with
//! recovery after a crash.

use egui::Context;
use prism_doc::Document;
use prism_file::{CameraData, Contents, Journal, Settings};
use prism_math::{DisplayUnits, LengthUnit};
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
    New,
    Open,
    Recent(usize),
    Close,
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
    /// Fields a newer Prism wrote, kept for the next save.
    extra: Map<String, Value>,
    recent: Vec<String>,
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
    offer: Option<(Journal, Vec<u8>)>,
    title: String,
}

fn contents(s: &Session, extra: &Map<String, Value>) -> Contents {
    let c = s.camera();
    Contents {
        model: s.model().clone(),
        settings: Settings {
            units: s.number.units,
            camera: Some(CameraData {
                eye: c.eye.to_array(),
                target: c.target.to_array(),
            }),
            extra: extra.clone(),
        },
    }
}

fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

impl Files {
    pub fn new(session: &Session) -> Self {
        let base = prism_file::write(&contents(session, &Map::new()), app_version());
        Self {
            name: None,
            path: None,
            saved_rev: Some(session.document().revision()),
            settings_changed: false,
            extra: Map::new(),
            recent: doc_files::recent(),
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
            offer: recovery::left_behind(),
            title: String::new(),
        }
    }

    pub fn dirty(&self, s: &Session) -> bool {
        self.settings_changed || self.saved_rev != Some(s.document().revision())
    }

    /// A dialog of ours is open (keys go to it, not to the tools).
    pub fn modal(&self) -> bool {
        self.asking.is_some() || self.settings_open || self.offer.is_some()
    }

    pub fn take_quit(&mut self) -> bool {
        std::mem::take(&mut self.quit)
    }

    /// The recent files' names, for the Open Recent menu.
    pub fn recent_names(&self) -> Vec<String> {
        self.recent
            .iter()
            .map(|p| doc_files::display_name(p))
            .collect()
    }

    /// The window title: "house.prism • — Prism" while unsaved.
    pub fn title(&self, s: &Session) -> String {
        let name = self.name.as_deref().unwrap_or("Untitled");
        let dot = if self.dirty(s) { " •" } else { "" };
        format!("{name}{dot} — Prism")
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
        match id {
            "file.new" => self.then(Next::New, s, chrome),
            "file.open" => self.then(Next::Open, s, chrome),
            "file.save" => self.save(false, None, s, chrome),
            "file.saveas" => self.save(true, None, s, chrome),
            "file.docsettings" => self.settings_open = true,
            _ => {
                if let Some(n) = id
                    .strip_prefix("file.recent")
                    .and_then(|n| n.parse::<usize>().ok())
                    && n >= 1
                {
                    self.then(Next::Recent(n - 1), s, chrome);
                }
            }
        }
    }

    /// The window's close button.
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

    fn go(&mut self, next: Next, s: &mut Session, chrome: &mut Chrome) {
        match next {
            Next::New => {
                s.open_document(Document::new(), None);
                s.set_units(DisplayUnits::default());
                doc_files::forget_handle();
                self.name = None;
                self.path = None;
                self.extra = Map::new();
                self.mark_saved(s, None);
                recovery::clear();
            }
            Next::Open => match doc_files::open() {
                Ok(Some(o)) => self.load(o, s, chrome),
                Ok(None) => {}
                Err(e) => chrome.status_note(format!("Could not open: {e}")),
            },
            Next::Recent(i) => {
                let Some(path) = self.recent.get(i).cloned() else {
                    return;
                };
                match doc_files::open_path(&path) {
                    Ok(o) => self.load(o, s, chrome),
                    Err(e) => {
                        self.recent.retain(|p| *p != path);
                        doc_files::set_recent(&self.recent);
                        chrome.status_note(e);
                    }
                }
            }
            Next::Close => {
                recovery::clear();
                self.quit = true;
            }
        }
    }

    fn load(&mut self, o: Opened, s: &mut Session, chrome: &mut Chrome) {
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
        s.open_document(Document::with_model(c.model), camera);
        s.set_units(c.settings.units);
        self.extra = c.settings.extra;
        self.name = Some(o.name.clone());
        self.path = o.path.clone();
        if let Some(p) = &o.path {
            self.remember(p);
        }
        self.mark_saved(s, Some(o.bytes));
        recovery::clear();
        chrome.status_note(format!("Opened {}", o.name));
    }

    fn remember(&mut self, path: &str) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_owned());
        self.recent.truncate(doc_files::MAX_RECENT);
        doc_files::set_recent(&self.recent);
    }

    /// The document as it is now counts as saved, and is the new base for
    /// recovery.
    fn mark_saved(&mut self, s: &Session, bytes: Option<Vec<u8>>) {
        self.saved_rev = Some(s.document().revision());
        self.settings_changed = false;
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
        recovery::clear();
        if let Some(n) = next {
            self.go(n, s, chrome);
        }
    }

    /// Every frame: finished web calls, autosave, and the dialogs.
    pub fn frame(&mut self, ctx: &Context, now: f64, s: &mut Session, chrome: &mut Chrome) {
        if let Some(r) = doc_files::take_opened() {
            match r {
                Ok(o) => self.load(o, s, chrome),
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
        if self.offer.is_none()
            && let Some(o) = recovery::take_found()
        {
            self.offer = Some(o);
        }
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
            recovery::write_base(&self.base.bytes);
            self.base.written = true;
        }
        let j = Journal {
            path: self.path.clone().or_else(|| self.name.clone()),
            commands: s.document().log().skip(self.base.log_len).cloned().collect(),
        };
        recovery::write_journal(&j.to_json());
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
                    recovery::clear();
                    // Don't Save: what comes next replaces the document.
                    self.saved_rev = Some(s.document().revision());
                    self.settings_changed = false;
                    self.go(next, s, chrome);
                }
                _ => {}
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
            && let Some((journal, base)) = self.offer.take()
        {
            if yes {
                self.restore(&journal, base, s, chrome);
            } else {
                recovery::clear();
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
                recovery::clear();
                return;
            }
        };
        let camera = c.settings.camera.map(|c| Camera {
            eye: prism_math::DVec3::from_array(c.eye),
            target: prism_math::DVec3::from_array(c.target),
        });
        let doc = j.restore(c.model);
        s.open_document(doc, camera);
        s.set_units(c.settings.units);
        self.extra = c.settings.extra;
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
