# Basic Materials: research for brief 28

Written 2026-11-11 (Guam), before the brief. It builds on the Basic
Materials section of `chunks-ahead.md` (2026-11-08). Sources are at the
end.

## What SketchUp does (the feel to match)

- **Paint Bucket (B)** paints the face under the cursor with the current
  material. Its modifiers, from SketchUp's help and its 2023 and 2026
  quick-reference cards:
  - **Ctrl:** fill connected. Paint every face connected to the clicked
    one that has the same material.
  - **Shift:** replace everywhere. Every face in the model with the clicked
    face's material gets the new one.
  - **Ctrl+Shift:** replace on the same object only (the open group).
  - **Alt (held):** sample. The material under the cursor becomes the
    current one. A forum thread notes that a very fast Alt press-click-
    release can leave sampling on by mistake, so we use our tap-or-hold
    rule (PLAN 1.7) and test it.

  PLAN's tool table already lists these, except Ctrl+Shift, which this
  brief adds.
- **Front and back.** A face has a front material and a back material. The
  Paint Bucket paints the side you click. A face with no material is
  "Default", drawn in the front or back colour of the style (Prism already
  has `face_front` and `face_back`).
- **Groups and components.** Painting a closed group sets the group's
  material. It shows only on the faces inside that are still Default;
  faces painted inside keep their own material. SketchUp's moderators
  stress that nothing is copied onto the faces: the group carries the
  material, and Default faces are *drawn* with it. Nested groups apply from
  the inside out: the nearest painted group wins. Prism should do exactly
  this, without copying. Then repainting the group, or opening it and
  setting faces back to Default, behaves as SketchUp users expect.
- **Texture size and placement.** A textured material has a real-world
  width and height (lock the aspect ratio by default). The texture repeats
  to fill each face. A face can carry its own placement (SketchUp's
  Position Texture with four pins), which overrides the material's
  default. Users report that moving or stretching geometry carries the
  texture with it. Position Texture works on flat faces only. Curved
  faces are usually done with a projected texture sampled from a flat
  helper face. SketchUp's texture origin and axes on a face are not
  documented; tutorials show textures lined up with the model's axes on
  walls and floors.
- **Materials window:** "In Model" (used and unused), library folders, an
  Edit tab (name, colour, texture image, size, opacity), and Purge Unused.

## What Prism should store (so Rendering needs no file change)

