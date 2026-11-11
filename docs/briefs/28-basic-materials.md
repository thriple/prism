# Basic Materials

**Goal.** A face can be painted with a colour or a texture and shows it in
the viewport, as in SketchUp: the Paint Bucket with its modifier keys, a
Materials window, materials on groups, textures that line up across faces
and wrap exactly round cylinders and cones, and materials kept through
every edit, saved in the file and carried in and out of glTF, OBJ and 3MF.
Materials store what the path tracer will need later (PLAN 4: a node graph
with one principled-surface node), so Rendering needs no file change.

**Read first:** PLAN 1.4 (Paint Bucket in the tool table), 1.6 (Window >
Materials), 1.13 (files), 1.14 (Shaded with textures, transparency), 4
(materials are node graphs); `docs/research/basic-materials.md`;
`docs/research/chunks-ahead.md` (Basic Materials); DECISIONS #20, #30,
#33, #34, #35 and #36.

## Parts (M chunk; each part is tested, documented and pushed on its own)
- **Part 1. Colours:** materials in the document and file, the Paint
  Bucket, group materials, materials kept through edits, colours in the
  viewport, the Materials window. Built in three steps:
  - **1a:** the core: the material table, paint on faces and groups,
    paint kept through edits, the file (schema 8). *(Built.)*
  - **1b:** the Paint Bucket and the viewport's colours.
  - **1c:** the Materials window, Entity Info, Select by material, Model
    Health and MCP; a `[release]`.
- **Part 2. Textures:** images in the file, placement on flat and curved
  faces, Shaded with textures, texture size, opacity.
- **Part 3. Placing and exchanging:** moving, turning and sizing a texture
  on a face; materials in glTF, OBJ and 3MF import and export.

