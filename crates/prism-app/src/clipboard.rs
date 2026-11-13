//! The system clipboard (closing gaps, group 3): a copied clip is also put
//! there as text (`prism_file::clip`), so another Prism window can paste
//! it. Text only. On macOS and Linux through `arboard`; on Windows through
//! the Win32 calls directly (arboard's Windows backend is BSL-1.0, which
//! is not on our licence list). Not on the web yet: the browser's
//! clipboard is asynchronous and asks for permission.

/// The clipboard's text, if it holds any.
pub fn get_text() -> Option<String> {
    imp::get()
}

/// Put `text` on the clipboard.
pub fn set_text(text: &str) {
    imp::set(text);
}

#[cfg(all(not(target_arch = "wasm32"), not(windows)))]
mod imp {
    use std::sync::{Mutex, OnceLock};

    /// One clipboard for the app's life: on Linux the text set is served
    /// from it, and goes when it is dropped.
    fn clipboard() -> &'static Mutex<Option<arboard::Clipboard>> {
        static C: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();
        C.get_or_init(|| Mutex::new(arboard::Clipboard::new().ok()))
    }

    pub fn get() -> Option<String> {
        let mut c = clipboard().lock().ok()?;
        c.as_mut()?.get_text().ok()
    }

    pub fn set(text: &str) {
        let Ok(mut c) = clipboard().lock() else {
            return;
        };
        if let Some(c) = c.as_mut()
            && let Err(e) = c.set_text(text)
        {
            log::warn!("could not set the clipboard: {e}");
        }
    }
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable,
        OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
    };

    /// `CF_UNICODETEXT`: UTF-16, ending in a zero.
    const UNICODE_TEXT: u32 = 13;

    pub fn get() -> Option<String> {
        // SAFETY: the clipboard is opened and closed around the read; the
        // handle's memory is locked while it is read and holds UTF-16
        // ending in a zero, as `CF_UNICODETEXT` promises.
        unsafe {
            if IsClipboardFormatAvailable(UNICODE_TEXT) == 0 {
                return None;
            }
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return None;
            }
            let mut out = None;
            let h = GetClipboardData(UNICODE_TEXT);
            if !h.is_null() {
                let p = GlobalLock(h) as *const u16;
                if !p.is_null() {
                    let mut n = 0;
                    while *p.add(n) != 0 {
                        n += 1;
                    }
                    out = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, n)));
                    GlobalUnlock(h);
                }
            }
            CloseClipboard();
            out
        }
    }

    pub fn set(text: &str) {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: the memory is allocated for the whole text, filled while
        // locked, and handed to the clipboard, which owns it from then on.
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return;
            }
            EmptyClipboard();
            let h = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
            if !h.is_null() {
                let p = GlobalLock(h) as *mut u16;
                if !p.is_null() {
                    std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                    GlobalUnlock(h);
                    SetClipboardData(UNICODE_TEXT, h);
                }
            }
            CloseClipboard();
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    pub fn get() -> Option<String> {
        None
    }

    pub fn set(_text: &str) {}
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    /// Needs a desktop (a display on Linux): run by hand with
    /// `cargo test -p prism-app clipboard -- --ignored`.
    #[test]
    #[ignore]
    fn text_goes_onto_the_clipboard_and_back() {
        let text = format!("{}\nabc123", prism_file::clip::HEADER);
        super::set_text(&text);
        assert_eq!(super::get_text().as_deref(), Some(text.as_str()));
    }
}
