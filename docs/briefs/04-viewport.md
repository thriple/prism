# Chunk 4: Viewport

**Goal.** Draw the model in the viewport well with our own wgpu renderer, and move around it the way SketchUp users expect: orbit, pan and zoom, crisp lines, shaded faces, the ground and axes, and a hover highlight and selection display that show exactly what a click will hit.

**Read first:** PLAN.md sections 1 (items 1, 2 and 7), 1.2 (highlighting), 1.5a and 1.8 (navigation, with the prototype's changes), 1.14 (only what is listed below), 2.4 and 4 (depth precision). `NOTES.md`, "Drawing order bug found in the prototype". DECISIONS #4 (input devices) and #8 (performance).

## The prototype: same behaviour, different programming
The feel test (`docs/prototype/Main.dc.html`, guide in `docs/prototype/README.md`) is the reference for **how the viewport looks and behaves**, not for how it is built. It draws with a 2D canvas, sorts faces back to front, finds hidden lines by casting rays from the CPU, and picks by testing every face. Prism does all of this differently, on the GPU, with a depth buffer. Do not port any of it.

**Take from the prototype (the numbers to start from):**
- Camera: 35° vertical field of view; one wheel notch moves 12% of the way to the point under the cursor (`onWheel(`); orbit 0.006 rad per pixel about the point under the cursor, object first, then the ground (`surfacePoint(`, `orbitBy(`); the eye stays above the ground and never looks straight up or down; pan keeps the grabbed point under the cursor (`panBy(`); Zoom Extents fits the model's bounds with a 15% margin, keeping the view direction (`zoomExtents(`).
- Look (`drawGround(`, `drawFaces(`, `drawEdges(`): sky gradient `#c9d9ea` to `#eef2f5`, ground `#e9ebe9` to `#d9dcd9`; a 1 m grid with every fifth line darker, fading with distance; red, green and blue axes, dashed on the negative side, with no negative blue axis; front faces `#f8f7f4` and back faces `#96a8c6`, lit as 0.66 + 0.34 × (normal · light); edges `#16181c` at 1.5 px; hover orange `#ff8a00` (edges 2.8 px, faces tinted 24%); selection blue `#1f5fe0` (edges 2.8 px, faces tinted 12% plus a dot stipple); vertices as 4.5 px dots with a white ring; the soft ground shadow (opacity 0.17).
- These colours belong to the viewport, not to the Soft theme. Put them in one viewport style table (the start of 1.14's styles), like `theme.rs` for the UI.

**Do not copy:** `faceOrder(`, `closedCull(`, `visRuns(`/`occl(` and `pickRay(`. These are the workarounds for a canvas without a depth buffer. The GPU's depth buffer makes them unnecessary.

## Requirements
- **A new crate, `prism-render`.** The pure parts have no wgpu types and are tested without a GPU: the camera, projection, picking, display tessellation, and the line and face instance lists. The GPU part uploads and draws them. `prism-app` gives it the well's rectangle (`chrome/frame.rs`) and the model.
- **Camera (PLAN 1.5a as changed in 1.8, DECISIONS #4).**
  - The wheel only zooms, toward the surface under the cursor, and never zooms through it. With nothing under the cursor it uses the ground, then the model's bounds.
  - Middle-drag orbits and Shift+middle-drag pans, at any time, even mid-operation.
  - Double middle-click re-centres on the point under the cursor.
  - Orbit (O) and Pan (H) are tools that work with a left-drag. Zoom (Z) zooms with a drag. Shift+Z is Zoom Extents.
  - O, H and Z picked while another tool is active are temporary: Esc or releasing the mouse returns to that tool.
  - Gravity keeps the horizon level.
  - Trackpad: two-finger scroll pans and pinch zooms at the cursor.
  - A short ease on wheel zoom and Zoom Extents. It is off when reduce motion is on, and can be switched off in the debug panel.
  - The camera is f64. Rendering is relative to the eye: positions are offset by the eye in f64 before they become f32.
- **Depth (PLAN 4).** Reversed-Z with a 32-bit float depth buffer. Near and far planes are fitted to the visible bounds, including the ground grid, every frame. No fixed line bias: lines may win only by a view-space amount scaled with distance and capped below the thinnest wall in view, or by an equivalent in-shader comparison.
- **Lines (PLAN 1 item 7, 2.4).**
  - Instanced screen-space quads with signed-distance anti-aliasing.
  - Widths are in logical pixels and scale with the DPI, so they look the same on a 2x screen.
  - A thicker profile (silhouette) pass, 0 to turn it off. Default: off, matching the prototype.
  - Edges hidden by faces are not drawn by default. One debug setting draws them dashed or faint instead (the Hidden Line style comes in View Settings).
  - Guides and dashes come from the same renderer, so the next chunk can draw inference guides with it.
- **Faces.**
  - The display mesh is made for drawing only (PLAN 1.1). Each planar face, holes included, is triangulated in its own plane using the exact predicates.
  - Write the triangulator ourselves or use a crate under an allowed licence. `earcutr` is ISC, which is not on the list, so ask before using it.
  - The mesh is rebuilt only for faces that changed, using the document's change count.
  - Front and back colours with the prototype's headlight-style shading. Reversed faces show their back colour, which Model Health relies on later.
- **Ground and axes:** sky, ground, grid, axes and ground shadow as above. Ground Shadow (`view.shadow`, already an event) toggles the shadow.
- **Picking (PLAN 1 items 1–2, 1.2).**
  - Picking is in screen space, with an aperture in pixels. The prototype's aperture slider (0.5 to 2.5 times) goes in the debug panel.
  - Vertices beat edges, and edges beat faces. Something hidden behind a face cannot be picked.
  - Picking runs on the CPU in f64 from the same camera, not by reading back GPU memory (readback is slow and asynchronous on the web).
  - Picking and drawing both ask one "visible scene" function, so hidden objects can later be skipped in one place (PLAN 1.14, "hidden must be free").
  - A brute-force pass is fine for now. Measure it on the large test scene, and leave a note if it misses 16 ms (DECISIONS #8).
- **Hover and selection display.**
  - Whatever is under the cursor gets the hover highlight: a vertex, an edge, or a face.
  - Selected items are drawn in blue. The face stipple is in screen space, so it reads at any zoom.
  - The Select tool clicks as in the feel test (`toolDown(`, select branch; the owner calls it perfect, so match it exactly):
    - **Click** selects one item. Click empty space to clear.
    - **Double-click** a face selects the face and its outer edges.
    - **Triple-click** anything selects everything connected to it (`prism_ops::connected`).
    - A press counts as the next click of a series when it is within 350 ms and 6 px of the previous press.
  - Right-click selects before the context menu opens.
  - Select All and Deselect All are already events and get wired here, and the context menu now shows the face and edge lists.
  - Shift, Ctrl and box select come in the next chunk.
  - The selection is view state, not a command, so it is not undone.
- **Scenes to look at.** There are no drawing tools yet, so the app starts with the prototype's cube hovering above the ground, built through `prism-doc` commands. Help > Debug > Test Scenes (registered actions, greyed in release builds if you prefer) loads:
  - the cube;
  - the pocket box from NOTES;
  - a thin plate (below);
  - a large scene for timing.
  File > New (already an event) empties the model.
- **Redraw only on change**, as now. Moving the mouse redraws only when the hover target changes. Orbiting redraws every frame. Idle stays at 0% CPU.
- **Web:** keeps building and works on WebGL2. If WebGL2 limits reversed-Z precision, record what was seen in NOTES and keep it working.

## Out of scope
Inference, glyphs and guides from the inference engine, drawing tools, the measurement box, Shift/Ctrl and box select (next chunk), Parallel Projection, Standard Views, Zoom Window, Field of View, Walk and Look Around (all stay greyed), the View Settings panel and styles, X-ray, the sun, textures, hide and unhide, groups, curved faces and the Outliner. The Smart Guides demo scene stays out (NOTES, "For the next Claude Code session").

## Tests
- **Camera (pure):**
  - Any number of wheel steps never reaches or passes the surface under the cursor.
  - The point under the cursor stays under it (within 0.5 px) while zooming and panning.
  - The orbit pivot stays where it was on screen.
  - The horizon stays level.
  - Zoom Extents fits the bounds in both directions, in wide and tall windows.
  - Each of these holds at model scales 1e-4, 1 and 1e3.
- **Picking (pure):**
  - A vertex beats an edge, and an edge beats a face, inside the aperture.
  - An edge behind a face is not picked.
  - The aperture follows the DPI scale.
  - Hover gives the same answer as click.
  - Click, double-click and triple-click give the feel test's selections, and the 350 ms and 6 px limits hold.
- **Tessellation (pure):**
  - For every golden-case model, at the three scales, the triangle areas add up to the face area.
  - No triangle covers a hole.
  - Triangles face the face's normal.
- **Screenshots** (headless, on the software renderer in CI). Check colours, not exact pixels, so they do not depend on the GPU:
  - **Pocket test** (NOTES): a box with a pocket, hole ratios 0.3 to 0.9 and depths 1.2 to 1.9, seen every 6° of azimuth at several elevations. The back-face colour must never show from outside.
  - **Thin plate test** (PLAN 4): a 0.5 mm plate with detail behind it, seen from 5 m, 100 m and 1 km. Nothing behind it shows through, and its own edges do not flicker away.
  - **Line width:** a 1.5 px edge measures 1.5 px (±0.5) at scale 1 and at scale 2.
- **Performance:** with the large scene, record frame time while orbiting and picking time in NOTES, measured here and, if the owner can, on the reference machine.

## Acceptance
- [ ] Natively and on the web, the cube and the test scenes draw in the well with the prototype's look: crisp, even lines, shaded faces, ground, grid, axes and shadow.
- [ ] Orbit, pan and zoom feel like the prototype, with the same mouse, keys and trackpad, including mid-drag with another tool active.
- [ ] Hover shows exactly what a click selects. Selected items show in blue with the face stipple.
- [ ] The pocket and thin-plate screenshot tests pass in CI.
- [ ] Idle CPU is still about 0%. Frame and pick times are in NOTES.
- [ ] PLAN 6.0 "Where we are" is updated, and NOTES lists what is left.
