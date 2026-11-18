# Annotation & Sections: dimensions, text, section planes, construction planes, the model's axes

**Goal.** Prism can cut a model open to look inside it, label it, and set its own working planes. This chunk is large, so it is built in five parts, each shippable on its own. The order inside the chunk is the builder's; the order of chunks is the owner's.

**Read first:** PLAN 1.4 (Text, Dimensions, Section Plane rows), 1.14 (section display), 1.17 (Inspector rows for Text and Section Plane), `docs/feature-map.md` (rows for this chunk), `prism-view/src/scene.rs`, `prism-render` (clip and depth), `prism-tools/src/session/axes.rs` (the Axes tool this chunk's model axes reuse).

## Size
XL: five parts.

## Parts
1. **Section Planes and Section Display** (first; self-contained, helps every later check of a solid).
2. **Dimensions** (linear, between two inferred points, with offset and a text label from the model's units).
3. **Text** (screen labels first, then 3D text: outline, face, solid; fonts and shaping need dependencies, see below).
4. **The model's own axes** (move the origin and directions of the whole model; reuses the Axes tool at the top level) and **Construction Planes** (a plane to draw on, with inference).
5. **Leftovers**: snap to grid (optional), section planes in the export, MCP, docs.

## Part 1: Section Planes and Section Display
- **Document.** A section plane is a document object: `{ id, name, origin, normal }` plus which one is active, kept in the file (schema 12, optional `sections.json`), in world coordinates (built so: one plane cuts the whole model). One undo step per edit: `Command::AddSection`, `MoveSection`, `FlipSection`, `SetSectionActive`, `DeleteSection`.
- **Section Plane tool** (Tools > Section Plane; Construct toolbar; greyed until now). Hover over a face: a ghost plane lies on it with inference, aligned to that face's plane. Click places it. Hover empty space: it asks for a face or an axis (the measurement-box axis keys X, Y, Z choose one). Esc cancels.
- **Cutting.** An active plane hides everything on its positive side in the viewport. The cut is a display effect: no geometry changes, picking ignores what is cut away, and export is unchanged (until part 5). Section Display (View > Section Display) turns every plane's cut on or off; Section Planes (View > Section Planes) shows or hides the plane drawings. Only one plane is active at a time (the newest placed or the one last Activated); the others stay as drawn.
- **Cut face fill.** Closed solids show a flat cap where the plane cuts them (drawn in the section colour), so a cut solid does not look hollow. Open surfaces show no cap.
- **Cut lines.** A thick line where the plane meets each face, in the section colour, drawn even when the cap is off.
- **Editing.** Select a plane (click its border) to Move it along its normal (drag; typed distance), Flip it, Reverse it, Activate it, or Delete it. Entity Info shows its position and direction.
- **Menus:** Tools > Section Plane; Edit > Section > Flip / Activate / Delete; View > Section Display, Section Planes; context menu on a plane. Every action has a menu path; none are greyed.
- **MCP:** `list_sections` (plane, active), `add_section` (origin, normal), `set_section_active`.

## Part 2: Dimensions
- **Document:** a dimension is `{ a, b, offset }` in world coordinates, a document object (schema 13, optional `dimensions.json`); commands `AddDimension`, `MoveDimension`, `DeleteDimension`, one undo step each. It changes no geometry.
- **Dimensions tool** (Tools > Dimensions; greyed until now): click two points with inference, then the side: the line sits square to the measured line towards that point. The label shows the length in the document's unit. Click a dimension's line to move it; Esc cancels or leaves it. Edit > Dimension > Delete Dimension; View > Dimensions shows or hides them. Colours in `style.rs`.
- **MCP:** `list_dimensions`, `add_dimension`.
- **Built as:** NOTES "Annotation & Sections part 2". A dimension does not yet follow the model if it is moved (a later pass); angle and radius dimensions come after the linear one.
- **Tests:** commands and exact undo; file round trip and golden; the tool's three clicks, Esc, pick-up, delete, the view toggle; the MCP calls.

## Part 3: Text
- **Fonts (owner, 2026-11-18):** three bundled fonts only (IBM Plex Sans, Serif, Mono; SIL OFL), no system fonts: the same everywhere, including the web, and deterministic in tests. Shaping with `rustybuzz` (MIT), outlines with `ttf-parser`, in a new pure crate `prism-text`.
- **Text tool** (Tools > Text; greyed until now) with a Text window (words, font, capitals' height, Outline / Face / Solid, depth). A click sets the text down as a new group named after it, on the flat face under the pointer or on the ground; a ghost shows where. `Command::AddText`, one undo step. MCP `add_text`.
- **Built as:** NOTES "Annotation & Sections part 3". Not built: exact glyph curves, text along a path, editable text, bold and italic, alignment, screen labels.
- **Tests:** layout (heights, holes, widths, lines, missing characters); faces with holes, closed solids; the command, its group and undo; the tool on the ground and on a face; the MCP call; a screenshot of solid text in each font.

## Part 4: Model axes and Construction Planes
- The model's own axes: the Axes tool at the top level moves the model's origin and directions (a command that transforms everything, one undo step, guides and section planes included). Construction Planes: a named plane the drawing tools use as their working plane, with inference.
- **Built as (2026-11-18):** see NOTES "Annotation & Sections part 4". `Command::ChangeModelAxes` moves everything (guides, groups, section and construction planes, dimensions) by the frame's inverse, and the camera goes with it. Construction planes are document objects (schema 14, `planes.json`); `Kind::OnPlane` lets the cursor land on the one in use, so every drawing tool draws on it; Construction Plane tool, Edit > Construction Plane, View > Construction Planes, MCP `list_construction_planes`, `add_construction_plane`, `set_construction_plane_active`. Not built: moving a plane after placing it, typed distances, a plane from three points, a grid on the plane, Text and Dimensions using it.
- **Tests:** model axes move everything and undo exactly; a property test that shapes keep their size; planes' commands, file round trip and golden; the tool on the ground; a rectangle drawn on a wall plane; hidden planes are not used; a face in front still wins; the MCP calls.

## Out of scope for this chunk
- Section planes in the renderer's path-traced output (Render chunk); scenes; dimension styles beyond one set; text along a path (a later pass).

## Tests (part 1)
- A plane through a box at its middle: the visible scene has half the triangles; picking at a cut-away face finds nothing behind it that the plane hides; the cap area equals the box's cross-section (exact for a rectangle and for a cylinder cut obliquely).
- A property test: a random plane through random boxes never changes the model (document equality before and after Add / Activate / Delete); undo and redo are exact.
- File round trip keeps the planes; an old file loads with none.
- Session: the Section Plane tool's click on a face makes a plane with that face's normal; Esc cancels; flipping swaps the visible side.
- The viewport screenshot test on the GPU: a cut box shows its cap in the section colour.
- Planned Features: `tool.section` and `view.section` leave the unbuilt list; the `every_unbuilt_feature_has_a_chunk` test still passes.

## Acceptance
- [ ] A box is cut open by a plane dragged through it and the inside is capped (owner).

**Built as (2026-11-18):** see NOTES "Annotation & Sections part 1". Not yet built from the list above: typing a distance, the context menu on a plane, and picking a plane with the Select tool (in NOTES' left-over list).
- [ ] PLAN, NOTES, the Planned Features list and the feature map are updated.
