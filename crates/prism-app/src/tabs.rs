//! Document tabs (brief 11, DECISIONS #27). The app keeps the active
//! document in its own `viewport` and `files`; the others wait here, and
//! switching swaps them. Each tab has its own document, undo, selection,
//! camera, tool step, file and recovery slot.

use prism_doc::Document;

use crate::chrome::Chrome;
use crate::doc_files::Opened;
use crate::files::{Files, Request};
use crate::recovery::Found;
use crate::viewport::Viewport;

pub struct Tabs {
    /// Every tab in order; the active one is `None` (it lives in the app).
    parked: Vec<Option<(Viewport, Files)>>,
    active: usize,
    /// Closing the window: each tab closes in turn.
    closing_all: bool,
    /// Every tab is closed: the app quits.
    pub exit: bool,
    next_slot: u32,
}

impl Tabs {
    /// One tab, the one the app already has (with recovery slot `slot`).
    pub fn new(slot: u32) -> Self {
        Self {
            parked: vec![None],
            active: 0,
            closing_all: false,
            exit: false,
            next_slot: slot + 1,
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.parked.len()
    }

    pub fn active(&self) -> usize {
        self.active
    }

    /// Each tab's name and whether it has unsaved changes.
    pub fn labels(&self, vp: &Viewport, f: &Files) -> Vec<(String, bool)> {
        self.parked
            .iter()
            .map(|t| match t {
                Some((v, fl)) => (fl.name().to_owned(), fl.dirty(&v.session)),
                None => (f.name().to_owned(), f.dirty(&vp.session)),
            })
            .collect()
    }

    fn slot(&mut self) -> u32 {
        self.next_slot += 1;
        self.next_slot - 1
    }

    /// Make tab `i` the active one. Returns whether it changed.
    pub fn switch(&mut self, i: usize, vp: &mut Viewport, f: &mut Files) -> bool {
        if i == self.active || i >= self.parked.len() {
            return false;
        }
        let Some((nv, nf)) = self.parked[i].take() else {
            return false;
        };
        f.flush(&vp.session);
        let old_v = swap_in(vp, nv);
        let old_f = std::mem::replace(f, nf);
        self.parked[self.active] = Some((old_v, old_f));
        self.active = i;
        true
    }

    /// The next (or previous) tab, wrapping round.
    pub fn step(&mut self, by: isize, vp: &mut Viewport, f: &mut Files) -> bool {
        let n = self.parked.len() as isize;
        let i = (self.active as isize + by).rem_euclid(n) as usize;
        self.switch(i, vp, f)
    }

    /// A new tab with `doc`, made active.
    fn push(&mut self, doc: Document, vp: &mut Viewport, f: &mut Files) {
        let slot = self.slot();
        let nv = Viewport::with_document(doc);
        let nf = Files::new(&nv.session, slot);
        f.flush(&vp.session);
        let old_v = swap_in(vp, nv);
        let old_f = std::mem::replace(f, nf);
        self.parked[self.active] = Some((old_v, old_f));
        self.parked.insert(self.active + 1, None);
        self.active += 1;
    }

    /// Where a file with this path is open already.
    fn find(&self, path: &str, f: &Files) -> Option<usize> {
        self.parked.iter().position(|t| match t {
            Some((_, fl)) => fl.path() == Some(path),
            None => f.path() == Some(path),
        })
    }

    /// What the active tab asked for. Returns whether the active tab
    /// changed.
    pub fn handle(&mut self, vp: &mut Viewport, f: &mut Files, chrome: &mut Chrome) -> bool {
        let mut changed = false;
        for r in std::mem::take(&mut f.requests) {
            match r {
                Request::NewTab => {
                    // An untitled, unchanged, empty tab is already new.
                    if !(f.pristine(&vp.session)
                        && vp.session.model().face_count() == 0
                        && vp.session.model().edge_count() == 0)
                    {
                        self.push(Document::new(), vp, f);
                        changed = true;
                    }
                }
                Request::Open(o) => changed |= self.open(o, vp, f, chrome),
            }
        }
        changed
    }

    /// Open a file: switch to it if it is open already; else in the active
    /// tab if that is untitled and unchanged; else in a new tab.
    pub fn open(
        &mut self,
        o: Opened,
        vp: &mut Viewport,
        f: &mut Files,
        chrome: &mut Chrome,
    ) -> bool {
        if let Some(p) = &o.path
            && let Some(i) = self.find(p, f)
        {
            return self.switch(i, vp, f);
        }
        let mut changed = false;
        if !f.pristine(&vp.session) {
            self.push(Document::new(), vp, f);
            changed = true;
        }
        f.load(o, &mut vp.session, chrome);
        changed
    }

    /// Work a crash left behind: one tab each, offered back.
    pub fn offer(&mut self, found: Vec<Found>, vp: &mut Viewport, f: &mut Files) -> bool {
        let mut changed = false;
        for fd in found {
            self.next_slot = self.next_slot.max(fd.slot + 1);
            if !(f.pristine(&vp.session) && !f.modal()) {
                self.push(Document::new(), vp, f);
                changed = true;
            }
            f.offer(fd);
        }
        changed
    }

    /// Close tab `i`: it asks "Save changes?" if it has any.
    pub fn close(
        &mut self,
        i: usize,
        vp: &mut Viewport,
        f: &mut Files,
        chrome: &mut Chrome,
    ) -> bool {
        let changed = self.switch(i, vp, f);
        f.close_requested(&mut vp.session, chrome);
        changed
    }

    /// The window's close button: close every tab in turn, asking each
    /// with unsaved changes; Cancel stops.
    pub fn close_window(&mut self, vp: &mut Viewport, f: &mut Files, chrome: &mut Chrome) {
        self.closing_all = true;
        f.close_requested(&mut vp.session, chrome);
    }

    /// After a frame: tabs that finished closing go, and the next one in a
    /// window close is asked. Returns whether the active tab changed.
    pub fn after_frame(&mut self, vp: &mut Viewport, f: &mut Files, chrome: &mut Chrome) -> bool {
        let mut changed = false;
        if f.take_cancelled() {
            self.closing_all = false;
        }
        while f.take_quit() {
            changed = true;
            self.parked.remove(self.active);
            if self.parked.is_empty() {
                if self.closing_all {
                    self.exit = true;
                    self.parked.push(None);
                    self.active = 0;
                    return changed;
                }
                // The last tab closed: one empty tab stays.
                let slot = self.slot();
                swap_in(vp, Viewport::with_document(Document::new()));
                *f = Files::new(&vp.session, slot);
                self.parked.push(None);
                self.active = 0;
                return changed;
            }
            self.active = self.active.min(self.parked.len() - 1);
            let (nv, nf) = self.parked[self.active].take().expect("a parked tab");
            swap_in(vp, nv);
            *f = nf;
            if self.closing_all {
                f.close_requested(&mut vp.session, chrome);
            }
        }
        changed
    }
}

/// Put another document in the viewport. The clipboard goes with the
/// viewport, so every open document shares it (brief 33).
fn swap_in(vp: &mut Viewport, mut nv: Viewport) -> Viewport {
    nv.session.set_clipboard(vp.session.clipboard());
    std::mem::replace(vp, nv)
}

#[cfg(test)]
mod tests {
    use prism_doc::Command;

