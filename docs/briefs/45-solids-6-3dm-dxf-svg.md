# Solids & CAD part 6: DXF, SVG and 3DM (2026-11-17)

**Goal.** Flat parts go to a router, laser or CNC program as DXF or SVG, with arcs as true arcs (CAM turns them into G2/G3 moves). DXF and SVG drawings come in as edges and faces. A model goes to Rhino and back as 3DM, with its curves, surfaces, layers and names (PLAN 1.3 and 1.13).

**Read first:**
- PLAN 1.3 and 1.13 (the formats table and the import and export rules);
- brief 44 and `prism-file/src/step.rs` (the last export and import, its notice and its window);
- `prism-app/src/transfer.rs` and `mesh_files.rs` (picking, saving, drops, the import window);
- `prism-geom` (`Curve`, `Spline`, `Nurbs`): what an edge can be.

## Libraries
- **DXF and SVG: our own code, no new crate.** Both are text. Writing them ourselves keeps the bytes the same each time (no clock in the header) and keeps arcs as arcs. SVG's XML is read with quick-xml, which we already use for 3MF.
  - (The `dxf` crate, MIT, was looked at: it pulls in a clock, an image library and UUIDs for what is a few hundred lines.)
- **3DM: cadmpeg-codec-rhino** (Apache-2.0, pure Rust, reads and writes 3DM archive versions 5 to 8). It is young (first release 2026-07) and its writer has limits, so part 6c starts with a trial; see there. openNURBS itself is C++ and would not build for the web.

## Size
XL: three parts, each pushed and tested on its own.

## Rules
- **What goes out is what you see** (PLAN 1.14): hidden objects are not exported. With a selection, only the selection goes.
- **Arcs are arcs.** A line goes out as a line, an arc or circle as an arc or circle, a spline as a spline. Nothing is cut into short segments unless the format cannot hold the curve, and then the notice says so.
- **Units.** Export writes the model's unit (Feet and inches write inches). Import reads the file's unit, shows the size, and offers a scale, as for meshes.
- **Counted, never silent.** The notice says what went and what was left out, and why (e.g. "Left out 2 text items: text is not imported yet").
- **An import is one undo step,** and comes in as a group named after the file.
- **Deterministic:** the same model writes the same bytes.

## Parts
- **Part 6a. DXF and SVG export (File > Export > DXF / SVG).**
  - `exp.dxf` is built. A small window picks DXF or SVG, the unit, and the view:
    - **Laid flat** (the default, for sheet parts): each selected flat face, or the largest flat face of each selected solid, is laid on the drawing plane, outside up, and set out in a row with a gap. Its outer loop and holes go out.
    - **Top, Front or Side:** the selection's edges (or all shown edges) seen straight on from that side. No hidden lines are removed yet (later, with Sections).
  - Curves stay exact: a view straight on is a flat projection, so a spline's projection is the spline of its projected control points. A circle seen straight on stays a circle; seen at a slant it is an ellipse (DXF ELLIPSE, SVG elliptical arc).
  - **DXF:** AutoCAD 2000 ASCII text (the version CAM programs read), with LINE, ARC, CIRCLE, ELLIPSE and SPLINE. A closed loop of lines and arcs goes out as one closed LWPOLYLINE with bulges, since CAM programs cut closed polylines as one path. `$INSUNITS` gives the unit.
  - **SVG:** one `<path>` per joined chain: lines, arcs (A) and cubic curves (a spline of degree 3 or less exactly; a higher one within the model's tolerance, counted). The page is in mm or inches, with a viewBox, so laser programs read the true size.
  - Each solid or group laid flat is its own DXF layer and SVG group, named as in Prism.
  - Golden files in `docs/acceptance/files/`: a plate with a round hole, a slot and a spline edge, as DXF and SVG.
- **Part 6b. DXF and SVG import (File > Import, and dropping a `.dxf` or `.svg` file).**
  - Edges come in on the ground plane, in a group named after the file. Closed loops become faces, by the usual drawing rule.
  - **DXF:** LINE, ARC, CIRCLE, ELLIPSE, LWPOLYLINE and POLYLINE (with bulges), SPLINE, POINT (as a guide point), and INSERT (a block placed, flattened in place). Text, hatches, dimensions and images are left out and counted. The unit is `$INSUNITS`, or asked when it is missing.
  - **SVG:** `path` (all commands), `line`, `polyline`, `polygon`, `rect`, `circle` and `ellipse`, with the transforms of their groups. Circles and arcs come in as arcs, cubic and quadratic curves as splines. Text, images and styles are left out (fills and strokes are not geometry). The unit comes from the page's size and viewBox (96 px to the inch otherwise).
  - Y up: SVG's y runs down the page, so it is turned over to come in as drawn.
- **Part 6c. 3DM in and out (File > Export > 3DM (Rhino), and import).**
  - **First, a trial** (in a test, before any app work): cadmpeg-codec-rhino builds for the web; a box, a cylinder, a lofted freeform solid and a cut freeform face write and read back the same; a written file opens in Rhino (the owner checks one file). If the trial fails, stop and tell the owner, with what failed and the options (wait for the library, or write our own 3DM writer for a smaller set).
  - **Out:** each solid or sheet as a Brep; loose edges as curves; groups and components as Rhino groups and block instances, named as in Prism; tags as layers; face colours.
  - **In:** Breps, surfaces, extrusions and curves come in exactly (planes, cylinders, cones, spheres and tori as Prism's own surfaces where the file says so, the rest as NURBS); layers as tags, block instances as components, names kept. Meshes and SubD are refused by name and counted (meshes come in through the mesh import instead).

**MCP:** new tools, replayed through the command log like the other imports:
- `export_dxf` and `export_svg` { view, unit, selection_only } return the text, with the notice;
- `import_dxf` and `import_svg` { text } import, as File > Import does;
- `export_3dm` and `import_3dm` carry the file as base64 (3DM is binary).

## Out of scope
- Sections and hidden-line views (with Annotation & Sections).
- Nesting parts to save sheet material (a later tool).
- DXF text, dimensions and hatches; SVG fills and text.
- DWG (not planned; PLAN 1.13).
- IGES (a later row).

## Tests
- **6a:**
  - a plate with a hole laid flat writes one closed polyline with bulges and one circle (DXF), or two paths with arcs (SVG);
  - a spline edge writes as a SPLINE with the same control points;
  - a circle seen at a slant writes an ellipse;
  - two writes are byte-identical; hidden objects are left out; mm and inches give the right sizes;
  - golden DXF and SVG files.
- **6b:**
  - the 6a golden files import as the same edges and faces (same lengths and area);
  - a hand-written DXF in inches with a block placed twice, a bulged polyline, a spline and a text item imports the geometry and counts the text;
  - an SVG with nested transforms, every path command and a circle imports at the right size; y comes in the right way up;
  - a drop of a `.dxf` file imports, in one undo step.
- **6c:** the trial above; then a model with two groups, a component used twice, two tags and a painted face goes to 3DM and back with the same names, layers, components, colours and volumes.

## Acceptance
- [x] A flat part exports to DXF and SVG with true arcs and the right size; a drawing from elsewhere imports as edges and faces.
- [x] A model goes to 3DM and back with its curves, surfaces, layers and names (or the trial's result is reported to the owner). The trial failed and was reported; the owner chose STEP for Rhino for now.
- [x] The DXF / SVG row ticks itself in Planned Features (3DM stays greyed).
- [x] PLAN, NOTES and HANDOFF are updated after each part.
