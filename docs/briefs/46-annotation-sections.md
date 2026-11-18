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

## Part 2: Dimensions (outline)
- Click two inferred points, then the offset side; the label shows the length in the model's units. A dimension is a document object that follows its two points if the model moves (stored as points; moves with Move on its group). Style and size live in the theme. Angle and radius dimensions come after the linear one.

## Part 3: Text (outline)
- Screen labels (always face the screen) and 3D text (outline, face, solid, extruded depth; along a path later). Fonts: system fonts and bundled OFL fonts; shaping with `rustybuzz` (MIT), glyph outlines with `ttf-parser` (MIT/Apache-2.0). Add each to `docs/DEPENDENCIES.md` first, with whether it builds for wasm. System font listing differs on the web (bundled fonts only there).

## Part 4: Model axes and Construction Planes (outline)
- The model's own axes: the Axes tool at the top level moves the model's origin and directions (a command that transforms everything, one undo step, guides and section planes included). Construction Planes: a named plane the drawing tools use as their working plane, with inference.

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
