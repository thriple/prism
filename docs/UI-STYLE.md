# UI style: Motion Kit, dark "Soft glass"

**Source.** The whole program uses the look of the **Motion Kit** (`docs/ui-kit/`), in its **Soft glass** set with the **dark theme** (`data-soft-theme="dark"`). The feel-test prototype used this theme: its `--soft-*` colour tokens are the kit's dark values, and its raised/pressed buttons, sliders, toggles and springy overshoot come from the kit's Soft components. (An earlier note in this repo wrongly said the style was invented for the prototype. It is the kit's.)

**How the prototype differs from the kit.** The prototype approximates the kit in plain CSS: it has the emboss shadows and the springs, but not the frosted "glass cutouts" onto a moving aurora, and its muted text colour is a little lighter (`#a4a9b5` against the kit's `#8b909c`). The full program should follow **the kit**, including the cutouts, where they make sense (see below). Where this file and the kit disagree, the kit wins.

**The kit is React, and Prism is Rust.** Do not use the React code. Read it as the specification, and re-implement the look in the Rust UI (egui plus our own painting and wgpu). Open `docs/ui-kit/soft.html` and `docs/ui-kit/demo.html` in a browser to see everything moving. The kit's README has the full component list and the motion language.

The viewport (the 3D drawing area) is drawn by our own renderer. This style applies to everything around it: menu bar, dock, context menus, panels, dialogs, windows, tooltips, sliders and toggles.

## Which kit component to copy for which Prism part
| Prism part | Kit component to follow |
|---|---|
| Right-click menu | `ContextMenu` (pops from the pointer, stays on screen, shows key hints) |
| Command search (Shift+S) | `CommandPalette` (fuzzy search, arrows, Enter, Esc) |
| Tool dock and its tooltips | `TooltipToolbar` and `Tooltip` (one shared tooltip that slides between buttons) |
| Hotkey hints everywhere | `Kbd` (keycap) |
| Mode switcher (top right, PLAN.md 1.11) | `ModeDial` or `SegmentedControl`, to be chosen when that chunk is built |
| Status messages, "done" and "can't do that" | `DynamicIsland` toasts |
| Undoable destructive actions | `UndoSnackbar`, and `HoldButton` for anything that cannot be undone |
| Docked, movable panels (Inspector, View Settings, Model Health, Outliner) | `PanelLayout` and `DockPanel` |
| Buttons, toggles, sliders, dropdowns inside panels | `SoftButton`, `SoftToggle`, `SoftSlider`, `SoftSelect` |
| Busy states (Model Health scan, rendering) | `StatusDot`, `RingThink`, `RenderProgress` |
| Icons | The kit's `Icon` set is a starting point. Prism needs its own tool icons (the prototype has them). Keep the same stroke weight and size. |

Use a kit component whenever one fits, as the kit's own README says, and do not invent a second style for the same job.

## Glass cutouts
In the kit, controls are one opaque raised material, and some parts (slider rails, toggle tracks, dropdown lists) are recessed windows onto a slowly morphing aurora, tinted and grained. In Prism, use cutouts for: slider rails, toggle tracks, dropdown lists, and large panel backgrounds if it looks good. Do not put the aurora behind the 3D viewport. The aurora must freeze when "reduce motion" is on, and must cost almost nothing (the kit draws it at 1/8 resolution). If it slows the viewport on a weak GPU, turn it off before anything else is cut.

## Colour tokens
| Token | Value | Use |
|---|---|---|
| surface | `#2a2c33` | Background of the app and every panel and button |
| ink | `#e6e8ee` | Main text and icons |
| muted | `#8b909c` (kit; the prototype uses `#a4a9b5`) | Secondary text, hotkey labels, values |
| shadow-dark | `rgba(0,0,0,.55)` | The dark half of every emboss |
| shadow-light | `rgba(255,255,255,.065)` | The light half of every emboss |
| frost | `#262830` | Slightly darker wells and inset areas |
| accent | `#02ceff` | Focus ring, the "active tool" dot, selection highlights |
| aurora base / 1 to 5 | `#0d1030`, `#3f3cc0`, `#e0457a`, `#0aa3d6`, `#a177ff`, `#2fd4a0` | The aurora behind glass cutouts (`aurora.ts` in the kit). Not used for ordinary controls. |

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
- Corners mix big and small radii for an asymmetric feel (the kit uses things like 14 / 14 / 14 / 4). Hit targets are at least 40 to 44 px, except in dense tool docks, where the prototype's 48 px buttons apply.
- Spacing is generous. Nothing is flat or outlined: depth comes from the shadows.

## Motion
- The kit's springs (`docs/ui-kit/components/motion.ts`): `SPRING` stiffness 420 / damping 32 (containers resizing), `SPRING_SNAPPY` 520 / 34 (menus, tooltips, pills), `SPRING_SOFT` 400 / 30 (toasts). Implement a small spring integrator in Rust and use these numbers.
- Content appears by `BLUR_IN` (opacity 0 to 1, blur 6 px to 0, scale .96 to 1). Panels open from their trigger by `POP_IN` (adds a rise of 8 px and blur 8 px).
- Soft buttons: a depth value goes 1 (raised) at rest, 1.3 on hover, -0.7 when pressed, 0 is flat. Press uses a firm spring (700 / 40), release a bouncy one (380 / 13) that overshoots past rest. The label sinks a beat behind the surface. Always four shadows, so it animates through flat.
- The prototype's CSS equivalents (`cubic-bezier(.34, 1.56, .64, 1)`, 0.12 s press, 0.38 s release) are fine as a fallback in places where a spring is not available.
- The active tool shows a small accent dot at the top-right of its dock button that scales in.
- Motion is short and never delays an action. A "reduce motion" setting (default: follow the operating system) makes springs instant and freezes the aurora.

## States
- **Greyed out:** an action with no implementation, or not available now, is shown dimmed, cannot be run, and never disappears. Hover shows no emboss.
- **Focus:** a 2 px accent outline, 4 px away from the control, for keyboard use.
- **Tooltips:** a small raised card to the right of the dock button, showing the name and hotkey.

## Rules for the build
1. Build one theme layer (colours, fonts, shadow recipe, radii) and use it for every widget. No colours or shadows written inline.
2. With egui, set the look through its `Visuals` and custom painting for the emboss. Check it against the prototype side by side.
3. The same style on every platform. The Mac system menu bar (when added) follows the system style instead.
4. The kit also has a light theme (`styles/soft.css`, default values). Prism ships dark only for now. If a light theme is added later, it is a second set of tokens from the same file, not new widgets.
5. Keep keyboard support, focus handling and accessibility labels that the kit's components have.