    use super::*;

    fn setup() -> (Tabs, Viewport, Files, Chrome) {
        let vp = Viewport::with_document(Document::new());
        let f = Files::new(&vp.session, 100);
        (
            Tabs::new(100),
            vp,
            f,
            Chrome::with_keymap(
                crate::actions::keymap::Platform::Linux,
                crate::actions::keymap::Keymap::from_preset(
                    crate::actions::keymap::Preset::SketchUp,
                    &crate::actions::registry::Registry::new(),
                ),
            ),
        )
    }

    fn edit(vp: &mut Viewport) {
        vp.session.open_document(
            {
                let mut d = Document::new();
                d.run(Command::Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 1.0,
                    y1: 1.0,
                    z: 0.0,
                });
                d
            },
            None,
        );
    }

    fn opened(name: &str, path: &str) -> Opened {
        let bytes = prism_file::write(
            &prism_file::Contents::with_model(
                prism_tools::scenes::cube().model().clone(),
                Default::default(),
            ),
            "test",
        );
        Opened {
            name: name.into(),
            path: Some(path.into()),
            bytes,
        }
    }

    #[test]
    fn opening_uses_an_untitled_tab_then_makes_new_ones() {
        let (mut t, mut vp, mut f, mut c) = setup();
        t.open(opened("a.prism", "/x/a.prism"), &mut vp, &mut f, &mut c);
        assert_eq!(t.len(), 1, "the untitled, unchanged tab was used");
        t.open(opened("b.prism", "/x/b.prism"), &mut vp, &mut f, &mut c);
        assert_eq!(t.len(), 2);
        assert_eq!(f.name(), "b.prism");
        // Opening a file that is open switches to it.
        t.open(opened("a.prism", "/x/a.prism"), &mut vp, &mut f, &mut c);
        assert_eq!((t.len(), t.active(), f.name()), (2, 0, "a.prism"));
        let labels = t.labels(&vp, &f);
        assert_eq!(labels[1].0, "b.prism");
    }

