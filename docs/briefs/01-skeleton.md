# Chunk 1: Skeleton

**Goal.** A Cargo workspace that opens an empty wgpu window as a native Windows app, a native macOS app and a web page, with CI that tests, builds and publishes all three.

**Why first.** Everything else needs somewhere to run, and the web build must never silently stop compiling.

## In scope
- A Cargo workspace with one crate, `prism-app`, using `winit` and `wgpu` (current stable versions), and `egui` through `egui-wgpu` for a tiny debug overlay.
- The window clears to a colour, draws a frame counter with egui, and resizes correctly on high-DPI screens.
- The same code builds natively and for the web (`wasm32-unknown-unknown`), using a standard tool such as `trunk`. Add a WebGL2 fallback only if wgpu gives it for free.
- Keyboard and mouse events are received and logged to the overlay (button, position, wheel, key code, modifiers), so later chunks can see what the browser and the OS deliver.
- **CI (GitHub Actions):**
  - On every push: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, a native build on Windows and macOS, and the web build.
  - On every push to `main`: deploy the web build to GitHub Pages.
  - On a version tag: attach a Windows zip and a macOS build (an `.app` in a zip, unsigned) to a GitHub Release.
- `NOTES.md` started, and `docs/DEPENDENCIES.md` created with a row for each dependency (licence, WASM status).

## Out of scope
Everything else: geometry, tools, menus, a renderer beyond clearing the screen, WASM threads, a PWA manifest, signing and notarising.

## Steps
1. Ask the owner to confirm before touching the Android video player (see DECISIONS.md #11). Do not delete it. If the workspace needs to live beside it, put it in a new folder.
2. Create the workspace and get the native window working on the machine you have.
3. Get the web build working locally.
4. Write the CI workflows. Test them by pushing a branch.
5. Open the hosted web page on a phone or tablet if the owner can, and record whether it runs, and which browser and OS version, in `NOTES.md` (DECISIONS.md #10).

## Acceptance
- [ ] `cargo run` opens a window with a frame counter on Windows and macOS (CI builds both, a person checks one).
- [ ] The web build opens the same view in Chrome.
- [ ] Mouse and key events appear in the overlay, with physical key codes.
- [ ] CI is green on a pull request. Merging to `main` publishes the web page. Tagging `v0.0.1` produces a release with a Windows zip and a macOS zip.
- [ ] `NOTES.md` records the iPad result, or says it could not be tested.

## Notes
- Keep the application code thin. Anything that is not window and event handling goes in a crate that later chunks can test without a window.
- No threads in the web build. `wgpu` objects are not `Send` or `Sync` on the web.
