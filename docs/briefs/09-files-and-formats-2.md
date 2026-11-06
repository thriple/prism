# Chunk 7: Files & Formats (part 2)

**Goal.** Models go in and out of other programs. Prism imports and exports STL, OBJ, 3MF, glTF (GLB) and PLY, and files can be dropped onto the window, natively and on the web.

**Read first:**
- PLAN.md 1.3 (export presets) and 1.13 (import and export rules);
- DECISIONS #25 (we write 3MF ourselves);
- `docs/research/files-and-formats.md` (crates, licences, web drag and drop);
- brief 08 (part 1).

## Requirements
- **Mesh import and export** live in `prism-file` as pure functions: bytes in, bytes out, no file input or output, the same bytes for the same model.
  - **Formats:**
    - **STL:** binary and ASCII to read, binary to write.
    - **OBJ:** `v` and `f` to read, with polygons, negative indices, and `o` and `g` ignored. Write without MTL, since there are no materials yet.
    - **3MF:** our own writer and reader with `zip` and `quick-xml` (DECISIONS #25); one object, in millimetres.
    - **glTF:** binary GLB to write; GLB and `.gltf` with embedded buffers to read. Only triangle primitives, with each node's transform applied.
    - **PLY:** ASCII and binary little-endian to read (vertices, and faces as lists); binary little-endian to write.
  - **Writers:** formats that need triangles get them from the same triangulator as the viewport. OBJ and PLY write each face as one polygon when it has no holes.
  - **Up axis:** export can turn Z up into Y up (the default for glTF and OBJ, as PLAN 1.13 says). Import of glTF turns Y up back into Z up.
  - **Units:** export writes in the unit picked: millimetres for 3MF and STL by default, metres for glTF.
  - **Import gives triangles or polygons**, then builds a model directly. It does not go through sticky geometry one segment at a time, which is too slow for big meshes:
    1. Weld vertices within the tolerance.
    2. Drop degenerate triangles.
    3. Optionally (on by default), **merge coplanar triangles** that share edges into planar faces, with holes where the merged region has them. A box from an STL comes back as 6 faces, not 12 triangles.
  - **Damaged or unsupported files** are refused with a reason, never a panic. A property test flips bytes, as for `.prism`.
- **Import (File > Import…):**
  - Pick a file, or drop one. A dialog shows the format, the triangle count and the size in the chosen unit.
  - It asks for the unit when the format does not say: STL, OBJ and PLY default to millimetres; 3MF and glTF say their own.
  - It offers "Merge coplanar triangles into faces".
  - The import is one undo step, as a new `Command::Import` holding the built geometry.
  - Imported geometry is added beside what is there. It is not stuck to it, and the overlap split does not run on it. Merging it with the rest comes with Groups & Components and Model Health (PLAN 1.13 allows "placed on its own").
  - After the import, the view fits the imported geometry, and the status bar gives the counts.
  - Files over 50 MB ask before importing. Background import with progress waits for threads (`docs/research/risks.md`, item 2).
- **Export (File > Export):**
  - The two presets that are built become active: "3MF / STL (3D printing)" and "glTF / OBJ / USD (media, web)". The rest stay greyed.
  - A dialog picks:
    - the format within the preset: 3MF or STL; GLB, OBJ or PLY (USD later);
    - the scope: the whole model or the selection;
    - the unit and the up axis.
  - It shows the triangle count.
  - The file is saved natively, or downloaded on the web.
  - The dialog says plainly that exporting to a mesh loses exact geometry (PLAN 1.13, "fidelity is stated").
- **Drag and drop:**
  - Natively: winit's `DroppedFile`.
  - On the web: `dragover` and `drop` listeners on the canvas, with a highlight while a file is over the window.
  - A dropped `.prism` file opens as File > Open would, asking "Save changes?" first. Any other format imports.
- **Greyed out until built (DECISIONS #20):** Import and the two built Export presets become active now.
- **DEPENDENCIES.md:** a row for `quick-xml`, `gltf-json` and `gltf` if they are used. Prefer our own code for STL, OBJ and PLY, which are small.

## Out of scope
- Materials and textures in any format.
- USD, STEP, 3DM, DXF and SVG.
- Recovering exact curves from meshes, and softening edges.
- Merging imports into existing geometry.
- Model Health after import (its chunk).
- Background import.
- Tabs (part 3).

## Tests
- **Round trips:** for each format, the test scenes and a model with holes go out and back in with the same faces, after merging coplanar triangles, and the same areas within the tolerance.
- **Reading:** small hand-written files read correctly:
  - ASCII STL and a cube in binary STL;
  - OBJ with a quad, negative indices and a polygon with five corners;
  - PLY in ASCII and binary;
  - GLB with a node transform;
  - 3MF.
- **Bytes:** the same model gives the same bytes twice, for every format.
- **Merge:** two coplanar triangles become one face; a ring of triangles becomes a face with a hole; non-coplanar triangles stay apart.
- **Damage:** damaged and cut-short files are refused without a panic (a property test).
- **Undo:** `prism-doc` undo and redo of `Import` are exact.
- **Scale:** the Large Scene exports and imports again, and the time is recorded in NOTES.

## Acceptance
- [ ] An STL made in another program imports with planar faces and the right size.
- [ ] A Prism model exported to 3MF opens in a slicer; one exported to GLB opens in a glTF viewer.
- [ ] Dropping files works natively and on the web.
- [ ] PLAN 6.0 "Where we are" is updated, and NOTES lists what is left.