    #[test]
    fn each_tab_has_its_own_document() {
        let (mut t, mut vp, mut f, mut c) = setup();
        edit(&mut vp);
        f.requests.push(Request::NewTab);
        t.handle(&mut vp, &mut f, &mut c);
        assert_eq!(t.len(), 2);
        assert_eq!(vp.session.model().face_count(), 0, "the new tab is empty");
        t.switch(0, &mut vp, &mut f);
        assert_eq!(
            vp.session.model().face_count(),
            1,
            "the first tab kept its work"
        );
        assert!(vp.can_undo());
    }

    #[test]
    fn closing_asks_only_that_tab_and_the_last_tab_stays_empty() {
        let (mut t, mut vp, mut f, mut c) = setup();
        edit(&mut vp);
        f.requests.push(Request::NewTab);
        t.handle(&mut vp, &mut f, &mut c);
        // Close the new (unchanged) tab: no question, it goes.
        t.close(1, &mut vp, &mut f, &mut c);
        t.after_frame(&mut vp, &mut f, &mut c);
        assert_eq!(t.len(), 1);
        assert_eq!(vp.session.model().face_count(), 1, "back on the edited tab");
        // The edited tab asks first.
        t.close(0, &mut vp, &mut f, &mut c);
        t.after_frame(&mut vp, &mut f, &mut c);
        assert!(f.modal(), "Save changes? is showing");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn closing_the_window_closes_unchanged_tabs_and_asks_the_rest() {
        let (mut t, mut vp, mut f, mut c) = setup();
        f.requests.push(Request::NewTab);
        t.handle(&mut vp, &mut f, &mut c);
        f.requests.push(Request::NewTab);
        t.handle(&mut vp, &mut f, &mut c);
        // Nothing to save: everything closes, and the app quits.
        let (mut t2, mut vp2, mut f2) = (t, vp, f);
        t2.close_window(&mut vp2, &mut f2, &mut c);
        t2.after_frame(&mut vp2, &mut f2, &mut c);
        assert!(t2.exit);
        // With an edited tab, it stops there and asks.
        let (mut t, mut vp, mut f, mut c) = setup();
        f.requests.push(Request::NewTab);
        t.handle(&mut vp, &mut f, &mut c);
        edit(&mut vp);
        t.switch(0, &mut vp, &mut f);
        t.close_window(&mut vp, &mut f, &mut c);
        t.after_frame(&mut vp, &mut f, &mut c);
        assert!(!t.exit);
        assert!(f.modal(), "the edited tab asks");
    }

    #[test]
    fn work_left_behind_comes_back_one_tab_each() {
        let (mut t, mut vp, mut f, _c) = setup();
        let base = prism_file::write(&prism_file::Contents::default(), "test");
        let found = |slot| Found {
            slot,
            journal: prism_file::Journal {
                path: None,
                commands: vec![Command::Rect {
                    x0: 0.0,
                    y0: 0.0,
                    x1: 1.0,
                    y1: 1.0,
                    z: 0.0,
                }],
            },
            base: base.clone(),
        };
        t.offer(vec![found(3), found(9)], &mut vp, &mut f);
        assert_eq!(t.len(), 2, "two journals, two tabs");
        assert!(f.modal(), "each asks Restore or Discard");
    }
}
