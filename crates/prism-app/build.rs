//! Windows: put the program's icon in the .exe (File Explorer, shortcuts,
//! the taskbar before the window opens). Elsewhere this does nothing. If
//! the resource compiler is missing the build still works, without it.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/windows/Prism.ico");
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("../../packaging/windows/Prism.ico");
            if let Err(e) = res.compile() {
                println!("cargo:warning=could not put the icon in the .exe: {e}");
            }
        }
    }
}
