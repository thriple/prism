//! Help > Check for Updates (brief 20): the newest release build is
//! downloaded from GitHub Releases, checked against the SHA-256 published
//! with it, and put in place of the running build, which keeps running.
//! The new build runs from the next start.

use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Where the releases are: `update.json` and the zips sit under the fixed
/// "latest" release.
pub const RELEASES: &str = concat!(env!("CARGO_PKG_REPOSITORY"), "/releases/download/latest");

/// What the release workflow publishes beside the zips.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Manifest {
    pub build: u32,
    #[serde(default)]
    pub commit: String,
    /// Each zip's name and SHA-256, in hex.
    pub files: BTreeMap<String, String>,
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest, String> {
        serde_json::from_str(text).map_err(|e| format!("The update list could not be read ({e})."))
    }
}

/// The number in "build 12" (the release workflow's `PRISM_BUILD`); none
/// for builds made by hand ("local").
pub fn build_number(build: &str) -> Option<u32> {
    build.strip_prefix("build ")?.trim().parse().ok()
}

/// This build's number.
pub fn this_build() -> Option<u32> {
    build_number(crate::overlay::BUILD)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether `bytes` have the published checksum.
pub fn checks_out(bytes: &[u8], expected: &str) -> bool {
    sha256_hex(bytes).eq_ignore_ascii_case(expected.trim())
}

/// The zip this system downloads.
pub fn asset() -> Option<&'static str> {
    if cfg!(windows) {
        Some("Prism-windows.zip")
    } else if cfg!(target_os = "macos") {
        Some("Prism-macos.zip")
    } else {
        None
    }
}

/// Unpack a zip into `into`, keeping folders and (on Unix) file modes.
/// Entries that would land outside `into` are refused.
pub fn unpack(zip: &[u8], into: &Path) -> io::Result<()> {
    let mut archive = zip::ZipArchive::new(io::Cursor::new(zip)).map_err(io::Error::other)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(io::Error::other)?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(io::Error::other(format!(
                "the update has a file outside its folder: {}",
                entry.name().unwrap_or_default()
            )));
        };
        let path = into.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&path)?;
        io::copy(&mut entry, &mut out)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode & 0o7777))?;
        }
    }
    Ok(())
}

fn remove(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// Rename, trying again for a moment if the file is busy (on Windows a
/// virus scanner often holds a new file briefly).
fn rename(from: &Path, to: &Path) -> io::Result<()> {
    let mut tries = 0;
    loop {
        match std::fs::rename(from, to) {
            Err(e) if tries < 5 && from.exists() && e.kind() != io::ErrorKind::NotFound => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            r => return r,
        }
    }
}

/// Put `new` where `current` is, moving `current` to `aside` first (a
/// running program can be renamed, even on Windows). If `new` cannot be
/// moved in, `current` is put back.
pub fn swap(current: &Path, new: &Path, aside: &Path) -> io::Result<()> {
    if aside.exists() {
        remove(aside)?;
    }
    rename(current, aside)?;
    if let Err(e) = rename(new, current) {
        let _ = rename(aside, current);
        return Err(e);
    }
    Ok(())
}

/// What an update replaces.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// Windows: the program file.
    Exe(PathBuf),
    /// macOS: the app bundle.
    Bundle(PathBuf),
}

impl Target {
    /// For the program at `exe`: its bundle on macOS, else the file.
    pub fn for_exe(exe: &Path, macos: bool) -> Result<Target, String> {
        if !macos {
            return Ok(Target::Exe(exe.to_path_buf()));
        }
        let text = exe.to_string_lossy();
        if text.contains("/AppTranslocation/") {
            return Err(
                "macOS is running Prism from a temporary copy. Move Prism.app out of \
                        Downloads (to Applications, for example), open it from there, and try \
                        again."
                    .to_owned(),
            );
        }
        // Prism.app/Contents/MacOS/Prism
        exe.ancestors()
            .nth(3)
            .filter(|b| b.extension().is_some_and(|e| e == "app"))
            .map(|b| Target::Bundle(b.to_path_buf()))
            .ok_or_else(|| "Prism is not running from an app bundle.".to_owned())
    }

