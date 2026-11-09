# Exact Curves, part 4: curved faces, soften and smooth

**Goal.** Push/Pull of a face with arcs makes true cylinders. Curved faces are shaded smoothly and picked exactly. Edges can be softened and smoothed, so the seams of a curved surface are hidden. Push/Pull on a cylinder changes its radius. The curve tools stay greyed in menus, toolbars and releases until all five parts are done (owner, 2026-11-09); this part is tried through tests, MCP and builds made by hand.

**Read first:** briefs 23 and 24 and their NOTES sections; `docs/research/exact-curves.md` ("Curved faces"); PLAN 1.1 (one model, display tessellation adaptive to screen size), the Eraser row of the tool table (Ctrl softens), the "Hidden lines" line in section 3 (soft and smoothed edges are hidden lines); DECISIONS #20 and #32.

## Steps (each one is tested, documented and pushed on its own)
- **4a. Curved surfaces in the core:** the cylinder surface; every planar-only operation works, refuses or skips. No visible change. *(Built.)*
- **4b. Cylinders from Push/Pull:** pulling a face with arcs; shading and picking of cylinder faces.
- **4c. Soften and smooth:** edge flags, the drawing, the Eraser with Ctrl, Edit > Soften/Smooth, picking a whole smooth surface.
- **4d. Push/Pull on a cylinder:** a new radius.

## Requirements by step

### 4a. Curved surfaces in the core
- **`Surface::Cylinder`:** a point on the axis, the axis direction, the radius, and which way the face points (out or in). Exact; no stored triangles.
- **A cylinder face** is bounded by arcs round the axis and straight lines along it, all on the surface. A face's loops still name their edges (part 1).
- **`Surface::plane()` becomes `Option`.** Every place that needs a plane decides what a curved face means for it, and the compiler finds them all:
  - works: Move, Rotate, uniform Scale, Copy, Erase, Hide, groups, files, mesh export;
  - refuses, with a reason through `Command::refusal`: uneven Scale, moving part of a curved face, Offset of a curved face, and (until 4b) Push/Pull of a curved face or one beside it;
  - skips: face finding and the coplanar rule (`regions`, `settle`) only make planar faces; lines drawn across a curved face do not split it yet; Intersect Faces does not cut curved faces (their cuts need ellipses, DECISIONS #32).
- **Model check:** every vertex and edge of a cylinder face lies on the cylinder.
- **Files:** the new surface is read and written; old files open unchanged.
- **Model Health:** lists a curved face with a point off its surface, with Repair moving it back.
- **MCP:** `get_geometry` lists a curved face's cylinder (built in 4a).

### 4b. Cylinders from Push/Pull
- Pulling a face whose loops have arcs makes, for each arc, a cylinder side face (the arc, its copy at the new height, and the lines between their ends). Lines make planar sides, as now. `ARC_FACE_PULL` goes.
- The lines between two cylinder sides of one circle or arc are made soft and smooth (step 4c's flags; until 4c they are drawn as normal).
- Pulling the cap of a cylinder again lengthens it.
- **Shading:** a cylinder face is drawn from triangles cut finely enough on screen (PLAN 1.1), with normals from the exact surface, so it looks round at any zoom.
- **Picking:** a ray meets the exact cylinder, inside the face's loops. Inference snaps "On Face" to it.

### 4c. Soften and smooth
- **Edge flags:** `soft` (not drawn, a hidden line) and `smooth` (shading runs across it). The seams of a cylinder are both.
- **Drawing:** soft edges are hidden, and shown dashed with Show Hidden (as other hidden lines).
- **Eraser with Ctrl** softens and smooths instead of erasing; Ctrl+Shift unsoftens.
- **Edit > Soften/Smooth** (`edit.soften`) softens and smooths the selected edges, or unsoftens them if all are soft already. It is active from this step (DECISIONS #20).
- **Picking:** a click on a face of a smooth surface picks every face joined to it by soft edges, as one.
- Files keep the flags.

### 4d. Push/Pull on a cylinder
- Push/Pull on a cylinder face (or a whole smooth cylinder) changes its radius: the cylinder stays round, about the same axis, and the arcs of the caps follow with it.
- The measurement box takes the distance (the change of radius). A radius that would reach zero or less is refused.

## MCP
- `get_geometry` lists each face's surface (plane, or cylinder with axis point, axis, radius and side), and each edge's soft and smooth flags.
- Edit > Soften/Smooth reaches `run_action` by itself.
- A test pulls a disc over MCP and reads the cylinder back with `get_geometry`; another softens edges and reads the flags.

## Out of scope
- Cones, spheres and tori, Follow Me, fillet and chamfer of solid edges (part 5).
- Lines drawn on a curved face splitting it; ellipses and the segment fallback (after part 5, DECISIONS #32).
- A dialog with an angle for Soften/Smooth (later; the flags come first).

## Tests
- **Geometry:** ray–cylinder hits; a point's distance to a cylinder; tessellation within its error on screen.
- **Ops:**
  - a disc pulled up is two cylinder faces and two caps, a closed solid with the exact volume (πr²h);
  - a rounded rectangle pulled makes planar and cylinder sides;
  - each refusal in 4a, with its reason;
  - Move, Rotate and uniform Scale keep cylinders exact;
  - a cylinder's new radius from 4d, with the caps following.
- **Session:** Push/Pull of a disc by clicks and typing; picking and inference on a cylinder; Eraser with Ctrl; Edit > Soften/Smooth.
- **Property tests:** random pulls of faces with arcs keep the model check clean, and undo restores the model.

## Acceptance
- [ ] Every step: tests and CI green, NOTES updated, pushed.
- [ ] In a build made by hand, a disc pulled over MCP looks round and smooth, with no seams drawn, and picks as one surface.
- [ ] PLAN 6.0 updated.
