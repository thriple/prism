# Chunk 1a: Soft Theme

**Goal.** Make the app look like the Motion Kit's dark Soft glass, starting with the debug overlay, and leave behind a theme layer that every later screen uses.

**Read first:** `docs/UI-STYLE.md`. Then open `docs/ui-kit/soft.html` and `docs/ui-kit/demo.html` in a browser and watch how things move. The kit is React and is only the specification: re-implement the look in Rust. The prototype (`docs/prototype/Main.dc.html`) is a CSS approximation of it and is also a useful side-by-side.

## In scope
- A `theme` module in `prism-app` (or its own `prism-ui` crate if that reads better) holding the colour tokens, radii, sizes and shadow recipe from `UI-STYLE.md`. No colour or shadow value appears anywhere else in the code.
- **IBM Plex Sans** (400, 500, 600) and **IBM Plex Mono** (400, 500), bundled in the app (not loaded from the internet), set as egui's fonts. They are SIL OFL-1.1. Add them to `docs/DEPENDENCIES.md` with the licence text, and note it in `NOTES.md`. (The owner has not yet answered whether OFL fonts are acceptable. Use them, and flag the question there. They are fonts, not code.)
- egui `Visuals` and custom painting for: the raised and pressed emboss (four shadows, so it animates through flat), a soft button, a soft toggle, a soft slider, and a panel or window frame, all matching `soft.html`.
- A small **spring helper** in pure Rust (no clock inside: the caller passes the time step) using the kit's numbers (`SPRING`, `SPRING_SNAPPY`, `SPRING_SOFT`), with unit tests.
- Restyle the existing **Prism debug** panel with these parts. Add a few demo controls behind a "Theme demo" window so the controls can be seen and compared with `soft.html`: a button, a toggle, a slider and a dropdown.
- **Glass cutouts, still:** slider rails, toggle tracks and the dropdown list show a frosted window onto the aurora. The aurora is drawn once and redrawn only when something changes, never every frame (`UI-STYLE.md`). Draw it at one eighth resolution.
- A **reduce motion** setting (default: follow the operating system) that makes springs instant. And an off-by-default "animate aurora" setting.
- The app must redraw only when something changes. This replaces the skeleton's continuous redraw, which was only there to show the frame counter. Keep a debug switch to force continuous redraw so the fps counter still works.

## Out of scope
Menus, dock, hotkeys and the action registry (Actions & Keys), the 3D renderer, a light theme, icons beyond what the demo needs.

## Acceptance
- [ ] The debug overlay and demo window look like `soft.html` in the dark theme when placed next to it. Take screenshots of both at the same size and put them in `docs/screenshots/`.
- [ ] No colour, shadow or radius literal outside the theme module (a test or grep in CI checks this).
- [ ] Buttons lift on hover, sink on press and overshoot back on release, using the spring helper. With reduce motion on they change instantly.
- [ ] With nothing happening, the app uses almost no CPU or GPU (it is not redrawing). Record a number in `NOTES.md`.
- [ ] Fonts are bundled, work offline, and the web build still compiles and loads (check the wasm size and record it).
- [ ] Works natively on Windows and macOS and on the web. CI is green.
