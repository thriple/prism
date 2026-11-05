# UI style: the dark "Soft" look

The whole program uses the look of the feel-test prototype: a dark, soft, embossed ("neumorphic") style. Buttons look raised out of the surface and sink in when pressed. The values below are copied from `docs/prototype/Main.dc.html` (search for `.soft` and `.raised`). If this file and the prototype disagree, the prototype wins, and this file should be fixed.

The viewport (the 3D drawing area) is drawn by our own renderer. This style applies to everything around it: menu bar, dock, context menus, panels, dialogs, windows, tooltips, sliders and toggles.

## Colour tokens
| Token | Value | Use |
|---|---|---|
| surface | `#2a2c33` | Background of the app and every panel and button |
| ink | `#e6e8ee` | Main text and icons |
| muted | `#a4a9b5` | Secondary text, hotkey labels, values |
| shadow-dark | `rgba(0,0,0,.55)` | The dark half of every emboss |
| shadow-light | `rgba(255,255,255,.065)` | The light half of every emboss |
| frost | `#262830` | Slightly darker wells and inset areas |
| accent | `#02ceff` | Focus ring, the "active tool" dot, selection highlights |
| aurora base / 1 to 5 | `#0d1030`, `#3f3cc0`, `#e0457a`, `#0aa3d6`, `#a177ff`, `#2fd4a0` | Decorative gradients (the prototype's monitor backdrop). Not used for UI controls. |

## Type
- **IBM Plex Sans**, 400 / 500 / 600, for labels, menus and text. Menus and buttons are 13 px, tooltips 12 px.
- **IBM Plex Mono**, 400 / 500, for numbers, values, hotkey hints (9 px on dock buttons) and the measurement box.
- Both are open licensed (SIL OFL). Bundle them in the app; do not load from the internet.

## The emboss recipe
Every control has four shadows so it can animate smoothly between raised and pressed:
- Raised: a dark shadow down-right and a light shadow up-left, each offset `a` px with blur `2a` px.
- Pressed: the same pair, inset, offset `b` px.
- Defaults: buttons `a = 5`, cards `a = 8`, slider handles `a = 4`, toggle knobs `a = 3`. Hover raises `a` to 7. Pressed or active sets `a = 0`, `b = 4`.
- Menu hover and open menu titles use a small inset (2 px, blur 4 px).
- Floating things (context menus, windows) use a stronger drop shadow: about 10 px offset with 24 px blur at 55% black, plus a light shadow up-left, and a 1 px border of `rgba(255,255,255,.06)`.

## Shapes and sizes
- Dock buttons are 48 × 48 px with a 16 px corner radius. Menu bar buttons are 34 px tall with an 11 px radius. Menus have a 14 px radius, 6 px padding and a 212 px minimum width. Large cards and windows use a 24 px radius.
- Grooves (dividers between button groups) are 4 px tall inset lines. Progress bars and slider tracks are inset wells.
- Spacing is generous. Nothing is flat or outlined: depth comes from the shadows.

## Motion
- Press and release animate over about 0.12 s and 0.38 s, with a gentle overshoot curve `cubic-bezier(.34, 1.56, .64, 1)`. Toggle knobs slide with `cubic-bezier(.3, 1.4, .5, 1)` over 0.4 s.
- The active tool shows a small accent dot at the top-right of its dock button that scales in.
- Motion is short and never delays an action. Provide a "reduce motion" setting that turns the animation off.

## States
- **Greyed out:** an action with no implementation, or not available now, is shown dimmed, cannot be run, and never disappears. Hover shows no emboss.
- **Focus:** a 2 px accent outline, 4 px away from the control, for keyboard use.
- **Tooltips:** a small raised card to the right of the dock button, showing the name and hotkey.

## Rules for the build
1. Build one theme layer (colours, fonts, shadow recipe, radii) and use it for every widget. No colours or shadows written inline.
2. With egui, set the look through its `Visuals` and custom painting for the emboss. Check it against the prototype side by side.
3. The same style on every platform. The Mac system menu bar (when added) follows the system style instead.
4. A light theme is not planned. If one is added later, it is a second set of tokens, not new widgets.