    /// The running Prism's.
    pub fn this_app() -> Result<Target, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        Target::for_exe(&exe, cfg!(target_os = "macos"))
    }

    /// The file and folder names the swap uses beside the target.
    fn aside(&self) -> PathBuf {
        match self {
            Target::Exe(exe) => exe.with_extension("old.exe"),
            Target::Bundle(app) => staging(app).join("old.app"),
        }
    }

    /// Put the zip's build in place. On failure the old build stays.
    pub fn install(&self, zip: &[u8]) -> Result<(), String> {
        let why = |e: io::Error, path: &Path| {
            let folder = path.parent().unwrap_or(path).display();
            if e.kind() == io::ErrorKind::PermissionDenied {
                format!(
                    "Prism may not write to {folder}. Move Prism to a folder you own, or \
                     download the new build from the Releases page."
                )
            } else {
                format!("The update could not be installed in {folder} ({e}).")
            }
        };
        match self {
            Target::Exe(exe) => {
                // The one program file in the zip (its folder name and
                // separators vary with the tool that made it).
                let mut archive = zip::ZipArchive::new(io::Cursor::new(zip))
                    .map_err(|e| format!("The download is not a zip ({e})."))?;
                let mut bytes = Vec::new();
                let mut found = false;
                for i in 0..archive.len() {
                    let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
                    let name = entry.name().map(|n| n.to_ascii_lowercase());
                    if name.is_ok_and(|n| n.ends_with(".exe")) {
                        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err("The download has no program in it.".to_owned());
                }
                let new = exe.with_extension("new.exe");
                std::fs::write(&new, &bytes).map_err(|e| why(e, exe))?;
                swap(exe, &new, &self.aside()).map_err(|e| {
                    let _ = std::fs::remove_file(&new);
                    why(e, exe)
                })
            }
            Target::Bundle(app) => {
                let dir = staging(app);
                let _ = remove(&dir);
                std::fs::create_dir_all(&dir).map_err(|e| why(e, app))?;
                let result = (|| {
                    unpack(zip, &dir)?;
                    let name = app.file_name().unwrap_or_default();
                    let mut new = dir.join(name);
                    if !new.exists() {
                        new = dir.join("Prism.app");
                    }
                    swap(app, &new, &self.aside())
                })();
                let _ = remove(&dir);
                result.map_err(|e| why(e, app))
            }
        }
    }

    /// Delete what an earlier update left beside the target.
    pub fn clean_up(&self) {
        let left = match self {
            Target::Exe(_) => self.aside(),
            Target::Bundle(app) => staging(app),
        };
        if left.exists() {
            let _ = remove(&left);
        }
    }
}

/// The folder a bundle's update is unpacked in, beside it.
fn staging(app: &Path) -> PathBuf {
    app.with_file_name(".prism-update")
}

/// Remove an earlier update's leftovers (at start).
pub fn clean_up() {
    if let Ok(t) = Target::this_app() {
        t.clean_up();
    }
}

/// Where the check or the download is.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Idle,
    Checking,
    /// A build made by hand: nothing to compare with.
    Local,
    /// The web version updates on every push.
    Web,
    /// No build is published for this system (Linux, for now).
    Unsupported,
    UpToDate(u32),
    Available {
        latest: u32,
        sha: String,
    },
    Downloading {
        latest: u32,
        done: u64,
        total: Option<u64>,
    },
    Installed(u32),
    Failed(String),
}

impl Status {
    /// Whether work is going on (the window redraws meanwhile).
    pub fn busy(&self) -> bool {
        matches!(self, Status::Checking | Status::Downloading { .. })
    }
}

/// Where to look, and what to replace.
#[derive(Clone, Debug)]
pub struct Config {
    /// The folder holding `update.json` and the zips.
    pub base: String,
    pub current: Option<u32>,
    pub asset: Option<&'static str>,
    pub target: Result<Target, String>,
}

impl Config {
    pub fn this_app() -> Config {
        Config {
            base: RELEASES.to_owned(),
            current: this_build(),
            asset: asset(),
            target: Target::this_app(),
        }
    }
}

/// Checks and downloads on a thread; the UI reads [`Updater::status`].
#[derive(Clone)]
pub struct Updater {
    config: Config,
    status: Arc<Mutex<Status>>,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new(Config::this_app())
    }
}

