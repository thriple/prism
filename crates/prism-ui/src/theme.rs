//! The one place where Prism's look is defined: colours, fonts, sizes, radii
//! and shadow depths, from the Motion Kit's dark Soft glass set
//! (`docs/UI-STYLE.md`). No colour, shadow or radius value may appear anywhere
//! else in the code; a test checks this.

use egui::{Color32, FontFamily, FontId, TextStyle};

/// Colour tokens (kit: `styles/soft.css`, dark theme).
pub mod color {
    use egui::Color32;

    /// The one material everything is made of.
    pub const SURFACE: Color32 = Color32::from_rgb(0x2a, 0x2c, 0x33);
    /// Main text and icons.
    pub const INK: Color32 = Color32::from_rgb(0xe6, 0xe8, 0xee);
    /// Secondary text, hotkey labels, values.
    pub const MUTED: Color32 = Color32::from_rgb(0x8b, 0x90, 0x9c);
    /// The dark half of every emboss: black at 55%.
    pub const SHADOW_DARK: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 140);
    /// The light half of every emboss: white at 6.5%.
    pub const SHADOW_LIGHT: Color32 = Color32::from_rgba_premultiplied(17, 17, 17, 17);
    /// Slightly darker wells, and the milky layer over the aurora.
    pub const FROST: Color32 = Color32::from_rgb(0x26, 0x28, 0x30);
    /// Focus ring, active dots, selection.
    pub const ACCENT: Color32 = Color32::from_rgb(0x02, 0xce, 0xff);
    /// The 1 px edge of floating things (white at 6%).
    pub const FLOAT_EDGE: Color32 = Color32::from_rgba_premultiplied(15, 15, 15, 15);
    /// Behind the 3D viewport until the renderer draws there.
    pub const VIEWPORT: Color32 = FROST;
    /// Fully clear.
    pub const CLEAR: Color32 = Color32::TRANSPARENT;
    /// Full white, used only to show a texture unchanged.
    pub const UNTINTED: Color32 = Color32::WHITE;

    /// Aurora base and its five blobs.
    pub const AURORA_BASE: Color32 = Color32::from_rgb(0x0d, 0x10, 0x30);
    pub const AURORA: [Color32; 5] = [
        Color32::from_rgb(0x3f, 0x3c, 0xc0),
        Color32::from_rgb(0xe0, 0x45, 0x7a),
        Color32::from_rgb(0x0a, 0xa3, 0xd6),
        Color32::from_rgb(0xa1, 0x77, 0xff),
        Color32::from_rgb(0x2f, 0xd4, 0xa0),
    ];

    /// Strength of the film grain over glass (premultiplied alpha, 0 to 1).
    pub const GRAIN_STRENGTH: f32 = 0.05;
    /// Frost opacity of a toggle track when off and on.
    pub const FROST_TOGGLE_OFF: f32 = 0.72;
    pub const FROST_TOGGLE_ON: f32 = 0.15;
    /// Frost opacity of a slider rail: filled side and empty side.
    pub const FROST_RAIL_FILLED: f32 = 0.12;
    pub const FROST_RAIL_EMPTY: f32 = 0.75;
    /// Frost opacity of a dropdown list.
    pub const FROST_LIST: f32 = 0.42;
    /// How dim a greyed-out control is.
    pub const DISABLED_OPACITY: f32 = 0.4;
    /// Greyed dock buttons and menu rows (prototype: .36).
    pub const GREYED_OPACITY: f32 = 0.36;
    /// Menu separators (white at 7%).
    pub const SEPARATOR: Color32 = Color32::from_rgba_premultiplied(18, 18, 18, 18);
    /// Behind dialogs: the rest of the app is dimmed (black at 35%).
    pub const SCRIM: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 89);
    /// Menu bar titles brighten on hover.
    pub const INK_BRIGHT: Color32 = Color32::WHITE;
}

/// Corner radii in points.
pub mod radius {
    pub const BUTTON: f32 = 16.0;
    pub const MENU_BUTTON: f32 = 11.0;
    pub const MENU: f32 = 14.0;
    pub const CARD: f32 = 24.0;
    pub const LIST: f32 = 22.0;
    pub const LIST_WELL: f32 = 16.0;
    pub const ITEM: f32 = 12.0;
    pub const TOOLTIP: f32 = 10.0;
    /// Use for pills and circles: larger than any control is tall.
    pub const ROUND: f32 = 1000.0;
    /// Tool dock buttons, menu rows, the top bar and dialogs (prototype CSS).
    pub const DOCK_BUTTON: f32 = 14.0;
    pub const MENU_ROW: f32 = 9.0;
    pub const KEYCAP: f32 = 6.0;
    pub const BAR: f32 = 4.0;
}

