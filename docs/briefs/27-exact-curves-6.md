# Exact Curves, part 6: curved faces in the standard tools

**Goal.** Cylinders, cones, spheres and tori work with the standard tools the way flat faces do: what Prism cannot store exactly falls back to fine segments marked approximate (DECISIONS #32), planes cut curved faces exactly, Intersect Faces and sticky geometry split them, Push/Pull booleans and stops see them, and every curved face can be resized by Push/Pull. At the end the curve tools become active (DECISIONS #20, #33). **A new kind of geometry is not done until the standard tools work with it.**

**Read first:** briefs 23 to 26 and their NOTES sections (especially the known gaps of 4, 5b, 5c and 5d); `docs/research/exact-curves.md`; `docs/research/chunks-ahead.md` (part 6 section: the four steps every boolean takes, and the table of plane crossings); DECISIONS #29 (Intersect Faces), #30 (coplanar faces merge), #31 (Push/Pull on a solid), #32 (the fallback) and #33.

## Steps (each one is tested, documented and pushed on its own)
- **6a. The segment fallback** (DECISIONS #32): approximate curves in the core, files and Model Health; the operations refused for want of an ellipse stop refusing. *(Built.)*
- **6b. Planes meet curved faces exactly:** the crossings, cutting a curved face in (θ, s), Intersect Faces, sticky crossings and snapping. Follow Me's result becomes sticky (5c's gap). Split in two:
  - **6b-1:** crossings and cutting, Intersect Faces. *(Built.)*
  - **6b-2:** sticky geometry (drawing across curved faces, Follow Me's result welded, snapping), crossings that cross inside a curved face, and faceting cut faces. *(Built.)*
- **6c. Push/Pull booleans and stops with curved faces:** inside or outside by exact ray hits; a cylinder pulled into a box cuts it; a pull or a new radius stops at a curved face; the solid overlap test, also used to refuse a Follow Me sweep that overlaps itself (5c's and 5d's gaps). Split in two:
  - **6c-1:** the inside test with curved faces, and Push/Pull booleans with them. *(Built.)*
  - **6c-2:** a pull and a new radius stop at curved faces; the solid overlap test for Follow Me. *(Built.)*
- **6d. A new size for cones, spheres and tori by Push/Pull; outline lines along curved faces.** Split in two:
  - **6d-1:** new sizes for cones, spheres and tori, stopped at what they meet. *(Built.)*
  - **6d-2:** outline lines.
- **6e. The curve tools become active** (moved from 5e): `GREYED_TOOLS` is emptied, a `[release]` build.

## Requirements by step

### 6a. The segment fallback
- **In the core:** an edge can be marked approximate (`Edge.approx`): a fine straight segment standing in for a curve Prism cannot store (an ellipse, a crossing of two cylinders). The segments of one such curve share a curve id, so they select and erase as one curve. Their points are exact (on both surfaces, where the curve is a crossing); only the chords between them are not.
- **How fine:** enough segments that a chord never strays from the true curve by more than 1/1000 of the curve's size, and never fewer than an arc of the same turning would draw with (`prism_geom::segments`).
- **The model check:** on a curved face an approximate edge is held when its two ends lie on the surface (its chord cuts inside by design).
- **Files:** geometry layout 6, schema 7; older files open unchanged (the flag is false). Golden files for schema 6 are kept.
- **Model Health:** a row lists approximate curves (information, not a problem: no Repair), and selects them. MCP `get_health` lists them.
- **What stops refusing:**
  - **Uneven Scale** of an arc or circle: it becomes approximate segments along the true ellipse (today it keeps a wrong arc and the check reports its ends off it). A curved face scaled unevenly becomes flat strips between its approximate curves, their long edges soft and smooth and marked approximate.
  - **Follow Me** (`ARCS`, `NOT_SQUARE` with arcs): a profile arc meeting a mitre, or a tilted profile with arcs, is swept as fine segments. A path arc that cannot be turned exactly (`OFF_AXIS`, `NOT_TANGENT`) is followed as fine segments of the arc, then mitred like lines. (As built: the sweep is faceted, seams soft and smooth; exact cylinder walls ending on approximate ellipses would be nicer, NOTES.)
  - **Fillet with a slanted end** (5a): the end face gets an approximate ellipse where the fillet's cylinder meets it.
- Refusals that are about impossible shapes stay (a sweep that crosses itself, a profile across an arc's axis, no path).

### 6b. Planes meet curved faces exactly
- **The crossings** (`docs/research/chunks-ahead.md` table): a plane square to the axis cuts every turned surface in circles; a plane through the axis cuts a cylinder or cone in lines, a sphere or torus in circles; any plane cuts a sphere in a circle. Those are exact arcs and lines. Every other crossing (a slanted cut: ellipse, parabola, hyperbola, spiric section) uses the fallback (6a).
- **Two curved faces:** exact only on the same axis (circles), parallel cylinder axes (lines) and a sphere centred on a cylinder's or cone's axis (circles); everything else uses the fallback.
- **Cutting a curved face:** in (θ, s) (5b's `Revolution` and `Model::param_loop`) a cut along a circle is a line s = constant and a cut along a line through the axis is θ = constant, so a curved face is split by the same 2D region rebuild flat faces use, done in (θ, s). Fallback crossings are polylines there too.
- **Faceting cut faces:** `approx::facet` (6a) handles curved faces that are rectangles in (θ, s); once faces are cut it must handle any loop there (6a's gap).
- **Intersect Faces** (DECISIONS #29) includes curved faces. **Sticky geometry:** a line, rectangle or circle drawn across a curved face splits it, and Follow Me's result is split and welded where it meets other faces. **Snapping** finds the new crossings (they are edges).

### 6c. Push/Pull booleans and stops with curved faces
- **Inside or outside:** the ray test that keeps or drops a piece (DECISIONS #31) counts curved faces by exact hits: a quadratic for cylinders, cones and spheres; tori through their drawn triangles followed by Newton steps (as picking does since 5b).
- **Booleans:** a cylinder or a box face pulled into a solid with curved faces cuts and joins as with flat faces; Push/Pull on a face beside a curved face stops refusing (`CURVED_PULL`) where the result can be built exactly or with the fallback.
- **Stops:** a pull stops where the face meets a curved face (DECISIONS #31, 2026-11-09), and a new radius (Exact Curves 4) stops at what it would run into.
- **The solid overlap test** (does a solid cross itself?) refuses a Follow Me sweep that overlaps itself further along a path or round past a sphere's poles (5c's and 5d's gaps).

### 6d. New sizes, and outline lines
- **Push/Pull on a cone, sphere or torus face** gives it a new size, as on a cylinder (Exact Curves 4): a sphere's radius, a torus's tube radius, a cone moved square to its side (same angle). The faces beside it follow; what cannot follow is refused with a reason.
- **Outline lines:** where a smooth curved surface turns away from the eye, a line is drawn along its outline (SketchUp's profile lines), on screen only, updated as the view turns. They are not edges and cannot be picked.

### 6e. The curve tools become active
- `GREYED_TOOLS` is emptied; the release notes say so. Everything from parts 2 to 5 is reachable from menus, the dock, search and hotkeys. The push carries `[release]`.

## MCP
- 6a: `get_health` lists approximate curves; `get_geometry` marks approximate edges. 6b to 6d: `use_tool` reaches the changed behaviour (tested where a tool changes).

## Out of scope
- Bending a curved face by moving one of its points (Move of a vertex of a cylinder): refused as now, until NURBS (NURBS Spike).
- Freeform surfaces, and replacing approximate curves with NURBS (NURBS Spike).
- Booleans between groups (Inspector & Features, `docs/research/chunks-ahead.md`).

## Tests
- **6a:** a circle scaled unevenly: approximate segments, every point on the true ellipse, the check clean; a cylinder scaled unevenly; Follow Me of a disc round an L (exact cylinder walls, approximate mitres, volume within the chords' error); a path arc at a corner; a fillet with a slanted end; files round trip; Model Health lists and selects the curves.
- **6b:** a box face cut square to a cylinder's axis and through it, a sphere cut by a slanted plane (an exact circle), a cylinder cut slanted (approximate); Intersect Faces with a cylinder through a box; drawing across a cylinder splits it; the model check clean and volumes kept.
- **6c:** a cylinder pulled into a box (a hole or a joined post), a box pulled into a cylinder, stops at curved faces, each Follow Me overlap refused.
- **6d:** new sizes for each surface with exact volumes; outline lines in a screenshot test.
- **6e:** the action registry test: no greyed tools left.

## Acceptance
- [ ] Every step: tests and CI green, NOTES updated (known gaps sorted, DECISIONS #33), pushed.
- [ ] In a build made by hand: a cylinder pushed through a box makes a hole; a slanted cut through a cylinder; the curve tools active.
- [ ] PLAN 6.0 updated.
