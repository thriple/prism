# Dependencies

Every direct dependency, its licence, and whether it builds for the web (`wasm32-unknown-unknown`).
Add a row before adding a dependency. Allowed by default: MIT, Apache-2.0, BSD, zlib, MPL.

| Crate | Version | Used by | Licence | WASM | Why |
|---|---|---|---|---|---|
| winit | 0.30 | prism-app | Apache-2.0 | Yes | Window and input events |
| wgpu | 30 | prism-app | MIT OR Apache-2.0 | Yes (WebGPU, WebGL2 fallback) | GPU access |
| egui | 0.37 | prism-app | MIT OR Apache-2.0 | Yes | Debug overlay, later panels |
| egui-wgpu | 0.37 | prism-app | MIT OR Apache-2.0 | Yes | Draws egui with wgpu |
| egui-winit | 0.37 | prism-app | MIT OR Apache-2.0 | Yes | Feeds winit events to egui |
| log | 0.4 | prism-app | MIT OR Apache-2.0 | Yes | Logging facade |
| web-time | 1.1 | prism-app | MIT OR Apache-2.0 | Yes | `Instant` that works in the browser |
| env_logger | 0.11 | prism-app (native only) | MIT OR Apache-2.0 | n/a | Log output on the console |
| pollster | 1.0 | prism-app (native only) | Apache-2.0 OR MIT | n/a | Waits for GPU setup natively |
| console_log | 1.0 | prism-app (web only) | MIT OR Apache-2.0 | Yes | Log output in the browser console |
| console_error_panic_hook | 0.1 | prism-app (web only) | Apache-2.0/MIT | Yes | Panics show in the browser console |
| wasm-bindgen-futures | 0.4 | prism-app (web only) | MIT OR Apache-2.0 | Yes | Async GPU setup in the browser |

## Indirect dependencies outside the default list

Checked with `cargo metadata` for Windows, macOS and the web. All are permissive.

| Crate | Licence | Note |
|---|---|---|
| epaint_default_fonts | (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0 | egui's built-in fonts (font files, not code). To be replaced by IBM Plex, which is also OFL-1.1. |
| libloading | ISC | Permissive, equivalent to MIT |
| self_cell | Apache-2.0 OR GPL-2.0-only | We use it under Apache-2.0 |
| unicode-ident, unicode-properties | (MIT OR Apache-2.0) AND Unicode-3.0 | Unicode data licence, permissive |

## Tools (not linked into Prism)

| Tool | Licence | Use |
|---|---|---|
| trunk 0.22.1 | MIT OR Apache-2.0 | Builds and bundles the web version |