/// Text sizes in points, and the font families.
pub mod text {
    /// Labels, menus and buttons.
    pub const BODY: f32 = 13.0;
    /// Tooltips.
    pub const SMALL: f32 = 12.0;
    /// Panel and dialog titles: the largest text in the app (no display type).
    pub const TITLE: f32 = 15.0;
    /// Numbers, values and the measurement box.
    pub const MONO: f32 = 12.0;
    /// Hotkey hints on dock buttons.
    pub const HINT: f32 = 9.0;
    /// Small uppercase group labels (dock groups, menu headings).
    pub const LABEL: f32 = 9.5;
    /// Keys shown at the right of menu rows.
    pub const MENU_KEY: f32 = 11.0;
    /// Dialog titles (Planned Features, Keyboard Shortcuts).
    pub const DIALOG_TITLE: f32 = 16.0;
    /// Extra spacing between letters of uppercase labels.
    pub const LABEL_SPACING: f32 = 1.0;

    pub const SANS: &str = "plex-sans";
    pub const SANS_MEDIUM: &str = "plex-sans-medium";
    pub const SANS_SEMIBOLD: &str = "plex-sans-semibold";
    pub const MONO_REGULAR: &str = "plex-mono";
    pub const MONO_MEDIUM: &str = "plex-mono-medium";
}

/// Sizes of controls in points (kit components).
pub mod size {
    pub const BUTTON_HEIGHT: f32 = 44.0;
    pub const BUTTON_PAD_X: f32 = 26.0;
    pub const TOGGLE: [f32; 2] = [64.0, 34.0];
    pub const TOGGLE_PAD: f32 = 4.0;
    pub const TOGGLE_DOT: f32 = 6.0;
    pub const SLIDER_WIDTH: f32 = 256.0;
    pub const SLIDER_RAIL: f32 = 14.0;
    pub const SLIDER_HANDLE: f32 = 30.0;
    pub const SLIDER_DIMPLE: f32 = 10.0;
    pub const SELECT_WIDTH: f32 = 240.0;
    pub const LIST_GAP: f32 = 14.0;
    pub const LIST_PAD: f32 = 6.0;
    pub const ITEM_HEIGHT: f32 = 40.0;
    pub const ITEM_PAD_X: f32 = 16.0;
    pub const ITEM_GAP: f32 = 4.0;
    pub const CHEVRON: f32 = 8.0;
    pub const CHEVRON_STROKE: f32 = 1.8;
    pub const CHECK_STROKE: f32 = 2.0;
    pub const GROOVE_HEIGHT: f32 = 4.0;
    pub const FOCUS_WIDTH: f32 = 2.0;
    /// The faint border of floating cards.
    pub const EDGE_WIDTH: f32 = 1.0;
    pub const FOCUS_GAP: f32 = 4.0;
    pub const PANEL_PAD: f32 = 18.0;
    pub const PANEL_WIDTH: f32 = 300.0;
    pub const PANEL_TITLE_GAP: f32 = 12.0;
    pub const VALUE_TIP_GAP: f32 = 12.0;
    pub const VALUE_TIP_PAD: [f32; 2] = [8.0, 4.0];
    pub const ROW_GAP: f32 = 10.0;
    /// One grain texel per point.
    pub const GRAIN_TILE: f32 = 64.0;

