# Chunk 10: View Settings (part 1: what is shown, lines and faces)

**Goal.** The model can be shown as Shaded, Monochrome, Hidden Line, Wireframe or X-ray, with profile lines, chosen from the View menu or a docked View Settings window, and saved with the file (PLAN 1.14).

**Read first:** PLAN 1.14; NOTES "Owner feedback, 2026-11-09 (corner line, profiles)"; `crates/prism-view/src/style.rs`; `crates/prism-render/src/viewport.wgsl`.

## Parts of this chunk
1. **What is shown, lines and faces** (this brief).
2. **Lighting and hidden things:** the sun from a date, time and place with shadows on faces, and "Show Hidden" ghosts.

Hide and unhide, and the hierarchy tree, came with Groups & Components part 3.

## Requirements
- **A view style** (`ViewStyle`, pure data in `prism-view`): what is shown, line and face settings. It is saved in the `.prism` file's settings (an optional field, so no new schema), and a new document starts with the Default style.
- **Show (toggles):** faces, lines, points (every vertex drawn as a small dot), axes, ground (sky and ground gradient, else a flat background), grid, guides.
- **Lines:**
  - line thickness and profile thickness, in points. Profiles follow PLAN 1.14 and are **on** in the Default style.
  - colour lines: all one colour, or by axis (edges parallel to red, green or blue drawn in that colour).
  - occluded lines: hidden, dashed or faint.
- **Face styles:**
  - **Shaded** (as now).
  - **Monochrome:** front and back faces in one colour, still lit.
  - **Hidden Line:** faces in the background colour, unlit; lines on top hide what is behind.
  - **Wireframe:** no faces; every line shows; faces cannot be picked or snapped to.
  - **X-ray** (a toggle on top of a face style): faces see-through at an opacity; lines behind faces show; picking and snapping reach points and edges behind faces.
- **Named styles:** Default, Hidden Line, X-ray, Technical (thin lines, no profiles, Monochrome), Wireframe. Choosing one sets every field; changing a field after that is kept in the document.
- **Window > View Settings:** a side card (as Model Health) with the style picker and every setting above. The View menu items become active, with check marks where they are toggles (DECISIONS #20).
- **Hidden must be free:** nothing new is drawn or picked for hidden objects.

## Out of scope
The sun, shadows on faces, lighting settings, "Show Hidden" ghosts (part 2). Colouring lines by material or tag (with Basic Materials). Order-independent transparency for X-ray ("Accurate" sorting).

## Tests
- `ViewStyle` round-trips through a `.prism` file; an old file without it opens with Default.
- Screenshots: Wireframe draws no face colour; Hidden Line faces are the background colour; X-ray shows a line behind a face; a profile line is thicker than a plain one; colour by axis.
- Session: in Wireframe a click on a face's middle picks nothing; in X-ray a vertex behind a face can be snapped to.
- The registry test: every View item has a menu path and is active.

## Acceptance
- [ ] Each face style and X-ray can be chosen and looks as described.
- [ ] Profiles are on in the Default style.
- [ ] The style is saved with the file.
- [ ] PLAN 6.0 and NOTES are updated.