The Paint Bucket stays greyed until part 1 makes it paint (DECISIONS #20).
Parts 2 and 3 add to an active tool.

## Requirements by part

### Part 1. Colours
- **The material** (`prism-doc`, document-wide, shared by every group's
  model). It has a stable id and a name, and a graph with one
  principled-surface node holding:
  - base colour (linear RGBA, where alpha is opacity);
  - roughness (default 0.5) and metalness (default 0);
  - an optional base-colour texture, empty until part 2.

  Names are unique in the document (a copy gets "Name #2").
- **On a face:** an optional front material and an optional back
  material. None means Default, drawn in the style's front or back colour.
  **On a group or component instance:** an optional material, drawn on
  its Default faces only, the nearest painted group winning (DECISIONS
  #35). Nothing is copied onto faces.
- **Painting is a command** (`Command::Paint`: faces and sides, or
  instances, old and new materials), so undo, redo, autosave and replay
  work. Adding, editing and removing materials are commands too.
- **The Paint Bucket (B):**
  - a click paints the side of the face you click;
  - **Ctrl** fills connected faces with the same material;
  - **Shift** replaces that material everywhere in the model;
  - **Ctrl+Shift** replaces it in the open group only;
  - **Alt** samples the material under the cursor (tap or hold, PLAN
    1.7);
  - the cursor shows the current material's swatch;
  - **hovering previews** the material on the face under the cursor (PLAN
    1.4: "material preview on the face under the cursor");
  - a closed group or component is painted as a whole: the click sets the
    instance's material.
- **Kept through edits** (DECISIONS #36): the regions rebuild and every
  operation that makes faces say where each new face's material comes
  from.
  - A face split by a line, a cut, or Intersect Faces keeps its material
    on every piece.
  - Merged faces (Eraser, the coplanar rule #30) take the largest piece's
    material.
  - Push/Pull: the moved face keeps its material; new walls are Default;
    a boolean's kept pieces keep theirs.
  - Copies keep theirs.
  - Follow Me: the walls take the profile face's material.
  - A reversed face swaps its front and back materials.
- **Viewport:** faces are drawn in their material's colour, front and back
  (back faces of Default stay tinted, so Model Health's reversed-face
  check still shows). The id buffer, picking and hidden lines are
  unchanged. Opacity is stored, but drawn from part 2.
- **The Materials window** (Window > Materials, in the Soft glass set):
  - **In Model:** swatches for every material in the document, used ones
    first;
  - a palette of plain colours (built in, in code);
  - **New**, **Duplicate**, **Delete** (faces using it go back to
    Default) and **Purge Unused**;
  - **Edit:** name, colour (egui's colour picker in the Soft theme), and
    roughness and metalness as fields;
  - clicking a swatch picks the Paint Bucket with that material.
- **Entity Info** shows the selected faces' material (front and back) and
  an instance's material.
- **Select:** right-click > Select > All with the Same Material (PLAN
  1.6).
- **Model Health:** an "unused materials" row (information, with Purge).
- **Files:** schema 8, with materials in the document and material ids on
  faces and instances. Schema 7 files open unchanged (all Default). Add a
  golden file.

### Part 2. Textures
- **Images:** PNG and JPEG through the `image` crate, with only those
  features (DEPENDENCIES rows first; it must build for the web). They are
  stored once each inside the `.prism` zip, by content hash. Model Health's
  purge removes images nothing uses.
- **A textured material** has a real-world width and height, with the
  aspect ratio locked by default. Its texture repeats to fill each face.
  The Edit panel loads an image (file dialog, or drag and drop onto the
  window), shows its size and lets you change it.
- **Placement** (DECISIONS #35):
  - **Flat faces:** a map from the face's plane to the texture, stored on
    the face side when painted. By default it is anchored to the model:
    u runs horizontally along the face (red on floors and ceilings), v
    runs up the face (green on floors), and the origin is the model origin
    projected onto the plane. So coplanar faces line up and a texture
    runs straight round a box's corners.
  - **Moving with the geometry:** Move, Rotate and Scale transform the
    placement with the face, so the texture travels with it. Push/Pull's
    new walls get the default placement.
  - **Curved faces** use their exact coordinates. A cylinder unrolls as
    (θ·r, s), exactly. A cone unrolls exactly into a sector of a disc.
    Spheres and tori are (θ·R, s·r), and stretch towards their poles and
    inner rim, as in every modeller.
- **Viewport:** **Shaded with textures** becomes the default style. The
  Shaded style without textures shows each textured material in its
  image's average colour. Each face gets texture coordinates. Faces are
  drawn in batches by material, with one bind group per textured material.
  Images over 2048 pixels are downscaled for the viewport (the WebGL2
  limit); the original stays in the file.
- **Opacity:** faces with alpha below 1 are drawn after opaque ones,
  sorted back to front, and do not hide what is behind them from picking.
- **No bundled textures** (DECISIONS #34, owner): the only built-in
  library is part 1's colour palette. Textures come from images the user
  loads.

### Part 3. Placing and exchanging
- **Texture position** (right-click a painted face > Texture > Position):
  move, turn and size the texture on that face with handles in the
  viewport, with inference. Enter or a click outside commits it, and Esc
  cancels. It is one command. Flat faces only (curved faces keep their
  exact coordinates). SketchUp's free four-pin distortion is out of scope.
- **Reset** (Texture > Reset Position) goes back to the default
  placement.
- **glTF:** export `pbrMetallicRoughness` (base colour factor, base colour
  texture, roughness, metalness) and texture coordinates with the
  placement baked in. Import the same into document materials.
- **OBJ:** write an `.mtl` beside the `.obj` (`Kd`, `d`, `map_Kd`, with
  textures copied beside it), and read it on import.
- **3MF:** colours, as its base materials. Textures are not exported.
- **STL and PLY:** unchanged, with no materials.

## MCP
- **Part 1:** `get_geometry` gives each face's front and back material,
  and `get_model` lists the document's materials (id, name, colour,
  used). A new `paint` tool paints faces or instances with a material by
  id or name, creating a colour material from a hex value if asked; it
  is undoable. `use_tool` with `tool.paint` reaches the Paint Bucket and
  its modifiers.
- **Part 2:** materials show their texture (image size, real-world size).
  `screenshot` shows textures.
- **Part 3:** nothing new (exports go through actions).

## Out of scope
- Node editing, procedural textures, normal and bump maps: Texture Nodes
  and Rendering.
- Projected textures, SketchUp's free-pin distortion, and texture
  positioning on curved faces: later, if the owner asks.
- Materials in the path tracer: Rendering.
- Colouring lines by material (PLAN 1.14): with View Settings' next
  part, once materials exist.

## Tests
- **Part 1:**
  - painting each side, each modifier, sampling, and a group's material
    on its Default faces only (nested groups, the nearest wins), with
    undo;
  - **a property test:** random edits (lines across painted faces, Push/
    Pull, Eraser, Move, Intersect Faces) never lose a material from a
    piece of a painted face, and every face's material exists in the
    document;
  - files round trip, the schema 7 golden file opens, and a new golden
    file;
  - a screenshot of painted faces;
  - the registry test (Paint Bucket active, Window > Materials);
  - MCP `paint`.
- **Part 2:**
  - default placement: a texture lines up across two coplanar faces and
    runs straight round a box's corner;
  - a cylinder's texture has no stretch (one texture tile equals its
    real-world width round the side);
  - a moved, turned or scaled face's texture moves with it;
  - images are stored once by hash, and purged;
  - a screenshot with textures, and one with a see-through face;
  - the web build compiles with `image`.
- **Part 3:**
  - Position moves, turns and sizes the texture, and Reset restores it;
  - glTF and OBJ round trip their materials (colours and textures);
  - 3MF exports its colours.

## Acceptance
- [ ] Every part: tests and CI green, NOTES updated (known gaps sorted,
  DECISIONS #33), pushed; a `[release]` at the end of part 1 and part 2
  for the owner to try.
- [ ] In a build made by hand: paint a box's faces and a group, fill
  connected, replace, sample; a brick image (loaded by hand) lines up round a house's
  corners and wraps a round tower without stretching.
- [ ] PLAN 6.0 updated.