    // The app's frame, from the prototype's CSS.
    /// Space around and between the top bar, dock, viewport and status bar.
    pub const GAP: f32 = 16.0;
    pub const TOP_BAR_HEIGHT: f32 = 52.0;
    pub const TOP_BAR_PAD: f32 = 14.0;
    pub const STATUS_HEIGHT: f32 = 36.0;
    pub const MENU_BAR_BUTTON_HEIGHT: f32 = 34.0;
    pub const MENU_BAR_BUTTON_PAD: f32 = 12.0;
    pub const MENU_BAR_GAP: f32 = 2.0;
    /// Tool dock: two columns of square buttons.
    pub const DOCK_BUTTON: f32 = 44.0;
    pub const DOCK_GAP: f32 = 8.0;
    pub const DOCK_PAD_X: f32 = 10.0;
    pub const DOCK_PAD_Y: f32 = 14.0;
    pub const DOCK_COLUMNS: usize = 2;
    pub const DOCK_GROUP_GAP: f32 = 8.0;
    pub const DOCK_ICON: f32 = 22.0;
    pub const DOCK_DOT: f32 = 6.0;
    pub const DOCK_DOT_INSET: f32 = 7.0;
    pub const DOCK_HINT_INSET: [f32; 2] = [6.0, 3.0];
    /// The dock's scroll bar is thinner than its side padding, so it never
    /// pushes the buttons sideways or adds a horizontal scroll bar.
    pub const DOCK_SCROLL_BAR: f32 = 4.0;
    /// Icons: drawn in a 24 by 24 box, stroked.
    pub const ICON_BOX: f32 = 24.0;
    pub const ICON_STROKE: f32 = 1.7;
    /// Menus.
    pub const MENU_PAD: f32 = 6.0;
    pub const MENU_MIN_WIDTH: f32 = 212.0;
    pub const MENU_ROW_HEIGHT: f32 = 30.0;
    pub const MENU_ROW_PAD: [f32; 2] = [4.0, 10.0];
    pub const MENU_CHECK: f32 = 14.0;
    pub const MENU_CHECK_DOT: f32 = 2.5;
    pub const MENU_KEY_GAP: f32 = 18.0;
    pub const MENU_SUB_GAP: f32 = 4.0;
    pub const MENU_GAP: f32 = 6.0;
    pub const MENU_SEPARATOR: [f32; 2] = [5.0, 8.0];
    pub const MENU_HEAD_HEIGHT: f32 = 26.0;
    pub const MENU_ARROW: f32 = 3.0;
    pub const SCREEN_MARGIN: f32 = 8.0;
    /// Tooltips beside dock buttons.
    pub const TIP_GAP: f32 = 10.0;
    pub const TIP_PAD: [f32; 2] = [10.0, 6.0];
    /// Keycaps.
    pub const KEYCAP_PAD: [f32; 2] = [6.0, 2.0];
    /// Dialogs.
    pub const DIALOG_PAD: [f32; 2] = [22.0, 20.0];
    pub const DIALOG_WIDTH: f32 = 520.0;
    pub const DIALOG_WIDE: f32 = 760.0;
    pub const DIALOG_MAX_HEIGHT: f32 = 0.8;
    pub const PROGRESS_HEIGHT: f32 = 8.0;
    pub const CLOSE_BUTTON: f32 = 30.0;
    /// Text fields.
    pub const FIELD_HEIGHT: f32 = 36.0;
    pub const FIELD_PAD: f32 = 12.0;
    /// Small buttons (table cells, dialog actions).
    pub const SMALL_BUTTON_HEIGHT: f32 = 30.0;
    pub const SMALL_BUTTON_PAD: f32 = 12.0;
}

/// Emboss depths: the shadow offset in points at depth 1 (`a` in `UI-STYLE.md`).
pub mod depth {
    pub const BUTTON: f32 = 6.0;
    pub const CARD: f32 = 8.0;
    pub const HANDLE: f32 = 4.0;
    pub const KNOB: f32 = 3.0;
    pub const PILL: f32 = 4.0;
    pub const TOOLTIP: f32 = 3.0;
    pub const DIMPLE: f32 = 1.5;
    pub const GROOVE: f32 = 1.5;
    /// How deep a glass cutout is recessed.
    pub const WELL: f32 = 3.0;
    pub const RAIL_WELL: f32 = 2.5;

    /// Button depth at rest, on hover, and pressed (1 raised, 0 flat, negative sunk).
    pub const REST: f32 = 1.0;
    pub const HOVER: f32 = 1.3;
    pub const PRESSED: f32 = -0.7;
    /// A dropdown trigger while its list is open.
    pub const OPEN: f32 = -0.5;
    /// A slider handle while dragged.
    pub const LIFTED: f32 = 1.7;

    /// CSS blur is twice the offset; egui's blur ramp is this many times the CSS blur.
    pub const BLUR_RAMP: f32 = 1.5;

    /// Tool dock buttons (prototype `.raised`: a = 5, hover 7, pressed b = 4).
    pub const DOCK: f32 = 5.0;
    pub const DOCK_HOVER: f32 = 1.4;
    pub const DOCK_PRESSED: f32 = -0.8;
    /// Greyed dock buttons stay slightly raised (a = 2).
    pub const DOCK_DISABLED: f32 = 0.4;
    /// Hover and open menu titles and menu rows: a small inset.
    pub const MENU_INSET: f32 = 2.0;
    /// Text fields and progress bars are wells.
    pub const FIELD_WELL: f32 = 2.0;
    pub const BAR_WELL: f32 = 1.0;

    /// Floating cards (windows, menus): the drop shadow, at depth 1.
    pub const FLOAT_OFFSET: f32 = 10.0;
    pub const FLOAT_BLUR: f32 = 24.0;
}

