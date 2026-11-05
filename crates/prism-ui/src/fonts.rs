//! IBM Plex Sans and Mono, bundled in the binary so the app works offline.
//! The files are IBM's originals, unmodified, under the SIL Open Font Licence
//! (`assets/fonts/OFL.txt`; DECISIONS #12).

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

use crate::theme::text;

const FONTS: [(&str, &[u8]); 5] = [
    (
        text::SANS,
        include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf"),
    ),
    (
        text::SANS_MEDIUM,
        include_bytes!("../assets/fonts/IBMPlexSans-Medium.ttf"),
    ),
    (
        text::SANS_SEMIBOLD,
        include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf"),
    ),
    (
        text::MONO_REGULAR,
        include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf"),
    ),
    (
        text::MONO_MEDIUM,
        include_bytes!("../assets/fonts/IBMPlexMono-Medium.ttf"),
    ),
];

/// Font set-up for egui: Plex Sans is the proportional font, Plex Mono the
/// monospace one, and each weight is also a named family.
pub fn definitions() -> FontDefinitions {
    let mut defs = FontDefinitions::empty();
    for (name, bytes) in FONTS {
        defs.font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
        // Each weight falls back to the regular face of its kind.
        let fallback = if name.starts_with("plex-mono") {
            text::MONO_REGULAR
        } else {
            text::SANS
        };
        let mut chain = vec![name.to_owned()];
        if name != fallback {
            chain.push(fallback.to_owned());
        }
        defs.families.insert(FontFamily::Name(name.into()), chain);
    }
    defs.families.insert(
        FontFamily::Proportional,
        vec![text::SANS.to_owned(), text::MONO_REGULAR.to_owned()],
    );
    defs.families.insert(
        FontFamily::Monospace,
        vec![text::MONO_REGULAR.to_owned(), text::SANS.to_owned()],
    );
    defs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_font_is_a_real_font_file() {
        for (name, bytes) in FONTS {
            // TrueType files start with 0x00010000.
            assert_eq!(&bytes[..4], &[0, 1, 0, 0], "{name} is not a TrueType file");
        }
    }

    #[test]
    fn families_point_at_loaded_fonts() {
        let defs = definitions();
        for chain in defs.families.values() {
            for font in chain {
                assert!(defs.font_data.contains_key(font), "{font} missing");
            }
        }
    }
}
