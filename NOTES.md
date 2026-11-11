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

**Start here (2026-11-06):** read `docs/HANDOFF.md`: where the project is, what comes next, and how the owner works.

**Research for the next chunks (2026-11-06):** read `docs/research/files-and-formats.md` (the next chunk) and `docs/research/groups-and-components.md` (the one after). The first has one fix to make before saving is built: turn on serde_json's `float_roundtrip`, or about 11% of coordinates come back from JSON one bit off.

The browser prototype was updated while the Soft Theme chunk was being built. Use the latest `docs/prototype/Main.dc.html`, and read `docs/prototype/README.md` for where things are.

1. **Buttons and menus:** the prototype's action table, menu bar, tool dock, icons, right-click menu and Planned Features list are the design for the Actions & Keys chunk. See "Pick up the prototype's buttons and menus" in `docs/briefs/03-actions-and-keys.md`.
2. **Smart Guides:** use the new system in `PLAN.md` section 5.1 and the prototype functions `smartInit(`, `smartSnap(`, `smartDraw(`. The rules to keep: alignment and equal spacing between whole objects, snap pull of about 8 px, guides only on axes the object is moving along, the centre-alignment line only once centres truly line up, a per-frame time budget (2 ms) that drops spacing guides first, and Ctrl+U to toggle. This is built in the Inference & Drawing chunk once groups exist (Groups & Components), not before. Until then only the action `view.smart` goes in the registry (greyed out).
3. **Box select:** window (left to right, fully inside) and crossing (right to left, touching), Shift toggles, Ctrl adds. A drag that starts on geometry is still a box. See `commitBox(` in the prototype. It belongs with the selection code when the Select tool is built.
4. **Soft Theme:** now merged or on its branch (`prism-ui`). Build the menus and dock from those parts.

## Actions & Keys

**What was built**
- **One registry** (`crates/prism-app/src/actions/`). The prototype's action table, menus, dock groups, icons and behaviour list are copied by `scripts/import_prototype.js` into `actions/table.rs` (generated; run the script again if the prototype's tables change). The script also makes the documented Rust-side changes: no Smart Guides demo scene; new Document Settings, Pen and tablet input, a greyed Scripting menu (Script Console, Run Script, Scripting Reference), and Debug Panel and Theme Demo toggles in Help. Arc and Primitives got their own rows in their submenus, so every action has a menu path.
- **How actions run in Rust:** tools (Select, Line, Rectangle, Push/Pull, Move, Rotate, Scale, Eraser, Orbit, Pan) can be picked and show the active dot, but do nothing yet. The prototype's other working commands (New, Undo, Redo, Delete, Select All, Deselect All, Zoom Extents, Ground Shadow) send events that later chunks will handle; Undo, Redo, Delete and Deselect stay greyed until there is something to act on. Built here: Planned Features, Command Search, Keyboard Shortcuts (also Shortcuts Reference), Debug Panel, Theme Demo. Everything else is greyed and says "Not built yet" on hover. Smart Guides (Ctrl+U) is greyed until Inference & Drawing, as the notes above ask.
- **Generated from the registry:** the menu bar (with submenus, check marks and keys), the tool dock (two columns, key hints, active dot, one tooltip that slides beside the dock), the right-click menu (as `onContext`: empty space, faces, edges), command search (Shift+S, fuzzy), the Keyboard Shortcuts window and Planned Features ("7 of 209 built": built means built in Rust, not in the prototype).
- **Keymap engine** (`actions/keymap.rs`): bindings by physical key (`KeyboardEvent.code` names), "Mod" is Ctrl on PC and Cmd on Mac and shows that way in menus. Presets: SketchUp (the default, plus Ctrl+Shift+Z for Redo and Ctrl+Shift+A for Deselect from PLAN 1.6), Blender and Fusion (empty). Stored as JSON: natively in the config folder (`%APPDATA%\Prism`, `~/Library/Application Support/Prism`), on the web in local storage. Export and Import: natively a file `Prism keymap.json` in Downloads; on the web a download and a file picker.
- **Keyboard Shortcuts window:** search by name or key, primary and alternate key per action, mode column, a dot and Reset where a key differs from the preset, Reset all, preset choice. Click a key to record: Esc cancels, Backspace clears. A taken key names the other action and offers Replace, Keep both (greyed: only for different modes, and modes are not built) or Cancel. Keys the browser takes (Ctrl+T, Ctrl+N, Ctrl+W, Ctrl+Tab, F-keys) warn on the web; Cmd+Q, Cmd+H, Cmd+M warn on a Mac and Alt+F4 on Windows. A Modifiers tab sets what Shift, Ctrl and Alt mean in tools (Option and Cmd on a Mac, PLAN 1.5) and warns when two meanings share a key.
- **Esc rule** (`actions/esc.rs`): cancel an operation (none exist yet), else close the topmost menu, search or window, else exit a group (none yet).
- **Dock width** is worked out from the buttons, gaps and padding (116 points, as the prototype), and its thin scroll bar floats inside the padding and shows only while the pointer is over the dock, so there is never a horizontal scroll bar (the owner saw one in the prototype). A compile-time check keeps the bar narrower than the padding.
- Screenshots at the same size, Edit menu open: `docs/screenshots/actions-prototype.png` and `docs/screenshots/actions-prism.png`.

**Checked here** (Linux, software renderer, and headless Chrome for the web build)
- Menus open from the bar, switch on hover, show submenus, greyed rows and keys; the right-click menu opens in the viewport, and the browser's own menu does not appear on the web.
- L, R and other tool keys pick tools; Shift+S opens search, typing filters it; Esc closes things one at a time.
- Recording Ctrl+N for Save shows "Ctrl+N is already used by New" with the three choices.
- Tests: every action has a menu path; menus, dock, context menu and icons name only registered actions; no key is bound twice in the SketchUp preset; the same physical key fires on QWERTY and AZERTY; Cmd is the primary key on a Mac; conflicts, Replace and Reset; JSON round trip; reserved keys; the Esc order; search ranking; every icon reads. 40 tests in `prism-app`, 25 in `prism-ui`.
- The web build is now 8.2 MB (was 7.8 MB).

**Decisions and defaults used**
- The prototype's Feel Panel (`help.feel`) is its own tuning panel, so it stays greyed; the Rust app's Debug Panel and Theme Demo are separate actions. Both start hidden.
- The window background is now the surface colour, and the viewport is a sunken well inside the frame, as in the prototype. The renderer will draw into the well.
- Greyed menu rows say why with egui's own tooltip, themed through `Visuals`; the dock uses the kit's sliding tooltip.
- Modes are not built, so every action applies in "All modes" and "Keep both" is greyed.

**Still to check by a person**
- Windows and macOS: menus, dock, search and the shortcuts window; that Cmd shows on the Mac and Cmd+S, Cmd+Z fire there.
- Export and import of the keymap on each platform (natively the file is `Prism keymap.json` in Downloads).

**Leftovers and ideas**
- Not built from PLAN 1.12: the Mouse and navigation tab, printing a cheat sheet, and the Mac system menu bar (out of scope here).
- Menus open by mouse, and by arrows once open; there is no key to open the menu bar itself (Alt or F10). Worth adding with the Mac menu bar.
- The right-click menu shows the empty-space list until there is a model to select; the face and edge lists are in place and tested.
- Blender and Fusion presets are empty, as the brief says.
- Native import reads a fixed file name in Downloads. A proper file dialog needs a dependency (for example `rfd`); consider it with Files & Formats.

### Drawing order bug found in the prototype (2026-11-05)
A pocket cut into a box (a face with a hole, pushed down) showed the near inner wall, drawn in the back-face blue, painted over the rim at some camera angles. Cause: the prototype's 2D canvas draws faces back to front by centroid distance, which is wrong for faces with holes. The real renderer uses a depth buffer, so it does not have this problem, but keep the case as a **screenshot test** in the Viewport chunk: a box with a pocket (hole ratios 0.3 to 0.9, depths 1.2 to 1.9) seen from every 6 degrees of azimuth and several elevations must never show a back face of a closed solid from outside. Prototype fix for reference: `faceOrder(` (Newell's plane tests) and `closedCull(` (back faces of watertight solids are skipped when the eye is outside).

## Core Model

