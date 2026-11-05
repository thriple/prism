# Notes

Things noticed while building that are outside the current chunk, defaults that shaped the work, and checks still to do.

## Chunk 1: Skeleton

**Checked by the owner (2026-11-05)**
- **Release:** the `v0.0.1` release built on GitHub, and the owner ran the Windows download from the Releases page. The screenshot below is from that download.
- **Windows:** the app runs on Windows 11 with an NVIDIA GeForce RTX 3050 over Vulkan, about 85 fps, at 1280x800 and scale 1.00. The debug panel logs mouse buttons and keys (see `docs/screenshots/skeleton-windows.png`). The overlay still uses egui's default look, so the Soft theme is the next job (`docs/briefs/02a-soft-theme.md`).
- **Licence files:** `LICENSE-MIT` and `LICENSE-APACHE` were added.
- **Skeleton branch** merged into `main` by `scripts/finish_chunk1.py`.

**Still to check by a person**
- **GitHub Pages:** Source must be "GitHub Actions" (settings > Pages). The script tries to set it. Confirm the web page loads at https://thriple.github.io/prism/.
- **iPad (DECISIONS #10):** not tested yet. Open the Pages URL on the iPad and record here: runs or not, iPadOS version, browser.
- **macOS:** checked by the owner (2026-11-05). The `v0.0.1` Mac zip was blocked at first by macOS's normal warning for unidentified developers, and ran after **System Settings > Privacy & Security > Open Anyway** (right-click > Open no longer skips the check on macOS 15 and later). The bundle in `v0.0.1` had no signature of its own; `packaging/macos/make-app.sh` now signs it ad hoc and checks the signature, as a guard against the stricter "damaged" refusal some Macs give unsigned downloads. It was not the cause here. Each new build needs Open Anyway once. macOS version and chip not recorded.
- **Proper signing (later, PLAN 3.1 item 5):** a paid Apple Developer ID and notarising remove the warning entirely. Not needed while Prism is only for the owner.

**Seen in the Windows screenshot (for Actions & Keys)**
- The Windows key shows up as `SuperLeft` and as the modifier `Logo`. Windows reserves many Win+key combinations (Win+Shift+S was in use for screenshots), so the keymap engine must never rely on the Win key.
- Pressing Enter reports both `Enter` and `NumpadEnter`. Treat them as the same by default.

**What was checked here (Linux cloud machine, no real GPU)**
- Native: the window opens under Xvfb on Mesa's software Vulkan (llvmpipe), clears to `#262830`, shows the frame counter, and logs mouse buttons.
- Web: `trunk build --release` works. In headless Chromium the page falls back to WebGL2 (no WebGPU there), shows the overlay, and logs mouse, wheel and keys with physical codes (`KeyL`, `ShiftLeft`) plus the typed text.
- High DPI on the web: with a real 2x device scale the canvas is 2400x1500 for a 1200x750 page. Playwright's emulated scale factor does not report device pixels, so use `--force-device-scale-factor` when testing that.

**Decisions and defaults used**
- Releases: the owner wants fast iteration, not versions. Every push replaces one release called `latest` (Windows and Mac zips with fixed names), and is also kept as a numbered release, "Prism build N" (tag `prism-build-N`), so an older build can be downloaded if a new one goes wrong. The newest 30 are kept. The debug panel shows the build number. Each numbered release names its commit, so the code can be rolled back to it too.
- Versions: winit 0.30 (0.31 is still beta), wgpu 30, egui 0.37. egui-wgpu 0.37 needs exactly these.
- The WebGL2 fallback came for free through egui-wgpu's default `webgl` feature, so it is on.
- The surface uses a plain (non-sRGB) 8-bit format when offered, because egui expects it. The Viewport chunk may want an sRGB or HDR target for shading; the clear colour already handles both.
- The window redrew continuously so the frame counter moved. Replaced in Soft Theme: it now redraws only when something changes.
- The overlay used egui's built-in fonts. Replaced in Soft Theme by bundled IBM Plex (the owner approved OFL fonts, DECISIONS #12). The Motion Kit is now in `docs/ui-kit/`.

**Leftovers and ideas**
- The release wasm was about 7.5 MB in Skeleton; see Soft Theme below for the current size.
- Native Linux needs `libxkbcommon-x11` at run time. Linux is not a target, but worth knowing.
- Consider `cargo-deny` in CI to check licences automatically, so the table in `docs/DEPENDENCIES.md` cannot drift.

## Soft Theme

**What was built**
- A new crate, `prism-ui`: the one theme layer. `theme.rs` holds every colour, size, radius, depth and font; `spring.rs` the kit's springs (solved exactly, no clock inside); `emboss.rs` the four-shadow raised and pressed surfaces, with inner shadows as a mesh because egui has none; `aurora.rs` the still aurora and film grain; `glass.rs` the cutouts; and soft button, toggle, slider, dropdown, panel and groove widgets.
- The debug panel is restyled, and a "Theme demo" panel shows the four controls next to the settings: reduce motion (Follow system, On, Off), animate aurora (off by default), continuous redraw (off by default; turn it on to read fps).
- A test (`crates/prism-ui/tests/style_literals.rs`) fails if a colour, shadow, corner radius or line width literal appears outside `theme.rs`. CI runs it with `cargo test`.
- Screenshots at the same size, dropdown open: `docs/screenshots/soft-theme-kit.png` (the kit's `soft.html`, dark) and `docs/screenshots/soft-theme-prism.png` (Prism, native, software renderer).

**Measured here (Linux, software renderer, so a real GPU is faster)**
- **Idle:** with nothing happening the app draws no frames and used 0 CPU ticks over 5 seconds (measured from `/proc`, 100 ticks = one core for one second). The web build drew 9 frames in its first 6 seconds and then stopped.
- **Web size:** the release wasm is 7.8 MB, 2.7 MB gzipped (GitHub Pages serves it compressed). The fonts add about 950 KB; egui's own fonts (about 1 MB) are no longer included.
- **Motion:** checked frame by frame: the button lifts on hover, sinks on press with the label shrinking, and is still coming back 50 ms after release. The slider shows its value bubble and lit dimple while dragged. The dropdown pops in with its pill and tick.

**Decisions and defaults used**
- Text sizes follow `UI-STYLE.md` (13 px controls, 15 px titles), not `soft.html`, which uses 15 px for controls. The kit's giant header is left out (DECISIONS #13).
- Button depth uses the kit's own numbers (offset 6, depths 1 / 1.3 / -0.7) where `UI-STYLE.md` says offset 5 and hover 7; the kit wins where they disagree, as `UI-STYLE.md` says.
- egui's blur is a straight ramp, not a Gaussian, so CSS blur is converted with a factor of 1.5 (`theme::depth::BLUR_RAMP`), judged by eye against the kit.
- The toggle knob's stretch comes from its speed, not a fixed keyframe list, so it stretches only while it travels.
- The aurora follows the window, as in the kit, so how colourful a cutout looks depends on where its panel sits. The demo panel starts on the right, over the teal and pink.
- The fonts are IBM's original TTF files, not subsets: Plex has a Reserved Font Name, and a subset would be a modified font that may not be called Plex.
- Reduce motion follows the system on Windows (client-area animation setting), macOS (Accessibility > Reduce motion) and the web (`prefers-reduced-motion`). It is re-read when the window gains focus. Linux reports nothing, so it counts as off.
- egui-winit asks for a repaint after every redraw event, which caused an endless loop; those events are no longer passed to it.

**Still to check by a person**
- Windows and macOS: the look, and the idle CPU in Task Manager or Activity Monitor (should be about 0% when nothing moves).
- Windows and macOS: turning on the system's reduce-motion setting makes the demo controls jump with no animation.

**Leftovers and ideas**
- Release build 4 failed once: GitHub answered 403 when creating its release, and build 5 with the same workflow worked. The release steps now retry up to four times.
- An old release from the Android project is still on the Releases page, tag `build-4`, also titled "Prism build 4". It is not one of these builds. Delete it if it confuses (left alone here: deleting needs the owner's OK).
- The kit's `BLUR_IN` (content arriving from a blur) is not done: egui cannot blur. Opacity, scale and rise are done.
- Popups close straight away when you pick an item, with no closing animation for the list contents (the list itself fades and shrinks).
- Panels float for now. Docking (`PanelLayout`, `DockPanel`) comes with the panels that need it.
- Area positions: a panel asked to start at x = 760 in a 1280-wide window opened at x = 680. egui seems to keep new areas fully on screen using an estimated size. Harmless, but worth knowing when panels get default places.

## For the next Claude Code session (written 2026-11-05)

The browser prototype was updated while the Soft Theme chunk was being built. Use the latest `docs/prototype/Main.dc.html`, and read `docs/prototype/README.md` for where things are.

1. **Buttons and menus:** the prototype's action table, menu bar, tool dock, icons, right-click menu and Planned Features list are the design for the Actions & Keys chunk. See "Pick up the prototype's buttons and menus" in `docs/briefs/03-actions-and-keys.md`.
2. **Smart Guides:** use the new system in `PLAN.md` section 5.1 and the prototype functions `smartInit(`, `smartSnap(`, `smartDraw(`. The rules to keep: alignment and equal spacing between whole objects, snap pull of about 8 px, guides only on axes the object is moving along, the centre-alignment line only once centres truly line up, a per-frame time budget (2 ms) that drops spacing guides first, and Ctrl+U to toggle. This is built in the Inference & Drawing chunk once groups exist (Groups & Components), not before. Until then only the action `view.smart` goes in the registry (greyed out).
3. **Box select:** window (left to right, fully inside) and crossing (right to left, touching), Shift toggles, Ctrl adds. A drag that starts on geometry is still a box. See `commitBox(` in the prototype. It belongs with the selection code when the Select tool is built.
4. **Soft Theme:** now merged or on its branch (`prism-ui`). Build the menus and dock from those parts.
