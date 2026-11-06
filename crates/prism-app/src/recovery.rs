//! The recovery store (briefs 08 and 11): for each tab, the base model
//! (`.prism` bytes) and the journal of commands since. Natively a folder
//! per tab in the config folder's `recovery` folder; on the web, IndexedDB
//! keys per tab (every browser has it).

use prism_file::Journal;

pub use imp::{clear, left_behind, take_found, write_base, write_journal};

/// What a closed or crashed Prism left behind for one tab.
pub struct Found {
    pub slot: u32,
    pub journal: Journal,
    pub base: Vec<u8>,
}

/// A journal and base read back, if both are there and the journal reads.
fn found(slot: u32, journal: Option<String>, base: Option<Vec<u8>>) -> Option<Found> {
    let journal = Journal::from_json(&journal?).ok()?;
    let base = base?;
    // An empty journal brings nothing back.
    (!journal.commands.is_empty()).then_some(Found {
        slot,
        journal,
        base,
    })
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::path::{Path, PathBuf};

    use super::Found;

    const BASE: &str = "base.prism";
    const JOURNAL: &str = "journal.json";

    fn root() -> Option<PathBuf> {
        crate::storage::config_dir().map(|d| d.join("recovery"))
    }

    fn dir(slot: u32) -> Option<PathBuf> {
        root().map(|r| r.join(slot.to_string()))
    }

    /// Every tab's work a crash left behind.
    pub fn left_behind() -> Vec<Found> {
        root().map_or_else(Vec::new, |r| left_behind_in(&r))
    }

    pub fn left_behind_in(root: &Path) -> Vec<Found> {
        let mut out = Vec::new();
        // Part 1 kept one journal at the top of the folder: slot 0.
        if let Some(f) = super::found(
            0,
            std::fs::read_to_string(root.join(JOURNAL)).ok(),
            std::fs::read(root.join(BASE)).ok(),
        ) {
            out.push(f);
        }
        let mut slots: Vec<u32> = std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str()?.parse().ok())
            .collect();
        slots.sort_unstable();
        for slot in slots {
            let d = root.join(slot.to_string());
            if let Some(f) = super::found(
                slot,
                std::fs::read_to_string(d.join(JOURNAL)).ok(),
                std::fs::read(d.join(BASE)).ok(),
            ) && !out.iter().any(|o| o.slot == slot)
            {
                out.push(f);
            }
        }
        out
    }

    /// Web only (the web reads the store in the background).
    pub fn take_found() -> Vec<Found> {
        Vec::new()
    }

    fn write(d: &Path, name: &str, bytes: &[u8]) {
        if std::fs::create_dir_all(d).is_ok()
            && let Err(e) = crate::doc_files::save_atomic(&d.join(name), bytes)
        {
            log::warn!("autosave: {e}");
        }
    }

    pub fn write_base(slot: u32, bytes: &[u8]) {
        if let Some(d) = dir(slot) {
            write_base_in(&d, bytes);
        }
    }

    pub fn write_base_in(d: &Path, bytes: &[u8]) {
        write(d, BASE, bytes);
        // A new base starts a new journal.
        let _ = std::fs::remove_file(d.join(JOURNAL));
    }

    pub fn write_journal(slot: u32, json: &str) {
        if let Some(d) = dir(slot) {
            write_journal_in(&d, json);
        }
    }

    pub fn write_journal_in(d: &Path, json: &str) {
        write(d, JOURNAL, json.as_bytes());
    }

    pub fn clear(slot: u32) {
        if let Some(d) = dir(slot) {
            clear_in(&d);
            let _ = std::fs::remove_dir(&d);
        }
        if slot == 0
            && let Some(r) = root()
        {
            clear_in(&r);
        }
    }

    pub fn clear_in(d: &Path) {
        for f in [BASE, JOURNAL, "base.prism.bak", "journal.json.bak"] {
            let _ = std::fs::remove_file(d.join(f));
        }
    }

    #[cfg(test)]
    mod tests {
        use prism_doc::{Command, Document};
        use prism_file::{Contents, Journal};
        use prism_math::DVec3;

        use super::*;

        fn scratch(name: &str) -> PathBuf {
            let d = std::env::temp_dir().join(format!("prism-test-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            d
        }

        fn journal_for(doc: &Document) -> String {
            Journal {
                path: None,
                commands: doc.log().cloned().collect(),
            }
            .to_json()
        }

        #[test]
        fn journals_left_behind_restore_one_tab_each() {
            let root = scratch("recovery");
            let base = prism_tools::scenes::cube();
            let bytes = prism_file::write(
                &Contents {
                    model: base.model().clone(),
                    ..Contents::default()
                },
                "test",
            );
            let mut a = Document::with_model(base.model().clone());
            a.run(Command::Rect {
                x0: 2.0,
                y0: 2.0,
                x1: 3.0,
                y1: 4.0,
                z: 0.0,
            });
            let mut b = Document::with_model(base.model().clone());
            b.run(Command::Segment {
                p: DVec3::new(2.0, 3.0, 0.0),
                q: DVec3::new(3.0, 3.0, 0.0),
            });
            for (slot, doc) in [(3u32, &a), (7, &b)] {
                let d = root.join(slot.to_string());
                write_base_in(&d, &bytes);
                write_journal_in(&d, &journal_for(doc));
            }
            let found = left_behind_in(&root);
            assert_eq!(found.iter().map(|f| f.slot).collect::<Vec<_>>(), [3, 7]);
            for (f, doc) in found.iter().zip([&a, &b]) {
                let model = prism_file::read(&f.base).expect("base").model;
                assert_eq!(
                    f.journal.restore(model).model().to_json(),
                    doc.model().to_json()
                );
            }
            clear_in(&root.join("3"));
            assert_eq!(left_behind_in(&root).len(), 1, "one cleared");
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn an_empty_or_damaged_journal_offers_nothing() {
            let root = scratch("recovery-empty");
            let d = root.join("1");
            write_base_in(&d, b"not a model");
            write_journal_in(&d, &Journal::default().to_json());
            assert!(left_behind_in(&root).is_empty());
            write_journal_in(&d, "{oops");
            assert!(left_behind_in(&root).is_empty());
            let _ = std::fs::remove_dir_all(&root);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;

    use js_sys::{Array, Uint8Array};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{IdbDatabase, IdbTransactionMode};

    use super::Found;

    const DB: &str = "prism";
    const STORE: &str = "recovery";

    thread_local! {
        static DATABASE: RefCell<Option<IdbDatabase>> = const { RefCell::new(None) };
        /// Writes asked for before the database was open.
        static WAITING: RefCell<Vec<(String, Option<JsValue>)>> = const { RefCell::new(Vec::new()) };
        static FOUND: RefCell<Vec<Found>> = const { RefCell::new(Vec::new()) };
        static STARTED: RefCell<bool> = const { RefCell::new(false) };
    }

    fn store(db: &IdbDatabase, mode: IdbTransactionMode) -> Option<web_sys::IdbObjectStore> {
        db.transaction_with_str_and_mode(STORE, mode)
            .ok()?
            .object_store(STORE)
            .ok()
    }

    /// Put (`Some`) or delete (`None`) a key, now or once the database opens.
    fn set(key: &str, value: Option<JsValue>) {
        let done = DATABASE.with(|d| {
            let d = d.borrow();
            let Some(db) = d.as_ref() else { return false };
            if let Some(s) = store(db, IdbTransactionMode::Readwrite) {
                let _ = match &value {
                    Some(v) => s.put_with_key(v, &JsValue::from_str(key)).map(|_| ()),
                    None => s.delete(&JsValue::from_str(key)).map(|_| ()),
                };
            }
            true
        });
        if !done {
            WAITING.with(|w| w.borrow_mut().push((key.to_owned(), value)));
        }
    }

    /// Open the database, read what closed tabs left behind, then do any
    /// writes that were waiting.
    fn start() {
        if STARTED.with(|s| std::mem::replace(&mut *s.borrow_mut(), true)) {
            return;
        }
        let Some(factory) = web_sys::window().and_then(|w| w.indexed_db().ok().flatten()) else {
            log::warn!("no IndexedDB: autosave is off");
            return;
        };
        let Ok(req) = factory.open_with_u32(DB, 1) else {
            return;
        };
        let up = req.clone();
        let on_upgrade = Closure::<dyn FnMut()>::new(move || {
            if let Ok(db) = up.result().and_then(|r| r.dyn_into::<IdbDatabase>()) {
                let _ = db.create_object_store(STORE);
            }
        });
        req.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
        on_upgrade.forget();
        let ok = req.clone();
        let on_success = Closure::<dyn FnMut()>::new(move || {
            let Ok(db) = ok.result().and_then(|r| r.dyn_into::<IdbDatabase>()) else {
                return;
            };
            read_back(&db);
            DATABASE.with(|d| *d.borrow_mut() = Some(db));
            for (k, v) in WAITING.with(|w| std::mem::take(&mut *w.borrow_mut())) {
                set(&k, v);
            }
        });
        req.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
        on_success.forget();
    }

    /// Every slot with a journal: read its journal and base.
    fn read_back(db: &IdbDatabase) {
        let Some(s) = store(db, IdbTransactionMode::Readonly) else {
            return;
        };
        let Ok(keys) = s.get_all_keys() else { return };
        let s2 = s.clone();
        let k2 = keys.clone();
        let on_keys = Closure::<dyn FnMut()>::new(move || {
            let Ok(list) = k2.result() else { return };
            for key in Array::from(&list).iter() {
                let Some(name) = key.as_string() else {
                    continue;
                };
                // Part 1 used "journal" and "base": slot 0.
                let slot: Option<u32> = match name.as_str() {
                    "journal" => Some(0),
                    n => n.strip_prefix("journal-").and_then(|n| n.parse().ok()),
                };
                let Some(slot) = slot else { continue };
                let base_key = if name == "journal" {
                    "base".to_owned()
                } else {
                    format!("base-{slot}")
                };
                let (Ok(jr), Ok(br)) = (s2.get(&key), s2.get(&JsValue::from_str(&base_key))) else {
                    continue;
                };
                let br2 = br.clone();
                let on_done = Closure::<dyn FnMut()>::new(move || {
                    let journal = jr.result().ok().and_then(|v| v.as_string());
                    let base = br2
                        .result()
                        .ok()
                        .filter(|v| !v.is_undefined())
                        .map(|v| Uint8Array::new(&v).to_vec());
                    if let Some(f) = super::found(slot, journal, base) {
                        FOUND.with(|x| x.borrow_mut().push(f));
                    }
                });
                br.set_onsuccess(Some(on_done.as_ref().unchecked_ref()));
                on_done.forget();
            }
        });
        keys.set_onsuccess(Some(on_keys.as_ref().unchecked_ref()));
        on_keys.forget();
    }

    /// The web reads the store in the background: see [`take_found`].
    pub fn left_behind() -> Vec<Found> {
        start();
        Vec::new()
    }

    pub fn take_found() -> Vec<Found> {
        FOUND.with(|f| std::mem::take(&mut *f.borrow_mut()))
    }

    pub fn write_base(slot: u32, bytes: &[u8]) {
        start();
        set(
            &format!("base-{slot}"),
            Some(Uint8Array::from(bytes).buffer().into()),
        );
        set(&format!("journal-{slot}"), None);
    }

    pub fn write_journal(slot: u32, json: &str) {
        start();
        set(&format!("journal-{slot}"), Some(JsValue::from_str(json)));
    }

    pub fn clear(slot: u32) {
        start();
        set(&format!("base-{slot}"), None);
        set(&format!("journal-{slot}"), None);
        if slot == 0 {
            set("base", None);
            set("journal", None);
        }
    }
}
