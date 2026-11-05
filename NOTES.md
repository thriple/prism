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
- **macOS:** the `v0.0.1` Mac download was blocked as soon as it was opened. Cause: the `.app` bundle had no signature at all (only the program inside had the linker's automatic one), and Apple Silicon Macs refuse such a download, usually with "damaged and can't be opened" and no way past it. Fixed by `packaging/macos/make-app.sh`, which signs the bundle ad hoc and checks the signature. CI now builds a signed test app on every push (artifact `prism-macos-test` on the run's page), so it can be tried before the next tag. It still needs one approval the first time: open it, click Done, then **System Settings > Privacy & Security > Open Anyway** (right-click > Open no longer skips the check on macOS 15 and later). Record the result here, with the macOS version and chip. `v0.0.1` itself was not rebuilt: tag `v0.0.2` to get a fixed release.
- **Proper signing (later, PLAN 3.1 item 5):** a paid Apple Developer ID and notarising remove the warning entirely. Not needed while Prism is only for the owner.

**Seen in the Windows screenshot (for Actions & Keys)**
- The Windows key shows up as `SuperLeft` and as the modifier `Logo`. Windows reserves many Win+key combinations (Win+Shift+S was in use for screenshots), so the keymap engine must never rely on the Win key.
- Pressing Enter reports both `Enter` and `NumpadEnter`. Treat them as the same by default.

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
