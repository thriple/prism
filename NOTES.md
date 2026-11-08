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
  rectangle gets no base when pulled up, so the box is open underneath.

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
