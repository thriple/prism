# Research: hiccups ahead in the plan

Written 2026-11-06 (Guam). Each item lists what was checked, the chunk it hits, and what to do. Facts come from crates.io and the sources at the end, as of today. PLAN section 7 points here.

## Will change how a chunk is built

1. **The path tracer cannot run on WebGL2.** (Rendering)
   - WebGL2 has no compute shaders, and wgpu's WebGL2 limits allow almost no storage buffers. The path tracer and the denoiser need both.
   - WebGPU is now on by default in:
     - Chrome and Edge;
     - Safari 26 (macOS, iOS and iPadOS, so the iPad plan works);
     - Firefox on Windows (141) and on Apple Silicon Macs (147).
   - Firefox on Linux still needs a flag.
   - **Plan:** the render view needs WebGPU, and says so when it is missing. The modelling viewport keeps its WebGL2 fallback.
2. **WASM threads on GitHub Pages need a workaround.** (Simulation, Rendering, large imports, the NURBS kernel)
   - Threads need cross-origin isolation (COOP and COEP headers), which GitHub Pages cannot set.
   - The known fix is `coi-serviceworker`: a small service worker that adds the headers. It reloads the page once, and other sites' files must opt in.
   - **Plan:**
     - stay single-threaded until a chunk truly needs threads;
     - put heavy work on the GPU, or spread it over frames with a time budget, as Smart Guides do;
     - try `coi-serviceworker` in a spike before the Simulation chunk.
3. **The Blender bridge must stay at arm's length.** (Rendering part 6)
   - Blender is GPL, and its add-ons count as GPL because they use Blender's Python API.
   - Prism is MIT/Apache and must not link Blender code (CLAUDE.md, licences).
   - **Plan:** Prism and Blender talk through files (glTF or USD) or a local socket. The Blender add-on lives in its own repository under the GPL.
4. **The pure-Rust NURBS kernels have very few users.** (NURBS Spike, Solids & CAD)
   - Downloads in the last 90 days: `monstertruck` 0.4.1 (Apache-2.0), about 1,200; `arris` 0.10.0 (MIT or Apache), about 270.
   - `truck` has not had a release since 2024.
   - `curvo` (MIT, about 105,000 downloads) has NURBS curves, surfaces and booleans, and is worth adding to the spike.
   - OpenCascade's Rust crate is LGPL, so it is not allowed.
   - **Plan:** keep the `Kernel` trait. Make the spike's tests hard (surface-surface intersection, booleans on curved faces). Expect to own fixes upstream, or to own the hard parts.
5. **STEP for exact NURBS** depends on the kernel. Only the truck family has STEP read and write. (Solids & CAD part 6)
   - **Plan:** judge STEP round trips in the NURBS Spike too.

## Costs and accounts (the packaging step)

6. **Code signing.** This is DECISIONS #28, open.
   - **macOS:**
     - **Problem:** unsigned and un-notarised apps are blocked by Gatekeeper. Users must right-click and choose Open, or the app reports as "damaged".
     - **Fix:** notarising needs the Apple Developer Program, USD 99 a year.
   - **Windows:**
     - **Problem:** unsigned apps get SmartScreen warnings.
     - **Cheapest fix:** Microsoft's Azure Artifact Signing (formerly Trusted Signing), about USD 10 a month. It is open to individual developers in the US and Canada; Guam is US, which probably qualifies, but check.
     - **Other fix:** an OV certificate, about USD 150–500 a year.
     - Either way, a new app still shows warnings for a few weeks, until it builds a download history.
   - **Default until decided:** ship unsigned, as now, and say so in the release notes.

## Smaller, but plan for them

7. **Platform differences show up only in CI.** This session found two Windows-only bugs: a 0.0 that became -0.0, and zip's "made by" byte. Keep CI on Windows, macOS and Linux, keep golden files and property tests, and do not trust a green Linux run alone.
8. **Dependencies change their APIs.**
   - wgpu is at version 30 and has a major release every few months; egui is at 0.37.
   - winit 0.31 (in beta) moves macOS into `winit-appkit`, with its own app delegate. That changes the Mac "open file" handler planned in Files & Formats part 3.
   - **Plan:** upgrade between chunks, as a small part of its own, never in the middle of one. For part 3, check whether winit 0.31 is released first.
9. **Opening a file in the running Prism on Windows.**
   - `interprocess` 2.4.7 is "0BSD OR Apache-2.0"; we take Apache-2.0, which is allowed. It is well used (about 4.9 million downloads).
   - `single-instance` has not been updated since 2021, so do not use it.
   - Or use named pipes through `windows-sys`, which we already depend on.
10. **Memory.**
    - **On the web:** a 32-bit WASM build is limited to about 4 GB (64-bit WASM is not ready in our tools), so very large models must be tested in the browser.
    - **Natively:** the undo history copies the whole model at each step.
    - **Plan:** Groups & Components shares unchanged parts between steps (`docs/research/groups-and-components.md`).
11. **Snapping on big models.**
    - The Large Scene takes 14.6 ms per inference today, and Groups, Smart Guides and NURBS will add more candidates.
    - **Plan:** add a spatial index (a BVH over edges, vertices and instances) before or with Groups & Components, and measure it against the 16 ms target (DECISIONS #8).
12. **The denoiser.**
    - Natively, the `oidn` crate (MIT) binds Intel's Open Image Denoise, a C++ library (Apache-2.0). That means a C++ build and large binaries in CI.
    - On the web it needs a WebGPU port.
    - **Plan:** when the denoiser part is written, compare that with a small denoiser of our own on the GPU (for example, à-trous filtering guided by albedo and normals).

## Sources
- WebGPU status: https://github.com/gpuweb/gpuweb/wiki/Implementation-Status, https://appdevelopermagazine.com/webgpu-in-ios-26/, https://www.firefox.com/ia/firefox/notes
- wgpu WebGL2 limits: https://docs.rs/wgpu/latest/x86_64-apple-darwin/wgpu/struct.Limits.html
- Threads on GitHub Pages: https://developers.zoom.us/docs/video-sdk/web/sharedarraybuffer/, https://docs.wasmer.io/javascript-sdk/how-to/coop-coep-headers
- NURBS kernels: https://docs.rs/monstertruck, https://github.com/ricosjp/truck, https://docs.rs/crate/curvo/0.1.86/source/README.md
- Code signing: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options, https://zettlr.com/post/selling-trust-part-3
- Crate data: the crates.io API, 2026-11-06.