impl Updater {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            status: Arc::new(Mutex::new(Status::Idle)),
        }
    }

    pub fn status(&self) -> Status {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or(Status::Idle)
    }

    fn set(&self, s: Status) {
        if let Ok(mut now) = self.status.lock() {
            *now = s;
        }
    }

    /// Look for a newer build.
    pub fn check(&self) {
        if self.status().busy() {
            return;
        }
        if cfg!(target_arch = "wasm32") {
            return self.set(Status::Web);
        }
        let Some(current) = self.config.current else {
            return self.set(Status::Local);
        };
        let Some(asset) = self.config.asset else {
            return self.set(Status::Unsupported);
        };
        self.set(Status::Checking);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let me = self.clone();
            std::thread::spawn(move || {
                let url = format!("{}/update.json", me.config.base);
                let result = net::get(&url, |_, _| {})
                    .and_then(|b| Manifest::parse(&String::from_utf8_lossy(&b)));
                me.set(match result {
                    Err(e) => Status::Failed(e),
                    Ok(m) if m.build <= current => Status::UpToDate(current),
                    Ok(m) => match m.files.get(asset) {
                        Some(sha) => Status::Available {
                            latest: m.build,
                            sha: sha.clone(),
                        },
                        None => Status::Failed(format!("Build {} has no {asset}.", m.build)),
                    },
                });
            });
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (current, asset);
    }

    /// Download the build found by [`Updater::check`], check it and put
    /// it in place.
    pub fn install(&self) {
        let Status::Available { latest, sha } = self.status() else {
            return;
        };
        let (Some(asset), Ok(target)) = (self.config.asset, self.config.target.clone()) else {
            if let Err(e) = &self.config.target {
                self.set(Status::Failed(e.clone()));
            }
            return;
        };
        self.set(Status::Downloading {
            latest,
            done: 0,
            total: None,
        });
        #[cfg(not(target_arch = "wasm32"))]
        {
            let me = self.clone();
            std::thread::spawn(move || {
                let url = format!("{}/{asset}", me.config.base);
                let progress = |done, total| {
                    me.set(Status::Downloading {
                        latest,
                        done,
                        total,
                    })
                };
                let result = net::get(&url, progress).and_then(|zip| {
                    if !checks_out(&zip, &sha) {
                        return Err("The download does not match its published checksum, so \
                                    nothing was changed. Try again later."
                            .to_owned());
                    }
                    target.install(&zip)
                });
                me.set(match result {
                    Ok(()) => Status::Installed(latest),
                    Err(e) => Status::Failed(e),
                });
            });
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (asset, target, sha);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod net {
    use std::io::Read;
    use std::time::Duration;

    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

    /// The largest download accepted.
    const MAX_DOWNLOAD: u64 = 1 << 30;

    /// GET `url` with the system's TLS and certificates, following
    /// redirects, calling `progress(done, total)` as it arrives.
    pub fn get(url: &str, mut progress: impl FnMut(u64, Option<u64>)) -> Result<Vec<u8>, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .tls_config(
                TlsConfig::builder()
                    .provider(TlsProvider::NativeTls)
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .timeout_connect(Some(Duration::from_secs(20)))
            .user_agent("Prism-updater")
            .build()
            .into();
        let mut response = agent
            .get(url)
            .call()
            .map_err(|e| format!("Could not reach the releases ({e})."))?;
        let total = response.body().content_length();
        let mut reader = response
            .body_mut()
            .with_config()
            .limit(MAX_DOWNLOAD)
            .reader();
        let mut out = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("The download stopped ({e})."))?;
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
            progress(out.len() as u64, total);
        }
        Ok(out)
    }
}

