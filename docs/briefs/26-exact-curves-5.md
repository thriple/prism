# Exact Curves, part 5: fillet, chamfer and Follow Me

**Goal.** Edges of solids can be filleted and chamfered, and Follow Me sweeps a profile along lines and arcs, making planes, cylinders, cones, spheres and tori, all exact. The curve tools stay greyed after this part: they become active at the end of part 6, once curved faces work with the standard tools (owner, 2026-11-11, DECISIONS #33). The fallback to fine segments (DECISIONS #32) comes after this part; until then an operation that would need an ellipse or a freeform surface is refused, with a reason.

**Read first:** briefs 23 to 25 and their NOTES sections; `docs/research/exact-curves.md`; PLAN 1.6 (the tool table: Follow Me and Fillet / Chamfer rows), the Follow Me line in section 9 ("select a path, then click a profile; Alt follows a face's perimeter"); DECISIONS #20, #30, #31 and #32.

## Steps (each one is tested, documented and pushed on its own)
- **5a. Fillet and chamfer of an edge:** the Fillet / Chamfer tool on straight edges between two flat faces. *(Built.)*
- **5b. Cones, spheres and tori in the core:** the surfaces, files, shading, picking, MCP. No new tool. *(Built.)*
- **5c. Follow Me along lines.** *(Built.)*
- **5d. Follow Me along arcs** (turned shapes), and Alt (a face's perimeter as the path). Also test a path drawn from the middle of the profile's face (5c's gaps). *(Built.)*
- **5e. Moved to the end of part 6** (owner, 2026-11-11, DECISIONS #33): the curve tools become active only once curved faces work with the standard tools.

## Requirements by step

### 5a. Fillet and chamfer of an edge
- **Which edges:** a straight edge between two flat faces at an angle (convex or concave), each end a plain corner (three edges meet there, and at most one face lies across the end).
- **Chamfer:** each face is cut back by the distance, square to the edge; a flat face joins the two cuts; the faces across the ends get a straight cut.
- **Fillet:** a cylinder face of the radius, tangent to both faces, its axis along the edge; the faces across the ends get an arc. The lines where it meets the faces are soft and smooth. A face across an end must be square to the edge (a slanted one would need an ellipse: refused until DECISIONS #32).
- **Refused, with a reason:** not such an edge; the faces too narrow for the size; a slanted end (fillet only). Refusals come through `Command::refusal`.
- **The tool** (PLAN 1.6 row): click an edge to round it with the current radius; type a radius and Enter to change the last one; Ctrl makes a chamfer instead. A click near a corner of a flat face (two straight edges) rounds the corner, as the Arc tool's double-click does (brief 24). Hover highlights the edge. Shift (tangent-connected edges) comes later.
- The tool is greyed until 5e; MCP `use_tool` reaches it.

### 5b. Cones, spheres and tori
- `Surface::Cone`, `Surface::Sphere`, `Surface::Torus`: exact, with the side they face. Model check, Model Health (points off the surface, Repair), transforms (moves, turns, mirrors and even scales keep them exact).
- Shading from a grid on the surface (normals from the exact surface); picking by the shaded triangles, the point then put on the exact surface.
- Files: geometry layout 5, schema 6; old files open unchanged. MCP `get_geometry` lists the new surfaces.

### 5c. Follow Me along lines
- **Use:** select a path (connected lines), then click a profile face. With nothing selected, the tool says to select a path first.
- **SketchUp's rules:** the profile need not touch the path; it is projected square to the path's first segment, moves along the path, and is mitred at each corner (the corner's bisecting plane); the far end is square to the last segment.
- Profile lines make flat faces. Profile arcs make cylinders only where the path runs straight along their axis; at a mitred corner they would need an ellipse: refused until DECISIONS #32.
- A closed path makes a closed ring with no ends.
- The result is a closed solid where the profile is a closed face; lines along it are soft where the faces meet smoothly.

### 5d. Follow Me along arcs, and Alt
- A path with arcs turns the profile about each arc's axis. The profile must lie in a plane through the axis (square to the path there) and joins with arcs must be tangent; otherwise refused (a skew line would make a hyperboloid, a mitre an ellipse).
- Profile lines make cones, cylinders or flat rings; profile arcs make tori, or spheres when the arc's centre is on the axis.
- **Alt** follows a face's perimeter: the path is the outer loop of the selected face.
- A lathe: a profile and a circle about its axis make a closed solid of revolution.

### 5e. (Moved to part 6, step 6e)
- `GREYED_TOOLS` is emptied; the release notes say so. Everything from parts 2 to 5 is reachable from menus, the dock, search and hotkeys.

## MCP
- 5a and 5c to 5d: `use_tool` reaches the tools (tested). 5b: `get_geometry` lists cones, spheres and tori with their parameters.

## Out of scope
- Fillets where filleted edges meet (a rounded corner of three fillets needs a sphere patch or a freeform blend), Shift for tangent-connected edges, editable fillets (Inspector & Features).
- Follow Me along freeform paths, and anything needing ellipses: DECISIONS #32, step 6a in part 6.

## Tests
- **Ops:** a box edge filleted and chamfered: a closed solid with the exact volume; a concave edge; each refusal; undo and redo. Follow Me: a square profile along an L path (exact volume), a closed path, a lathe of a rectangle (a tube) and of a half disc (a sphere, exact volume 4/3 πr³), a torus; each refusal.
- **Session:** the Fillet / Chamfer tool by clicks and typing, Ctrl for a chamfer; Follow Me with a selected path and with Alt.
- **Property tests:** random fillets and chamfers on boxes keep the model check clean, and undo restores the model.

## Acceptance
- [ ] Every step: tests and CI green, NOTES updated, pushed.
- [ ] In a build made by hand: a box with a rounded edge, a lathe-turned vase, and the curve tools active in the menus.
- [ ] PLAN 6.0 updated.
