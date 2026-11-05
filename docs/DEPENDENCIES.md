# Dependencies

Every direct dependency, its licence, and whether it builds for the web (`wasm32-unknown-unknown`).
Add a row before adding a dependency. Allowed by default: MIT, Apache-2.0, BSD, zlib, MPL. Bundled fonts may also be SIL OFL (DECISIONS #12).

| Crate | Version | Used by | Licence | WASM | Why |
|---|---|---|---|---|---|
| winit | 0.30 | prism-app | Apache-2.0 | Yes | Window and input events |
| wgpu | 30 | prism-app, prism-render | MIT OR Apache-2.0 | Yes (WebGPU, WebGL2 fallback) | GPU access |
| egui | 0.37 | prism-app, prism-ui | MIT OR Apache-2.0 | Yes | Panels and controls (no built-in fonts) |
| egui-wgpu | 0.37 | prism-app | MIT OR Apache-2.0 | Yes | Draws egui with wgpu |
| egui-winit | 0.37 | prism-app | MIT OR Apache-2.0 | Yes | Feeds winit events to egui |
| log | 0.4 | prism-app | MIT OR Apache-2.0 | Yes | Logging facade |
| web-time | 1.1 | prism-app | MIT OR Apache-2.0 | Yes | `Instant` that works in the browser |
| serde | 1.0 | prism-app, core crates | MIT OR Apache-2.0 | Yes | Keymap and model file formats |
| serde_json | 1.0 | prism-app, prism-topo, prism-doc, prism-file | MIT OR Apache-2.0 | Yes | Keymap, model and `.prism` manifest as JSON. The `float_roundtrip` feature is on, so f64 values come back bit for bit |
| glam | 0.34 | prism-math, prism-render (and the crates above them) | MIT OR Apache-2.0 | Yes | f64 vectors and rotations |
| robust | 1.2 | prism-math | MIT OR Apache-2.0 | Yes | Exact orientation predicates (Shewchuk) |
| proptest | 1.12 | prism-ops, prism-doc (tests only, native only) | MIT OR Apache-2.0 | No (not needed: tests run natively) | Property tests |
| bytemuck | 1.25 | prism-render | Zlib OR Apache-2.0 OR MIT | Yes | Vertex and uniform data as bytes for the GPU (already used inside wgpu and egui) |
| pollster | 1.0 | prism-render (tests only) | Apache-2.0 OR MIT | n/a | Waits for the GPU in the screenshot tests |
| env_logger | 0.11 | prism-app (native only) | MIT OR Apache-2.0 | n/a | Log output on the console |
| pollster | 1.0 | prism-app (native only) | Apache-2.0 OR MIT | n/a | Waits for GPU setup natively |
| console_log | 1.0 | prism-app (web only) | MIT OR Apache-2.0 | Yes | Log output in the browser console |
| console_error_panic_hook | 0.1 | prism-app (web only) | Apache-2.0/MIT | Yes | Panics show in the browser console |
| wasm-bindgen-futures | 0.4 | prism-app (web only) | MIT OR Apache-2.0 | Yes | Async GPU setup in the browser |
| web-sys | 0.3 | prism-app (web only) | MIT OR Apache-2.0 | Yes | Reduce-motion setting, local storage for the keymap, keymap download and file picker, Mac detection |
| js-sys | 0.3 | prism-app (web only) | MIT OR Apache-2.0 | Yes | Builds the keymap download |
| wasm-bindgen | 0.2 | prism-app (web only) | MIT OR Apache-2.0 | Yes | Callbacks for the keymap file picker |
| windows-sys | 0.59 | prism-app (Windows only) | MIT OR Apache-2.0 | n/a | Reads Windows' animation setting (reduce motion) |
| objc2-app-kit | 0.3 | prism-app (macOS only) | Zlib OR Apache-2.0 OR MIT | n/a | Reads macOS's reduce-motion setting |

## Fonts (bundled in `crates/prism-ui/assets/fonts/`)

| Font | Files | Licence | Note |
|---|---|---|---|
| IBM Plex Sans | Regular, Medium, SemiBold (TTF) | SIL OFL 1.1, Reserved Font Name "Plex" | IBM's original files, unmodified (subsetting or converting would count as a modified font, which may not use the name). Licence text in `assets/fonts/OFL.txt`. |
| IBM Plex Mono | Regular, Medium (TTF) | SIL OFL 1.1, Reserved Font Name "Plex" | As above. |

## Indirect dependencies outside the default list

Checked with `cargo metadata` for Windows, macOS and the web. All are permissive.

| Crate | Licence | Note |
|---|---|---|
| libloading | ISC | Permissive, equivalent to MIT |
| self_cell | Apache-2.0 OR GPL-2.0-only | We use it under Apache-2.0 |
| unicode-ident, unicode-properties | (MIT OR Apache-2.0) AND Unicode-3.0 | Unicode data licence, permissive |
| arrayref | BSD-2-Clause | Permissive |
| rustix, linux-raw-sys, wasip2, wit-bindgen | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Test tools only (through proptest); we use them under MIT or Apache-2.0 |
| walkdir, same-file | Unlicense/MIT | We use them under MIT |

## Tools (not linked into Prism)

| Tool | Licence | Use |
|---|---|---|
| trunk 0.22.1 | MIT OR Apache-2.0 | Builds and bundles the web version |
| Node.js | MIT | Runs `scripts/import_prototype.js`, which copies the prototype's action tables into Rust (only when they change) |
| zip | 9.4 | prism-file | MIT | Yes (default features off: deflate only, no C) | The `.prism` container (PLAN 1.13) |
| flate2 | 1.1 | prism-file | MIT OR Apache-2.0 | Yes (`rust_backend`: miniz_oxide, pure Rust) | Deflate for the zip |
| postcard | 1.1 | prism-file | MIT OR Apache-2.0 | Yes | The geometry entry: compact, f64 stored as raw bits |
