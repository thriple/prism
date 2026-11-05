# Notes

Things noticed while building that are outside the current chunk, defaults that shaped the work, and checks still to do.

## Chunk 1: Skeleton

**Still to check by a person**
- **iPad (DECISIONS #10):** not tested yet. Open the Pages URL on the iPad once `main` is deployed and record here: runs or not, iPadOS version, browser.
- **Windows and macOS:** CI builds both, but nobody has opened the window on either yet. Run `cargo run` on one of them and tick the box.
- **GitHub Pages:** in the repo settings, set Pages > Source to "GitHub Actions", or the deploy job fails.
- **Licence files:** DECISIONS #1 says to add `LICENSE-MIT` and `LICENSE-APACHE` when the first release is cut. Add them before tagging `v0.0.1`.

**What was checked here (Linux cloud machine, no real GPU)**
- Native: the window opens under Xvfb on Mesa's software Vulkan (llvmpipe), clears to `#262830`, shows the frame counter, and logs mouse buttons.
- Web: `trunk build --release` works. In headless Chromium the page falls back to WebGL2 (no WebGPU there), shows the overlay, and logs mouse, wheel and keys with physical codes (`KeyL`, `ShiftLeft`) plus the typed text.
- High DPI on the web: with a real 2x device scale the canvas is 2400x1500 for a 1200x750 page. Playwright's emulated scale factor does not report device pixels, so use `--force-device-scale-factor` when testing that.

**Decisions and defaults used**
- Versions: winit 0.30 (0.31 is still beta), wgpu 30, egui 0.37. egui-wgpu 0.37 needs exactly these.
- The WebGL2 fallback came for free through egui-wgpu's default `webgl` feature, so it is on.
- The surface uses a plain (non-sRGB) 8-bit format when offered, because egui expects it. The Viewport chunk may want an sRGB or HDR target for shading; the clear colour already handles both.
- The window redraws continuously so the frame counter moves. Later chunks should redraw only when something changes (it matters for laptops and for "hidden must be free").
- The overlay uses egui's built-in fonts. IBM Plex Sans and Mono (CLAUDE.md) should be bundled when real UI arrives (Actions & Keys). They are OFL-1.1, which is not on the default licence list: a fonts-only exception needs the owner's OK.
- Motion-Kit's dark theme UI is not in the repo or the plan. The owner offered it; it may matter for Actions & Keys.

**Leftovers and ideas**
- The release wasm is about 7.5 MB. Look at size (wasm-opt level, egui features) before the iPad test.
- Native Linux needs `libxkbcommon-x11` at run time. Linux is not a target, but worth knowing.
- Consider `cargo-deny` in CI to check licences automatically, so the table in `docs/DEPENDENCIES.md` cannot drift.
