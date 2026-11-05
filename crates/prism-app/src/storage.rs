//! Where settings live: a file in the user's config folder natively, and the
//! browser's local storage on the web.

const KEYMAP: &str = "keymap.json";
/// The name used when the keymap is exported for sharing.
pub const EXPORT_NAME: &str = "Prism keymap.json";

/// The saved keymap's JSON, if there is one.
pub fn load_keymap() -> Option<String> {
    imp::read(KEYMAP)
}

pub fn save_keymap(json: &str) {
    if let Err(e) = imp::write(KEYMAP, json) {
        log::warn!("could not save the keymap: {e}");
    }
}

/// Share the keymap as a file. Returns where it went, for the status bar.
pub fn export_keymap(json: &str) -> Result<String, String> {
    imp::export(EXPORT_NAME, json)
}

/// Save a recorded session as a file the user can send (natively in
/// Downloads, on the web as a download). Returns where it went.
pub fn export_session(json: &str) -> Result<String, String> {
    imp::export("Prism session.json", json)
}

/// Read a shared keymap file back. On the web this opens a file picker and
/// the result arrives later through [`take_imported`].
pub fn import_keymap() -> Result<Option<String>, String> {
    imp::import(EXPORT_NAME)
}

/// A keymap picked in the web file picker, once it has been read.
pub fn take_imported() -> Option<String> {
    imp::take_imported()
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::path::PathBuf;

    /// The per-user config folder: %APPDATA%\Prism, ~/Library/Application
    /// Support/Prism, or $XDG_CONFIG_HOME/prism.
    fn config_dir() -> Option<PathBuf> {
        let env = |k: &str| {
            std::env::var_os(k)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        let base = if cfg!(windows) {
            env("APPDATA")?
        } else if cfg!(target_os = "macos") {
            env("HOME")?.join("Library/Application Support")
        } else {
            env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config")))?
        };
        Some(base.join(if cfg!(any(windows, target_os = "macos")) {
            "Prism"
        } else {
            "prism"
        }))
    }

    /// Where exports go: the Downloads folder, else the home folder.
    fn export_dir() -> Option<PathBuf> {
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)?;
        let downloads = home.join("Downloads");
        Some(if downloads.is_dir() { downloads } else { home })
    }

    pub fn read(name: &str) -> Option<String> {
        std::fs::read_to_string(config_dir()?.join(name)).ok()
    }

    pub fn write(name: &str, text: &str) -> Result<(), String> {
        let dir = config_dir().ok_or("no config folder")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        // Write a temporary file, then rename it over the old one, so a
        // crash never leaves half a file.
        let tmp = dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, dir.join(name)).map_err(|e| e.to_string())
    }

    pub fn export(name: &str, text: &str) -> Result<String, String> {
        let path = export_dir().ok_or("no home folder")?.join(name);
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        Ok(path.display().to_string())
    }

    pub fn import(name: &str) -> Result<Option<String>, String> {
        let path = export_dir().ok_or("no home folder")?.join(name);
        std::fs::read_to_string(&path)
            .map(Some)
            .map_err(|_| format!("put the file at {} first", path.display()))
    }

    pub fn take_imported() -> Option<String> {
        None
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    thread_local! {
        static IMPORTED: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    fn key(name: &str) -> String {
        format!("prism.{name}")
    }

    pub fn read(name: &str) -> Option<String> {
        storage()?.get_item(&key(name)).ok()?
    }

    pub fn write(name: &str, text: &str) -> Result<(), String> {
        storage()
            .ok_or("no local storage")?
            .set_item(&key(name), text)
            .map_err(|_| "local storage refused".to_owned())
    }

    /// Download the text as a file.
    pub fn export(name: &str, text: &str) -> Result<String, String> {
        let err = |_| "the browser refused the download".to_owned();
        let doc = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("no document")?;
        let parts = js_sys::Array::of1(&wasm_bindgen::JsValue::from_str(text));
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type("application/json");
        let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts).map_err(err)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
        let a: web_sys::HtmlAnchorElement = doc
            .create_element("a")
            .map_err(err)?
            .dyn_into()
            .map_err(|_| "no link".to_owned())?;
        a.set_href(&url);
        a.set_download(name);
        a.click();
        let _ = web_sys::Url::revoke_object_url(&url);
        Ok("your downloads".to_owned())
    }

    /// Open a file picker; the chosen file is read in the background.
    pub fn import(_name: &str) -> Result<Option<String>, String> {
        let err = |_| "the browser refused the file picker".to_owned();
        let doc = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("no document")?;
        let input: web_sys::HtmlInputElement = doc
            .create_element("input")
            .map_err(err)?
            .dyn_into()
            .map_err(|_| "no input".to_owned())?;
        input.set_type("file");
        input.set_accept(".json,application/json");
        let picked = input.clone();
        let on_change = Closure::<dyn FnMut()>::new(move || {
            let Some(file) = picked.files().and_then(|f| f.get(0)) else {
                return;
            };
            let Ok(reader) = web_sys::FileReader::new() else {
                return;
            };
            let r = reader.clone();
            let on_load = Closure::<dyn FnMut()>::new(move || {
                if let Some(text) = r.result().ok().and_then(|v| v.as_string()) {
                    IMPORTED.with(|i| *i.borrow_mut() = Some(text));
                }
            });
            reader.set_onload(Some(on_load.as_ref().unchecked_ref()));
            on_load.forget();
            let _ = reader.read_as_text(&file);
        });
        input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        on_change.forget();
        input.click();
        Ok(None)
    }

    pub fn take_imported() -> Option<String> {
        IMPORTED.with(|i| i.borrow_mut().take())
    }
}
