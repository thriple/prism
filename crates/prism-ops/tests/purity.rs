//! The core crates stay pure: no window, GPU, clock or randomness
//! (CLAUDE.md, "Pure, deterministic logic").

use std::fs;
use std::path::Path;

const CORE: [&str; 8] = [
    "prism-math",
    "prism-geom",
    "prism-topo",
    "prism-ops",
    "prism-doc",
    "prism-view",
    "prism-infer",
    "prism-tools",
];
const BANNED_DEPS: [&str; 6] = ["wgpu", "winit", "egui", "web-time", "rand", "getrandom"];
const BANNED_CODE: [&str; 5] = [
    "Instant::now",
    "SystemTime",
    "thread_rng",
    "rand::",
    "HashMap",
];

fn rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn core_crates_have_no_window_gpu_clock_or_randomness() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut problems = Vec::new();
    for name in CORE {
        let dir = crates.join(name);
        let manifest = fs::read_to_string(dir.join("Cargo.toml")).expect("manifest");
        let deps = manifest.split("[dev-dependencies]").next().unwrap_or("");
        for bad in BANNED_DEPS {
            if deps.lines().any(|l| {
                l.trim_start().starts_with(&format!("{bad} "))
                    || l.trim_start().starts_with(&format!("{bad}."))
                    || l.trim_start().starts_with(&format!("{bad}="))
            }) {
                problems.push(format!("{name} depends on {bad}"));
            }
        }
        let mut files = Vec::new();
        rs_files(&dir.join("src"), &mut files);
        for f in files {
            let text = fs::read_to_string(&f).expect("source");
            for bad in BANNED_CODE {
                if text.contains(bad) {
                    problems.push(format!("{} uses {bad}", f.display()));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}
