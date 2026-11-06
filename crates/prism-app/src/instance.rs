//! One running Prism (brief 11, DECISIONS #24): a file double-clicked while
//! Prism runs opens there, in a tab, instead of in a second copy.
//!
//! - Windows and Linux: a second start finds the first through a local
//!   socket (a named pipe on Windows), hands over its files and quits.
//! - macOS starts one copy anyway, and sends a double-clicked file to it as
//!   an Apple event (`application:openURLs:`), which winit does not pass
//!   on; Prism adds that method to winit's app delegate.
//!
//! Files arrive through `deliver`, which wakes the event loop.

use std::sync::Mutex;

/// Files handed over, waiting for the app to open them.
static ARRIVED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Wakes the event loop when files arrive.
static WAKE: Mutex<Option<Box<dyn Fn() + Send>>> = Mutex::new(None);

/// Set what wakes the event loop.
pub fn set_wake(f: impl Fn() + Send + 'static) {
    *WAKE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(f));
}

/// Files to open, from another start or from the system.
pub fn deliver(paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }
    ARRIVED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(paths);
    if let Some(w) = WAKE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        w();
    }
}

/// The files that arrived since last asked.
pub fn take() -> Vec<String> {
    std::mem::take(&mut *ARRIVED.lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos")))]
mod socket {
    use std::io::{BufRead, BufReader, Write};

    use interprocess::local_socket::{
        GenericNamespaced, ListenerOptions, Stream, ToNsName, prelude::*,
    };

    /// One name per user, so two people on one computer do not meet.
    pub fn name_for(user: &str) -> String {
        let safe: String = user
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        format!("prism-{safe}.sock")
    }

    fn name() -> String {
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_default();
        name_for(&user)
    }

    /// Hand `paths` to a Prism already running under `name`. True if one
    /// took them (this start should then quit).
    pub fn hand_over_to(name: &str, paths: &[String]) -> bool {
        let Ok(n) = name.to_ns_name::<GenericNamespaced>() else {
            return false;
        };
        let Ok(mut s) = Stream::connect(n) else {
            return false;
        };
        let mut msg = String::new();
        for p in paths {
            msg.push_str(p);
            msg.push('\n');
        }
        // An empty line: no files, just bring the window forward.
        msg.push('\n');
        s.write_all(msg.as_bytes()).is_ok()
    }

    pub fn hand_over(paths: &[String]) -> bool {
        hand_over_to(&name(), paths)
    }

    /// Listen under `name` for later starts. False if another Prism has it.
    pub fn listen_as(name: &str) -> bool {
        let Ok(n) = name.to_ns_name::<GenericNamespaced>() else {
            return false;
        };
        let Ok(listener) = ListenerOptions::new().name(n).create_sync() else {
            return false;
        };
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let mut paths = Vec::new();
                for line in BufReader::new(conn).lines() {
                    let Ok(line) = line else { break };
                    if line.is_empty() {
                        break;
                    }
                    paths.push(line);
                }
                // Even with no files, wake the app (it comes forward).
                if paths.is_empty() {
                    super::deliver(vec![String::new()]);
                } else {
                    super::deliver(paths);
                }
            }
        });
        true
    }

    pub fn listen() -> bool {
        listen_as(&name())
    }
}

/// At start: hand the command line's files to a running Prism and return
/// true (this start quits), or become the one others hand files to.
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos")))]
pub fn claim(files: &[String]) -> bool {
    if socket::hand_over(files) {
        return true;
    }
    if !socket::listen() {
        log::warn!("could not listen for other starts: files open in new windows");
    }
    false
}

/// macOS keeps one copy itself; files come as Apple events.
#[cfg(target_os = "macos")]
pub fn claim(_files: &[String]) -> bool {
    mac::install();
    false
}

#[cfg(target_arch = "wasm32")]
pub fn claim(_files: &[String]) -> bool {
    false
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::sel;
    use objc2_foundation::{NSArray, NSURL};

    /// `application:openURLs:` on winit's app delegate.
    unsafe extern "C-unwind" fn open_urls(
        _this: &AnyObject,
        _cmd: Sel,
        _app: &AnyObject,
        urls: &NSArray<NSURL>,
    ) {
        let mut paths = Vec::new();
        for url in urls.iter() {
            if let Some(p) = url.path() {
                paths.push(p.to_string());
            }
        }
        super::deliver(paths);
    }

    /// Add the method to winit's delegate class (winit 0.30 names it
    /// `WinitApplicationDelegate`). Call after the event loop is made.
    pub fn install() {
        let Some(cls) = AnyClass::get(c"WinitApplicationDelegate") else {
            log::warn!("no winit app delegate: double-clicked files will not open");
            return;
        };
        // SAFETY: the function matches the Objective-C signature
        // `- (void)application:(NSApplication *)app openURLs:(NSArray<NSURL *> *)urls`,
        // described by the type string "v@:@@".
        unsafe {
            let imp: Imp = std::mem::transmute::<
                unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &NSArray<NSURL>),
                Imp,
            >(open_urls);
            let added = objc2::ffi::class_addMethod(
                cls as *const AnyClass as *mut AnyClass,
                sel!(application:openURLs:),
                imp,
                c"v@:@@".as_ptr(),
            );
            if !added.as_bool() {
                log::warn!("winit's app delegate already handles openURLs");
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), not(target_os = "macos")))]
mod tests {
    use super::socket;

    #[test]
    fn a_second_start_hands_its_files_to_the_first() {
        let name = socket::name_for(&format!("test-{}", std::process::id()));
        assert!(
            !socket::hand_over_to(&name, &["/x/a.prism".into()]),
            "nobody is listening yet"
        );
        assert!(socket::listen_as(&name));
        assert!(socket::hand_over_to(
            &name,
            &["/x/a.prism".into(), "/x/b.prism".into()]
        ));
        // The listener runs on its own thread; wait for it.
        let mut got = Vec::new();
        for _ in 0..200 {
            got.extend(super::take());
            if got.len() >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(got, ["/x/a.prism", "/x/b.prism"]);
    }
}
