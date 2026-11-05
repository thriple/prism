//! Colours, shadows, corner radii and line widths are defined only in
//! `prism-ui/src/theme.rs`. This test fails if a literal one appears in any
//! other source file of the workspace (test code excepted).

use std::fs;
use std::path::{Path, PathBuf};

/// Patterns that start a literal style value, when followed by a number.
const NUMERIC: [&str; 8] = [
    "Color32::from_rgb(",
    "Color32::from_rgba_premultiplied(",
    "Color32::from_rgba_unmultiplied(",
    "Color32::from_gray(",
    "Color32::from_black_alpha(",
    "Color32::from_white_alpha(",
    "CornerRadius::same(",
    "Stroke::new(",
];

/// Patterns that are literal style values on their own.
const ALWAYS: [&str; 4] = ["Shadow {", "Rgba::from", "Color32::WHITE", "Color32::BLACK"];

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The problems in one file's text (only the part before its test module).
fn problems(text: &str) -> Vec<String> {
    let code = text.split("#[cfg(test)]").next().unwrap_or("");
    let mut found = Vec::new();
    for (n, line) in code.lines().enumerate() {
        let line = line.split("//").next().unwrap_or("");
        for pat in NUMERIC {
            let mut rest = line;
            while let Some(i) = rest.find(pat) {
                let after = rest[i + pat.len()..].trim_start();
                if after.starts_with(|c: char| c.is_ascii_digit() || c == '.' || c == '-') {
                    found.push(format!("line {}: {}", n + 1, line.trim()));
                }
                rest = &rest[i + pat.len()..];
            }
        }
        for pat in ALWAYS {
            if line.contains(pat) {
                found.push(format!("line {}: {}", n + 1, line.trim()));
            }
        }
    }
    found
}

#[test]
fn style_values_live_only_in_the_theme() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let theme = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/theme.rs");
    let mut files = Vec::new();
    for krate in fs::read_dir(&crates).expect("read crates").flatten() {
        let src = krate.path().join("src");
        if src.is_dir() {
            sources(&src, &mut files);
        }
    }
    assert!(files.len() > 10, "found too few sources: {}", files.len());
    let mut report = Vec::new();
    for file in files {
        if file.canonicalize().ok() == theme.canonicalize().ok() {
            continue;
        }
        for p in problems(&fs::read_to_string(&file).expect("read")) {
            report.push(format!("{}: {p}", file.display()));
        }
    }
    assert!(
        report.is_empty(),
        "style literals outside theme.rs:\n{}",
        report.join("\n")
    );
}

#[test]
fn the_check_catches_literals() {
    assert_eq!(problems("let c = Color32::from_rgb(1, 2, 3);").len(), 1);
    assert_eq!(problems("let s = Stroke::new(1.5, c);").len(), 1);
    assert_eq!(problems("let s = Stroke::new(size::W, c);").len(), 0);
    assert_eq!(
        problems("let c = Color32::from_rgb(c[0], c[1], c[2]);").len(),
        0
    );
    assert_eq!(problems("let x = Color32::WHITE;").len(), 1);
    assert_eq!(problems("#[cfg(test)]\nlet c = Color32::WHITE;").len(), 0);
}
