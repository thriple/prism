//! The recovery store (brief 08): the base model (`.prism` bytes) and the
//! journal of commands since. Natively two files in the config folder's
//! `recovery` folder; on the web, IndexedDB (every browser has it).

use prism_file::Journal;

pub use imp::{clear, left_behind, take_found, write_base, write_journal};

/// A journal and base read back, if both are there and the journal reads.
fn found(journal: Option<String>, base: Option<Vec<u8>>) -> Option<(Journal, Vec<u8>)> {
    let j = Journal::from_json(&journal?).ok()?;
    let base = base?;
    // An empty journal brings nothing back.
    (!j.commands.is_empty()).then_some((j, base))
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::path::PathBuf;

    use prism_file::Journal;

    const BASE: &str = "base.prism";
    const JOURNAL: &str = "journal.json";

    fn dir() -> Option<PathBuf> {
        crate::storage::config_dir().map(|d| d.join("recovery"))
    }

    /// What a crash left behind.
    pub fn left_behind() -> Option<(Journal, Vec<u8>)> {
        let d = dir()?;
        super::found(
            std::fs::read_to_string(d.join(JOURNAL)).ok(),
            std::fs::read(d.join(BASE)).ok(),
        )
    }

    /// Web only (the web reads the store in the background).
    pub fn take_found() -> Option<(Journal, Vec<u8>)> {
        None
    }

    fn write(name: &str, bytes: &[u8]) {
        let Some(d) = dir() else { return };
        if std::fs::create_dir_all(&d).is_ok()
            && let Err(e) = crate::doc_files::save_atomic(&d.join(name), bytes)
        {
            log::warn!("autosave: {e}");
        }
    }

    pub fn write_base(bytes: &[u8]) {
        write(BASE, bytes);
        // A new base starts a new journal.
        if let Some(d) = dir() {
            let _ = std::fs::remove_file(d.join(JOURNAL));
        }
    }

    pub fn write_journal(json: &str) {
        write(JOURNAL, json.as_bytes());
    }

    pub fn clear() {
        if let Some(d) = dir() {
            for f in [BASE, JOURNAL, "base.prism.bak", "journal.json.bak"] {
                let _ = std::fs::remove_file(d.join(f));
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;

    use js_sys::Uint8Array;
    use prism_file::Journal;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{IdbDatabase, IdbTransactionMode};

    const DB: &str = "prism";
    const STORE: &str = "recovery";

    thread_local! {
        static DATABASE: RefCell<Option<IdbDatabase>> = const { RefCell::new(None) };
        /// Writes asked for before the database was open.
        static WAITING: RefCell<Vec<(String, Option<JsValue>)>> = const { RefCell::new(Vec::new()) };
        static FOUND: RefCell<Option<(Journal, Vec<u8>)>> = const { RefCell::new(None) };
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

    /// Open the database, read what a closed tab left behind, then do any
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

    fn read_back(db: &IdbDatabase) {
        let Some(s) = store(db, IdbTransactionMode::Readonly) else {
            return;
        };
        let (Ok(jr), Ok(br)) = (s.get(&"journal".into()), s.get(&"base".into())) else {
            return;
        };
        let br2 = br.clone();
        let on_done = Closure::<dyn FnMut()>::new(move || {
            let journal = jr.result().ok().and_then(|v| v.as_string());
            let base = br2
                .result()
                .ok()
                .filter(|v| !v.is_undefined())
                .map(|v| Uint8Array::new(&v).to_vec());
            if let Some(f) = super::found(journal, base) {
                FOUND.with(|x| *x.borrow_mut() = Some(f));
            }
        });
        // The base is read after the journal in the same transaction.
        br.set_onsuccess(Some(on_done.as_ref().unchecked_ref()));
        on_done.forget();
    }

    /// The web reads the store in the background: see [`take_found`].
    pub fn left_behind() -> Option<(Journal, Vec<u8>)> {
        start();
        None
    }

    pub fn take_found() -> Option<(Journal, Vec<u8>)> {
        FOUND.with(|f| f.borrow_mut().take())
    }

    pub fn write_base(bytes: &[u8]) {
        start();
        set("base", Some(Uint8Array::from(bytes).buffer().into()));
        set("journal", None);
    }

    pub fn write_journal(json: &str) {
        start();
        set("journal", Some(JsValue::from_str(json)));
    }

    pub fn clear() {
        start();
        set("base", None);
        set("journal", None);
    }
}