- **A material** (PLAN 4: "materials are node graphs from day one, even
  while the UI only shows a simple PBR form"). The file stores a graph
  with a single principled-surface node. Its inputs follow glTF's
  metallic-roughness model, which every format and renderer reads:
  - base colour (linear RGBA, where alpha is opacity), and optionally a
    base colour texture;
  - roughness and metalness;
  - emission, kept for Rendering, with no UI yet.

  The viewport shows colour, texture and opacity. Roughness and metalness
  are stored and shown in the Edit panel; the path tracer uses them later.
- **On a face:** an optional front material, an optional back material,
  and an optional texture placement per side. Materials are document-wide
  (one table shared by every group's model), so a face keeps a material
  id. A group or component instance keeps an optional material too.
- **Texture placement.**
  - **Flat face:** a map from the face's plane to texture coordinates (an
    origin, a u axis and a v axis in model space, scaled by the material's
    size).
  - **Default placement:** the u axis is horizontal along the face. On a
    floor or ceiling it is the red axis. The v axis runs up the face, or
    along green on a floor. The origin is the model origin projected onto
    the face's plane. So coplanar faces line up, and a texture runs
    straight round the corners of a box. This guess matches what SketchUp
    tutorials show.
  - **Moving with the geometry:** a placement is stored when a face is
    painted. Move, Rotate and Scale transform it with the face, so the
    texture travels with the geometry, as users report. Push/Pull's new
    walls take the default placement.
- **Curved faces get exact coordinates for free.** A face of revolution
  already has (θ, s) from Exact Curves 5b (`Revolution`).
  - A cylinder is (θ·r, s): exact, with no stretch.
  - A cone unrolls exactly into a sector of a disc; texture coordinates
    come from that sector.
  - Spheres and tori stretch towards their poles and inner rim, as they
    do in every modeller.

  Projected textures (SketchUp's trick for curves) can come later as a
  per-face option. They are not needed here.

## Carrying materials through edits

This is the risky part. Every operation that makes or splits faces must
say where each new face's material comes from:
- a face split by a line, Intersect Faces or a cut keeps its material and
  placement on every piece;
- faces merged into one (Eraser, the coplanar rule #30) take the material
  of the largest piece;
- Push/Pull: the moved face keeps its material, and new walls are Default
  (SketchUp does the same). A boolean's kept pieces keep their own;
- copies keep their materials;
- Follow Me: the walls take the profile face's material;
- undo restores everything (it is all in the command log).

The plan: the regions code that rebuilds faces passes the old face's
material and placement to the pieces it makes, and a property test checks
that random edits never lose a painted face's material where an unchanged
piece remains.

## The viewport

- **Shaded with textures** is the default style (as in SketchUp). Shaded,
  without textures, shows each textured material as its average colour,
  which is quick to compute when the image loads.
- **Draw calls:** the mesh gets per-vertex texture coordinates and a
  material index. Faces are drawn in batches by material: one bind group
  per textured material, and plain colours as a vertex attribute. Models
  have tens of materials, not thousands, so this is fine.
- **Opacity:** faces with alpha below 1 are drawn after the opaque ones,
  sorted back to front. That is enough for SketchUp-like glass. The
  accurate order-independent mode stays with X-ray (PLAN 1.14).
- **The web.** wgpu's WebGL2 limits guarantee 2048-pixel textures and 256
  layers in an array; WebGPU is now baseline in current browsers.
  Textures over 2048 pixels are downscaled for the viewport, and the
  original stays in the file for rendering. No texture arrays, so
  WebGL2-only browsers still work.

## Images and the library

- **Decoding:** the `image` crate (MIT OR Apache-2.0, licence changed in
  0.24.9), with only its PNG and JPEG features. Its JPEG decoder is now
  `zune-jpeg` (MIT, Apache or Zlib, pure Rust). Both are pure Rust, so the
  web build is expected to compile. Confirm it in the first commit with
  the wasm32 clippy job, and add the rows to DEPENDENCIES.
- **In the file:** each image is stored once inside the `.prism` zip,
  named by its content hash, so a texture used in many places or files
  costs nothing extra. This needs a new schema (8), with a migration from
  7 and a golden file.
- **Bundled library:** Poly Haven's FAQ says all its assets are CC0
  ("absolutely any purpose, including commercial"), and ambientCG is CC0
  too. CC0 is not on Prism's licence list, which covers code and fonts:
  **DECISIONS #34** (open; default: allow CC0 for bundled textures, as #12
  allowed OFL for fonts). Ship a small set (about 30 materials at 1024
  pixels, a few MB), with a credits file naming the sources. Poly Haven
  asks, though it does not require, that its assets be credited; we do.
  Keep plain colours (a SketchUp-like palette) in code.
- **Colour picker:** egui's picker re-themed in the Soft set (as the
  View Settings colour wells are, if they exist), so no new crate is
  needed.

## Formats

- **glTF:** export base colour, texture and UVs (`pbrMetallicRoughness`,
  `baseColorFactor` in linear RGBA, `baseColorTexture`). Import the same.
  `KHR_texture_transform` is not needed: we bake the placement into the
  UVs.
- **OBJ:** an `.mtl` with `Kd` and `map_Kd` beside the `.obj`. Import
  reads it.
- **3MF:** colours (its base materials). Textures are not common there.
- **STL and PLY:** no materials.

## How it splits (M chunk, three parts)

1. **Colours.** Materials in the document and file (schema 8), the Paint
   Bucket with its modifiers and sampling, group materials, carrying
   materials through edits, the viewport drawing colours, and the
   Materials window (In Model, colour, edit, purge).
2. **Textures.** Images in the file, default placement on flat and curved
   faces, Shaded with textures, texture size, the bundled library (#34),
   opacity.
3. **Placing and exchanging.** Move, rotate and scale a texture on a face
   (a simple Position Texture), and materials in glTF, OBJ and 3MF import
   and export.

## Sources

- SketchUp Help, "Replacing material" (Paint Bucket modes and modifier keys): https://help.sketchup.com/ru-ru/sketchup/replacing-material
- SketchUp quick reference card 2026 (Windows): https://download.sketchup.com/QRC2026/HC-QRC2026-en-SU-windows.pdf
- SketchUp quick reference guide 2023.1 (Windows): https://cdn.shopify.com/s/files/1/0605/0358/9072/files/sketchup_for_desktop_quick_reference_guide_2023.1_windows.pdf
- SketchUp forum, the Alt sampling timing: https://forums.sketchup.com/t/has-the-timing-function-of-alt-commands-changed-paintbucket-eyedropper/321256
- SketchUp blog, "Assigning materials to groups and components": https://blog.sketchup.com/article/assigning-materials-groups-and-components
- SketchUp forum, painting a group's faces: https://forums.sketchup.com/t/how-do-i-re-paint-all-faces-of-a-group-after-painted-a-single-face-of-a-group/53896
- SketchUp forum, group face shows white: https://forums.sketchup.com/t/group-face-is-white/49794
- SketchUp blog, projecting textures on curved surfaces: https://blog.sketchup.com/article/sketchup-skill-builder-painting-and-projecting-textures-curved-surfaces
- SketchUcation, texture size and tiling: https://community.sketchucation.com/post/1263058
- SketchUp forum, texture proportions on import: https://forums.sketchup.com/t/textures-are-importing-with-incorrect-proportions/175514
- Khronos, glTF PBR: https://www.khronos.org/gltf/pbr/
- glTFast docs, PbrMetallicRoughness: https://docs.unity3d.com/Packages/com.unity.cloud.gltfast@5.0/api/GLTFast.Schema.PbrMetallicRoughness.html
- luma.gl, glTF extensions (KHR_texture_transform): https://luma.gl/docs/api-reference/gltf/gltf-extensions
- Poly Haven licence and FAQ: https://polyhaven.com/license and https://polyhaven.com/faq
- Free PBR textures guide (ambientCG CC0): https://app.cinevva.com/guides/free-textures-hdris-materials
- zune-jpeg docs (licence): https://docs.rs/zune-jpeg
- image crate release notes (licence change, zune-jpeg): https://docs.rs/crate/image/latest/source/CHANGES.md
- wgpu limits (WebGL2 downlevel): https://wgpu.rs/doc/src/wgpu_types/limits.rs.html
