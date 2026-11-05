//! Reading and writing `.prism` files where the user keeps them (brief 08).
//!
//! Natively: `rfd` dialogs, an atomic save that keeps a `.bak`, and a list
//! of recent files in the config folder. On the web: the browser's file
//! pickers; Chrome and Edge save in place through the File System Access
//! API, other browsers download a copy. Web calls finish later, so their
//! results are picked up with [`take_opened`] and [`take_saved`].

/// A file that was opened: its name (for the title), where it lives (a
/// path natively), and its bytes.
pub struct Opened {
    pub name: String,
    pub path: Option<String>,
    pub bytes: Vec<u8>,
}

/// The file name shown for a path: "house.prism" from "C:\models\house.prism".
pub fn display_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_owned()
}

pub use imp::*;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use super::{Opened, display_name};

    const RECENT: &str = "recent.json";
    pub const MAX_RECENT: usize = 10;

    fn dialog() -> rfd::FileDialog {
        rfd::FileDialog::new().add_filter("Prism model", &[prism_file::EXTENSION])
    }

    /// Ask for a file and read it. `Ok(None)` if the user cancelled.
    pub fn open() -> Result<Option<Opened>, String> {
        let Some(path) = dialog().pick_file() else {
            return Ok(None);
        };
        open_path(&path.display().to_string()).map(Some)
    }

    pub fn open_path(path: &str) -> Result<Opened, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("could not read {path}: {e}"))?;
        Ok(Opened {
            name: display_name(path),
            path: Some(path.to_owned()),
            bytes,
        })
    }

    /// Save to `path`, or ask where first (Save As, or a new document).
    /// Returns where it went, or `Ok(None)` if the user cancelled.
    pub fn save(
        path: Option<&str>,
        suggested: &str,
        bytes: &[u8],
    ) -> Result<Option<(String, String)>, String> {
        let path = match path {
            Some(p) => PathBuf::from(p),
            None => {
                let Some(mut p) = dialog().set_file_name(suggested).save_file() else {
                    return Ok(None);
                };
                if p.extension().is_none() {
                    p.set_extension(prism_file::EXTENSION);
                }
                p
            }
        };
        save_atomic(&path, bytes)?;
        let p = path.display().to_string();
        Ok(Some((display_name(&p), p)))
    }

    /// Write a temporary file beside `path`, flush it to disk, keep the old
    /// file as `.bak`, then rename the new one into place. A failure leaves
    /// the old file as it was.
    pub fn save_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
        let file_name = path
            .file_name()
            .ok_or_else(|| format!("{} is not a file name", path.display()))?
            .to_string_lossy()
            .into_owned();
        let tmp = path.with_file_name(format!("{file_name}.tmp"));
        let bak = path.with_file_name(format!("{file_name}.bak"));
        let write = || -> std::io::Result<()> {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()
        };
        if let Err(e) = write() {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("could not save {}: {e}", path.display()));
        }
        if path.exists() {
            // Replaces an older backup (rename replaces files on Windows too).
            if let Err(e) = std::fs::rename(path, &bak) {
                let _ = std::fs::remove_file(&tmp);
                return Err(format!("could not keep a backup of {}: {e}", path.display()));
            }
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            // Put the old file back.
            let _ = std::fs::rename(&bak, path);
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("could not save {}: {e}", path.display()));
        }
        Ok(())
    }

    pub fn recent() -> Vec<String> {
        crate::storage::config_dir()
            .and_then(|d| std::fs::read_to_string(d.join(RECENT)).ok())
            .and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok())
            .unwrap_or_default()
            .into_iter()
            .take(MAX_RECENT)
            .collect()
    }

    pub fn set_recent(list: &[String]) {
        let Some(dir) = crate::storage::config_dir() else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(json) = serde_json::to_string_pretty(list) {
            let _ = save_atomic(&dir.join(RECENT), json.as_bytes());
        }
    }

    /// Web only.
    pub fn take_opened() -> Option<Result<Opened, String>> {
        None
    }

    /// Web only.
    pub fn take_saved() -> Option<Result<Option<(String, String)>, String>> {
        None
    }

    /// Web only: the browser warns before closing the tab.
    pub fn set_unsaved(_unsaved: bool) {}
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;

    use js_sys::{Array, Object, Promise, Reflect, Uint8Array};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    use super::Opened;

    pub const MAX_RECENT: usize = 0;

    thread_local! {
        static OPENED: RefCell<Option<Result<Opened, String>>> = const { RefCell::new(None) };
        static SAVED: RefCell<Option<Result<Option<(String, String)>, String>>> = const { RefCell::new(None) };
        /// The file the document came from or was saved to, where the
        /// browser lets us write to it again.
        static HANDLE: RefCell<Option<JsValue>> = const { RefCell::new(None) };
        static UNSAVED: RefCell<bool> = const { RefCell::new(false) };
        static WARNS: RefCell<bool> = const { RefCell::new(false) };
    }

    fn window() -> Option<web_sys::Window> {
        web_sys::window()
    }

    /// The browser's File System Access function `name`, if it has it.
    fn picker(name: &str) -> Option<js_sys::Function> {
        let w = window()?;
        Reflect::get(&w, &JsValue::from_str(name))
            .ok()?
            .dyn_into::<js_sys::Function>()
            .ok()
    }

    fn picker_options(suggested: Option<&str>) -> Object {
        let accept = Object::new();
        let _ = Reflect::set(
            &accept,
            &JsValue::from_str("application/x-prism"),
            &Array::of1(&JsValue::from_str(".prism")),
        );
        let ty = Object::new();
        let _ = Reflect::set(&ty, &"description".into(), &"Prism model".into());
        let _ = Reflect::set(&ty, &"accept".into(), &accept);
        let opts = Object::new();
        let _ = Reflect::set(&opts, &"types".into(), &Array::of1(&ty));
        if let Some(s) = suggested {
            let _ = Reflect::set(&opts, &"suggestedName".into(), &JsValue::from_str(s));
        }
        opts
    }

    async fn call_promise(target: &JsValue, method: &str, args: &Array) -> Result<JsValue, JsValue> {
        let f: js_sys::Function = Reflect::get(target, &JsValue::from_str(method))?.dyn_into()?;
        let p: Promise = Reflect::apply(&f, target, args)?.dyn_into()?;
        JsFuture::from(p).await
    }

    fn err(e: JsValue) -> String {
        e.as_string()
            .or_else(|| {
                Reflect::get(&e, &"message".into())
                    .ok()
                    .and_then(|m| m.as_string())
            })
            .unwrap_or_else(|| "the browser refused".to_owned())
    }

    fn cancelled(e: &JsValue) -> bool {
        Reflect::get(e, &"name".into())
            .ok()
            .and_then(|n| n.as_string())
            .is_some_and(|n| n == "AbortError")
    }

    async fn read_file(file: web_sys::File) -> Result<Vec<u8>, String> {
        let buf = JsFuture::from(file.array_buffer()).await.map_err(err)?;
        Ok(Uint8Array::new(&buf).to_vec())
    }

    /// Ask for a file; it is read in the background ([`take_opened`]).
    pub fn open() -> Result<Option<Opened>, String> {
        if let Some(show) = picker("showOpenFilePicker") {
            let w = window().ok_or("no window")?;
            let p: Promise = show
                .call1(&w, &picker_options(None))
                .map_err(err)?
                .dyn_into()
                .map_err(err)?;
            wasm_bindgen_futures::spawn_local(async move {
                let r = async {
                    let handles = JsFuture::from(p).await?;
                    let handle = Array::from(&handles).get(0);
                    let file: web_sys::File =
                        call_promise(&handle, "getFile", &Array::new()).await?.dyn_into()?;
                    Ok::<_, JsValue>((handle, file))
                }
                .await;
                let result = match r {
                    Ok((handle, file)) => {
                        let name = file.name();
                        match read_file(file).await {
                            Ok(bytes) => {
                                HANDLE.with(|h| *h.borrow_mut() = Some(handle));
                                Some(Ok(Opened {
                                    name,
                                    path: None,
                                    bytes,
                                }))
                            }
                            Err(e) => Some(Err(e)),
                        }
                    }
                    Err(e) if cancelled(&e) => None,
                    Err(e) => Some(Err(err(e))),
                };
                if result.is_some() {
                    OPENED.with(|o| *o.borrow_mut() = result);
                }
            });
            return Ok(None);
        }
        // Other browsers: a file input.
        let doc = window().and_then(|w| w.document()).ok_or("no document")?;
        let input: web_sys::HtmlInputElement = doc
            .create_element("input")
            .map_err(err)?
            .dyn_into()
            .map_err(|_| "no input".to_owned())?;
        input.set_type("file");
        input.set_accept(".prism");
        let picked = input.clone();
        let on_change = Closure::<dyn FnMut()>::new(move || {
            let Some(file) = picked.files().and_then(|f| f.get(0)) else {
                return;
            };
            wasm_bindgen_futures::spawn_local(async move {
                let name = file.name();
                let r = read_file(file).await.map(|bytes| Opened {
                    name,
                    path: None,
                    bytes,
                });
                HANDLE.with(|h| *h.borrow_mut() = None);
                OPENED.with(|o| *o.borrow_mut() = Some(r));
            });
        });
        input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        on_change.forget();
        input.click();
        Ok(None)
    }

    pub fn open_path(_path: &str) -> Result<Opened, String> {
        Err("the web build cannot open files by path".to_owned())
    }

    /// Save: in place where the browser allows (finishes later, see
    /// [`take_saved`]); else a download, at once. `path` is unused on the
    /// web: `Some` means "save again where it was".
    pub fn save(
        path: Option<&str>,
        suggested: &str,
        bytes: &[u8],
    ) -> Result<Option<(String, String)>, String> {
        let again = path.is_some();
        let handle = HANDLE.with(|h| h.borrow().clone()).filter(|_| again);
        let show = picker("showSaveFilePicker");
        if handle.is_none() && show.is_none() {
            download(suggested, bytes)?;
            return Ok(Some((suggested.to_owned(), String::new())));
        }
        let data = Uint8Array::from(bytes);
        let suggested = suggested.to_owned();
        let w = window().ok_or("no window")?;
        let pick: Option<Promise> = match (&handle, &show) {
            (Some(_), _) => None,
            (None, Some(f)) => Some(
                f.call1(&w, &picker_options(Some(&suggested)))
                    .map_err(err)?
                    .dyn_into()
                    .map_err(err)?,
            ),
            (None, None) => None,
        };
        wasm_bindgen_futures::spawn_local(async move {
            let r = async {
                let h = match (handle, pick) {
                    (Some(h), _) => h,
                    (None, Some(p)) => JsFuture::from(p).await?,
                    (None, None) => return Err(JsValue::from_str("no file")),
                };
                let w = call_promise(&h, "createWritable", &Array::new()).await?;
                call_promise(&w, "write", &Array::of1(&data)).await?;
                call_promise(&w, "close", &Array::new()).await?;
                let name = Reflect::get(&h, &"name".into())?
                    .as_string()
                    .unwrap_or(suggested);
                Ok::<_, JsValue>((h, name))
            }
            .await;
            let result = match r {
                Ok((h, name)) => {
                    HANDLE.with(|x| *x.borrow_mut() = Some(h));
                    Ok(Some((name, String::new())))
                }
                Err(e) if cancelled(&e) => Ok(None),
                Err(e) => Err(err(e)),
            };
            SAVED.with(|s| *s.borrow_mut() = Some(result));
        });
        Err(PENDING.to_owned())
    }

    /// What [`save`] returns while a web save is still running.
    pub const PENDING: &str = "pending";

    fn download(name: &str, bytes: &[u8]) -> Result<(), String> {
        let doc = window().and_then(|w| w.document()).ok_or("no document")?;
        let parts = Array::of1(&Uint8Array::from(bytes));
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type("application/x-prism");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
            .map_err(err)?;
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
        Ok(())
    }

    pub fn recent() -> Vec<String> {
        Vec::new()
    }

    pub fn set_recent(_list: &[String]) {}

    pub fn take_opened() -> Option<Result<Opened, String>> {
        OPENED.with(|o| o.borrow_mut().take())
    }

    pub fn take_saved() -> Option<Result<Option<(String, String)>, String>> {
        SAVED.with(|s| s.borrow_mut().take())
    }

    /// Forget the file handle (File > New).
    pub fn forget_handle() {
        HANDLE.with(|h| *h.borrow_mut() = None);
    }

    /// The browser warns before the tab closes while there are unsaved
    /// changes.
    pub fn set_unsaved(unsaved: bool) {
        UNSAVED.with(|u| *u.borrow_mut() = unsaved);
        if WARNS.with(|w| std::mem::replace(&mut *w.borrow_mut(), true)) {
            return;
        }
        let Some(w) = window() else { return };
        let warn = Closure::<dyn FnMut(web_sys::BeforeUnloadEvent)>::new(
            |e: web_sys::BeforeUnloadEvent| {
                if UNSAVED.with(|u| *u.borrow()) {
                    e.prevent_default();
                    e.set_return_value("You have unsaved changes.");
                }
            },
        );
        let _ = w.add_event_listener_with_callback("beforeunload", warn.as_ref().unchecked_ref());
        warn.forget();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub const PENDING: &str = "pending";

#[cfg(not(target_arch = "wasm32"))]
pub fn forget_handle() {}
