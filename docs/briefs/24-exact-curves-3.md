# Exact Curves, part 3: the Arc tools, tangents, Offset with arcs, fillets

**Goal.** Arcs are drawn directly, with real tangents. Arc tools: 2-point, 3-point, centre and pie. Lines and arcs snap tangent to arcs. Offset works on faces bounded by arcs, and a double-click on a corner with the Arc tool rounds it. Like the Circle and Polygon tools, the Arc tools stay greyed in menus, toolbars and releases until all five parts of Exact Curves are done (owner, 2026-11-09). They are tried through tests, MCP `use_tool` and builds made by hand.

**Read first:** brief 23 and its NOTES sections; `docs/research/exact-curves.md`; PLAN 1.1 point 10, the Arc row of the tool table, and "The arc tool turns cyan when the arc is tangent and magenta for a half-circle"; DECISIONS #20 and #32.

## Steps (each one is tested, documented and pushed on its own)
- **3a. The Arc tools:** 2-point, 3-point, centre and pie. *(Built.)*
- **3b. Tangent inference:** tangent guides from arcs, and arcs that continue tangent from an edge. *(Built.)*
- **3c. Offset with arcs:** Offset of faces whose loops have arcs.
- **3d. Corner fillets:** double-click a corner with the Arc tool.

## Requirements by step

### 3a. The Arc tools
- **One drawn arc is one curve:** one `Curve::Arc` edge (sweep under a full turn), its own curve id. A sweep of exactly a full turn is a circle (two halves, as in part 2).
- **2-point arc (`arc.2`; `tool.arc`, A, picks it):**
  - click the start, click the end, then move to set the bulge and click;
  - the measurement box takes the bulge height after the second click, or a radius as `2r`;
  - the preview turns magenta at a half circle (bulge = half the chord).
- **3-point arc (`arc.3`):** start, a point on the arc, end. The arc passes through all three. Three points in a line make no arc.
- **Centre arc (`arc.c`):** the centre, the start (sets the radius and plane), then the end angle. The measurement box takes the radius after the first click and the angle in degrees after the second.
- **Pie (`arc.p`):** as the centre arc, plus the two radius lines, so it closes into a face.
- **The plane** comes as for Rectangle and Circle: the face under the cursor, an arrow-key lock, the ground, else the view's main axis. For the 2-point and 3-point arcs, the plane also has to hold the clicked points.
- Arcs are added with the sticky rules of part 2: they cut and are cut by what they cross, and they close faces.
- All four stay greyed, as `GREYED_TOOLS` (brief 23). Only `use_tool` reaches them.

### 3b. Tangent inference
- **Kind `Tangent`:**
  - a woken arc end gives a guide along the arc's tangent there;
  - Line and the 2-point arc snap along it ("Tangent");
  - the guide is exact, from `Curve::start_tangent` / `end_tangent`.
- **Arcs that continue an edge:** a 2-point arc that starts at the end of a line or arc shows a cyan preview where its start tangent matches that edge's direction, and snaps the bulge to it.
- A line drawn from an arc end along its tangent meets it smoothly. Face finding already sorts tangent half-edges by curvature (2b), so faces still close.

### 3c. Offset with arcs
- Offset of a face whose loops have arcs:
  - an arc offsets to a concentric arc (radius ± the distance);
  - lines offset as now;
  - corners join where the offset pieces meet, exactly (line–line, line–arc, arc–arc).
- An arc whose radius would reach zero or less is dropped, and its neighbours join directly. If that is not possible, the offset distance shrinks, as Offset does now for invalid outlines.
- The new loops are added with the sticky rules, with arcs kept exact.
- The 2a refusal of Offset for faces with arcs goes away. Its status-bar message from 2c goes with it.

### 3d. Corner fillets
- With an Arc tool active, a double-click on a corner between two straight edges in one face rounds it:
  - the arc is tangent to both edges;
  - the edges are trimmed to the arc's ends;
  - the face keeps its shape otherwise.
- The radius is the last one typed (`2r`), or else a tenth of the shorter edge. Typing a radius right after a fillet changes it.
- Corners whose edges are too short for the radius are refused, with a status-bar message.

## MCP
- No new MCP tools. `get_geometry` already lists each arc's centre, axis, radius and curve id (part 2).
- The Arc tools reach `use_tool` by id (`arc.2`, `arc.3`, `arc.c`, `arc.p`), with typed bulge, radius (`2r`) and angle. Its description names them.
- A test draws each kind of arc over MCP and reads it back with `get_geometry`.

## Out of scope
- Curved faces, cylinders, soften and smooth (part 4).
- Follow Me, and fillet and chamfer of solid edges (part 5).
- Ellipses and the segment fallback (after part 5, DECISIONS #32).

## Tests
- **Geometry:** an arc through three points; an arc from two points and a bulge; tangent directions; offset of an arc.
- **Ops:**
  - each Arc tool's arc is exact (centre, radius and sweep checked);
  - a pie is a face;
  - Offset of a disc, of a rounded rectangle and of a face with an arc dent;
  - a fillet keeps the face's area minus the corner piece (r² − πr²/4 for a right angle).
- **Session:**
  - each Arc tool by clicks and typing;
  - a line snapped tangent off an arc end;
  - a 2-point arc tangent to a line;
  - a double-click fillet, and retyping its radius.
- **Property tests:** random arcs drawn among lines and circles keep the model check clean, and undo restores the model.

## Acceptance
- [ ] Every step: tests and CI green, NOTES updated, pushed.
- [ ] In a build made by hand, arcs and fillets drawn over MCP `use_tool` are round, tangent where they should be, and stay exact when moved.
- [ ] PLAN 6.0 updated.
