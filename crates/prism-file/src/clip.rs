//! A copied clip as text, for the system clipboard (closing gaps, group
//! 3): a header line, then the clip written as a `.prism` file, in hex.
//! Another Prism (or another window) reads it back; any other text is
//! not a clip.

use prism_doc::Assembly;

use crate::{Contents, Settings, read, write};

/// The first line of a clip's text.
pub const HEADER: &str = "Prism clip 1";

/// The clip as clipboard text.
pub fn to_text(clip: &Assembly, app: &str) -> String {
    let bytes = write(
        &Contents {
            assembly: clip.clone(),
            settings: Settings::default(),
        },
        app,
    );
    let mut out = String::with_capacity(HEADER.len() + 1 + bytes.len() * 2);
    out.push_str(HEADER);
    out.push('\n');
    for b in bytes {
        out.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(b & 15), 16).unwrap_or('0'));
    }
    out
}

/// The clip in clipboard text, if the text is one.
pub fn from_text(text: &str) -> Option<Assembly> {
    let body = text.strip_prefix(HEADER)?.trim_start_matches(['\r', '\n']);
    let body = body.trim_end();
    if body.len() % 2 != 0 {
        return None;
    }
    let digits: Vec<u8> = body
        .chars()
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<_>>()?;
    let bytes: Vec<u8> = digits.chunks(2).map(|p| p[0] << 4 | p[1]).collect();
    read(&bytes).ok().map(|c| c.assembly)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_math::DVec3;
    use prism_topo::Model;

    #[test]
    fn a_clip_goes_through_text_and_back() {
        let mut m = Model::new();
        prism_ops::add_rect(&mut m, 0.0, 0.0, 2.0, 1.0, 0.0);
        prism_ops::add_segment(&mut m, DVec3::ZERO, DVec3::new(0.0, 0.0, 3.0));
        let clip = Assembly::with_model(m);
        let text = to_text(&clip, "test");
        assert!(text.starts_with(HEADER));
        let back = from_text(&text).expect("reads back");
        assert_eq!(back.root().model, clip.root().model);
        // Other text, or a damaged clip, is not a clip.
        assert!(from_text("hello").is_none());
        assert!(from_text(&text[..text.len() - 3]).is_none());
        assert!(from_text(&format!("{HEADER}\nzz")).is_none());
    }
}