/// Small motion amounts that are not springs (springs live in `crate::spring`).
pub mod motion {
    /// Label sink while a button is pressed (points down, and scale).
    pub const LABEL_SINK: f32 = 1.0;
    pub const LABEL_SCALE: f32 = 0.96;
    /// Slider handle grows this much while dragged.
    pub const HANDLE_GROW: f32 = 1.1;
    /// Toggle knob stretch per point/second of travel speed, and its cap.
    pub const KNOB_STRETCH: f32 = 0.0012;
    pub const KNOB_STRETCH_MAX: f32 = 0.28;
    /// The highlighted dropdown item is a little larger than its row.
    pub const PILL_SCALE: f32 = 1.03;
    /// The highlighted item's label moves right by this much.
    pub const ITEM_NUDGE: f32 = 4.0;
    /// Popups rise into place by this much as they open.
    pub const POP_RISE: f32 = 8.0;
    /// Popups start at this scale.
    pub const POP_SCALE: f32 = 0.92;
    /// Dock icons shrink a little while pressed or active.
    pub const ICON_PRESS: f32 = 0.94;
    /// Longest time step a spring takes in one frame (seconds).
    pub const MAX_STEP: f64 = 0.1;
}

/// Font for body text.
pub fn body() -> FontId {
    FontId::new(text::BODY, FontFamily::Proportional)
}

/// Body text in medium weight.
pub fn body_medium() -> FontId {
    FontId::new(text::BODY, FontFamily::Name(text::SANS_MEDIUM.into()))
}

/// Body text in semibold, as the kit uses for button labels and values.
pub fn body_strong() -> FontId {
    FontId::new(text::BODY, FontFamily::Name(text::SANS_SEMIBOLD.into()))
}

/// Panel and dialog titles.
pub fn title() -> FontId {
    FontId::new(text::TITLE, FontFamily::Name(text::SANS_SEMIBOLD.into()))
}

/// Numbers and values.
pub fn mono() -> FontId {
    FontId::new(text::MONO, FontFamily::Monospace)
}

/// Install fonts and egui's base style. Call once at start.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(crate::fonts::definitions());
    ctx.all_styles_mut(apply_style);
}

fn apply_style(style: &mut egui::Style) {
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(text::SMALL, FontFamily::Proportional),
        ),
        (TextStyle::Body, body()),
        (TextStyle::Button, body()),
        (TextStyle::Heading, title()),
        (TextStyle::Monospace, mono()),
    ]
    .into();

    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.override_text_color = Some(color::INK);
    v.weak_text_color = Some(color::MUTED);
    v.panel_fill = color::SURFACE;
    v.window_fill = color::SURFACE;
    v.extreme_bg_color = color::FROST;
    v.faint_bg_color = color::FROST;
    v.code_bg_color = color::FROST;
    v.hyperlink_color = color::ACCENT;
    v.selection.bg_fill = color::ACCENT.gamma_multiply(0.35);
    v.selection.stroke = egui::Stroke::new(1.0, color::ACCENT);
    v.window_stroke = egui::Stroke::new(size::EDGE_WIDTH, color::FLOAT_EDGE);
    v.window_corner_radius = corner(radius::CARD);
    v.menu_corner_radius = corner(radius::MENU);
    v.window_shadow = egui::Shadow {
        offset: [depth::FLOAT_OFFSET as i8, depth::FLOAT_OFFSET as i8],
        blur: depth::FLOAT_BLUR as u8,
        spread: 0,
        color: color::SHADOW_DARK,
    };
    v.popup_shadow = v.window_shadow;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.bg_fill = color::SURFACE;
        w.weak_bg_fill = color::SURFACE;
        w.bg_stroke = egui::Stroke::NONE;
        w.corner_radius = corner(radius::MENU_BUTTON);
        w.fg_stroke = egui::Stroke::new(1.0, color::INK);
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, color::MUTED);

    let s = &mut style.spacing;
    s.item_spacing = egui::vec2(size::ROW_GAP, size::ROW_GAP);
    s.window_margin = egui::Margin::same(size::PANEL_PAD as i8);
    s.button_padding = egui::vec2(size::ITEM_PAD_X, size::LIST_PAD);
}

/// A corner radius for egui's integer type, clamped to what it can hold.
pub fn corner(r: f32) -> egui::CornerRadius {
    egui::CornerRadius::same(r.clamp(0.0, f32::from(u8::MAX)) as u8)
}

/// `c` with its opacity multiplied by `f` (0 to 1).
pub fn fade(c: Color32, f: f32) -> Color32 {
    c.gamma_multiply(f.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_tokens_match_the_kit() {
        // rgba(0,0,0,.55) and rgba(255,255,255,.065), premultiplied.
        assert_eq!(color::SHADOW_DARK.a(), (0.55f32 * 255.0).round() as u8);
        let light = color::SHADOW_LIGHT;
        assert_eq!(light.a(), (0.065f32 * 255.0).round() as u8);
        assert_eq!(light.r(), light.a(), "premultiplied white");
    }

    #[test]
    fn corner_is_clamped() {
        assert_eq!(corner(radius::ROUND).nw, u8::MAX);
        assert_eq!(corner(radius::BUTTON).nw, 16);
    }
}