// Native only: they use threads, a local server and files.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("prism-update-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("scratch dir");
        d
    }

    /// A zip of (name, contents, unix mode); a name ending in / is a folder.
    fn zip_of(entries: &[(&str, &str, u32)]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (name, text, mode) in entries {
            let o = zip::write::SimpleFileOptions::default().unix_permissions(*mode);
            if name.ends_with('/') {
                w.add_directory(*name, o).expect("dir");
            } else {
                w.start_file(*name, o).expect("file");
                w.write_all(text.as_bytes()).expect("write");
            }
        }
        w.finish().expect("zip").into_inner()
    }

    #[test]
    fn the_manifest_and_build_numbers_read() {
        let m = Manifest::parse(
            r#"{"build": 180, "commit": "abc", "files": {"Prism-windows.zip": "00ff"}}"#,
        )
        .expect("parses");
        assert_eq!(m.build, 180);
        assert_eq!(m.files["Prism-windows.zip"], "00ff");
        assert!(Manifest::parse("nope").is_err());
        assert_eq!(build_number("build 12"), Some(12));
        assert_eq!(build_number("local"), None);
        assert_eq!(build_number("build x"), None);
    }

    #[test]
    fn checksums_accept_the_right_bytes_only() {
        // SHA-256 of "abc".
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(sha256_hex(b"abc"), abc);
        assert!(checks_out(b"abc", &abc.to_uppercase()));
        assert!(!checks_out(b"abd", abc));
    }

    #[test]
    fn unpacking_keeps_folders_and_modes_and_stays_inside() {
        let d = scratch("unpack");
        let z = zip_of(&[
            ("Prism.app/", "", 0o755),
            ("Prism.app/Contents/MacOS/Prism", "program", 0o755),
            ("Prism.app/Contents/Info.plist", "plist", 0o644),
        ]);
        unpack(&z, &d).expect("unpacks");
        let exe = d.join("Prism.app/Contents/MacOS/Prism");
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&exe).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "still a program");
        }
        let bad = zip_of(&[("../outside.txt", "x", 0o644)]);
        assert!(unpack(&bad, &d.join("inner")).is_err());
        assert!(!d.join("outside.txt").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_swap_keeps_the_old_one_aside_or_puts_it_back() {
        let d = scratch("swap");
        let (cur, new, aside) = (d.join("Prism.exe"), d.join("new"), d.join("Prism.old.exe"));
        std::fs::write(&cur, "old").unwrap();
        std::fs::write(&new, "new").unwrap();
        std::fs::write(&aside, "older still").unwrap();
        swap(&cur, &new, &aside).expect("swaps");
        assert_eq!(std::fs::read_to_string(&cur).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), "old");
        // Nothing to move in: the current one is put back.
        assert!(swap(&cur, &d.join("missing"), &aside).is_err());
        assert_eq!(std::fs::read_to_string(&cur).unwrap(), "new");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_target_is_the_program_or_its_bundle() {
        let exe = Path::new("/Apps/Prism.app/Contents/MacOS/Prism");
        assert_eq!(
            Target::for_exe(exe, true),
            Ok(Target::Bundle("/Apps/Prism.app".into()))
        );
        assert_eq!(
            Target::for_exe(exe, false),
            Ok(Target::Exe(exe.to_path_buf()))
        );
        assert!(Target::for_exe(Path::new("/usr/bin/prism"), true).is_err());
        let moved =
            Path::new("/private/var/folders/x/AppTranslocation/y/d/Prism.app/Contents/MacOS/Prism");
        assert!(
            Target::for_exe(moved, true)
                .unwrap_err()
                .contains("Move Prism.app")
        );
        assert_eq!(
            Target::Exe("C:/P/Prism.exe".into()).aside(),
            PathBuf::from("C:/P/Prism.old.exe")
        );
    }

    #[test]
    fn installing_replaces_the_program_and_cleans_up_later() {
        let d = scratch("install-exe");
        let exe = d.join("Prism.exe");
        std::fs::write(&exe, "build 1").unwrap();
        let t = Target::Exe(exe.clone());
        // Windows' zip tool may write backslashes.
        t.install(&zip_of(&[("Prism\\Prism.exe", "build 2", 0o755)]))
            .expect("installs");
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "build 2");
        assert!(d.join("Prism.old.exe").exists());
        t.clean_up();
        assert!(!d.join("Prism.old.exe").exists());
        // A zip with no program changes nothing.
        assert!(t.install(&zip_of(&[("readme.txt", "x", 0o644)])).is_err());
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "build 2");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn installing_replaces_the_bundle() {
        let d = scratch("install-app");
        let app = d.join("Prism.app");
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::write(app.join("Contents/MacOS/Prism"), "build 1").unwrap();
        let t = Target::Bundle(app.clone());
        t.install(&zip_of(&[
            ("Prism.app/Contents/MacOS/Prism", "build 2", 0o755),
            ("Prism.app/Contents/Info.plist", "plist", 0o644),
        ]))
        .expect("installs");
        assert_eq!(
            std::fs::read_to_string(app.join("Contents/MacOS/Prism")).unwrap(),
            "build 2"
        );
        assert!(app.join("Contents/Info.plist").exists());
        assert!(!d.join(".prism-update").exists(), "leftovers removed");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Serve `files` over plain HTTP on a local port, for one request each.
    fn serve(files: Vec<(&'static str, Vec<u8>)>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming().take(files.len()) {
                let Ok(mut s) = stream else { continue };
                let mut req = [0u8; 2048];
                let n = s.read(&mut req).unwrap_or(0);
                let line = String::from_utf8_lossy(&req[..n]);
                let path = line.split_whitespace().nth(1).unwrap_or("");
                let body = files
                    .iter()
                    .find(|(name, _)| path.ends_with(name))
                    .map(|(_, b)| b.clone());
                let _ = match body {
                    Some(b) => s
                        .write_all(
                            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b.len())
                                .as_bytes(),
                        )
                        .and_then(|_| s.write_all(&b)),
                    None => s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                };
            }
        });
        base
    }

    fn wait(u: &Updater) -> Status {
        for _ in 0..500 {
            let s = u.status();
            if !s.busy() {
                return s;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("still busy: {:?}", u.status());
    }

    #[test]
    fn a_newer_build_is_found_downloaded_checked_and_installed() {
        let d = scratch("flow");
        let exe = d.join("Prism.exe");
        std::fs::write(&exe, "build 7").unwrap();
        let zip = zip_of(&[("Prism/Prism.exe", "build 9", 0o755)]);
        let manifest = format!(
            r#"{{"build": 9, "files": {{"Prism-windows.zip": "{}"}}}}"#,
            sha256_hex(&zip)
        );
        let base = serve(vec![
            ("update.json", manifest.into_bytes()),
            ("Prism-windows.zip", zip),
        ]);
        let u = Updater::new(Config {
            base,
            current: Some(7),
            asset: Some("Prism-windows.zip"),
            target: Ok(Target::Exe(exe.clone())),
        });
        u.check();
        assert!(matches!(wait(&u), Status::Available { latest: 9, .. }));
        u.install();
        assert_eq!(wait(&u), Status::Installed(9));
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "build 9");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_same_build_is_not_offered_and_a_bad_download_changes_nothing() {
        let d = scratch("flow-bad");
        let exe = d.join("Prism.exe");
        std::fs::write(&exe, "build 7").unwrap();
        let zip = zip_of(&[("Prism/Prism.exe", "build 9", 0o755)]);
        let same = r#"{"build": 7, "files": {"Prism-windows.zip": "00"}}"#;
        let wrong = r#"{"build": 9, "files": {"Prism-windows.zip": "00"}}"#;
        let base = serve(vec![("update.json", same.into())]);
        let config = |base| Config {
            base,
            current: Some(7),
            asset: Some("Prism-windows.zip"),
            target: Ok(Target::Exe(exe.clone())),
        };
        let u = Updater::new(config(base));
        u.check();
        assert_eq!(wait(&u), Status::UpToDate(7));
        let base = serve(vec![
            ("update.json", wrong.into()),
            ("Prism-windows.zip", zip),
        ]);
        let u = Updater::new(config(base));
        u.check();
        assert!(matches!(wait(&u), Status::Available { .. }));
        u.install();
        assert!(matches!(wait(&u), Status::Failed(e) if e.contains("checksum")));
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "build 7");
        // A build made by hand is not compared.
        let u = Updater::new(Config {
            current: None,
            ..config(String::new())
        });
        u.check();
        assert_eq!(u.status(), Status::Local);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Reaches GitHub over HTTPS with the system's TLS (needs the network,
    /// so run by hand: `cargo test -p prism-app -- --ignored https`).
    #[test]
    #[ignore]
    fn https_reaches_github() {
        let page = net::get("https://github.com/thriple/prism", |_, _| {}).expect("reaches");
        assert!(!page.is_empty());
    }
}
