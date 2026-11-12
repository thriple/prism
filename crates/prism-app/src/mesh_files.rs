//! Picking mesh files to import, saving exports, and files dropped onto
//! the window (brief 09). On the web, picks and drops finish later and are
//! picked up with [`take_picked`] and [`take_dropped`].

use crate::doc_files::Opened;

/// The extensions File > Import offers (and drops accept).
pub const IMPORTS: [&str; 6] = ["stl", "obj", "3mf", "glb", "gltf", "ply"];

/// The images the Materials window loads (Basic Materials part 2).
pub const IMAGES: [&str; 3] = ["png", "jpg", "jpeg"];

/// Whether a file name is an image the Materials window loads.
pub fn is_image(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    IMAGES.iter().any(|e| lower.ends_with(&format!(".{e}")))
}

pub use imp::*;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::{IMAGES, IMPORTS, Opened};

    /// Ask for an image (a texture). `Ok(None)` if the user cancelled.
    pub fn pick_image() -> Result<Option<Opened>, String> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &IMAGES)
            .pick_file()
        else {
            return Ok(None);
        };
        crate::doc_files::open_path(&path.display().to_string()).map(Some)
    }

    /// Web only.
    pub fn take_picked_image() -> Option<Result<Opened, String>> {
        None
    }

    /// Ask for a mesh file. `Ok(None)` if the user cancelled.
    pub fn pick() -> Result<Option<Opened>, String> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Meshes", &IMPORTS)
            .pick_file()
        else {
            return Ok(None);
        };
        crate::doc_files::open_path(&path.display().to_string()).map(Some)
    }

    /// Save an export. Returns where it went, or `Ok(None)` if cancelled.
    pub fn save(suggested: &str, ext: &str, bytes: &[u8]) -> Result<Option<String>, String> {
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter(ext.to_uppercase(), &[ext])
            .set_file_name(suggested)
            .save_file()
        else {
            return Ok(None);
        };
        if path.extension().is_none() {
            path.set_extension(ext);
        }
        crate::doc_files::save_atomic(&path, bytes)?;
        Ok(Some(path.display().to_string()))
    }

    /// Web only.
    pub fn take_picked() -> Option<Result<Opened, String>> {
        None
    }

    /// Web only (natively, winit reports dropped files).
    pub fn take_dropped() -> Option<Result<Opened, String>> {
        None
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use std::cell::RefCell;

    use js_sys::{Array, Uint8Array};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen_futures::JsFuture;

    use super::{IMAGES, IMPORTS, Opened};

    thread_local! {
        static PICKED: RefCell<Option<Result<Opened, String>>> = const { RefCell::new(None) };
        static PICKED_IMAGE: RefCell<Option<Result<Opened, String>>> = const { RefCell::new(None) };
        static DROPPED: RefCell<Option<Result<Opened, String>>> = const { RefCell::new(None) };
        static LISTENING: RefCell<bool> = const { RefCell::new(false) };
    }

    fn read_into(
        file: web_sys::File,
        slot: &'static std::thread::LocalKey<RefCell<Option<Result<Opened, String>>>>,
    ) {
        wasm_bindgen_futures::spawn_local(async move {
            let name = file.name();
            let r = match JsFuture::from(file.array_buffer()).await {
                Ok(buf) => Ok(Opened {
                    name,
                    path: None,
                    bytes: Uint8Array::new(&buf).to_vec(),
                }),
                Err(_) => Err("the browser could not read the file".to_owned()),
            };
            slot.with(|s| *s.borrow_mut() = Some(r));
        });
    }

    pub fn pick() -> Result<Option<Opened>, String> {
        pick_into(&IMPORTS, &PICKED)
    }

    /// The image comes later, through [`take_picked_image`].
    pub fn pick_image() -> Result<Option<Opened>, String> {
        pick_into(&IMAGES, &PICKED_IMAGE)
    }

    pub fn take_picked_image() -> Option<Result<Opened, String>> {
        PICKED_IMAGE.with(|p| p.borrow_mut().take())
    }

    fn pick_into(
        exts: &[&str],
        slot: &'static std::thread::LocalKey<RefCell<Option<Result<Opened, String>>>>,
    ) -> Result<Option<Opened>, String> {
        let doc = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("no document")?;
        let input: web_sys::HtmlInputElement = doc
            .create_element("input")
            .map_err(|_| "no input".to_owned())?
            .dyn_into()
            .map_err(|_| "no input".to_owned())?;
        input.set_type("file");
        let accept: Vec<String> = exts.iter().map(|e| format!(".{e}")).collect();
        input.set_accept(&accept.join(","));
        let picked = input.clone();
        let on_change = Closure::<dyn FnMut()>::new(move || {
            if let Some(file) = picked.files().and_then(|f| f.get(0)) {
                read_into(file, slot);
            }
        });
        input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        on_change.forget();
        input.click();
        Ok(None)
    }

    /// Exports download on the web.
    pub fn save(suggested: &str, _ext: &str, bytes: &[u8]) -> Result<Option<String>, String> {
        let doc = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("no document")?;
        let parts = Array::of1(&Uint8Array::from(bytes));
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type("application/octet-stream");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
            .map_err(|_| "the browser refused the download".to_owned())?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|_| "the browser refused the download".to_owned())?;
        let a: web_sys::HtmlAnchorElement = doc
            .create_element("a")
            .map_err(|_| "no link".to_owned())?
            .dyn_into()
            .map_err(|_| "no link".to_owned())?;
        a.set_href(&url);
        a.set_download(suggested);
        a.click();
        let _ = web_sys::Url::revoke_object_url(&url);
        Ok(Some("your downloads".to_owned()))
    }

    pub fn take_picked() -> Option<Result<Opened, String>> {
        PICKED.with(|p| p.borrow_mut().take())
    }

    /// Files dropped on the page. The first call starts listening (winit
    /// does not report drops on the web).
    pub fn take_dropped() -> Option<Result<Opened, String>> {
        if !LISTENING.with(|l| std::mem::replace(&mut *l.borrow_mut(), true)) {
            listen();
        }
        DROPPED.with(|d| d.borrow_mut().take())
    }

    fn listen() {
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let over = Closure::<dyn FnMut(web_sys::DragEvent)>::new(|e: web_sys::DragEvent| {
            // Without this the browser opens the file itself.
            e.prevent_default();
        });
        let drop = Closure::<dyn FnMut(web_sys::DragEvent)>::new(|e: web_sys::DragEvent| {
            e.prevent_default();
            if let Some(file) = e
                .data_transfer()
                .and_then(|d| d.files())
                .and_then(|f| f.get(0))
            {
                read_into(file, &DROPPED);
            }
        });
        let _ = doc.add_event_listener_with_callback("dragover", over.as_ref().unchecked_ref());
        let _ = doc.add_event_listener_with_callback("drop", drop.as_ref().unchecked_ref());
        over.forget();
        drop.forget();
    }
}
