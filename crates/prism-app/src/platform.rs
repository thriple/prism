//! Questions for the operating system that the window library does not answer.

/// Whether the system asks apps to reduce motion, if we can tell.
pub fn system_reduces_motion() -> Option<bool> {
    imp::reduces_motion()
}

#[cfg(target_arch = "wasm32")]
mod imp {
    pub fn reduces_motion() -> Option<bool> {
        let list = web_sys::window()?
            .match_media("(prefers-reduced-motion: reduce)")
            .ok()??;
        Some(list.matches())
    }
}

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };

    pub fn reduces_motion() -> Option<bool> {
        let mut animate: i32 = 1;
        // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the pointer given.
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                (&mut animate as *mut i32).cast(),
                0,
            )
        };
        (ok != 0).then_some(animate == 0)
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_app_kit::NSWorkspace;

    pub fn reduces_motion() -> Option<bool> {
        Some(NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion())
    }
}

#[cfg(not(any(target_arch = "wasm32", windows, target_os = "macos")))]
mod imp {
    pub fn reduces_motion() -> Option<bool> {
        None
    }
}

/// Which platform's key names to show (Ctrl or Cmd).
pub fn keys() -> crate::actions::keymap::Platform {
    use crate::actions::keymap::Platform;
    #[cfg(target_arch = "wasm32")]
    {
        let mac = web_sys::window()
            .and_then(|w| w.navigator().user_agent().ok())
            .is_some_and(|ua| ua.contains("Mac"));
        Platform::Web { mac }
    }
    #[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
    {
        Platform::Mac
    }
    #[cfg(all(not(target_arch = "wasm32"), windows))]
    {
        Platform::Windows
    }
    #[cfg(all(not(target_arch = "wasm32"), not(windows), not(target_os = "macos")))]
    {
        Platform::Linux
    }
}