**What was built**
- Five new crates, none of which depend on a window, GPU, clock or randomness (a test checks the manifests and sources):
  - `prism-math`: f64 vectors (glam), the one relative tolerance `Tol` (1e-6 of the model's extent, clamped between 1e-12 and 1e-2, plus one angle tolerance), exact orientation predicates (`robust`), point in polygon, planes, Newell normals, 2D frames on a plane.
  - `prism-geom`: `Curve::Line` and `Surface::Plane`, as enums ready for arcs and NURBS.
  - `prism-topo`: vertices, edges and faces with holes, in ordered maps, with ids that are never reused (counters only go up, also across undo). A face owns its plane. Saved as deterministic JSON.
  - `prism-ops`: sticky geometry, overlap split, holes, Push/Pull, move, rotate, scale, weld, erase, duplicate, connected, and `check` (the rules from `core-model-acceptance.md`).
  - `prism-doc`: the document; every edit is a `Command` in a history with exact undo and redo (snapshots of the model before and after), plus replay of the log.
- **Faces come from tracing the plane's graph**, not from a search: every edge in the plane, half-edges sorted by angle with exact predicates, traced into regions with their islands as holes. A region becomes a face if an existing face covered it, or if its outer loop uses an edge just added. Faces whose region did not change keep their ids. There is no limit on loop size (a test makes a 40-sided face; the prototype gave up at 14).
- **Planes to rebuild after a segment:** the planes of faces the segment lies in, and the plane it makes with each edge at its vertices.
- **Transforms** refit each touched face's plane. A face that is no longer planar is folded along a diagonal into planar pieces (a test lifts one corner of a rectangle: two faces, not triangles). A rotated or scaled box keeps all six faces whole. Vertices that end up together are welded.

**Tests:** all 18 verified golden cases pass at scales 1e-4, 1 and 1e3, with every rule checked on each result. Property tests (256 random cases each, and 3000 when run by hand) check: the rules hold, coverage and area, adding again changes nothing, order does not matter, scale does not matter, deterministic output, faces stay planar after rotate and scale, and undo then redo of any sequence is exact. 113 tests in the workspace.

**Decisions and defaults used**
- **`pushpull_ring_face`:** decided by the owner (DECISIONS #14): the pulled face is the top and the bottom stays open, unless Alt keeps the original. Prism already did this (16 vertices, 24 edges, 10 faces), and the case is now verified and run with the others.
- **Erase:** decided by the owner (DECISIONS #15): erasing an edge deletes both faces that share it; erasing a hole's edge closes the hole. That is what Prism does; PLAN 1.4 is updated to match.
- **A drawn ring encloses a face:** rectangles that together enclose an empty region make it a face, as any closed coplanar loop does (found by the property tests).
- **Push/Pull does not run sticky geometry afterwards**, as in the prototype, so a wall can overlap a coplanar face next to it. Worth adding with the Push/Pull tool.
- **A face with holes that becomes non-planar** is put back onto its best-fit plane (its vertices move slightly), because folding along one diagonal does not work with holes.
- Undo uses whole-model snapshots: simple and exact. A persistent data structure would make them cheaper for big models.

**Leftovers and ideas**
- Lookups scan all vertices and edges (`edge_between`, `insert_point`), so drawing is O(n) per point. Fine for now; a spatial index and an edge map are needed before large models (DECISIONS #8).
- Loops that touch themselves at one vertex (two squares meeting at a corner) are traced as one pinched loop. Not covered by the golden cases.
- `dup` and `connected` follow the prototype. A face selected without its edges still gets its edges copied, as there.

## Viewport

**Checked by the owner (2026-11-05)**
- The release build works. The Large Scene takes a few seconds to load, then orbits and zooms fast, with no flicker or lag. Zooming into it dips to about 40 fps for a moment, which the owner does not notice.
- The defaults below are accepted. O, H and Z staying active after the mouse is released, with Esc to go back, is how they should work (DECISIONS #16; PLAN 1.5a updated).

**What was built**
- A new crate, `prism-render`. Its pure parts use no GPU or clock and are tested on their own: the camera (`camera.rs`), the display triangulator (`tess.rs`, our own ear clipping with bridged holes and exact predicates, as the owner chose), the visible scene that drawing and picking share (`scene.rs`, the one place hidden objects will be left out), picking (`pick.rs`), the Select tool's clicks (`clicks.rs`) and the viewport's colours (`style.rs`, taken from the feel test). `gpu.rs` and `viewport.wgsl` draw with wgpu.
- **Drawing:** sky and ground gradients, the 1 m grid fading with distance, axes (dashed on the negative side), faces lit as in the feel test with the back colour on reversed faces, the ground shadow, 1.5 pt anti-aliased edges, hover (orange) and selection (blue, with the screen-space dot stipple, and vertex dots). Profile lines and hidden lines (dashed or faint) are in the renderer and set from the debug panel; both start off, as in the feel test.
- **Depth:** reversed-Z in a 32-bit float buffer, with the far plane at infinity and the near plane fitted every frame to the model, grid and axes. (A fitted far plane failed the thin-plate test at 1 km on Windows and macOS CI: depth then comes from subtracting two nearly equal numbers. Infinite far does not.) **No line bias at all:** a second pass records which face is nearest at each pixel and its depth. An edge is never hidden by its own faces, and any other face hides it if it is nearer at the edge's own centre-line point (worked out exactly from the face's depth gradient). So edges never flicker against their faces, and lines behind a 0.5 mm plate stay hidden at 1 km when they are at least 1e-5 of the distance behind it (1 cm at 1 km, 1 mm at 100 m). **Limit:** 32-bit depth resolves about 1e-7 of the distance, but the Windows and macOS CI GPUs round line depths by several steps more, so a box only 1.5 mm behind a plate seen from 1 km (13 depth steps) showed its edges there. Faces are not affected; only lines that close behind a wall that far away. Positions are sent as two f32 parts and taken relative to the eye on the GPU.
- **Navigation** (PLAN 1.5a as changed in 1.8): the wheel zooms 12% per notch toward the surface under the cursor and stops short of it; middle-drag orbits about the point under the cursor; Shift+middle-drag pans with the grabbed point under the cursor; double middle-click re-centres; Orbit (O), Pan (H) and Zoom (Z, drag up to zoom in) are tools on left-drag, and Esc returns to the tool before; Shift+Z is Zoom Extents; two-finger trackpad scroll pans and pinch zooms. Wheel zoom and Zoom Extents ease briefly (off with reduce motion, or from the debug panel).
- **Select tool:** click, double-click (face and its outer edges) and triple-click (everything connected), within 350 ms and 6 px, as in the feel test. Right-click selects first, and the context menu now shows the face and edge lists. Select All and Deselect All work.
- **Help > Test Scenes:** Cube (the start), Box with a Pocket, Thin Plate, Large Scene (2,500 boxes). File > New empties the model.
- The debug panel has the pick aperture, profile lines, hidden lines and ease settings, the model's counts, and the last pick and scene-build times.

**Tests:** camera (zoom never reaches the surface and keeps the point under the cursor, pan keeps the grabbed point, orbit keeps the pivot and a level horizon, Zoom Extents fits wide and tall windows, all at three scales), picking (vertex over edge over face, hidden things not picked, aperture), triangulation of every golden-case model at three scales (areas add up, no triangle in a hole, all facing the right way, a 1,000-corner face with 36 holes), clicks, and three screenshot tests on the GPU: the pocket from 2,700 views never shows a back face, the thin plate at 5 m, 100 m and 1 km shows nothing behind it (detail 1 mm, 1 mm and 1 cm behind), and a 1.5 pt edge is 1.5 pt wide at 1x and 2x. CI installs Mesa's software renderer for them.

**Measured here** (Linux, CPU software renderer, so a real GPU is much faster)
- Large scene, 15,000 faces and 30,000 edges: scene built in 43 ms (only when the model changes), a pick in under 1 ms, an orbit frame at 1200x800 in about 100 ms including reading the picture back. To measure: `cargo test --release -p prism-render --test screens timing -- --ignored --nocapture`.
- Idle: 0 CPU ticks over 5 seconds with the cube on screen.
- Web: works on WebGL2 in headless Chrome (SwiftShader). The release wasm is 8.7 MB, 3.1 MB gzipped (was 8.2 MB).

**Decisions and defaults used**
- **Undo, Redo and Delete are wired** to the document (Delete erases the selection as one undoable command), because a selection made Delete enabled and it would otherwise do nothing. Not asked for in the brief; easy to take out.
- **Selected vertices hidden behind a face show no dot.** The feel test draws them anyway; a dot showing through a solid looked wrong.
- **O, H and Z** are normal tools, and Esc goes back (confirmed by the owner, DECISIONS #16).
- The camera never comes closer than 100 model tolerances to the surface, so zooming in cannot get lost in rounding.
- The prototype's ground and grid sizes are absolute (1 m grid, 30 m out). Sub-millimetre models show no grid; View Settings can scale it later.
- Fixed in Core Model while here: `Model::edge_faces` looked each edge up with a scan, which took 2.3 s on the large scene. It now uses a map (43 ms); the results are the same.

**Still to check by a person**
- macOS: the trackpad (two-finger pan direction, pinch), if not yet tried.
- A number for the frame time while orbiting the Large Scene on the reference machine (DECISIONS #8), from the debug panel with continuous redraw on.

**Leftovers and ideas**
- Clicks are timed by frame, so on a very slow software renderer a quick double-click can count as two single clicks. Fine at normal frame rates.
- Picking and the scene build scan everything; a spatial index comes with larger models (DECISIONS #8).
- The Large Scene's few seconds of loading are in building the model: `Model::add_edge` looks for an existing edge by scanning all edges, so adding 30,000 edges is slow. An edge map in `prism-topo` fixes it, with the spatial index.
- Dashes on a line that is cut at the screen's edge or the near plane restart from the cut, so they can shift while orbiting.
- Parallel Projection, Standard Views, Zoom Window, Field of View, Walk and Look Around stay greyed.

## Inference & Drawing

**Checked by the owner (2026-11-06)**
- Waking points, line locks and the measurement box feel good.
- Asked for: points in box select, Shift with double and triple click, and greyed-out items to match what is built. Done (DECISIONS #20, #21).

**What was built**
- **Three new pure crates** (in the purity test with the core crates):
  - `prism-view`: the viewport's pure parts, moved out of `prism-render` so the engine and tools can use them without the GPU (camera, scene, picking, triangulation, clicks, style), plus the description of tool feedback (`overlay.rs`).
  - `prism-infer`: the inference engine as one pure function, `infer(scene, view, cursor, context, lock, memory, settings)`. It follows the feel test: Endpoint 6/14 px, Guide Cross 5.5/12, Midpoint 5/12, Intersection 4.6/12, Guide 3.3/8, On Edge 3/9, then On Face and On Ground; score = priority − distance/radius; the held snap stays to 1.6× its radius and gives way only to a score 0.5 better. Woken points (220 ms hover, at most 5) cast axis guides; woken edges cast "Parallel to Edge" from the start point; guides cross each other and edges. Locks: Shift on an axis guide, Up/Right/Left for blue/red/green, Down releases (DECISIONS #17); while locked, points project onto the axis. Pixel radii and the model's relative tolerance replace the prototype's absolute numbers, and the time is passed in.
  - `prism-tools`: the Line, Rectangle and Select tools, the measurement box's units, the test scenes, and a **session** that runs the document, camera, tools, inference memory, selection and typing from a stream of input events. The app only turns egui's mouse and keys into those events.
- **Line (L):** click, click; lines chain and stop when one closes a face; Esc stops; a typed length goes along the rubber band or the locked axis.
- **Rectangle (R):** on the locked axis's plane, the face under the cursor, the ground, or the view's main plane; sides along the world axes where possible; typed `w,h` or one number for a square; Enter after the first click reuses the last size.
- **Retyping the last operation:** after a rectangle, or after a line has stopped (Esc or a closed face), typing a new value and Enter changes it (undo plus the new command).
- **Measurement box** in the status bar (a Soft well, Plex Mono 14, accent colour while typing), with a lock chip in the axis colour beside it, and the tool's hint on the left. Units: mm, cm, m, in or `"`, ft or `'`, and `2'6"`. Separator `,` (or `;`), with `;` only when the decimal mark is `,`.
- **Select (Space):** Shift toggles, Ctrl (Cmd) adds; window (left to right) and crossing (right to left) boxes, with Shift and Ctrl; a press on geometry that drags is still a box.
- **Feedback** in the feel test's look: glyph shapes and colours, the snap target's glow and ring, dashed guides, woken diamonds, the rubber band with its length, the tinted rectangle preview with its size, and label pills.
- **Help > Record Session** records the mouse and keys with the starting model and camera; picking it again saves `Prism session.json` (natively in Downloads, on the web as a download). Every file in `docs/acceptance/sessions/` is replayed by a test, which checks the model it ends with. One session is there now.
- The debug panel has the Feel Panel sliders: pick aperture, snap release, hover wake. Planned Features now counts the tools and commands that work.
- `prism-doc` got a `Loop` command: a rectangle on any plane in one undo step.

**Tests:** 12 for the engine (each kind at three scales, hidden points, hysteresis to 1.6×, start-point guides, woken guides and their cross, at most 5 woken, guide-edge intersections, locks, excluded vertices, and a property test that a sub-pixel jitter never flickers between snaps), 4 for units, 11 scenario tests driving the session (a square from four lines makes a face and the line stops; a typed length on a locked axis is exact; rectangles by clicks and typed size, undone in one step; retyping the last rectangle and line; a line across a face splits it; waking a corner and drawing to where the guides cross; Shift lock; box select window and crossing with Shift and Ctrl; Shift and Ctrl clicks; record and replay; saved sessions), and a screenshot of every glyph.

**Measured here** (release build, Linux)
- Inference on the Large Scene (15,000 faces, 30,000 edges) with a start point: **14.6 ms per mouse move**, inside the 16 ms target but close. Most of it is scanning every vertex and edge and the occlusion checks; a spatial index (DECISIONS #8) is the fix when models get bigger. To measure: `cargo test --release -p prism-tools --test timing -- --ignored --nocapture`.
- Web: works on WebGL2 in headless Chrome (`docs/screenshots/drawing-web.png`). The release wasm is 8.8 MB.

**Decisions and defaults used**
- Down releases the lock and only "Parallel to Edge" guides exist (DECISIONS #17, #19). Up in the Rotate tool will lock the protractor to a face's plane (DECISIONS #18), for Edit Tools.
- Label text is drawn by egui over the viewport picture (in the viewport's own colours); the glyphs, guides and previews are drawn by the renderer. A text renderer in `prism-render` did not seem worth it for labels.
- Box select picks points as well as edges and faces (the owner's decision, DECISIONS #21): points are the glue between them. Moving an edge or face always moves its points too, so nothing falls apart either way.
- Shift or Ctrl with a double or triple click works from the selection as it was before the clicks, so other selections stay (DECISIONS #21). "Connected" now crosses faces with holes, so triple-clicking the pocket box takes the pocket too.
- Only built tools and commands are active; the rest are greyed (DECISIONS #20). Tools come from `prism_tools::session::TOOLS`, so Push/Pull, Move, Rotate, Scale and Eraser are greyed again until Edit Tools builds them.
- Recorded sessions save natively to the Downloads folder (or the home folder) as `Prism session.json`, then `Prism session 2.json` and so on; on the web the browser downloads them. The status bar says where.
- Retyping a line's length works once the line has stopped. While still chaining, typing applies to the next segment, as in the feel test.
- The decimal mark is assumed to be `.` (Rust has no portable way to read it). Document Settings can set it later.
- Typing goes to the measurement box before shortcuts once a step is in progress or there is a last operation, so `2.5m` is a length, not the Move tool.
- The Smart Guides demo and Smart Guides themselves wait for groups.

**Still to check by a person**
- The feel: drawing a box from lines and rectangles, waking points, guides, Shift and arrow locks, typed values, box select. Natively and on the web.
- If something feels off, Help > Record Session, reproduce it, pick it again, and send the file.

**Leftovers and ideas**
- Inference is a brute-force scan (see the timing above).
- While a line is chaining, there is no way to retype the previous segment (SketchUp has one).
- Measurement box arithmetic and `[x,y,z]` / `<dx,dy,dz>` coordinates are not built (out of scope).

## Edit Tools (part 1)

**What was built** (brief 06), all in the pure session in `prism-tools` (`session/edit.rs`):
- **Push/Pull (P):** the face under the cursor is tinted. Click it, then pull and click, or type a distance and press Enter. A typed minus pushes the other way.
  - The pull snaps to an endpoint's height ("Match Endpoint", with a magenta guide), to the ground, and to zero within 6 px.
  - **Alt** toggles new loop.
  - **Double-click** repeats the last distance.
  - Typing right after a pull changes it.
- **Move (M):** click a point, move, click, with inference (the moving points are left out of snapping) and with locks.
  - With nothing selected, it moves the item under the cursor; at a corner, that is the point alone, as in SketchUp.
  - **Ctrl** (Option on a Mac) toggles copy. After a copy, `x5` makes an array of five and `/5` puts five evenly between.
  - Typing right after a move changes it.
- **Rotate (Q):** the full-circle protractor shows on the plane of the face under the cursor, the ground, or the view's main plane. Click the centre, then either click a start direction or drag from the centre and release. Move, then click.
  - **Up** locks the protractor to the face under it (DECISIONS #18), and it can then be moved anywhere; "Plane locked" shows by it. **Right** and **Left** lock red and green, and **Down** releases.
  - Angles snap softly to 15°, or to 45° steps while **Shift** is held.
  - **Ctrl** copies. Type degrees and press Enter. Typing right after a rotation changes it.
  - With nothing selected, it rotates the item under the cursor alone, as in SketchUp (DECISIONS #26, changed after the owner tested). A hovered face twists, and the faces attached to it fold into flat pieces.
- **Scale (S):** uniform, about the selection's box centre, from where you click. The factor is clamped between 0.05 and 20 and snaps to 1 within 2%. Type a factor and press Enter.
- **Eraser (E):** click, or press and drag over edges. They show red, and are erased with the faces they bound on release (DECISIONS #15).
- **Previews:** each step previews on a copy of the model, drawn instead of the document until it is finished; Esc cancels it and the tool stays.
- **Commands:** each step is one command. A new `Copy` command (duplicate and move, or duplicate and rotate, any number of times) makes copies and arrays one undo step.
- **Active tools:** these five are now active in the dock and menus (DECISIONS #20).

**Tests:** 13 new scenario tests:
- Push/Pull: typed distance and retype, double-click repeat, Alt new loop on the cube's top, matching an endpoint's height.
- Move: an exact typed move, a copy, an `x3` array, a `/4` division and one undo for the whole array.
- Rotate: typed 90° and retype, the drag-set start arm, Up's plane lock on the cube and moving the protractor away from the face, a rotated copy.
- Scale: a typed factor.
- Eraser: a drag that takes both faces beside an edge.
- Esc: cancelling a step leaves the model as it was.

`prism-doc`'s undo test covers the new `Copy` command, and a recorded session, `docs/acceptance/sessions/small-building.json` (a box, a ridge moved up 1.5 m for a roof, and a window pushed 0.2 m into a wall), replays in CI. Screenshot: `docs/screenshots/edit-tools-prism.png`.

**Decisions and defaults used**
- Ctrl and Alt are taps that toggle, as in the feel test. Ctrl toggles copy only while moving or rotating, so shortcuts such as Ctrl+Z do not flip it.
- With nothing selected, Rotate and Scale take what is connected to the item under the cursor, and Move takes the item itself (the feel test does the same).
- The Rotate arm follows the cursor on the protractor's plane (not inference snaps), as in the feel test.
- The small-building session does not copy the window yet: copying a face that is a hole in a wall leaves the copy unattached to the wall, which needs sticky geometry after Move (a leftover below).

**Leftovers and ideas**
- After Move and Push/Pull, the moved geometry does not merge with what it now touches (the feel test does not either). Merging coplanar faces after an edit (PLAN 1.9, "when the check runs") is the next step for this.
- The Eraser samples where the mouse is; a fast drag can step over an edge. Testing the path between samples would fix it.
- Previews rebuild the whole scene each mouse move: fine for small models, slow on the Large Scene (about 40 ms per move).
- Shift (hide) and Ctrl (soften) in the Eraser wait for hiding and soft edges.

## Edit Tools (part 2)

Brief: `docs/briefs/07-edit-tools-2.md`. Built: Offset, Tape Measure, Protractor, and guides stored in the model.

**What to try**
- **Offset (F):** click a face, move in or out, and click; or type a distance and press Enter.
  - Every loop moves, holes included, with mitred corners.
  - It stops just short if the outline would cross itself or meet a hole, so the result is always valid.
  - Double-click a face to repeat the last distance. Typing right after an offset changes it.
  - With one face selected, it offsets that face wherever you click.
- **Tape Measure (T):**
  - Start on an edge, and it makes a guide line parallel to it, measured square to the edge.
  - Start on a point, and it makes a guide point.
  - **Ctrl** (Option on a Mac) switches to measuring only.
  - Arrows lock an axis. Type a length and press Enter; typing right after changes it.
  - Double-click an edge for a guide along it, moved the last tape distance towards its face.
- **Protractor:** the Rotate protractor, with the same plane rules and Up, Right, Left and Down locks. Click the centre, set the base direction, then place an angled guide line through the centre. Type degrees to set the angle.
- **Guides:**
  - Drawn dashed and dark grey. Faces in front hide them, as they hide edges.
  - Every drawing and editing tool snaps to them: "On Guide", "Guide Point", and where they cross each other, axis guides and edges.
  - The Eraser erases them with the edges in the same drag, as one undo step.
  - **View > Guides** hides them. Hidden guides are left out of the scene, so inference never looks at them (PLAN 1.14).

**Tests**
- `prism-ops/tests/offset.rs`:
  - a square in and out, an L shape, a face with a hole, a tilted wall;
  - the clamp, and a hole meeting the outline;
  - a property test that convex offsets stay inside and parallel.
- `prism-doc`: Offset, adding a guide and erasing one undo and redo exactly (16 kinds now).
- `prism-infer`: a stored guide line, a crossing of two guides, a guide crossing an edge, a guide point, and hidden guides.
- Scenario tests (10 new):
  - Offset: typed and retyped, by mouse in and out, double-click repeat.
  - Tape: a parallel guide, its snaps and the double-click guide; a guide point; measure only.
  - Protractor: a typed angle and retype.
  - Eraser: a guide and an edge in one step.
  - Hiding guides.
- GPU: `faces_hide_guides` checks a guide through the cube is hidden inside it.
- A recorded session, `docs/acceptance/sessions/window-from-guides.json`, replays in CI. It covers:
  - two tape guides on a wall;
  - a 1.5 by 1.2 m window drawn from where they cross, pushed in;
  - the roof offset 0.3 m.

**Decisions and defaults used** (none were open in DECISIONS.md)
- Guides live in the model (`prism_topo::Guide`), so undo, saving and replay cover them with no extra work. They are not geometry: edges do not stick to them.
- Offset's distance is how much nearer to or further from the outline the cursor has moved since the click, so a click away from the edge does not jump. Clicking near the edge, as in SketchUp, gives the same result.
- Offset's clamp searches for the largest valid distance, to within the tolerance, and stops 10 tolerances short of touching.
- A tape started on an edge's midpoint snaps to the midpoint, so it makes a guide point. Start elsewhere on the edge for a parallel guide.
- The Protractor has no hotkey; SketchUp has none either.
- Rectangle now takes its plane from the face under the cursor when it starts on a guide or an edge that lies in that face. Before, it used the view's main plane there.

**Leftovers and ideas**
- Guides cannot be selected or moved yet, and there is no Edit > Delete Guides.
- Move and Rotate do not take guides with the geometry.
- Offset works on faces only. Edge chains wait for arcs.
- Tape Measure's "type a length to resize the model" is not built.
- Screenshot (web build, WebGL2): `docs/screenshots/edit-tools-2-web.png`, with the cube's top offset, a vertical tape guide and a guide point.
- A guide line is drawn 60 times the scene's size each way, not to infinity. This is far enough to look endless, but a guide could end on screen when zoomed far out from a tiny model.

## Files & Formats (part 1)

Brief: `docs/briefs/08-files-and-formats.md`. Research: `docs/research/files-and-formats.md`.

**What to try**
- **File > New, Open…, Save, Save As…** (Ctrl+N, O, S, Shift+S).
  - The title shows the file's name, with a dot while there are unsaved changes.
  - New, Open and closing the window ask "Save changes?" first. On the web, the browser warns before the tab closes.
- **Saving natively** is atomic: a temporary file, then a rename. The previous file is kept as `name.prism.bak`.
- **Saving on the web:**
  - Chrome and Edge save in place, through the File System Access API.
  - Other browsers download a copy.
- **File > Open Recent** lists the last 10 files (native only).
- **Autosave:** every 10 seconds while there are unsaved changes, Prism writes a recovery journal. The journal is the model at the last save plus the commands since. Natively it goes in the config folder's `recovery` folder; on the web, in IndexedDB. Start Prism after a crash, or after killing it, and it offers "Restore your work?".
- **File > Document Settings…:** the display unit (mm, cm, m, inches, feet and inches) and decimals. Typing still accepts any unit, including `2' 6 1/2"`.
- **Command line:** `prism model.prism` opens a file. The Mac app declares the `.prism` type, but double-clicking in Finder needs an Apple event handler, which waits for packaging (DECISIONS #24).
- **Rotate (owner's change):** with nothing selected, Rotate turns the item under the cursor alone, as in SketchUp (DECISIONS #26). A hovered face twists, and the faces attached to it fold.

**Tests**
- `prism-file` (8 tests):
  - exact round trips of every test scene, the Large Scene and a rotated model with guides;
  - the same document gives the same bytes;
  - f64 values come back from JSON bit for bit;
  - unknown fields survive;
  - a newer schema is refused, and a made-up schema 0 migrates;
  - empty, cut-short and damaged files are refused, including a property test that flips random bytes;
  - the recovery journal replays.
- Golden files: `docs/acceptance/files/` (3 files). CI opens each, checks it, saves it again and compares the bytes.
- `prism-app`: the atomic save keeps a `.bak` and no `.tmp`; a failed save leaves the old file; a journal left behind restores the same model; an empty or damaged journal offers nothing.
- Units: every unit shows and reads back; fractions of an inch.
- The web build was tried in headless Chrome: the File menu shows, and the title gets its dot after an edit.

**Decisions and defaults used**
- `.prism`:
  - a zip, deflate only;
  - `manifest.json` and `document.json` (pretty JSON);
  - `geometry/model.bin` (postcard, in its own versioned layout);
  - every zip entry has a fixed timestamp, so the bytes are reproducible.
- Unknown fields are kept only in `document.json`, because postcard has no field names.
- Undoing past the last save starts a new recovery base from the model as it is, so the journal never needs commands that were undone.
- "Don't Save" clears the recovery journal.
- The web build has no recent-files list: a browser cannot reopen a file by path.

**Not tried by hand yet** (please check):
- native save and open dialogs on Windows and macOS;
- "Save changes?" when closing the window;
- recovery after killing the app;
- Save in place in Chrome (headless Chrome cannot show the picker).

**Leftovers**
- Part 2: STL, OBJ, 3MF, glTF and PLY import and export; drag and drop (the web needs its own canvas listeners); Model Health after import.
- A thumbnail in the file.
- Windows file-type registration and the Mac Apple event handler (packaging).
- Recent files on the web would need stored file handles in IndexedDB.
- **Fixed after the owner tested the web build:** the mouse wheel panned instead of zooming. Browsers report a wheel in points, as they do a trackpad. Now a vertical step of 40 points or more with no sideways part counts as a wheel notch (100 points) and zooms. Smaller or sideways steps still pan, as DECISIONS #4 says trackpads do. Natively the wheel already zoomed.
- **Changed after the owner tested (DECISIONS #15):** erasing the edge between two faces on one plane now merges them into one face, as in SketchUp, so the merged face can be offset or pulled. Where faces meet at an angle, erasing their edge still takes both. In `prism_ops::remove`, such an edge is removed on its own and the plane's faces are rebuilt.

## Files & Formats (part 2)

Brief: `docs/briefs/09-files-and-formats-2.md`.

**What to try**
- **File > Import…** (or drop a file on the window): STL, OBJ, 3MF, glTF (GLB, or `.gltf` with embedded buffers) and PLY.
  - The dialog shows the triangle count and the size, and asks for the unit when the file does not say it (STL, OBJ and PLY).
  - "Y is up" is on by default for glTF and OBJ.
  - "Merge coplanar triangles into faces" is on by default: an STL box comes in as 6 faces.
  - The import is one undo step. The view fits it, and the status bar gives the face count.
- **File > Export:** "3MF / STL (3D printing)" or "glTF / OBJ / USD (media, web)".
  - Pick the format (3MF or STL; GLB, OBJ or PLY), the unit, Y up, and "Only the selected faces".
  - The file saves natively, or downloads on the web. The dialog says that a mesh keeps only triangles.
- **Drag and drop:** natively through winit, and on the web through the page's own drop listeners. A dropped `.prism` opens, asking "Save changes?" first; anything else imports.
- Screenshot: `docs/screenshots/import-dialog-web.png`, a dropped STL box on the web build. Imported, it became 6 faces.

**Tests**
- `prism-file/tests/mesh.rs`:
  - round trips through every format for the cube and the box with a pocket (same faces and areas after merging);
  - the same bytes twice;
  - hand-written STL, OBJ (a quad, a pentagon, negative indices) and PLY;
  - a GLB with a node transform;
  - merging (two triangles, a ring with a hole, a bend that stays apart);
  - selection-only export, units and up axis;
  - a damaged-file fuzz test.
- `prism-doc`: undo of `Import` (17 command kinds).
- **Large Scene timing** (15,000 faces, release build): export takes 5–60 ms and import 44–96 ms per format. Import was 1.2 s until `Model::add_new_edge` skipped `add_edge`'s scan for an existing edge.

**Decisions and defaults used**
- STL, OBJ, PLY and GLB are written and read with our own code. Only 3MF uses a crate, `quick-xml`, to read its XML (DECISIONS #25), so `gltf`, `stl_io`, `tobj` and the PLY crates were not needed.
- Imports are added beside the model, not joined to it, as PLAN 1.13 allows.
- Files over 50 MB get a note that they may take a while, instead of a question before reading. Background import waits for threads.
- There is no highlight while a file is dragged over the web page; the native window shows a status note.

**Leftovers**
- Materials and colours in all formats.
- USD.
- Joining imports to existing geometry, which needs Groups & Components or Model Health.
- Recovering exact curves from meshes, and softening edges.
- A progress bar for big imports.
- Exports are triangles even where the format could keep holes.

## Edit Tools (part 3)

Brief: `docs/briefs/10-edit-tools-3.md` (DECISIONS #29).

**What to try**
- **Snapping to crossings:** hover a face that another shape passes through. The lines where they cross show as purple dashed guides, labelled "Face Intersection", and can be snapped along within their length. Their corners, and where they meet edges, snap as Guide Cross or Intersection. This works for connected and unconnected pieces, as the owner asked. Trace one shape's outline onto another with Line or Rectangle.
- **Edit > Intersect Faces > With Model:** every crossing becomes an edge, and the faces split along it. **With Selection:** only crossings that involve a selected face (greyed when nothing is selected). One undo step.
  - For the owner's tunnel: push a shape through a box, run Intersect Faces, then erase the faces inside to open it.

**How:** `prism_ops::intersect`:
1. Intersect the two planes.
2. Clip the line to each face in its own 2D frame, holes included.
3. Keep where both clips overlap, but drop parts on both outlines (their shared edge).

Snapping only checks the face under the cursor against the others, with a bounding-box test first.

**Tests**
- `prism-ops/tests/intersect.rs`:
  - crossing squares, at three scales;
  - a box (no crossings);
  - a face with a hole (two crossings);
  - a wall standing on a floor;
  - the owner's case: a box pushed out through the back of another. Intersect Faces makes the 2 × 1 opening a face of its own, and leaves nothing more to cut.
- `prism-infer`: the crossing guide and its corner, on the same model.
- `prism-doc`: undo of Intersect Faces (18 command kinds).
- A session test runs the menu action as one undo step.
- **Large Scene:** 12.3 ms per inference with crossings (it was 14.6 ms before this chunk), under the 16 ms budget.

**Leftovers**
- Curved faces.
- Intersect within a group (Intersect Faces > With Context stays greyed until Groups).
- Model Health's crossing-faces list, which will reuse `all_crossings`.
- Intersect Faces checks every pair of faces with a bounding-box test, O(n²). That is fine for now; the spatial index (risks item 11) will speed it up.

**Owner feedback: Move was sluggish on a 26,000-triangle STL (macOS build).**
- **Cause:** every mouse move during Move or Rotate copied the model, applied the transform, and rebuilt and re-triangulated the whole scene for the preview. On the Large Scene that took 334 ms per move.
- **Fix:** when the moving geometry is whole faces and edges (a whole body, as the owner moved), the preview moves the drawn scene instead (`Scene::rigidly_moved`): 12 ms per move, 28 times faster.
- **Still slow:**
  - copies, scaling, and moves that reshape faces still rebuild;
  - the GPU gets the whole mesh again each move.
- **Next steps:**
  - draw the moving part with a transform on the GPU, so nothing is re-uploaded;
  - make rebuilds incremental, with the spatial index (`docs/research/risks.md` item 11).
- The owner notes that exact NURBS files should be much lighter than 20,000-triangle meshes. That is the plan's point: an exact face is one face, not thousands of triangles.


## Files & Formats (part 3)

Brief: `docs/briefs/11-files-and-formats-3.md` (DECISIONS #24, #27).

**What to try**
- **Tabs:** a compact strip (22 points) under the menu bar, as wide as the viewport, as the owner asked, after a floating row and a dropdown were turned down.
  - Each tab shows the file's name, a dot while unsaved, and ×. "+" makes a new document.
  - Each tab has its own document, undo, selection, camera, tool step, file and autosave journal.
  - Window > Next Document and Previous Document are Ctrl+Tab and Ctrl+Shift+Tab. The browser keeps those keys for itself on the web, and they can be remapped.
- **What opens where:**
  - File > New, Open, Open Recent and a dropped `.prism` make a new tab.
  - The untitled, unchanged tab Prism starts with is reused.
  - A file that is already open switches to its tab.
- **Closing:**
  - × on a tab asks "Save changes?" for that document only.
  - Closing the window asks each tab with changes in turn; Cancel stops.
  - Closing the last tab leaves one empty tab.
- **Recovery:** one journal per tab (`recovery/<slot>/`, or IndexedDB keys `journal-<slot>`). After a crash, each comes back in its own tab, asking Restore or Discard. Part 1's single journal is read as slot 0.
- **One running Prism:**
  - **Windows and Linux:** a second start hands its files to the first through a local socket (a named pipe on Windows; the `interprocess` crate) and quits. The files open in tabs, and the window comes forward.
  - **macOS:** `application:openURLs:` is added to winit's `WinitApplicationDelegate` at start (`objc2`), so a file double-clicked in Finder or dropped on the Dock icon opens in a tab.
  - Windows still needs "Open With > Prism" until an installer registers `.prism` (packaging).
- Screenshot: `docs/screenshots/tabs-web.png`, two tabs on the web build.

**Tests**
- `tabs.rs` (5):
  - an untitled tab is reused, then new tabs are made;
  - opening an open file switches to it;
  - each tab keeps its own document and undo;
  - closing asks only that tab, and the last tab stays;
  - closing the window closes unchanged tabs and stops to ask an edited one;
  - two journals left behind become two tabs.
- `recovery.rs`: journals in two slots restore one tab each.
- `instance.rs`: a second start's files reach the listening first (a real local socket, in one process).
- Built and linted for macOS and Windows from this container (`cargo clippy --target aarch64-apple-darwin` and `x86_64-pc-windows-msvc`). The double-click itself needs the owner's hands.
- Tried on the web build: "+" makes an empty tab, a rectangle drawn there leaves the first tab's cube alone, and switching back redraws everything.

**Decisions and defaults used**
- On the web, the "save in place" file handle is kept per tab, so saving one tab never writes another's file.
- A tab writes its recovery journal when you switch away from it, so a parked tab never waits ten seconds.

**Leftovers**
- Reordering tabs by dragging; more than one window.
- Copy and paste between tabs (no clipboard yet).
- The Windows installer and file-type registration.
- A double-click while the Mac app is starting: the handler is added once the event loop exists, which should be before macOS sends the file, but it is untested.

### Erase merge with two pulled boxes (owner feedback)

Two rectangles sharing an edge, both pulled up, then erasing the vertical
edge where their fronts line up deleted both fronts. Two causes, both fixed:
Push/Pull made new points on top of existing ones (now `weld_new` reuses
them, checking only the new points), and the merge needed the edge to have
exactly two faces (now: exactly two of its faces on one plane merge, and
others on that edge, such as the wall between the boxes, go with it).

### Push/Pull snaps to parallel faces, and drag-release (owner feedback)

- While pulling, hovering a face parallel to the pulled one pulls level
  with it ("On Face"), e.g. a lower stair step up to the next.
- Press, drag and release commits on release; click, move, click still works.
- Left over: two boxes of different heights sharing a wall leave two
  overlapping wall faces (the check reports it). The owner asked for one
  rule for coplanar faces everywhere: after any edit, faces on one plane
  that touch or overlap are rebuilt as one set of regions (as the Eraser
  merge and drawing already do). Do this with Groups & Components, or
  before it.

### Push/Pull edge by edge, and the coplanar rule (owner feedback, 2026-11-08)

- **Trench when pushing a ring back:** Push/Pull only stretched faces with
  no holes, so a ring (a face with a hole) always extruded and left its old
  walls behind. Now it decides edge by edge: an edge whose other faces run
  along the push and lie behind the face slides (those faces stretch);
  every other edge gets a wall. The ring's outer walls get shorter, and the
  middle stays up on four new walls.
- **Missing face and a stray triangle:** pushing a cube's top down past a
  box pulled out of its side left the side face with a hole crossing its
  outline (drawn as a stray triangle that could not be picked). The
  coplanar rule (DECISIONS #30, `prism-ops/src/settle.rs`) now runs after
  Push/Pull, Move, Rotate, Scale and copies: crossing edges in each touched
  plane are split and the plane's faces rebuilt; the opening at the pulled
  box's back gets a face, facing out.
- Two boxes of different heights sharing a wall now share one wall face.
- An array of overlapping copies is split where the copies overlap (the
  rule runs once, after every copy is placed).
- **Speed (Large Scene, 15,000 faces, release):** Push/Pull 26 ms per
  preview (it was about 350 ms, from a whole-model weld it no longer
  does); moving the whole scene commits in 425 ms (367 ms before). Each
  plane is checked cheaply first and rebuilt only when something meets;
  geometry moved together is not checked against itself.
- **Left over:** a face with an opening that the edit did not close stays
  open (an open box whose top is pushed keeps its opening). The rule does
  not yet run after Offset or Import.

### Offset snaps (owner feedback, 2026-11-08)

- Offset now runs the inference engine while its drag is in progress, as
  Move does: an endpoint, midpoint, edge, intersection or guide anywhere in
  the model is put onto the face's plane and sets the distance, with the
  usual glyph and label. With no snap, the cursor on the face's plane sets
  it, as before. (Since the later fix below, a snapped point is where the
  new loop goes, wherever the first click was.)

### Faces pulled flush join the solids (owner feedback, 2026-11-08)

- Pulling a block's side until it meets another block left both faces
  where they touched, back to back inside one solid; pushing the front in
  later uncovered them (reversed blue faces, a trench). Now, after an edit,
  a region covered by faces facing opposite ways gets no face, as in
  SketchUp, so the blocks join. Two boxes pulled side by side join too.
- Left over (older behaviour): a rectangle that shares an edge with another
  rectangle gets no base when pulled up, so the box is open underneath. *(Fixed 2026-11-12, as SketchUp does.)*

### The solid rule (owner asked for one general rule, 2026-11-08)

DECISIONS #31, `prism-ops/src/solid.rs`. Push/Pull on a closed solid is a
boolean with the block the face sweeps (union when pulling out, difference
when pushing in). The inside test uses the solids as they were before the
edit (a ray count over their faces) and the swept block, so it is exact
even while the model is half rebuilt.

**Steps after a pull:** the coplanar rule (#30) with that inside test (fills
openings exactly), then cut the touched faces where they cross others, rebuild
the planes around the swept block if anything was cut, keep only faces with
the solid on exactly one side (turning round any that face in), keep one of
any duplicates, and drop edges and points left without a face.

**Fixed along the way:**
- Intersect Faces could skip a crossing that was a shared edge for only
  part of its length (it tested the midpoint only); it now tests each piece
  between corners.
- The coplanar rule missed edges owned by faces in other planes that lie in
  the plane (a corner edge along a wall).
- Welding a face onto an identical one facing the other way now removes
  both (a box pushed flat), and faces count as identical only with the same
  holes.
- Region rebuilds no longer test every face against every region, or scan
  every edge in the model per lookup (the Large Scene's ground plane was
  2,500 x 2,500).

**The test points** sit 1.2 tolerances off each face (CI's thin-plate
screenshot test, a 0.5 mm plate 300 m wide, failed at 100 tolerances:
both points landed outside the plate and its faces went). Points within
one tolerance of a face or edge count as unsure, and unsure faces stay.

**Tests:** the owner's cases, a block pulled into another, a block
swallowed, a very thin plate, a tunnel pushed through, open geometry untouched, and a property
test (`tests/solids.rs`) of random pulls and notches on a box that must
stay a valid closed solid. CI runs it with a fixed seed.

**Speed (Large Scene, release):** Push/Pull 99 ms per preview (26 ms this
morning without the solid rule). The inside test and the cutting step look
at every face's flat form once per pull; a cache or spatial index would
bring it down. Small models are instant.

**Left over:**
- Random sequences still find rare failures: about half the seeds find
  one in 2,000 cases. Two known kinds: a face whose outline touches itself at one
  corner (a "pinched" loop, an older region-tracing limit), and a push
  deeper than a neighbouring wall reaches, which slides that wall past
  its own end. Run more with `PROPTEST_RNG_SEED=<n> PROPTEST_CASES=2000
  cargo test --release -p prism-ops --test solids`.
- Move, Rotate and Scale do not apply the solid rule (#31): moving a box
  into another leaves the crossing for Intersect Faces, as before.
- Behaviour change: pushing a window out through the back of a box now
  cuts a tunnel by itself. Before, the tube stuck out and Intersect Faces
  was needed.

## Groups & Components (part 1: groups)

Brief: `docs/briefs/12-groups-and-components.md`.

**What was built**
- `prism-doc`: an `Assembly` of definitions, each a `Model` in its own
  coordinates, placed by instances (`DAffine3`); the root is a definition
  too. Definitions sit behind `Arc`, so undo steps share what they did
  not change (tested with `Arc::ptr_eq`).
- Commands: `MakeGroup`, `Explode`, `Place` (move, rotate, scale groups),
  `CopyInstances` (each copy gets its own definition, DECISIONS #22),
  `EraseInstances`, `Batch` (geometry and groups in one step), and `In`
  (a command inside an open group). Because the context is in the
  command, the log, autosave journals and replay need no change.
- Explode places the group's geometry in its parent, welds what lands on
  existing points, runs the coplanar rule (#30) and keeps one of any
  repeated faces. The PLAN 1.9 worked example passes, with boxes, at three
  scales and turned off the axes.
- `.prism` schema 2: each group's model is its own entry
  (`geometry/defs/<id>.bin`) and `groups.json` holds the instances, both
  only when there are groups. Schema 1 files open unchanged
  (`docs/acceptance/files/schema-1/`); new goldens include `groups.prism`.
- Session: the open group's model is worked on in world coordinates with
  its own ids, so every tool works inside a group unchanged, and commands
  are put into the group's coordinates when they run (`Command::to_local`).
  Everything else is merged into the scene with ids from 2^31 up and an
  owner (`Scene::merge`), so inference snaps to all of it.
- Select: a click picks a group whole; Shift and Ctrl work as for
  geometry; box select takes groups (window: every edge inside; crossing:
  touched). Double-click or Enter opens a group; Esc (last in its order)
  or a click on geometry outside or on empty space closes it, one level at
  a time, leaving it selected. Undo of an edit inside a group opens it.
- Move, Rotate and Scale take the selected groups along (or the group
  under the cursor), with Ctrl copies; previews show the groups moving.
  Delete erases groups. Push/Pull, Offset and the Eraser ignore closed
  groups.
- Drawing: outside the open group, faces are washed out and edges faint;
  a dashed box shows the open group and a blue box each selected group.
- Menus: Make Group (G) and Explode are active (Explode needs a group
  selected); the right-click heading counts groups.
- Mesh export writes every group, placed.

**Tests:** `prism-doc/tests/groups.rs` (isolation both ways, nesting,
copies independent, the overlap case, exact undo, replay, flattening),
session scenarios in `prism-tools/tests/session.rs`, the schema 2 goldens,
and a screenshot of an open group (`an_open_group_fades_the_rest`).

**Decisions and defaults used**
- A group's origin is the low corner of what went in, with the parent's
  axes, so its coordinates stay near zero.
- Clicking empty space closes the open group on release (so a box drag
  inside the group still works).
- Scale inside a rotated group works along the group's own axes, as in
  SketchUp.

**Leftovers**
- Explode does not select the exploded geometry yet.
- Arrays typed after a copy (x3, /4) repeat only loose geometry, not
  groups.
- Drawing is flattened: each group's scene is rebuilt from its model on
  every change. GPU instancing comes with components (part 2).
- Recorded sessions store the root model only; a recording that makes
  groups replays, but its saved result compares the root.
- The Eraser does not erase a closed group by dragging over it (Delete
  does).
- Not tried by hand: the native app and the web page with groups. The
  logic is covered by session tests and the screenshot test.

## Groups & Components (part 2: components)

Brief: `docs/briefs/13-groups-and-components-2.md`.

**What was built**
- A definition with a name is a component; without, a group. Copies of a
  component's instance (Move with Ctrl, Rotate copies, `CopyInstances`)
  share its definition, so editing one changes all; a component nested in
  a copied group stays the same component.
- Make Component (Edit menu): as Make Group, named "Component" (or
  "Component#2", ...). Make Unique gives each selected instance its own
  copy as "Name#2". Rename refuses empty or taken names.
- Flip Along red, green or blue mirrors selected groups and instances
  about their own centre (their placement; the geometry is not changed).
  Exploding a mirrored instance turns its loops round, so faces face out.
- While an instance is open, the other instances of the same definition
  are drawn normally, so the edit shows on all of them.
- Window > Components lists the components with their instance counts,
  selects all instances of one in the open group, and renames.
- Each definition's scene is built once and placed for each instance
  (`Scene::placed`, for moves, turns, mirrors and even scales; a stretch
  is rebuilt). 100 instances of a 150-face component: 33 ms (74 ms when
  each was rebuilt). Most of what is left is merging the copies into one
  scene; GPU instancing would remove that.
- Files: `groups.json` records a component's name; part 1 files open as
  groups. Golden `components.prism`.
- Planned Features: "Groups and components, open and exit" is ticked.

**Tests:** components share and change together, Make Unique, Rename,
Explode of one instance keeps the component, a mirrored instance explodes
into a valid model (prism-doc); a session scenario (copy, open, draw, see
the other change, Make Unique, Flip, Explode); `Scene::placed` against
building from the moved model; the golden file.

**Leftovers**
- GPU instancing (one draw per definition) is not built; see the timing
  above.
- A component's own axes and insertion point (the Axes tool), and the
  library browser, are later.
- The Components window cannot place a new instance yet (copy an
  existing one).

## Groups & Components (part 3: hiding, tags and panels)

Brief: `docs/briefs/14-groups-and-components-3.md`.

**What was built**
- Instances can be hidden, named and put on a tag; the document has tags,
  each shown or hidden. These live in the assembly by instance id (ids are
  unique in the document), are cleaned up when instances go, and copies
  keep their original's name and tag. All are commands, so undo works.
- Hide Selected, Unhide All (in the open group), Unhide Last and Isolate
  in the Edit menu.
- Hidden must be free: hidden instances and those on hidden tags are not
  built into the scene, so they are not drawn, picked, snapped to,
  box-selected or taken by Select All, and mesh export leaves them out.
  100 groups with 90 hidden build in 9.3 ms; a model of the 10 visible
  alone takes 5.9 ms (the rest is walking the tree and the root model).
- Window > Outliner: the tree of instances, an eye per row, a click
  selects (opening the parent group if needed), a double-click opens.
- Window > Entity Info: the selected group's name (editable), what it is,
  its tag and a Hide/Show button; for loose geometry, counts and area.
- Window > Tags: show or hide, rename, add, and put the selection on a tag.
- Files: `groups.json` keeps names, tags and hidden flags; older files
  open with none.

**Tests:** hiding, tags, names with copies and erasing (prism-doc); a
file round trip; session scenarios (a hidden group is not picked, snapped
to or selected; Unhide Last; Isolate; the Outliner selecting inside a
nested group; Entity Info naming); the 90%-hidden timing (`timing.rs`,
run by hand).

**Leftovers**
- Hiding loose faces and edges, Show Hidden (ghosted), Lock.
- The Outliner cannot drag rows to regroup, search or filter yet.
- Not tried by hand: the three windows in the native app and on the web.

## Model Health (part 1: checks, panel and fixes)

Brief: `docs/briefs/15-model-health.md`.

**What was built**
- `prism-ops::health`: duplicates and near misses, reversed faces, open
  edges (and watertight shells), edges with 3 or more faces, interior
  faces, degenerate faces and edges, stray edges, crossing faces, tiny
  pieces, and far from the origin. Pure, on the open group's model.
- Reversed: in each connected piece the faces that disagree with most of
  their neighbours; for a closed shell facing into its own solid, the rest.
- Interior: a point on the face, with the face left out, is inside the
  closed shells around it along every ray. (A first try, "every ray from
  either side hits something", flagged 980 box sides facing a close
  neighbour in the Large Scene; counting crossings with the face itself
  in was fooled by the interior faces it looks for.)
- Fixes, each one undo step: Orient Faces, Reverse Faces (Edit menu),
  Clean Envelope (the selection, or everything), Remove Stray Edges,
  Intersect Faces (the existing command).
- Window > Model Health: a row per check with its count; a click selects
  the items and zooms to them; fix buttons; nesting and scale as
  information. The checks rerun only when the model changes while it is
  open.
- Speed: every check on the Large Scene (15,000 faces) takes 179 ms (the
  tiny-pieces check first took 12.8 s; it is now one union-find pass).

**Tests:** `prism-ops/tests/health.rs` (each problem on a small case, a
clean box with none, Orient on a flipped face and an inside-out box,
Clean Envelope, Remove Stray), a session scenario (a row selects, the fix
works and undoes).

**Leftovers (part 2):** problems drawn in the viewport, checks running in
the background while the window is closed, checks before export.

## Model Health (part 2: in the model, and on export)

Brief: `docs/briefs/16-model-health-2.md`.

- While the Model Health window is open (and its "Show in the model"
  switch is on), open edges are drawn red, edges with 3 or more faces
  orange, stray edges grey, reversed faces tinted, and interior and
  crossing faces outlined. The marks follow each edit.
- Exporting a mesh checks what was written; the status says what it
  found ("4 open edges: the model is not watertight; see Model Health").
  STL and 3MF mention watertight; the export still happens.
- Not built (a decision in the brief): checks while the window is closed.
  They cost up to 180 ms per edit on big models and the web build has no
  threads; the panel and the export check cover the need.

**Tests:** marks for an open box, a flipped face and a clean box (session),
the export note (prism-ops).

## Owner feedback, 2026-11-08 (after Model Health)
- **Plan order:** a standing rule now says chunks are built in the order of
  the PLAN 6.0 table. The table was reordered so each chunk comes after what
  it needs: Exact Curves and Annotation & Sections before Inspector &
  Features, Exact Curves before Shapes & Placement, Basic Materials before
  Rendering. Next is View Settings.
- **The blue box follows the move:** while Move, Rotate or Scale is under
  way, a selected group's or component's box is drawn with the step applied
  (`Session::group_change`). It used to stay where the group started.
- **Offset through a snapped point:** when Offset snaps to a point, an edge
  or a guide, the new loop now goes through that point
  (`offset::distance_through`). It used to measure from the first click
  (so it was off by how far that click was from the edge), and straight to
  the nearest corner (past a corner the mitred loop is further out). A free
  cursor still moves the loop by how far it has moved from the first click.
  Left over: on a face with notches, a point can lie on more than one moved
  edge; the smallest distance wins.

## Owner feedback, 2026-11-08 (side windows, groups)
- **Side windows:** Outliner, Entity Info, Components, Tags and Model
  Health are no longer dialogs over a dimmed app. They dock as cards in a
  column on the right of the viewport (`SideCard` in `prism-ui`), which
  shrinks the viewport while one is open. Each card has a fold arrow and a
  close button in its top-right corner; clicking the title folds it too.
  They do not take the keyboard, so tools and shortcuts work while they
  are open, and Esc no longer closes them. Screenshot:
  `docs/screenshots/side-panels.png` (the Outliner folded). Left over: the
  cards cannot be reordered, resized or moved to the left yet (the kit's
  `PanelLayout`), and the column has a fixed width.
- **Double- and triple-click inside an open group:** they looked the face up
  in the root model instead of the group's, and found nothing. Fixed.
- **Push/Pull and Offset on a closed group:** the hover already ignored a
  closed group's faces, but a click still started a step on them. Now the
  click does nothing, as SketchUp does.
- **The open group's dashed box** stands off its contents by 5% of their
  largest size on every side, so it shows round a flat rectangle.
- **CI hang found and fixed:** the random undo test hit a sequence where
  `Command::Duplicate` (a copy in place, never used by the app) left
  coincident points, and the next Offset multiplied faces until memory ran
  out. The command is removed; Copy welds its copies and covers the need.

## Owner feedback, 2026-11-09 (recorded session: corner tangents)
- **Moving the top of a box bent a neighbour's corner edge.** In the
  recorded session a small box touched two taller blocks only along their
  vertical corner lines ("corner tangents"). Its top corners split those
  corner edges, so Move took the split points along and bent the taller
  blocks' edges: a sliver, a reversed triangle and two overlapping faces.
  Rule now (`transform::keep_straight`, used by Move, Rotate, Scale and
  Copy): a moving point that only splits a straight edge of a face that
  stays put leaves that edge straight; the face keeps a copy of the point,
  and only faces that turn a corner there follow it. A point that is
  straight in every face (a point picked on an edge) still bends the edge,
  and so does a point where the following faces use both halves of the
  edge (a box set flat against a wall is one shape with it).
- The rest of the session replays clean: no point sits inside an edge it
  does not split, and Model Health finds nothing at any step before the
  Move. "Corner lines not redrawn" did not show in the model, so it may be
  a drawing or preview problem; it needs the step it happened on.

## Owner feedback, 2026-11-09 (corner line, profiles)
- **Shared corner line z-fought:** two boxes touching along a corner line
  share one edge with four faces (two walls of each). The renderer let an
  edge show through only its first two faces, so the other two hid it in
  dashes on the owner's GPU. A first fix kept four faces per edge; the
  owner asked for a general rule (exploded rotated copies can put dozens
  of faces on one line). Rule now: **a face never hides a line lying in
  its own plane.** The face-id pass also writes each pixel's face plane
  (normal and offset from the eye, as bits, in a second Rgba32Uint target),
  and a line whose two ends are on that plane (within 2e-6 of their
  distance) is shown. It also fixes lines on a face seen nearly edge on,
  which broke up before. The ground grid and axes are left out (they sit
  under faces on the ground). Lines behind a thin wall stay hidden. The
  geometry was already right: one shared line, split where one box rises
  above the other (`prism-view/tests/corner.rs`).
- **Profile lines** are now spelled out in PLAN 1.14 and named in the View
  Settings chunk (next). They already work in the renderer, off by default
  (Help > debug panel, "Profile lines").

## View Settings (part 1: what is shown, lines and faces)
Brief: `docs/briefs/17-view-settings-1.md`.
- **ViewStyle** (`prism-view/src/view_style.rs`): what is shown (faces,
  lines, points, axes, ground, grid, guides), line and profile thickness,
  line colour (one, or by axis), hidden lines (not drawn, dashed, faint),
  face style (Shaded, Monochrome, Hidden Line, Wireframe) and X-ray with
  an opacity. Named styles: Default, Hidden Line, X-ray, Technical,
  Wireframe. `apply` turns it into the renderer's `Style`, so Monochrome
  and Hidden Line are only colours and lighting; Wireframe and X-ray are
  `FaceDraw::None` and `FaceDraw::XRay` in the renderer.
- **Profiles are on in Default** (3 pt over 1.5 pt lines), as the owner
  asked.
- **Picking follows the style:** `Scene::faces_hide` and `faces_pick`. In
  X-ray faces hide nothing, so points and edges behind them snap; in
  Wireframe faces cannot be picked, snapped to or pushed.
- **Window:** View > View Settings (and Line Thickness, Outline Thickness,
  Edge Style, Face Style, Styles) opens a View Settings side card. The View
  > Show toggles and X-ray are menu toggles; their check marks follow the
  style. The profile and hidden-line controls left the debug panel.
- **Saved with the file** under `view` in the settings (an old file opens
  with Default; no new schema). Changing the style marks the document
  unsaved, and changing it back does not.
- **Left over:**
  - X-ray is the "Fast" kind: only the nearest layer of faces is
    see-through (order-independent "Accurate" is for later).
  - Profiles still skip edges with three or more faces, and do not yet
    leave out faces hidden behind others.
  - Points are drawn for every vertex, faded groups' too.
  - Choosing a named style replaces every setting; there is no "save as
    new style" yet.
  - Lighting and the sun, View > Hidden Geometry and "Show Hidden" ghosts:
    part 2.
  - Not yet clicked through by the owner, and not run on the web build.

## View Settings (part 2: the sun, shadows and Show Hidden)
Brief: `docs/briefs/18-view-settings-2.md`. View Settings is done (2 parts).
- **The sun** (`prism-math/src/sun.rs`): NOAA solar position from month,
  day, local time, UTC offset, latitude, longitude and north angle. Pure;
  tests cover noon overhead at the equinox, east at sunrise, polar night,
  the north angle and longitude against time zone. The default place is
  Guam, 21 June, 1:30 pm.
- **Lighting in the view style:** Default lighting (the old fixed light)
  or the sun; light and dark levels; one Shadows switch (View > Lighting,
  formerly Ground Shadow). At night: dark level only, no shadows.
- **Shadow map:** a 2048² depth pass from the sun over the faces, fitted
  to the model's bounds, relative to the eye; faces sample it with a
  comparison sampler, 3×3 taps. It uses bind group 3 (lines use group 2).
  Hidden objects are not in the scene, so they cast nothing. The ground
  shadow follows the sun.
- **Show Hidden** (View > Hidden Geometry): hidden groups and components
  (and those on hidden tags) are built as faded ghosts with dashed edges;
  they can be clicked and box-selected, and Edit > Unhide Selected shows
  them. Off by default; off, hidden things still cost nothing.
- **Left over:**
  - Shadow quality is one fixed map over the whole model: on a large
    model shadows are blurry (no cascades yet).
  - "Unhide Selected" does not show something hidden by its tag (switch
    the tag on), and picking a ghost nested inside a visible group selects
    the visible group.
  - Fog, ground opacity, below ground, edge shadows, and a time slider in
    a toolbar are not built.
  - Not yet run on the web build or clicked through by the owner.

## Toolbars (part 1: layout, docks, floating, edit mode)
Brief: `docs/briefs/19-toolbars.md` (owner request, built before Updates).
- **Layout** (`prism-app/src/toolbars.rs`, pure): each toolbar is docked
  on the left rail or in the top dock (with an order), floating at a point
  from the app's corner, or closed. Saved in the user's settings
  (`toolbars.json`, as the keymap), not in documents. View > Reset
  Toolbars puts everything back on the left rail.
- **Shapes:** a toolbar is 1 button thick with 5 tools or fewer, 2 with 6
  or more, docked or floating; a dock is as thick as its thickest toolbar
  (the top dock runs the same shapes sideways). Principal alone in the top
  dock is one row.
- **Docks:** the left rail and a top dock between the menu bar and the
  tabs. Both take no space when empty, and show a thin strip to drop on
  while a toolbar is dragged.
- **Floating toolbars:** small windows in the app (egui areas, above the
  viewport and under menus), tighter padding, a grip to drag them, kept on
  screen when the window shrinks, and snapping to each other's and the
  viewport's edges within 8 points.
- **Edit mode:** right-click a toolbar's empty space (or View > Edit
  Toolbars): each toolbar gets a box, a title bar to drag and an ×; Done
  or Esc leaves it. Buttons do not run and show no tooltips meanwhile. The
  right-click menu opens after the menus are drawn (the same click closed
  it otherwise), and the frame asks for a redraw.
- **New buttons:** a View toolbar with X-ray and Hidden Geometry (new
  icons).
- **No labels** (owner, 2026-11-09): a toolbar's name is the tooltip over
  its empty space (the kit's tooltip, as the buttons'), and faint dividers
  separate toolbars in a dock, to save room.
- **Left over (part 2):** the Toolbars window (View > Toolbars and Panels,
  still greyed) with drag-out, and reshaping a toolbar by dragging its
  edge. Docking on the right or bottom is not planned.
- Checked in the real app under Xvfb: right-click menu, edit mode, docking
  on top, floating, dragging back, Done. Not yet tried by the owner.

## Toolbars (part 2: the Toolbars window, reshaping)
Brief: `docs/briefs/19-toolbars.md`.
- **Toolbars window** (View > Toolbars and Panels, and the toolbar
  right-click menu): centred, does not dim the app (`SoftDialog::undimmed`).
  Each toolbar is a row with its icons; showing ones are greyed, closed
  ones are dragged out and drop like any dragged toolbar. Reset Toolbars
  is at the bottom. Esc or × closes it.
- **Reshaping:** drag a floating toolbar's right or bottom edge; it snaps
  to whole buttons (1 to all). The shape is saved per toolbar
  (`columns`, left out of the file when unset, so part 1 layouts read)
  and kept through docking and floating. Closing a toolbar or Reset
  forgets it.
- **Docked shape:** a toolbar keeps its shape in a dock, turned so its
  long side runs along it: its thickness is the smaller of its columns and
  rows. So a one-row strip docks one button thick; the defaults are as in
  part 1. A dock is as thick as its thickest toolbar (so a 3 by 3 block
  makes the rail three buttons wide).
- **Done** moved to a small card at the bottom of the viewport: at the top
  it covered the Toolbars window's title.
- Checked in the real app under Xvfb: closing a toolbar, dragging it out of
  the window, reshaping it to a row, docking that row in the top dock.
- Not built: "Panels" in the window's name (the side windows are in the
  Window menu); docking on the right or bottom.

## Owner feedback, 2026-11-09 (after Toolbars part 1)
- **View Settings** (the window with the Lighting section) moved from the
  View menu to the Window menu. Lighting > Sun and Shadow Settings… still
  opens it.
- **Hidden geometry ghosts** are edges only now: no faces, so they cast no
  shadow and hide nothing, and they are picked by their edges. They are
  drawn like hidden lines: faint (`hidden_alpha`), and dashed unless
  Hidden Lines is set to Faint. With Hidden Lines Off they are dashed too
  (assumed: otherwise Show Hidden would show nothing by default).

## Updates
Brief: `docs/briefs/20-updates.md` (PLAN 3.1 decision 8, owner request).
- **Published:** the release workflow writes `update.json` (build number,
  commit, each zip's SHA-256) into every release. Prism reads it from the
  fixed "latest" link: no GitHub API, no rate limit.
- **Help > Check for Updates…** (`help.update`) opens a window: "Build M
  is available" with Download and Install, progress while downloading,
  then "Installed build M. Restart Prism to use it." Builds made by hand
  ("local"), Linux and the web build say why there is nothing to do.
- **Code:** `prism-app/src/update.rs`. The check and the download run on a
  thread (`Updater`, status behind a mutex); the window only reads it.
  HTTPS through `ureq` with `native-tls` and the system's certificates
  (Schannel, Security framework); `HTTPS_PROXY` is honoured. New
  dependencies: ureq, native-tls (through ureq), sha2.
- **Swap:** Windows renames the running `Prism.exe` to `Prism.old.exe` and
  writes the new one in its place (deleted at the next start). macOS
  unpacks the new `Prism.app` into `.prism-update` beside the old one
  (keeping file modes), swaps the bundles and deletes the leftovers. If the
  new one cannot go in, the old one is put back. Renames retry for a
  second (virus scanners hold new files). Running from App Translocation is
  refused with a "move Prism.app first" message.
- **Tests:** manifest and build numbers, checksums, unpacking (modes, no
  paths outside the folder), the swap and its undo, both installs, and the
  whole check-download-install flow against a local HTTP server. They run
  on Windows and macOS in CI too. An ignored test reaches GitHub over
  HTTPS (passes here through the proxy).
- **Not tried yet:** a real update on Windows or macOS. It can only be
  tried once a release with `update.json` exists and a newer one follows
  it: the first build after this one has none to update to. Updating from
  an older build than this one is not possible (they have no updater).
- **Left out:** checking at start, a Restart Now button (needs the quit
  path that asks to save), signature checks (once builds are signed).

## MCP Server (part 1: the server, reading, and actions)
Brief: `docs/briefs/21-mcp-server.md` (PLAN section 8 decision 6).
- **Help > MCP Server…** (`help.mcp`): a switch (off by default), the
  address, the token (New Token), the `claude mcp add --transport http …`
  command with a Copy button, and the last few calls. Kept in `mcp.json`
  in the user's settings; a token is made and saved on first start.
- **Transport:** `http://127.0.0.1:47615/mcp` (tiny_http, its own
  thread), local machine only. Bearer token (401), `Origin` must be local
  (403, against DNS rebinding), GET and DELETE 405, POST answers JSON
  only (no SSE), notifications 202, no sessions.
- **Both protocol eras** (researched from the spec repository, 2026-11-09:
  2026-07-28 is the newest release and removed the handshake):
  `initialize`/`ping` for 2025-03-26 to 2025-11-25; `server/discover`,
  the version in each request's `_meta`, `UnsupportedProtocolVersion`
  (-32022, 400), `HeaderMismatch` (-32020, 400) and 404 for unknown
  methods for 2026-07-28. Every result has `resultType: "complete"`.
- **Tools:** `get_model`, `get_selection`, `get_health`, `list_actions`
  (with a query), `run_action` (through the registry as a menu click,
  `Source::Assistant`; greyed ones refused with the reason).
- **Threads:** the socket thread speaks the protocol and sends tool calls
  to the main thread (a channel and `UserEvent::Mcp`); it waits up to 60
  seconds. The action dispatch from `redraw` became `app::dispatch`, used
  by both. Stopping drops waiting calls (so nothing hangs) and joins the
  thread; starting retries the port for a second (tiny_http lets go of it
  a moment after stopping, which a restart on a new token hit).
- **Checked:** tests for the protocol, the socket (token, origin, method,
  restart on the same port, a call answered by the "main thread"), and
  the tools on the real chrome and viewport (Select All, Make Group and
  Undo through the history). In the real app under Xvfb: curl with both
  eras, `run_action` Select All and X-ray showing in the viewport, the
  window, and the switch stopping and starting the server.
- **Left for part 2:** drawing at model coordinates, screenshots,
  replaying sessions, face and edge detail, units. A file dialog opened
  by an action blocks the answer until it closes (the call times out
  after 60 seconds). `get_model` lists every group; very large models
  give long answers.
- **Not tried yet:** Claude Code itself connecting (not installable here
  against the app's window; curl stood in for it).

## MCP Server (part 2: drawing and seeing)
Brief: `docs/briefs/21-mcp-server.md`.
- **`use_tool`:** picks a tool, then clicks and moves at world points in
  metres (projected through the camera, after `Session::settle_camera`
  finishes any ease), types into the measurement box, presses keys, locks
  axes (arrow keys). Every step is an ordinary session event, so it is
  undoable and recorded. Each click reports where inference put it. A
  point outside the view stops the steps with a message. Clicking the same
  spot twice in a row counts as a double-click, as for a person.
- **`get_geometry`:** faces (unit normal, area, outer loop, holes) and
  edges (ends, length) of the open group, world coordinates, up to a limit
  (200), with totals. Areas are the group's own (a scaled group's are not
  scaled).
- **`screenshot`:** the app holds the call, draws the next frame, reads
  the viewport's colour texture back (`Gpu::read_viewport`), shrinks it to
  at most 1024 pixels wide and sends a PNG (our own small encoder on
  `flate2`, which Prism already had). If the window is minimised, no frame
  is drawn and the call times out after 60 seconds.
- **`replay_session`:** reads a recording from a path or its JSON, replays
  it in a separate session, and says whether the end matches.
- **`get_model`** also gives the display unit and the model's bounds.
- **Checked:** tests draw a 2 by 1 m rectangle by clicks and push it 1.5 m
  by typing (top face area and height checked), refuse a point behind the
  camera, undo; list the cube's geometry; replay a recording made in the
  test. In the real app under Xvfb, curl drew a box with `use_tool` and
  `screenshot` returned a 1024 by 600 PNG showing it.
- **Push/Pull clicks report no inference point** (the tool picks a face,
  not a point); the face's change shows in the counts and `get_geometry`.

## Owner feedback, 2026-11-09 (after MCP Server)
- **MCP line in every brief:** added to PLAN 6.0a as a standing rule.
- **Web wheel jitter:** on the web build a wheel panned for a moment
  before zooming. Browsers report wheels and trackpads alike (points), and
  each event was sorted alone: small first steps of a wheel notch panned.
  Now a gesture (events under 0.25 s apart) is sorted as a whole: small
  vertical steps at its start are held (up to three) until a big step says
  wheel (all zoom) or more small or sideways steps say trackpad (all pan,
  nothing lost); a wheel gesture's small tail zooms too. Not reproduced
  here (no browser with a wheel); the owner should check it. Native
  trackpads (pixel scrolls) pass the same way and may wait up to three
  events (a few milliseconds) before panning.

## Owner feedback, 2026-11-09 (Push/Pull stops at faces; Option on a Mac)
- **Push/Pull never passes through a face** (DECISIONS #31, changed):
  `prism_ops::reach::limit` finds how far a face can move before its sweep
  first touches another face: another face's corner inside the swept
  block, the moving face's corner reaching another face, edges crossing,
  or a parallel face overlapping it; a neighbour sharing an edge that leans
  over the path blocks at once. `push_pull` clamps to it unless Alt's new
  loop is on, and the drag shows "Stops at Face" and the clamped distance.
  Faces far from the swept block are skipped by their boxes. The test that
  swallowed a smaller block now checks that the pull stops at it and joins
  it; the tunnel test (pushed out through the back) still passes.
- **Option did nothing on a Mac**, and neither did Alt or Ctrl anywhere:
  the session had the toggles (`AltTap`, `CtrlTap`) but the app never sent
  them. Now a modifier pressed and let go on its own (no other key or click
  between) sends them: Ctrl copies and Alt makes a new loop on Windows and
  Linux; Option does both on a Mac (each tool uses its own), as in
  SketchUp. The status hints still say "Alt" on a Mac.

## Owner feedback, 2026-11-09 (build in the title; curve tools greyed)
- **The window title names the build:** "Untitled — Prism - Build 120"
  ("Prism - Local build" for a build made by hand), so a successful
  update shows. `files::app_name()`, also the window's first title.
- **Curve tools stay greyed until Exact Curves is finished** (all five
  parts), even where a part has built one. Written into brief 22 and the
  PLAN row.


## Exact Curves (part 1): loops list their edges
- **What changed:** each face loop names its edges (`Face` keeps a private
  list per loop; edge `i` joins vertex `i` to `i + 1`). `Face::new` makes
  a face (the model finds its edges when it is added); `Face::with_edges`
  gives them (imports, copies, files: no lookups). `loop_edges(k)`,
  `sides()`, `edge_ids()` and `uses_edge()` read them.
- **Kept in step by the model:** `add_face` and `edit_face` (which replaces
  `face_mut`) link the loops: an edge the face named that still joins the
  same two vertices is kept, the rest are looked up. `relink_face` is for
  code that adds edges after editing loops (moving a box top, Push/Pull's
  sliding walls were reordered so the edges come first). `split_edge` puts
  both halves into each loop directly. A loop with no edge for a side keeps
  a marker the check reports, as the missing edge was before.
- **Check:** `bookkeeping_problems` also finds a loop whose edges are
  missing, too few, or join other vertices. The property tests run it after
  every random operation (both the Core Model ones and the undo test of all
  17 commands).
- **Walks that looked edges up by their ends now use the stored edges:**
  Erase, Copy, Make Group, Push/Pull, solids, Model Health's inside-face
  removal, the coplanar rule (`settle`), double-click selection.
- **Files:** schema 3, geometry layout 2 (loop edges saved). Layout 1 is
  read by finding the edges. The schema 2 golden files were kept in
  `docs/acceptance/files/schema-2/`; a test opens each and checks it gives
  the same model as the new golden file. JSON (journals, recordings, schema
  0) without loop edges finds them on reading. Recorded sessions compare
  through `Recording::ends_with` (reads the recorded result as a model),
  so old recordings still match; the MCP `replay_session` uses it too.
- **Tests changed only in form:** helpers that built `Face { .. }` by hand
  use `Face::new`, and the saved-sessions test uses `ends_with`.
- **Left for part 2 (arcs):**
  - linking by ends cannot tell two edges between the same two vertices
    apart. Code that makes such loops must give their edges
    (`with_edges`, `split_edge`);
  - face finding (`regions`), Model Health's orientation (`runs`) and the
    reach rule still key edges by their two vertices. They are right for
    straight edges and need edge ids when arcs come.

## Exact Curves (part 2, step 2a): arcs in the core
Brief 23 splits part 2 into four steps (2a core, 2b crossings, 2c picking
and inference, 2d the Circle and Polygon tools).
- **Arcs:** `Curve::Arc { centre, axis, radius }`, running counter-clockwise
  about the axis from the edge's `a` to `b`. A circle is two half arcs
  between two seam points with one `CurveId` (a new id kind with its own
  counter). Arcs are never merged with lines: `add_edge` reuses only a
  straight edge (`line_between`); arcs come in with `add_curve_edge`.
  Splitting an arc gives two arcs of the same circle and curve.
- **Drawn points:** `Model::edge_points` and `loop_points` give an arc's
  points at 128 even steps per turn (`prism_geom::SEGMENTS_PER_TURN`).
  Faces are triangulated and picked from them, and edges drawn and box
  selected by their pieces. Adaptive tessellation is part 4.
- **Exact area:** `face_area` adds each arc's circular segment to the
  polygon of the vertices (a circle of radius 1 has area π to 1e-12).
- **Face finding traces edges** (`regions`): half-edges at a vertex are
  sorted by their tangent, then curvature; regions keep their edges, so a
  circle's two halves make a face. Every existing test passed unchanged
  after this rewrite.
- **Faces turn round with their edges:** `Face::reverse`. Linking now tries
  the edge named at the same side first, so a circle's halves stay apart.
- **Transforms** work on an affine: arcs with both ends moving go through
  it exactly. A move that would bend an arc (one end only, or an uneven
  scale) changes nothing, for now; the tools do not say why yet (2c/2d).
  `affine` (placing groups) with an uneven scale leaves arcs approximate
  and the model check reports them: a known gap until the segment fallback
  (DECISIONS #32).
- **Refused until their part, changing nothing:** Push/Pull (part 4),
  Offset (part 3) and Intersect Faces (part 4) on faces with arcs.
- **Until 2b:** straight-edge crossing and on-edge tests skipped arcs (2b
  added the crossings).
- **Model check:** arc ends on their circle, a proper radius and axis, an
  arc's axis along the normal of each face using it; a loop of two sides
  is fine when one is an arc. Drawing the same circle twice changes
  nothing. The property tests now add circles too (on a grid, clear of the
  rectangles until 2b).
- **Files:** schema 4, geometry layout 3 (arcs and curve ids). The schema 3
  golden files are kept in `docs/acceptance/files/schema-3/`; a new golden
  file has a circle in a plate.
- **MCP:** `get_geometry` gives an arc's centre, axis and radius, and a
  `curve` number shared by the edges of one curve.
- Checked in the app (Xvfb): the circle file opens, and the circle is drawn
  smooth and unbroken.
- **Bug found by the new check (CI on Windows, part 1's push):** two
  overlapping rotated copies can leave a face whose loop runs along one
  edge twice (a bridge to an island). `split_edge` put the new point only
  into the first run (it did before part 1 too), so a later Push/Pull left
  a face using an edge that was gone. It now splits every run. Push/Pull's
  sliding step has the same first-run-only habit; not seen failing, left
  as is.

## Exact Curves (part 2, step 2b): crossings
- **Exact crossings** (`prism-geom`): `circle_line` and `circle_circle`
  (the quadratic in the circle's plane, or a circle through another
  plane), `circle_distance`, and `Curve::closest_point`. The tolerance
  decides touching: a tangent line or circle meets at one point.
- **Sticky geometry:** a point landing on an arc splits it
  (`insert_point`); a segment is cut where it crosses arcs (up to two per
  arc, and an arc sharing an end with it can still cross it); a circle is
  cut where it meets lines and arcs, and made of arcs between every vertex
  on it, all with one new curve id. Pieces already there (the same arc
  between the same points) are not added again.
- **Straight edge and arc between the same two points:** `add_segment`
  now looks for a straight edge there (`line_between`), not any edge, so a
  diameter through both seams is drawn. The topology check now flags only
  repeated straight edges (it flagged a line next to an arc).
- **Face finding with tangent edges:** half-edges that leave a vertex the
  same way within tolerance (circles touching inside, a line along an arc)
  sort by curvature; before, rounding noise in their tangents decided.
- **Interior points** are now ranked by the box that fits there, not the
  widest scan gap: a thin face between two touching circles took a point
  where the drawn arcs touch, so the overlap check misfired. The drawn
  polygons of two arcs that touch can still cross near the touching point
  (equal angle steps); exact point-in-face for arcs comes with adaptive
  tessellation (part 4).
- **The coplanar rule after edits** (`settle`) knew only straight edges:
  it cut arcs at their chords' crossings, off their circle. It now uses
  exact crossings and on-arc tests, an arc's box is its circle's, and a
  face's containment test uses its drawn outline.
- **Moves refused:** a face with arcs that moves only in part must stay in
  its plane (it cannot fold into flat pieces yet); lifting a corner of a
  square with a circle crossing it now changes nothing. Found by the undo
  property test once it drew a circle.
- **Tests:** a line across a circle, through both seams, ending on it; a
  circle across a square; two overlapping circles; touching circles and a
  tangent line; drawing a cut circle again; a moved copy overlapping its
  circle; the refused lift. The property tests draw circles of radius 1-3
  anywhere on the grid, crossing everything (two regressions kept), and
  the undo property test draws a circle across both squares.
- Checked in the app (Xvfb): two circles across a square and a line open
  as twelve faces, drawn smooth.
- **Not yet:** the tools still do not say why a move was refused (2c/2d). *(Done since: the hint shows the reason while a move is refused.)*
- **CI for 2b (59f7db9):** all code jobs green. The Web build failed twice
  at "Install trunk": the download from GitHub releases returned a 504
  page. If it happens again, make that step retry
  (`curl --fail --retry 5 --retry-all-errors`).

## Exact Curves (part 2, step 2c): picking, inference, Model Health

- **Part 1, whole curves:** a click on an arc (or any edge with a curve
  id) selects every edge of its curve (`Model::curve_edges`); Shift and
  Ctrl clicks follow, as they go through `click_select`. The Eraser picks
  arcs along their drawn pieces, highlights the whole curve and erases it;
  the faces it bounded merge.
- **Open:** a line across a circle keeps all the pieces in one curve, so
  a click still takes the whole circle. SketchUp may split the curve
  there; left as the brief says (whole curve) until the owner decides.
- **Box select** still takes only the pieces it touches or holds.
- **Part 2, snapping to arcs** (`prism-infer`): an arc's midpoint is on
  the curve; "On Arc" lands exactly on it (`Curve::closest_point`);
  quadrants (Kind::Quadrant) where they lie on an arc and are not already
  an end or a midpoint; the centre (Kind::Centre) once the arc is hovered
  or woken (a woken arc casts no guides, only its centre); guides meet
  arcs on the circle (`circle_line`), not on the chord. Scene edges carry
  their curve. The tools draw the arc highlight along the curve and a
  cross at the centre of a hovered or woken arc.
- Not checked by eye in the app yet (needs a hover); the owner should try
  it. Glyphs: Centre a ring in the endpoint colour, Quadrant a triangle in
  the midpoint colour.
- **Disk:** the build cache filled the session's disk (a linker bus
  error); deleting `target/debug/incremental` and building with
  `CARGO_INCREMENTAL=0` fixed it.
- **Part 3, Model Health:** a row "Arcs off their circle" lists arcs with
  an end farther than tolerance from their circle (drawn in the model when
  problems are shown); its fix, Repair (`Command::RepairArcs`, one undo
  step), moves each such end to the nearest point on the circle. A point
  shared by two such arcs follows the first. Model Health's lines now
  follow arcs; its face outlines are still drawn through the corners only.
- **Part 4, refusals say why:** while a Move, Rotate or Scale would bend
  an arc or fold a face with arcs, or Push/Pull or Offset is on a face
  with arcs, the status bar's hint is the reason
  (`Command::refusal`; the words are constants in prism-ops). The edit
  still changes nothing. Not checked by eye in the app.
- **2c is done.** For the owner to try: click and erase a circle, snap to
  an arc's centre (hover the arc first), quadrants, Model Health's Repair,
  and the refusal hints.

## Exact Curves (part 2, step 2d): the Circle and Polygon tools

- **Circle (`tool.circle`) and Polygon (`tool.polygon`)**, in
  `prism-tools/src/session/shapes.rs`: click the centre, then a radius
  point; the plane is the Rectangle's (face under the cursor, an arrow
  lock, the ground, else the view's main axis). The measurement box takes
  the radius; Polygon takes sides as `8s` (6 by default, 3 to 1000). A
  double-click places the last radius again.
- A polygon is inscribed, its first corner towards the radius point; its
  sides share one curve id (`prism_ops::add_polygon`,
  `Command::Polygon`), so a click on one selects all.
- **Greyed:** both stay out of `TOOLS`, so menus, toolbars and the dock
  grey them (owner, until all five parts are done). `GREYED_TOOLS` lets MCP
  `use_tool` pick them; its description says so. Test: drawn over MCP,
  read back with `get_geometry` (arc centre, radius and curve id: brief
  23's 2a MCP test).
- **Not yet:** retyping the radius after a circle is finished (Rectangle
  and Line can) does nothing; *(Fixed 2026-11-12: a radius, or a polygon's 8s, typed right after changes it.)* a centre that lands on a guide over empty
  ground takes the view's main axis as its plane, as the Rectangle does
  (an arrow key picks the plane). Not checked by eye in the app; brief 23's
  acceptance asks the owner to draw circles over MCP in a build and move
  them.

## Exact Curves (part 3, step 3a): the Arc tools

- **Core:** `prism_ops::add_arc` (from a start point, counter-clockwise
  about the normal by a sweep) and `add_pie`; `add_circle` is now
  `add_arc` with a full turn (two halves, as before). Arcs cut and are cut
  by what they cross, like circles. `prism_geom::arc_through` (three
  points), `arc_bulge` (two points and a bulge), `bulge_for_radius`.
  `Command::Arc { centre, normal, radius, start, sweep, pie }`; inside a
  group a mirror turns it about the opposite normal.
- **Tools** (`prism-tools/src/session/arcs.rs`): `arc.2` (and `tool.arc`,
  A): start, end, bulge, typed bulge or `2r`, a magenta half circle within
  8 px; `arc.3`: three points; `arc.c`: centre, start, end, typed radius
  then angle; `arc.p`: the same, closed with the two radius lines. All in
  `GREYED_TOOLS`: greyed in menus, reachable over MCP `use_tool` (its
  description lists them). New style colour `half_circle`.
- **Fixed:** on a drawing plane, a guide along the plane's normal (the
  blue axis from the last click on the ground) is one point, so the
  cursor ignored the plane there and the shape collapsed. Circle, Polygon
  and the Arc tools now ignore such guides (`plane_point`).
- **Fixed (found by the undo property test with a pie):** lifting a corner
  of a face with holes flattens it by moving its points, which took an arc
  end off its circle. Such a move is now refused when the face touches an
  arc's end (`arcs_refusal`).
- Tests: arcs and pies in the core, each tool by clicks and typing, each
  over MCP with `get_geometry`, random arcs and pies in the property tests
  (3000 cases run once), a pie in the undo property test (1500 cases).
- Not checked by eye in the app.

## Exact Curves (part 3, step 3b): tangents

- **Tangent guides** (`prism-infer`, Kind::Tangent, GuideSource::Tangent):
  from the tool's start point and from woken points that are an arc's
  end, along the arc's exact tangent there; a circle's seam gives one.
  Line (and any tool with a start point) snaps along them, "Tangent".
  Drawn in the new style colour `tangent` (cyan).
- **2-point arc:** after its second click, the bulge snaps (within 8 px)
  to the arc that leaves the start tangent to an edge ending there (line
  or arc); the preview turns cyan and says "Tangent". This wins over the
  magenta half circle.
- Tests: the guide at three scales; a line tangent off an arc end; a
  2-point arc tangent to a line (exact centre).
- Not checked by eye in the app.

## Exact Curves (part 3, step 3c): Offset with arcs

- **Offset of faces with arcs** (`prism-ops/src/offset.rs`,
  `offset_arc_loops`): each loop as pieces (lines and arcs, walked with the
  face on the left); a line moves sideways, an arc keeps its centre and
  its radius changes by the distance; neighbours join exactly where their
  offsets meet (line–line, `circle_line`, `circle_circle`). An arc whose
  radius would reach zero is dropped and its neighbours join directly (a
  rounded rectangle offset past its corner radius has sharp corners).
  Otherwise the distance shrinks to the largest valid one, as before.
  New arcs are added with `add_arc`. Faces without arcs use the old code,
  unchanged.
- `distance_through` and `signed_distance` follow arcs too (the Offset
  tool's hover). The 2a refusal and its 2c message are gone.
- Tests: a disc both ways; a rounded rectangle in, and past its corners;
  a square with a half-circle bite (exact areas); the Offset tool on a disc
  by a click and by typing; the undo property test reaches Offset on faces
  with arcs (1500 cases run once).
- `face_at` still goes by a face's corners, so it misses a lone disc
  (two seams); exact point-in-face for arcs is part 4's.

## Exact Curves (part 3, step 3d): corner fillets

- **`prism_ops::fillet`:** a corner (a point with exactly two straight
  edges) is rounded by an arc tangent to both; the edges are trimmed and
  the corner piece goes with the two short pieces (erased as the Eraser
  does). Refusals: `NOT_A_CORNER`, `TOO_SHORT`. `Command::Fillet { corner,
  radius }` (one undo step; `Command::refusal` gives the reason).
- **Tools:** with any Arc tool, a double-click on a corner rounds it. The
  radius is the last one typed (`2r`, or a bare length when nothing is
  being drawn), else a tenth of the shorter edge. A radius typed right
  after a fillet changes it (undo and redo with it). A refused fillet says
  why in the status bar until the next click.
- Tests: a square corner and a 60° corner (exact areas), the refusals,
  the double-click and retyping in a session, a fillet in the undo
  property test (1500 cases run once).
- **Part 3 is done.** Not checked by eye in the app; brief 24's acceptance
  asks the owner to draw arcs and fillets over MCP in a build.
- Not built: a fillet over MCP has no test of its own (the brief asks
  only for each kind of arc).

## Exact Curves (part 4, step 4a): curved surfaces in the core

Brief 25 (`docs/briefs/25-exact-curves-4.md`) splits part 4 into 4a–4d.
- **`prism_geom::Cylinder`:** axis point, unit axis, radius, and whether
  the face points out. `holds` says whether a line (along the axis) or an
  arc (one of its circles) lies on it. `Surface::Cylinder` is the new
  surface.
- **`Surface::plane()` and `Face::plane()` now give `Option`.** Every use
  was decided:
  - moves, turns, mirrors and even scales of a whole curved face move its
    cylinder exactly (`move_curved` in `transform.rs`; a mirror turns the
    loops round, so `outward` flips with it);
  - refused, with reasons: moving part of a curved face
    (`CURVED_WOULD_BEND`); Push/Pull of a curved face or a face touching
    one (`CURVED_PULL`, until 4b); Offset of a curved face
    (`CURVED_OFFSET`); uneven scales already refuse because of the arcs;
  - skipped: face finding, the coplanar rule, the overlap check, and
    Intersect Faces (curved faces are not cut; their cuts need ellipses).
    The brief said Intersect Faces would refuse; skipping is gentler and
    the brief now says so.
  - `reach::limit` meets a curved face only by its corners and edges, not
    its surface (4b).
  - the viewport does not draw curved faces yet (4b).
- **Unrolled faces:** `Model::unrolled(face, k)` gives a cylinder face's
  loop flat, as (distance round, height), walking its sides so the angle
  never jumps. Areas, the model check (points on the surface, winding)
  and mesh export use it.
- **Tessellation:** `tess::triangulate_unrolled`. A band (two chains along
  the curve joined by seams: what Push/Pull makes) is zipped chain to
  chain, so no triangle spans more than one step round. Other shapes are
  triangulated with each straight run bowed a little into the face, so
  three points of one arc are never joined (they would stick out on the
  surface). The plain triangulator made both mistakes: the export test
  found them.
- **Files:** `SurfaceV1::Cylinder`; old files read unchanged, so the
  layout number stays. An older Prism cannot open a file with a cylinder.
- **Model Health:** the "arcs" row (now "Off their curve") also lists
  curved faces with a point off their surface; Repair moves such points
  straight back onto it.
- **MCP:** `get_geometry` gives a curved face's `cylinder` (origin, axis,
  radius, outward) in place of `normal`.
- Tests: a cylinder built by hand (`prism_tools::scenes::cylinder`, and a
  copy in `prism-ops/tests/curved.rs`) is clean with exact areas; moves,
  turns, even scales and mirrors keep it exact; bends and uneven scales
  are refused; Push/Pull and Offset refuse; Intersect Faces leaves its
  sides; Model Health finds and repairs a point pushed off; the file
  round trip; mesh export (on the surface, facing out, the right area);
  MCP; the refusals through `Command::refusal`; both tessellation paths.
- Not built: curved faces in the viewport, picking and inference on them
  (4b).

## Exact Curves (part 4, step 4b): cylinders from Push/Pull

- **Push/Pull of a face with arcs:** each arc side gets a cylinder wall
  (the arc, its moved copy, and the lines between their ends); lines get
  flat walls as before. The moved face keeps its arcs (each moved curve
  gets a new curve id). A disc pulled is two half cylinders and two caps,
  a closed solid with volume πr²h. `ARC_FACE_PULL` is gone.
- **The cap again:** a face whose curved neighbours run along the pull
  (a cylinder's cap) moves in place and the sides stretch; arcs now move
  with their ends in `translate_few`. A pull that would bend a curved
  face beside it is refused (`CURVED_PULL`), and so is pulling a curved
  face itself (a new radius is 4d). An arc with one end moving and one
  staying (a slide) is refused too.
- **Faces name their edges when made:** the moved face and the base are
  made with their edges (`Face::with_edges`), since two arcs can join the
  same two points (a disc). Edges are made in the same order as before,
  so recorded sessions still replay exactly.
- **Found and fixed (outside the brief):** a lone face pushed back
  (against its normal) left only one face: its walls faced in and the
  inside test removed them. The walls now turn round with the face, so it
  makes a box the other way. Test: `a_lone_face_pushed_back_makes_a_box`.
  The owner should try it by hand.
- **Not yet with curved faces:** the solid boolean of a pull into a solid
  (`Swept`) and the inside test after it (`keep_boundary`) go by flat
  faces, so they are skipped when the face has arcs or the model has a
  curved face. A cylinder pulled into a box does not cut it yet.
  `reach::limit` still meets curved faces only by corners and edges.
- **Viewport:** `SceneFace::curved` holds the cylinder, the outer loop in
  space, and its loops unrolled. Shading: `tess::triangulate_unrolled`,
  with the surface's normal at every point, so it shades round. Picking:
  `Cylinder::ray_hits`, then inside the unrolled loops. Placed instances
  (components) and moving previews carry the cylinder along.
- The tessellation is the arcs' fixed 128 steps per turn, not adaptive to
  the screen yet (PLAN 1.1); the normals hide the steps inside the face,
  but a big cylinder's outline shows them up close.
- **Tools:** a curved face gives no drawing plane (`flat_normal`): the
  Rectangle, Circle and Arc tools and the protractor fall back to the
  ground or the view's axis. Face crossings for inference skip curved
  faces. Inference snaps "On Face" on the exact surface.
- The seams between two halves are drawn as lines until 4c softens them.
- Tests: a disc pulled up and down (closed, exact areas and volume), the
  cap again, Alt's new loop, a rounded rectangle; shading (triangles on
  the surface, round normals), picking (exact distances, all the way
  round, placed and mirrored); the Push/Pull tool by typing, with picking
  and snapping on the side and one undo per pull; MCP; the undo property
  test pulls faces with arcs (1500 cases and 3000 for the model rules,
  run once).

## Exact Curves (part 4, step 4c): soften and smooth

- **Edge flags:** `Edge::soft` (not drawn) and `Edge::smooth` (shading
  runs across), set with `Model::set_soft`. They survive splitting,
  copying, merging and welding.
- **Push/Pull softens its seams:** the lines along a curved wall's sides
  are soft and smooth where the faces on both sides meet without a crease
  (the two halves of a cylinder; a rounded corner and its flat
  neighbours). Sharp corners stay plain.
- **Drawing:** soft edges are left out of the scene (`Scene::drop_soft`),
  so nothing draws, picks or snaps to them, unless View > Hidden Geometry
  is on: then they are drawn as ghosts, like hidden objects. A new
  session drops them too (it showed them until the first edit).
- **Smooth shading:** where faces meet along a smooth edge, their mesh
  points there share the average normal, so flat faces shade across too.
  Points are matched by exact position (corners; an arc's inner points
  are only matched where both faces compute them the same way).
- **Eraser:** Ctrl softens and smooths instead of erasing; Ctrl+Shift
  makes edges plain again (with Hidden Geometry on, to reach soft ones).
  Guides are left alone while softening. The hint says so.
- **Edit > Soften/Smooth** (`edit.soften`) is active (DECISIONS #20): it
  softens the selected edges, or makes them plain if all are soft. It is
  in the edge context menu already. `Command::Soften { edges, soft }`.
- **Picking:** a click on a face of a smooth surface picks every face
  joined to it by soft edges (`prism_ops::select::surface`); a
  double-click adds their outlines, leaving out soft edges.
- **Files:** geometry layout 4 (soft and smooth per edge), schema 5. The
  golden files were rewritten; copies of the schema 4 ones are kept in
  `docs/acceptance/files/schema-4` and open the same. A new golden file,
  `cylinder.prism`, holds a pulled disc (curved faces, soft seams).
- **MCP:** `get_geometry` marks edges `soft` and `smooth`; Edit >
  Soften/Smooth reaches `run_action`.
- Tests: soft seams of a disc and a rounded rectangle; a smooth surface
  picks whole; flags kept through a split; the scene drops soft edges and
  shows them as ghosts; shading across a smoothed fold; the file round
  trip; the Eraser with Ctrl and Ctrl+Shift; Edit > Soften/Smooth; MCP;
  Soften in the undo property test (600 cases run once).
- Not built: an angle setting for Soften/Smooth (the brief leaves it for
  later); profile lines along a curved surface's outline (the renderer
  draws profiles only along edges).

## Exact Curves (part 4, step 4d): Push/Pull on a cylinder

- **A new radius:** Push/Pull on a cylinder face changes its radius
  (`prism_ops::radius`). Every face on the same cylinder joined to it by
  shared points follows (both halves of a pulled disc, soft seams or
  not). Points move straight out from the axis, and arcs round the axis
  that touch them take the new radius, so the caps follow. The axis stays.
- **The distance** is the change of radius along the face's normal: out
  for a cylinder's outside, towards the axis for a hole's wall. The tool
  pulls along the normal at the click (not at the face's first corner).
- **Refused:** a radius of zero or less (`RADIUS_GONE`), and a cylinder
  whose other neighbours would bend: only caps square to the axis and
  arcs round it follow, so a rounded corner between flat sides is refused
  (`CURVED_PULL`, reworded now that cylinders pull).
  `pushpull::refusal` takes the distance now.
- **Not built:** the new radius is not stopped by faces in its way
  (`reach::limit` lets it through), nor cut into a solid it meets; Alt
  (new loop) does nothing different on a cylinder; shrinking a cap that
  holds other edges inside it is not checked for crossings.
- **MCP:** nothing new; `use_tool` with `tool.push` reaches it (tested).
- Tests: both halves and the caps follow, out and in, with the exact
  volume; a hand-built cylinder; a hole's wall shrinks; a rounded corner
  is refused; zero is refused; a property test of random radii; the
  session by clicks and typing, with undo; undo and redo in the document
  test (a new kind, last); MCP.

## Fixes from the owner's session 5 (2026-11-09): a wedge on a box

- **Push/Pull:** an edge slides only when its neighbour also faces away
  from the pulled face across it (the walls under a box's top). A box's
  side facing a triangle drawn against it now gets a wall instead: the
  side used to be squashed flat, leaving the wedge open with two walls
  facing in. This refines DECISIONS #30's "runs along the push behind the
  face"; the owner should confirm.
- **Push/Pull:** a face with free sides drawn against a solid keeps its
  base when it extrudes whole, so the wedge is closed. A flat drawing (a
  ring round another face) still keeps none (DECISIONS #14).
- **Drawing:** a line drawn over a side of a flat loop with no face fills
  it, as in SketchUp (it used to need a line across the opening).
- Lines between coplanar faces after a pull stay (the stairs rule, "split,
  never merged"); erasing one merges the faces. The owner asked to look at
  the shape rules against SketchUp as a whole.
- Tests: the wedge (`pushpull_feedback.rs`), retracing (`sticky.rs`).

## Exact Curves (part 5, step 5a): fillet and chamfer of an edge

- **Brief 26** splits part 5 into 5a (fillet and chamfer), 5b (cones,
  spheres and tori in the core), 5c (Follow Me along lines), 5d (Follow
  Me along arcs, Alt for a face's perimeter) and 5e (the curve tools
  become active). Follow Me follows SketchUp's rules (the profile is
  projected square to the path's first segment; the end is square to the
  last).
- **`prism_ops::round`:** a straight edge between two flat faces at an
  angle (convex or concave), with plain corners at its ends, is cut off
  flat (chamfer: each face cut back by the size, square to the edge) or
  rounded (fillet: a cylinder face tangent to both, soft and smooth where
  it meets them). The faces across the ends get a line or an arc. A
  fillet needs those faces square to the edge (else an ellipse:
  `SLANTED_END`, until DECISIONS #32). Other refusals: `NOT_AN_EDGE`,
  `TOO_WIDE`. `Command::RoundEdge { edge, size, fillet }`.
- **The Fillet / Chamfer tool** (`tool.fillet`, greyed until 5e, MCP
  `use_tool` reaches it): click an edge to round it with the size (a
  tenth of the narrower face when none was typed); Ctrl chamfers; typing
  a size + Enter sets the next one and changes the last (a refused size
  leaves the last as it was, with the reason in the hint). A click near a
  flat corner rounds the corner (brief 24's fillet).
- **Model Health fix:** the reversed check went by rays through a closed
  shell's flat faces only, so a shell with a cylinder face could look
  inside out (a large fillet showed every face reversed). With curved
  faces it now goes by the enclosed volume (`health::face_volume`, exact
  for cylinder faces).
- Not built: fillets where filleted edges meet (a corner of three), Shift
  for tangent-connected edges, a ghost preview before the click (the
  click applies at once; typing adjusts it).
- Tests: a cube edge chamfered and filleted (exact volumes), a concave
  edge, each refusal, the default size, a property test of random rounds
  on random boxes (3000 cases run once), undo and redo, the session tool
  (clicks, typing, Ctrl, a refused size), MCP.

## Fixes from the undo property test (2026-11-09, after 5a)

- **macOS CI failed** on a random sequence: a pie, two offsets, an array.
  The second offset drew an arc 2⁻¹⁸ m off the pie's own circle (its
  validity check only looks at the face's own loops); the array made the
  model bigger, so its relative tolerance grew past that gap, and the
  coplanar rule merged the two arcs' ends, leaving one off its circle.
  Fix: an arc drawn within ten tolerances of an existing circle (same
  centre and axis) lands on it exactly, as points snap to vertices.
- **Loops closed back onto their start** (found at 3000 cases): rebuilding
  a plane after a rotate gave a loop whose last point repeated its first,
  naming an edge from a point to itself. A face's loops now drop a point
  repeated next to itself when linked.
- **Move of one corner of a face with a hole (fixed):** lifting the
  corner bent the face, and flattening it moved its hole's points too,
  which the ring inside shares, leaving the ring bent (later edits then
  built odd faces from it). A move that would bend a face with holes is
  now refused, like one that would fold a face with arcs
  (`HOLE_FACE_WOULD_FOLD`); a move that keeps it flat is fine. Folding
  faces with holes (auto-fold, Alt) is not built. The undo property test
  ran 9000 cases clean after this, and the core properties 3000.

## The Scale tool's box and grips (owner, 2026-11-09)

- **Asked for:** a box like the group box (5% off the selection) with
  handles, Alt to scale about the centre, and no inference (it jumped
  between an axis and the ground).
- **Built (`session/scalebox.rs`):** with Scale, a click on something
  picks it with what is connected (with nothing selected), and a dashed
  box shows round the selection (and selected groups), 5% off it on
  every axis it has size along; a flat selection keeps a flat box.
  Grips sit at the corners (scale evenly), edge middles (scale both of
  that edge's axes) and face middles (scale one axis). Dragging a grip
  scales about the opposite grip, along the line from it through the
  grip (no inference; within 2% of 1× snaps to 1×). Alt (a tap)
  switches to scaling about the centre. Shift: a face grip scales
  evenly; an edge grip scales each of its axes on its own. Typing a
  factor + Enter applies it to the grip's axes; typing again after
  changes the last scale. Where grips overlap on screen, the nearer wins.
- PLAN 1.6's Scale row is updated (Ctrl was "about centre"; the owner
  chose Alt).
- **Later (owner):** locking an edge or point and scaling from two points
  with inference; Shift on a corner grip (each axis on its own) is not
  built; the box follows the world axes (a group's own axes later).
- Tests: corner grip by typing, Alt about the centre, a face grip, a
  drag along the anchor line with one undo, a cube's top grip.

### Owner's bugs (2026-11-09, session 2 recording)
- **Erasing a line across a face removed the face.** The line ran from
  midpoint to midpoint, so the merged face's middle (the point used to
  find which old face covered it) lay on the erased line, inside neither
  half. Now, when that point finds no face, the region's own edges decide:
  an old face that runs along one of them with the region on its side
  covers it (exact, no sampling). The old test that said both halves go
  had passed only because of this bug; it now expects one face
  (DECISIONS #15).
- **The Scale box did not follow the drag.** It is drawn scaled by the
  current factors about the anchor.
- **Push/Pull snaps to an edge's middle** ("Match Midpoint"), as SketchUp
  does, when the middle is in sight (not behind a face); an endpoint as
  near wins. Hidden endpoints are still matched, as before.
- Tests: a line through each face of a box, both ways, erased; the drag
  test checks the grips move; a pull matched to an upright edge's middle.

### Owner's session 4 (2026-11-10): a seam drawn as an outline from some views
- The line was the seam round a face left inside the solid: Push/Pull's
  new loop (Alt) keeps the face it starts from, as SketchUp does, so the
  seam's edges have three faces (two walls and the inside face; Model
  Health lists it as an inside face). The renderer judged outlines by an
  edge's first two faces; after a wall was erased and drawn back, those
  were a wall and the inside face, so the seam was an outline from views
  where one faced you and the other away. Now, where two of an edge's
  faces lie flat on one side, those two are used. Test: `prism-view`
  `tests/inside.rs`.

## Exact Curves (part 5, step 5b): cones, spheres and tori in the core

Built 2026-11-10 (brief 26). No new tool: Follow Me (5c, 5d) makes these.

- **Surfaces** (`prism-geom/src/revolved.rs`): `Cone` (apex, axis into the
  cone, half angle; one nappe), `Sphere` (centre, the axis it was turned
  about, radius) and `Torus` (centre, axis, major and minor radius), each
  with the side it faces. Distance, nearest point (`project`), normal,
  flipped, moves (`Surface::transformed`; an uneven scale gives None, and
  `transformed_loosely` the nearest even one), and which curves lie on them
  (`holds`): lines through a cone's apex, circles round the axis, any circle
  on a sphere, a torus's tube circles.
- **One description for all turned surfaces:** `Revolution` gives a place
  on any of them (cylinders too) as θ round the axis and s along the
  profile. `Model::param_loop` walks a face's loop in (θ, s) without jumps;
  a point on the axis (a pole, an apex) comes twice, at the angle of the
  side before and after, crossing the way an outward face goes. The model
  check uses it for winding, and the volume uses it too (Green's theorem in
  (θ, s), Gauss-Legendre along the profile sides): a sphere, a cone and a
  torus give their exact volumes at 1e-3, 1 and 1e3 m.
- **The duplicate check:** on a sphere or torus, a loop walked one way and
  walked back are two faces (the two halves of a sphere between two half
  circles), so it only counts faces walking a loop the same way.
- **Push/Pull** refuses a cone, sphere or torus face, and faces beside one
  (`CURVED_PULL`): no new size for them yet.
- **Model Health:** points off these surfaces are found and put back
  (nearest point). A point on two circles (a torus's corner) is now moved
  onto each in turn until it settles where they meet (it used to follow
  the first only).
- **Files:** geometry layout 5, schema 6. The new surfaces were added at
  the end of the stored list, so layout 4 entries read unchanged (layout 5
  is layout 4 with more kinds of surface). Golden files rewritten; the
  schema 5 ones are kept in `docs/acceptance/files/schema-5`, and a new
  golden file `turned.prism` holds a sphere, a cone and a torus.
- **Drawing** (`prism_view::tess::turned`): a grid over the face's extent
  in (θ, s), with as many steps as an arc of that sweep is drawn with, so
  the grid meets the edges at their own drawn points, and points on the
  outline reuse the outline's own points (no cracks between faces). A face
  that is a rectangle in (θ, s) (every face Follow Me makes) is one shared
  grid: a torus draws in about 4 ms (release). Other faces are triangulated
  in (θ, s) and cut by the grid. Normals come from the exact surface. Slivers
  at a pole are turned to face out. Mesh export uses the same grid.
- **Picking:** a ray meets the drawn triangles, then a few Newton steps put
  the point on the exact surface (tested to 1e-12).
- **MCP:** `get_geometry` lists `cone` (apex, axis, half angle in degrees),
  `sphere` (centre, axis, radius) and `torus` (centre, axis, major and minor
  radius), each with `outward`.
- **Test shapes:** `prism_tools::scenes::{sphere, cone, torus}` build them
  by hand (two lunes; two cone halves and a base; four torus patches).
- **Tests:** geom unit tests; `prism-tools/tests/turned.rs` (check, exact
  volumes, moves and mirrors, shading on the surface, picking, Model Health,
  the general grid path); a screenshot test (no back face shows from two
  views); golden files; mesh export; MCP.
- **Known gaps:** the grid is fixed (128 steps per turn each way: a torus
  is 32k triangles); faces must not go all the way round their axis (seams
  split them, as for cylinders); a lune's winding can't be checked (at the
  poles the crossing direction is chosen from the face's side); Intersect
  Faces, sticky crossings and `reach::limit` ignore these surfaces;
  Push/Pull gives them no new size.

## Exact Curves (part 5, step 5c): Follow Me along lines

Built 2026-11-11 (brief 26).

- **The operation** (`prism_ops::follow`): a flat profile face swept along a
  chain of lines. The path starts at the end (or, closed, the corner)
  nearest the profile; lines in a row are merged. The profile is projected
  square to the first line (moved only if no other face shares its points,
  else refused), carried along each line to the corner's halving (mitre)
  plane, and ends square to the last line. An open path keeps the profile
  as the start cap and adds an end cap; a closed path makes a ring and the
  profile face goes.
- **Walls:** profile lines make flat walls; profile arcs make cylinder
  walls, so only on a single straight line square to them (`ARCS`
  otherwise). Where the profile turns smoothly into or between arcs, the
  lines along the sweep are soft and smooth.
- **Refusals** (through `Command::refusal`): no path (`NO_PATH`), not one
  chain of lines (`NOT_A_PATH`, also when the path uses the profile's own
  edges), not a flat profile, a profile along the first line, a tilted
  profile joined to other faces, arcs at a corner or tilted, and a corner
  too sharp for the profile's width (a point would travel backwards to the
  mitre: `CROSSES_ITSELF`).
- **A closed path always closes.** Carrying points along each line onto
  the corners' halving planes brings the profile back exactly, flat path
  or not (checked numerically and by test), so `RING_TWISTS` is only a
  safety net.
- **The command** `Command::FollowMe { path, profile }` ("Follow Me" in
  Undo). **The tool** `tool.followme` (`session/follow.rs`): select the
  path, then click the profile face; with nothing selected the hint says to
  select a path first; a refusal shows its reason in the hint. Greyed
  until part 6 ends; MCP `use_tool` reaches it.
- **Tests:** an L path (mitred, exact volume at three sizes), a ring (and a
  ring whose path is not flat), a disc along a line (a cylinder, soft
  seams, exact volume), a tilted free profile squared up, each refusal; the
  tool by clicks with undo; MCP `use_tool`.
- **Known gaps, sorted (DECISIONS #33):**
  - A sweep that overlaps itself further along (a profile wider than a
    later line, turning back) is not caught: only folding at a corner is.
    Step 6c (Push/Pull booleans) adds the solid overlap test and uses it
    here too. *(Done in 6c-2.)*
  - The sweep is not stuck to what it touches (no welding or splitting
    against existing faces). Step 6b (sticky crossings) covers it. *(Done in 6b-2.)*
  - A path through the profile's own face (drawn from its middle, as in
    SketchUp) is not tested: step 5d's tests add it.
  - Arcs in the path and Alt (a face's perimeter): step 5d. *(Done in 5d.)*


## Exact Curves (part 5, step 5d): Follow Me along arcs, and Alt

Built 2026-11-11 (brief 26). Greyed like the other curve tools until part
6 ends; MCP `use_tool` reaches it.

- **Paths of lines and arcs** (`prism_ops::follow`). `chain` now walks
  edges, not points, so a circle's two halves (two arcs joining the same
  two points) make a path; it returns the points, the pieces and whether
  the path closes. Lines in a row still merge; arcs never do.
- **Along an arc the profile turns about its axis.** The profile must lie
  in a plane through the arc's centre and axis (`OFF_AXIS`) and stay on
  one side of the axis (`CROSSES_AXIS`). If the path starts with an arc
  and the profile is elsewhere round the axis, it is turned to the start
  (a free profile with no arcs only, as squaring up in 5c: `NOT_SQUARE`).
  Where a line or an arc meets an arc they must be tangent (`NOT_TANGENT`);
  between two lines the mitre stays.
- **What the sides make:** along an arc, a profile line square to the axis
  makes a flat ring (a pie slice when it starts on the axis), one parallel
  to it a cylinder, any other a cone (its apex where the line meets the
  axis); a profile arc makes a torus, or a sphere when centred on the
  axis. A point on the axis stays one vertex (a pole: walls there are
  triangles, or two-sided lunes for a half circle from pole to pole); a
  profile line along the axis makes nothing and goes. Each wall faces
  outward by comparing the surface's normal at the side's middle with
  t × f (along the side, along the path), so one rule covers lines and
  arcs, open and closed paths.
- **Arcs in the profile** need a rigid motion from the profile to every
  section (`Plan::motions`): a move along a line square to the profile, a
  turn about an arc. A mitre or a projection has none, so arcs there are
  refused (`ARCS`, as in 5c; the check is no longer "more than one line").
- **Soft and smooth:** the edges along the path where the profile turns
  smoothly (as 5c), and now also the section edges where the path turns
  smoothly (an arc meeting a line or arc tangentially, a circle's halves).
- **Alt** (tapped, like Push/Pull's Alt) in `tool.followme`: the path is
  the outer loop of the one selected face (`NO_FACE` otherwise). The hint
  says which mode is on.
- **Tests:** `prism-ops/tests/follow_arcs.rs` (a tube at three sizes, a
  sphere from a half disc, a cone from a triangle, a torus, a quarter-arc
  elbow, lines and a tangent arc, a profile turned to an arc's start, each
  refusal, a path drawn from the middle of the profile as in SketchUp);
  `prism-tools/tests/turned.rs` (Follow Me spheres and tori shade like the
  hand-built ones); the session test for Alt.
- **Known gaps, sorted (DECISIONS #33):**
  - A sphere's profile arc of more than half a circle (centred on the
    axis) reaches round past the poles and overlaps itself: not caught.
    It goes with 6c's solid overlap test, with 5c's overlap gap. *(Done in 6c-2: refused, test `a_sphere_profile_past_the_poles_is_refused`.)*
  - A profile arc whose circle reaches the axis but whose arc does not (a
    lemon or apple shape) is refused (`CROSSES_AXIS`): a torus with a
    minor radius at least its major one is not handled. Minor: NOTES only.
  - A profile with arcs, or joined to other faces, away from an arc's
    start is refused rather than turned there (turning would need its arcs
    moved too). Minor: NOTES only; draw the profile at the start.
  - Two halves of a flat ring meet at a soft seam where SketchUp would make
    one face (a face may not go all the way round). Minor: NOTES only.
  - 5c's gaps stand: overlap further along (6c), not sticky (6b).

## Exact Curves (part 6, step 6a): the segment fallback

Built 2026-11-11 (brief 27, DECISIONS #32).

- **Approximate edges** (`Edge.approx`, `Model::set_approx`,
  `Model::add_approx_curve`): fine straight segments standing in for a
  curve Prism cannot store exactly. The segments of one curve share a curve
  id. Their ends are exact; the chords are not. The model check accepts one
  on a curved face when both ends lie on the surface. `Model::unrolled`
  now counts the turn a straight edge makes round a cylinder's axis (zero
  along it), so an ellipse's segments unroll properly.
- **Files:** geometry layout 6, schema 7 (migration 6 to 7 does nothing);
  schema 6 golden files kept in `docs/acceptance/files/schema-6`; a new
  golden case `approximate.prism` (a cylinder scaled unevenly).
- **`prism_ops::approx`:** `fine_count` (a chord strays at most 1/1000 of
  the radius; a curve never has fewer segments than an arc is drawn with),
  `segment_arc(s)`, `facet` and `flatten_for`.
  - **Uneven Scale** (`transform::scale` and `affine`, so Scale, Explode and
    placing groups) no longer refuses a whole arc (`ARC_WOULD_BEND` is now
    only for bending part of one): before the scale, each curved face it
    moves whole becomes flat facets and each such arc fine segments, all
    marked approximate. Facets come from a grid in (θ, s): on any turned
    surface, two points on each of two circles about the axis at the same
    two angles are flat (an isosceles trapezoid), so the facets stay flat
    under the scale and the points land on the true ellipse, elliptic
    cylinder or ellipsoid. A surface's facets follow the chord rule alone
    (about 71 a turn): with the drawing count both ways a sphere was 8,192
    facets.
  - **Follow Me** (`follow::best`): when the exact sweep is refused for
    want of an ellipse (`NOT_TANGENT`, `OFF_AXIS`, `ARCS`, `NOT_SQUARE`), it
    tries again with the path's arcs as fine chords, then the profile's arcs
    as fine segments, then both. The result is faceted, its seams soft,
    smooth and approximate. A profile lying past the first piece of the path
    (a short chord) starts at the path's start instead of where it stands.
  - **Fillet with a slanted end** (`round`): the end gets an approximate
    ellipse: the fillet's circle square to the edge at the corner, each
    point carried along the edge onto the slanted plane (so every point is
    on the cylinder and the end face).
- **Model Health:** a row "Approximate curves" (information: not counted
  as a problem, no fix, click to select). MCP `get_health` has it as
  `approx`; `get_geometry` marks approximate edges `approx`.
- **Tests:** `prism-tools/tests/approx.rs` (a disc, a cylinder, a sphere
  and a torus scaled unevenly: points on the true shape, check clean,
  volume within the chords' error; an even scale stays exact),
  `prism-ops/tests/follow_fallback.rs`, the slanted fillet in
  `prism-ops/tests/round.rs`, the golden file, the MCP test.
- **Known gaps, sorted (DECISIONS #33):**
  - Only curved faces that are rectangles in (θ, s) can be faceted (every
    face Push/Pull, Fillet and Follow Me make today). Once 6b cuts curved
    faces, a cut face scaled unevenly would keep a wrong surface (the
    check reports it): **step 6b** makes `approx::facet` handle cut faces. *(Done in 6b-2.)*
  - Follow Me's fallback facets the whole sweep where the brief hoped for
    exact cylinder walls ending on approximate ellipses. It looks smooth
    (soft, smooth seams) and is within the chord rule. Minor: NOTES only.
  - The model check is slow on thousands of facets (about 2.5 s in a debug
    build for a sphere's 2,592): it compares faces in pairs. Minor for now
    (Model Health and tests); NOTES only, look again if big imports suffer. *(Fixed 2026-11-12: see "Known gaps session".)*
  - A line meeting an arc at a corner, with a wide profile, is still
    refused (`CROSSES_ITSELF`): the arc's first chord is short, and the
    sweep really does fold there. Correct, not a gap.

## Exact Curves (part 6, step 6b-1): planes meet curved faces exactly

Built 2026-11-11 (brief 27). Step 6b was split as the brief allowed:
**6b-1** (crossings, cutting curved faces, Intersect Faces) is this;
**6b-2** (sticky geometry: drawing across curved faces, Follow Me's result
welded, and faceting cut faces) is next.

- **Cutting a curved face** (`prism_ops::cut::split_face`,
  `split_curved`): the edges lying on a face's surface inside it split it,
  by the same region tracing flat faces use, done in (θ, s). Each loop is
  walked side by side and unwrapped; a pole (or a cone's apex) is a short
  stretch along s = constant between the angles of the sides at it, and a
  cut ending at the pole lands on that stretch at its own angle. Islands
  become holes. A face laid out clockwise (pointing into its surface) is
  mirrored for the tracing. Faces that wind round the axis (a band with no
  seam) cannot be laid out and are left alone (Prism never makes one).
- **Where a curved face crosses another face** (`prism_ops::crossing`):
  traced on a grid in the curved face's (θ, s) (half the chord rule's
  step), every point then put on the crossing by Newton steps; cut where
  it leaves either face, each end put exactly on the edge it crosses (on
  that edge and on the other surface). A crossing whose exact points all
  lie on one line or circle is kept as that line or arc, so the exact
  cases of the table (a plane square to the axis or through it, any plane
  through a sphere, a plane parallel to a cylinder's axis, surfaces on one
  axis) need no case list. Everything else (a slanted cut through a
  cylinder or cone, two cylinders crossing) is fine segments marked
  approximate, chosen from the traced points by the chord rule.
  Two faces on one surface (neighbouring halves of a cylinder) are skipped.
- **Intersect Faces** now includes curved faces, and flat faces with arcs
  (a cylinder's end discs): those are clipped by their drawn outline and
  the ends then put exactly on the arcs. **Model Health**'s Crossing faces
  row finds curved crossings too. MCP: `run_action isect.model` cuts a
  cylinder (tested).
- **The model check** lays out every curved face in (θ, s) by
  `param_loop` (cylinders used the older `unrolled`, which starts each
  loop at 0, so a hole in a cylinder face was reported outside it), and
  moves each hole by whole turns into its outer loop.
- **Fixed on the way:** an arc drawn between two points already joined by
  an arc of the same centre and radius in another plane was taken for that
  arc and not added (`sticky::add_arc_from`'s "the same arc" test only
  checked that the axes pointed the same half-way).
- **Tests:** `prism-ops/tests/cut.rs` (a cylinder cut square to its axis,
  through it and slanted; a sphere cut by a slanted plane and through its
  poles; a torus cut square to its axis; a box through a cylinder; two
  cylinders crossing; check clean, volumes kept, nothing left to cut, Model
  Health), `curved.rs` (a square through a cylinder and its discs), the
  MCP test.
- **Known gaps, sorted (DECISIONS #33):**
  - Faceting a cut curved face (an uneven Scale of it) still needs a
    rectangle in (θ, s) (6a's gap). Until then an uneven Scale of a curved
    face that is not one is refused (`CUT_CURVED_SCALE`, checked with
    `approx::can_facet`; the undo property test found the wrong surface it
    left before): **step 6b-2** facets any loop and drops the refusal.
    *(Done in 6b-2.)*
  - Two crossings that cross each other in the middle of a curved face are
    not split where they meet, so the face is not cut along them there.
    Intersect Faces' crossings meet at their ends in every case tested;
    drawing across a face (6b-2) needs it: **step 6b-2**. *(Done in 6b-2.)*
  - Drawing a line, rectangle or circle across a curved face does not cut
    it yet; Follow Me's result is not welded where it meets other faces;
    snapping only sees crossings once they are edges: **step 6b-2**, as
    planned. *(Done in 6b-2.)*
  - A crossing smaller than a grid step (about 2.5°, or a plane barely
    grazing a sphere) can be missed, and a plane only touching a surface
    (along a line, or at a point) adds nothing. Minor: NOTES only.
  - Fine segments follow the chord rule alone, not the drawing minimum
    `fine_count` uses. Within the 1/1000 rule; minor, NOTES only.
  - A crossing ending on an approximate edge ends on its chord, not on the
    curve the chord stands for (it cannot do better), and the face it lies
    on is then not cut there. Minor: NOTES only.

## Exact Curves (part 6, step 6b-2): sticky geometry on curved faces

Built 2026-11-11 (brief 27). Step 6b is done.

- **Drawing on a curved face splits it** (`cut::split_along`, called by
  `sticky::add_segment` and the arc and circle drawing): a line along a
  cylinder or through a cone's apex, a circle round the axis, any circle on
  a sphere. A straight line across a curved face's middle is not on it and
  splits nothing (as before).
- **Crossings that meet in a curved face's middle** are split where they
  meet before the face is cut (`cut::split_meetings`): exactly where both
  are lines or arcs (`sticky::meet`), else at the crossing found in
  (θ, s), on the surface (and on the exact one of the two). Edges are
  compared by box, swept along θ.
- **Follow Me sticks** (`follow::stick`): its new points weld onto old
  ones, the planes it touches are rebuilt as after Push/Pull (an end flush
  with a wall cuts the wall: the wall gets the end as a hole), duplicates
  go, and curved faces its edges lie on are split.
- **Faceting any curved face** (`approx::facet`, rewritten): grid lines in
  (θ, s) are laid across the face as soft, smooth, approximate edges, the
  face's own edges split exactly where they cross them (also where a line
  passes through one of an arc's drawn points), and the face cut along them
  by `cut::split_face`. A whole cell is flat (an isosceles trapezoid on any
  turned surface); a cell the outline crosses is cut into triangles, fanned
  from one of its points or from a new point inside on the surface, so no
  triangle is made of the cut's points alone (it would lie in the cutting
  plane, over the flat face there). Uncut faces give the same facets as
  before (2,592 for a sphere, 5,184 for a torus). An uneven Scale of a cut
  curved face now works; `CUT_CURVED_SCALE` is left only for a face that
  cannot be laid out (a band round the axis, which Prism never makes).
- **Fixed on the way:**
  - duplicate faces (`settle::dedupe_faces`) are found by their sides
    (start and edge), not their corners, and the weld's "same face" check
    leaves curved faces to it: a sphere's two halves both run pole to pole
    and one was dropped as a copy of the other;
  - a curved face's box (`cut::face_box`) samples its surface: a sphere's
    half has its outline in one plane, so a box round the outline missed
    the half itself (Intersect Faces and the splitter skipped it);
  - the splitter takes a face's own points as on it even where they lie on
    an approximate side's chord.
- **Snapping** finds the crossings (they are edges): tested in
  `prism-infer/tests/curved.rs`.
- **Tests:** `prism-ops/tests/sticky_curved.rs` (a line along a cylinder;
  a circle round one, then a line crossing it; two crossings meeting
  mid-face, exact meeting points; Follow Me ending on a wall), the cut
  cylinder scaled unevenly in `cut.rs`, the snap test, and an MCP test (a
  line drawn up a cylinder with `use_tool` splits it).
- **Known gaps, sorted (DECISIONS #33):**
  - A straight segment drawn across a curved face (not lying on it) does
    not split it where it passes through, and a flat face drawn through a
    curved one is not cut until Intersect Faces: as in SketchUp, faces
    passing through each other are cut by Intersect Faces only. Not a gap.
  - Faceting cuts the cells its outline crosses into triangles; on a
    cylinder (one cell from bottom to top) a slanted cut leaves long thin
    triangles. They are flat and within the chord rule. Minor: NOTES only.
  - The faceted outline's pieces follow the grid, not the chord rule, where
    a slanted curve crosses a cell corner to corner (up to about 1.4 times
    the rule's stray). Minor: NOTES only.
  - The model check is slow on many facets (10 s in a debug build for a
    torus's 5,184); unchanged from 6a. Minor: NOTES only. *(Fixed 2026-11-12.)*

## Exact Curves (part 6, step 6c-1): Push/Pull booleans with curved faces

Built 2026-11-11 (brief 27). Step 6c was split as 6b was: **6c-1** (the
inside test with curved faces, and Push/Pull booleans) is this; **6c-2**
(stops at curved faces for a pull and a new radius, and the solid overlap
test for Follow Me) is next.

- **The inside test** (`prism_ops::shell`, used by `solid::Swept` and
  `solid::keep_boundary`): a ray from a point counts the faces it crosses.
  Flat faces have exact arcs: an outline with each arc's chord, then the
  arc's piece of its circle beyond the chord added or taken away. A ray
  meets a curved surface where the distance to it changes sign along the
  ray, found in steps of a twentieth of the surface's size and halved down
  to the crossing (no quartic for a torus; a ray grazing the surface
  between steps crosses it twice or not at all, which counts the same).
  A crossing near a face's outline, or tangent, is unsure: the next ray.
- **Push/Pull** is now a boolean with arcs and curved faces too: the
  exclusion of faces with arcs and of models with curved faces is gone.
  `keep_boundary` splits the touched faces where they cross others with
  `intersect::all_crossings` (arcs exact) and `crossing::curved_crossings`,
  and tests a point inside each face (on a curved face, from its (θ, s)
  layout, `shell::interior`). New edges lying on a curved face split it
  (`cut::split_along`), so a wall on a cylinder's own surface takes its
  piece of the cylinder with it.
- **Part of a cap pushed in** (an arc side that would slide down a
  cylinder with its ends staying): on a solid, every side gets a wall and
  the solid rule takes what ends up inside (a notch in a cylinder's rim);
  off a solid it is refused as before.
- **Fixed on the way:**
  - Push/Pull's check for an arc that would slide compared the side's
    points sorted with the slide list's, which keeps them in walking
    order, so it missed half the cases (the neighbour then got a straight
    chord where the arc was);
  - `face_at` goes by the drawn outline, so it finds a disc (it went by
    corners: a disc has two);
  - a crossing traced as only its two ends gets a point between them on
    the crossing first, so a short arc is not taken for a line.
- **Tests:** `prism-ops/tests/booleans_curved.rs` (a disc pushed into a box:
  blind hole, then through; a slot pushed across a hole's wall; a notch in
  a cylinder's rim; a cup in a cylinder's cap; a hole pushed up into a
  cone; all with exact volumes and the check clean), the inside test on a
  sphere and a torus (`shell` unit test). `cut.rs`'s box through a
  cylinder is now placed, not pulled (pulled, it joins the cylinder).
- **Known gaps, sorted (DECISIONS #33):**
  - A pull stops at the first face in its way only by corners and edges
    when that face is curved, and a new radius is not stopped at all:
    **step 6c-2**, as planned. *(Done in 6c-2.)*
  - The solid overlap test (a Follow Me sweep overlapping itself further
    along): **step 6c-2**, as planned. *(Done in 6c-2.)*
  - Model Health's "faces inward" check still counts flat faces only (it
    goes by the enclosed volume when a shell has curved faces, as before).
    Minor: NOTES only. *(2026-11-12: the volume rule is exact for a closed shell, but checking it found a real bug: a cylinder's rim, two arcs between the same two points, cut the caps off from the sides, so an inside-out cylinder was half found. Fixed; see "Known gaps session".)*

## Exact Curves (part 6, step 6c-2): stops at curved faces, Follow Me overlap

Built 2026-11-11 (brief 27).

- **A pull stops at a curved face** (`reach::limit` with the new private
  module `prism_ops::meet`). The face moves until it first meets the other
  face. Three places can be first:
  - one of the other face's edges, sampled, with each lowest point found
    exactly and each place where an edge leaves the moving face's outline
    found by halving;
  - a ray from the moving face's outline, in the direction it moves, onto
    the other face, refined the same way;
  - a point where a sphere or torus faces the moving face head on, worked
    out exactly. A cylinder or cone faces it along whole lines, and those
    end on edges.

  Arcs on either face's outline use the same search, and the old
  edge-against-edge test now takes straight edges only (it used to treat
  arcs as their chords).
- **A new radius stops at what it would run into.** The same search runs
  on the cylinder's faces, whose points move straight out from or in
  towards the axis. Rays go out from their outline. Inside a curved face
  in the way, a grid search finds the nearest point. `push_pull` and the
  tool both apply the limit.
- **Follow Me refuses a sweep that runs into itself** (`follow::OVERLAPS`).
  The sweep is built in a copy, and any two of its faces that share no
  point and cross each other mean a refusal. That covers a profile wider
  than the gap to a later part of the path, or a path that comes back
  across itself. A profile arc that reaches across the axis between its
  ends (a sphere's arc of more than half a circle, past the poles) is
  refused as `CROSSES_AXIS`.
- **Fixed on the way:** a face pushed toward a parallel face stopped only
  if their corners overlapped, so a disc never stopped at another disc (a
  cylinder's cap pushed in passed the far cap). It now goes by the drawn
  outlines.
- **Tests:**
  - `prism-ops/tests/stops_curved.rs`: a plate pushed down onto a sphere
    (it stops at the top); a plate off to one side (it stops where its edge
    meets the sphere); a wall pushed sideways onto a cylinder, and one that
    passes over it; a cylinder's cap pushed in (it stops at the far cap); a
    new radius stopped by a wall, by a post inside, and by a sphere beside
    it; a pulled plate landing on a sphere.
  - `prism-ops/tests/follow_overlap.rs`: a square too wide for a path that
    comes back over itself is refused, and nothing is made; a narrow one
    passes, and the check is clean; a sphere profile past the poles is
    refused.
- **Known gaps, sorted (DECISIONS #33):**
  - A Follow Me sweep with one part wholly inside another, where their
    faces never cross, is not caught. A sweep of a single profile can
    hardly do this. Minor: NOTES only.
  - A face that already crosses the moving face is passed over, and the
    pull goes on to the next face, as with flat faces before. Minor: NOTES
    only. *(2026-11-12: not a gap. Stopping there would block every pull from a face that already touches another.)*
  - A ray from the outline that only grazes a curved surface between
    sample steps can be missed. The face then goes a hair past a surface it
    only just touches. Minor: NOTES only.
  - Follow Me builds the sweep twice, once to check it and once to make
    it, so big sweeps are slower. Minor: NOTES only. *(Fixed 2026-11-12.)*
  - Push/Pull on a cone, sphere or torus face is still refused: **step
    6d**, as planned. *(Done in 6d-1.)*

## Exact Curves (part 6, step 6d-1): new sizes for cones, spheres and tori

Built 2026-11-11 (brief 27). Step 6d was split: **6d-1** (new sizes) is
this; **6d-2** (outline lines) is next.

- **Push/Pull on any curved face gives it a new size** (`radius.rs`, now
  for every turned surface). Every face on the same surface that is joined
  to the pulled one moves square to the surface by the pull:
  - a sphere takes a new radius;
  - a torus takes a new tube radius;
  - a cone keeps its angle, and its apex slides along the axis;
  - a cylinder takes a new radius, as before.
- **Flat faces beside it follow** when their points can stay in their
  planes:
  - a plane the points move along anyway: a face through the axis, or
    through a sphere's centre (a half ball's base widens);
  - a round cut: a cap square to the axis, or any plane across a sphere.
    Its circle takes the new size where that plane cuts the new surface
    (a cone's base stays where it is, and so does a sphere's cap cut off
    at z = 0.5).

  Arcs are rebuilt through their moved ends and middle. Anything else
  beside it would bend and is refused (`CURVED_PULL`). A size that would
  reach zero is refused (`RADIUS_GONE`); so is a torus whose tube would
  reach its axis (`TUBE_AXIS`), and a cone whose points would pass the
  axis or whose cut would vanish.
- **The new size stops at what it meets** (`meet::Mover::Grow` now works
  on any turned surface: points move along the surface's normal). A
  growing sphere or torus meets a flat face head on, worked out exactly.
  This fixed a miss: the nearest point of a wall in the middle of the wall
  was not found.
- **Tests:** `prism-ops/tests/sizes_curved.rs`. A whole sphere, a half ball
  and a cap cut off at z = 0.5, a torus and a cone each take a new size,
  with exact volumes and the check clean. Each refusal is tested, and a
  growing sphere stops at a wall.
- **Known gaps, sorted (DECISIONS #33):**
  - A cone pulled so its apex would move while a flat face holds the apex
    (a cone cut through its apex by a plane square to the axis) is
    refused. Rare. Minor: NOTES only.
  - Where several flat faces meet at one point of the surface and they are
    not all along the move, the pull is refused, even where a single
    position would satisfy them all. Minor: NOTES only.

## Exact Curves (part 6, step 6d-2): outline lines

Built 2026-11-11 (brief 27).

- **Outline lines** (`prism_view::outline`). Where a smooth curved face
  turns away from the eye, a line is drawn along its outline, as SketchUp
  does with profile lines on smooth surfaces. Each drawn triangle of a
  curved face knows the surface's normal at its corners. The outline runs
  where the normal is square to the line of sight, and the line joins the
  two places on the triangle's sides where that happens. The lines are
  on screen only: not edges, not pickable, never saved.
- **Drawing:** `prism-render` keeps the curved faces' triangles from
  `set_scene`, and finds the outline again whenever the eye or the line
  style changes. It draws them with the edges, in the edge colour, as wide
  as the profile lines (or the edge width if profiles are off), faint
  outside the open group. Its own face never hides an outline line. There
  are no outlines in Wireframe, where faces are not drawn.
- **Tests:**
  - `prism-view/tests/outline.rs`: a ball seen from afar has a round
    outline where the line of sight grazes it, which moves when the view
    turns; a cylinder has outline lines up its sides, and none seen down
    its axis; flat faces have none.
  - The screenshot test `a_smooth_ball_shows_its_outline` in
    `prism-render/tests/screens.rs`: a ball with soft seams, from two
    views; the line is drawn along the outline, and not in Wireframe.
- **Known gaps, sorted (DECISIONS #33):**
  - The outline follows the drawn triangles, so it is within the chord
    rule of the true outline, not on it. Minor: NOTES only.
  - Outline lines hidden behind faces are not drawn dashed in the
    hidden-line modes, as hidden edges are. SketchUp does not draw them
    either. Not a gap.
  - The outline is found on the CPU whenever the eye moves. The cost grows
    with the curved faces' triangles (a few thousand for a torus, well
    under a millisecond). If big models make orbiting slow, move it to the
    GPU. Minor: NOTES only.

## Exact Curves (part 6, step 6e): the curve tools are active

Built 2026-11-11 (brief 27). **Exact Curves is done.**

- **The curve tools are active.** Circle (C), Polygon, the Arc tools (A,
  and Arc by two points, by three points, by centre, and Pie), Fillet /
  Chamfer and Follow Me are ordinary tools now (`session::TOOLS`): in the
  menus, on the dock, in command search, and on their hotkeys.
  `GREYED_TOOLS` is gone, and so is MCP `use_tool`'s special path for
  greyed tools.
- **Release:** this push carries `[release]`. The release notes
  (`packaging/release-notes.md`) start with what Exact Curves brings.
- **Tests:** `the_curve_tools_are_active` in the action registry. Each
  tool is built, enabled and runs as a tool, has a menu path, and Circle,
  Follow Me and Fillet are on the dock with their hotkeys. The MCP tests
  now expect the tools to be enabled.
- **For the owner to try by hand** (the brief's acceptance list):
  - a cylinder pushed through a box makes a hole;
  - a slanted cut through a cylinder;
  - the curve tools active.
- **Known gaps:** none new. Part 6's steps list theirs above. The ones
  kept for later chunks are the NURBS work (freeform surfaces, bending a
  curved face by moving a point) and booleans between groups (Inspector &
  Features).

## Basic Materials (part 1, step 1a): materials in the core

Built 2026-11-11 (brief 28). Part 1 (colours) is built in three steps:
**1a** (this, the core), **1b** (the Paint Bucket and colours in the
viewport), **1c** (the Materials window, Entity Info, Select by material,
Model Health, MCP, and a release). Nothing is visible yet.

- **Materials** (`prism_doc::material`). The document keeps one table of
  materials, with stable ids (`MaterialId`), shared by every group's
  model. A material is a name and a graph. For now the graph is one
  principled-surface node: base colour in linear RGBA (alpha is opacity),
  roughness and metalness, as in glTF (PLAN 4). `Material::colour` takes
  sRGB, as colour pickers show it. Names are kept unique ("Brick #2").
- **On faces:** `Face.paint` holds an optional front and back material
  (`prism_topo::Paint`). None is Default. Reversing a face swaps its
  front and back. **On groups and components:** an instance can be
  painted as a whole (`Assembly::painted`). Nothing is copied onto its
  faces (DECISIONS #35). Copies keep it, and erasing an instance drops it.
- **Commands** (undoable like every edit): `AddMaterial`, `EditMaterial`,
  `RemoveMaterial` (what used it goes back to Default), `PurgeMaterials`,
  `Paint` (face sides), `PaintInstances`, and `ReplaceMaterial` (in the
  open group or everywhere). A material the document does not have is not
  painted with. Helpers for the tool: `material::connected_same` (fill
  connected) and `sides_with`.
- **Paint kept through edits** (DECISIONS #36):
  - every piece of a split face keeps its paint, whether flat (the regions
    rebuild) or curved (`cut::split_face`, `approx::facet`), and so does a
    face folded by Move;
  - merged faces take the paint of the largest old face inside the
    region;
  - Push/Pull: the moved face keeps its paint, the stretched sides keep
    theirs, and new walls are Default;
  - Follow Me's walls and end take the profile's paint, on the outside;
  - Make Group copies faces with their paint.
- **Files:** schema 8. A new `materials.json` entry holds the table and
  the painted instances. Geometry layout 7 adds each face's paint. Schema
  7 files open unchanged. Golden files: `painted.prism` (painted faces,
  front and back, and a painted group). The schema 7 files are kept in
  `schema-7/`, and the current golden files were re-saved from them, not
  rebuilt, so only their format changed. (`approximate.prism` built from
  today's code differs slightly from the one saved in 6a.)
- **Tests:** `prism-doc/tests/materials.rs`:
  - unique names, purge, and remove (faces back to Default, undo brings
    them back);
  - replace and fill connected;
  - paint through a split, a merge, Push/Pull and Reverse;
  - a pulled lone rectangle (only the pulled face keeps its paint);
  - a group painted as a whole, its copy, and erasing it;
  - Follow Me's walls taking the profile's paint;
  - **a property test**: random lines across a painted cube's faces,
    lines erased again, and the whole moved never leave a face unpainted.
- **Known gaps, sorted (DECISIONS #33):**
  - A pulled lone rectangle's new base is Default, so its paint stays on
    the moved face only. SketchUp keeps the original face as the base,
    with its paint. Minor: check with the owner in 1b. *(Fixed 2026-11-12: the owner asked for it; the base shows the colour outside.)*
  - Fillet and chamfer faces are Default. SketchUp gives them a
    neighbour's material. Minor: NOTES only. *(Fixed 2026-11-12.)*
  - Imported meshes have no materials until part 3 (glTF, OBJ, 3MF), as
    planned.

## Basic Materials (part 1, step 1b): the Paint Bucket and colours

Built 2026-11-12 (brief 28). The Paint Bucket (B) is active, and faces
show their materials in the viewport. Choosing a colour waits for the
Materials window (1c): until then the bucket starts with the first
palette colour (Brick Red), and Alt picks up any material in the model.
No `[release]` yet (1c has it).

- **The tool** (`prism_tools::session::paint`):
  - a click paints the side of the face you see (`Scene::seen_front`,
    from the hit triangle's normals); a smooth surface (a cylinder's
    side) is painted as a whole, as a click selects it;
  - **Ctrl** fills the joined faces of the same material
    (`material::connected_same`); **Shift** replaces that material
    everywhere; **Ctrl+Shift** in the open group only
    (`Command::ReplaceMaterial`);
  - recordings save the brush (and its material), so a replay paints
    the same colour (`a_recording_replays_with_the_brush_it_started_with`);
  - **Alt** samples: held while clicking (`Input::Press` has a new `alt`
    field, read as false from older recordings), or tapped, so the next
    click samples (the hint says SAMPLE). Sampling picks up what is
    drawn, a group's material included; Default is a brush too;
  - a group or component inside the open one is painted as a whole
    (`Command::PaintInstances`); faces outside the open group are not
    paintable from inside it;
  - **the brush** (`Brush`: Default, a document material, or a palette
    colour). A palette colour is added to the document with the paint,
    as one undo step ("Paint"). The brush keeps a copy of its material,
    so after Undo took it away, the next click adds it again;
  - **the palette:** 16 plain colours in code (`material::PALETTE`). No
    textures are bundled (DECISIONS #34, owner);
  - **hover** previews the paint: the faces a plain click would paint,
    drawn lit in the brush's colour (`Session::paint_preview`, a new tint
    mode in the renderer). The cursor has a swatch and the material's
    name.
- **The viewport:** each face vertex carries its front and back colours
  (sRGB, alpha 0 for Default, which keeps the style's colours, so
  reversed faces still show blue). A group's material is drawn on its
  Default faces, the nearest painted group winning (`SceneFace.inherited`,
  set when the scene is built). The scene holds the colours
  (`Scene::palette`). Monochrome and Hidden Line ignore materials
  (`Style::materials`). X-ray keeps its see-through alpha. Picking, the
  id buffer and hidden lines are unchanged.
- **Tests:** session tests for each side, Ctrl, Shift, Ctrl+Shift in an
  open group, Alt held and tapped, Undo, a group and a nested group (the
  nearest wins), a cylinder's whole side and the preview; a screenshot
  (`painted_faces_show_their_colours`: colours, Monochrome without them,
  the preview); the registry test (`the_paint_bucket_is_active`).
- **Gaps closed the same day (owner asked, 2026-11-12):**
  - the preview follows the keys held: Ctrl shows the connected fill,
    Shift every face it would replace (Ctrl+Shift: the open group's),
    Alt held samples (no preview, the hint says SAMPLE). The app sends
    `Input::Held { ctrl, alt }` when they change;
  - a bucket cursor: over the viewport the arrow is hidden and a tipped
    pail pours the brush's colour onto the pointer's tip (an eyedropper
    while sampling), drawn by the app (`viewport::draw_bucket`);
  - recordings keep the document's materials too, so painted faces
    replay in their colours.
- **Known gaps, sorted (DECISIONS #33):**
  - MCP's `use_tool` cannot hold Alt yet: in 1c, with the `paint` tool.
  - Colours other than Brick Red, and editing them: the Materials window,
    1c.

## Known gaps session (owner, 2026-11-12)

The owner asked for a session of known gaps. Each was checked against
the code first; some had been closed by later steps (marked where they
are listed above).

- **Fillet and chamfer faces take their faces' colour:** the one they
  share, else the painted one (`round.rs`). Test:
  `a_rounded_edge_takes_its_faces_colour`.
- **Model Health: an inside-out cylinder.** The check that finds faces
  turned the wrong way linked neighbours by the two points an edge joins.
  A cylinder's rim is two arcs between the same two points, so the caps
  were never linked to the sides, and an inside-out cylinder was only
  half found (and Orient Faces could leave it half wrong). It now links
  them by the edge itself (`health::runs`). Test:
  `model_health_turns_an_inside_out_cylinder_round`.
- **The model check is fast on big models.** The overlap rule worked out
  each face's outline again for every pair of faces: 10 s for 1,600
  faces on one plane (debug build). Each face's plane, outline and box
  are now found once, and only faces whose box holds the test point are
  compared: 0.07 s. Doubled points are found by sorting along x (0.42 s
  to 0.01 s), repeated faces by grouping faces with the same edges.
  Tests: `prism-ops/tests/check_rules.rs` (each rule still finds its
  fault; the old code finds the same).
- **Follow Me builds a sweep once.** The tool checks a sweep and then
  runs it; the check's result is kept (for the same model, path and
  profile) and used by the run (`follow::attempt`). Test:
  `checking_then_sweeping_makes_the_same_model_as_sweeping`.
- **Typing right after a circle or polygon changes it** (a radius, or
  the polygon's sides as 8s), as the Rectangle and Line already did, one
  undo step, and not once anything else has changed. Test:
  `typing_right_after_a_circle_or_polygon_changes_it`.
- **Checked and already closed:** a sphere profile past its poles (6c-2),
  moves saying why they are refused (the hint), and the step-planned
  items of 5c, 6a, 6b-1 and 6c-1.
- **Not a gap:** a face already crossing the moving face is passed over
  by Push/Pull (stopping there would block every pull).
- **A rectangle beside another, pulled, gets a base.** The owner checked
  SketchUp (screenshots): two rectangles sharing an edge, one pulled up,
  make a closed box whose base faces down. Prism now does the same: a
  face with free sides and no sliding side keeps its base when its other
  sides border faces on its own plane, not only when they are drawn
  against a solid. A ring round another face still keeps none (DECISIONS
  #14). Test: `a_rectangle_beside_another_pulled_up_gets_a_base` (up and
  down; the base faces out, the other rectangle stays, Model Health finds
  nothing turned wrong).
- **Still open (minor, NOTES only):** a lemon-shaped torus, a profile
  turned away from its arc's start, a flat ring's soft seam, sweeps
  wholly inside themselves, rays grazing a curved surface, outline lines
  on the CPU, a cone's apex held by a flat face, X-ray's single layer,
  shadow cascades, the solids property test's rare failures (pinched
  loops), Move and Rotate not applying the solid rule.

## Owner fixes (2026-11-12): a pulled face's base, faces closing holes

- **The base keeps its colour.** A lone face pulled into a solid leaves
  its base behind: the base now shows the colour the face had, outside,
  whichever side was painted (it used to end up inside, turned over).
  Pushed back the other way, the moved face shows it outside too.
  (`pushpull.rs`, test `a_pulled_rectangle_keeps_its_paint_and_its_new_walls_are_default`,
  all four cases.) The new walls stay Default.
- **A face closing a hole faces out.** A new face with no face in its
  plane to copy used to face along its plane's frame, which is arbitrary,
  so a face closing a hole in a body came out reversed at random. Now the
  faces round its edges vote: on a consistent solid two faces sharing an
  edge run along it in opposite directions (`regions::wall_facing`).
  Test: `prism-doc/tests/closing.rs` erases each face of a box, turned
  three ways, and redraws an edge; each new face faces out (it failed
  without the fix).

## Owner feedback on the 6e build (2026-11-11, late)

- **Edges showed through curved faces** (a cylinder's bottom rim through
  its wall; the edges of an extruded shape with arcs). Cause: a line is
  never hidden by its own faces, which is right for flat faces. But each
  half of a cylinder's side wraps round, so the far rim belongs to the
  same face as the wall in front of it. Fix: a line with a curved face
  (`CURVED_OWN`) gets a depth test against its own face, with a slack of
  3/1000 of the distance for the facets' chords. Outline lines are exempt,
  because they lie on their face's rim as seen. Screenshot test
  `a_cylinders_far_rim_is_hidden_by_its_near_wall`.
- **Hovering lit half a cylinder, and half a circle's edge.** A click
  already selected the whole smooth surface and the whole curve; the hover
  now highlights what a click would select (`Session::hover_more`,
  `Highlight.hover_more`). Session test
  `hovering_a_cylinder_takes_its_whole_side_and_whole_rim`.
- **"A circle pushed through a box made a new circle face."** The owner's
  sessions 6 and 7 were replayed command by command:
  - session 6: a circle on a box's side pushed right through makes a
    through hole (both sides open, volume exact);
  - session 7: a circle pulled out makes a post joined to the box.

  **Corrected the same evening:** the replays were not what the owner saw.
  Each recording also saves the real final model. Session 6's has the
  circle's original face left over the hole, and replaying with that push
  done as a **new loop** gives exactly that model. So new loop was on
  unasked (it also explains the triangles pushed through a body that left
  their faces). Causes, all fixed:
  - a Ctrl or Alt counted as a tap when the key it went with never
    reached the viewport: a menu shortcut (the recording shows a Ctrl tap
    after every Ctrl+Z), or Alt+Tab to another window. Now any other key,
    and a change of window focus, cancel the tap (`Viewport::cancel_tap`);
  - once on, new loop stayed on for every later pull, and nothing showed
    it. Now it lasts one pull, choosing a tool turns it off, and the hint
    says "NEW LOOP" while it is on;
  - recordings did not save whether it was on, so replays started
    differently. `Recording.alt_loop` now saves it.

  Tests: `a_new_loop_is_shown_and_lasts_one_pull` (session) and
  `a_modifier_used_with_another_key_is_not_a_tap` (viewport). The first
  explanation (edges showing through) was real too, but was not this. A regression test covers both
  (`a_circle_on_a_side_pulled_out_joins_and_pushed_through_makes_a_hole`).
  If it happens again, a recording of it will show the steps.
- **Fillet / Chamfer:** it works: click an edge to round it with the
  current size, Ctrl+click to chamfer, type a size and press Enter to
  change the last one. But the tool does not show this. Owner request: a
  panel to set the size (and fillet or chamfer) with a live preview before
  applying. **Plan step:** "Fillet panel", after Basic Materials part 1
  (owner confirmed, 2026-11-12). Editable fillets after the fact belong to
  Inspector & Features.
