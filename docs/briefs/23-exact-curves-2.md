# Exact Curves, part 2: arcs and circles in the core

**Goal.** Edges can be exact arcs. A circle is drawn, split by lines and other circles, filled as a face, moved, rotated, scaled, erased and saved, and stays exact. The Circle and Polygon tools make them. The tools stay greyed in menus, toolbars and releases until all five parts of Exact Curves are done (owner, 2026-11-09); they are tried through tests, the MCP server and builds made by hand.

**Read first:** brief 22 and `docs/research/exact-curves.md`; PLAN 1.1, 1.9 and the Circle/Polygon row of the tool table; DECISIONS #9 and #32; NOTES "Exact Curves (part 1)".

## Steps (each one is tested, documented and pushed on its own)
Part 2 is too big for one go, so it is built in four steps:
- **2a. Arcs in the core:** the curve, curve ids, face finding with arcs (a circle alone is a face), files, areas, transforms, drawing. *(Built.)*
- **2b. Crossings:** lines and arcs cut each other, and points split arcs. *(Built.)*
- **2c. Picking and inference:** arcs are picked, erased and snapped to. *(Built.)*
- **2d. The Circle and Polygon tools.** *(Built.)*

## How an arc is kept
- `Curve::Arc { centre, axis, radius }`:
  - the edge runs counter-clockwise about the unit `axis`, from vertex `a` to vertex `b` (an edge's `a` is its smaller id);
  - its ends lie on the circle, within the model tolerance;
  - a full turn is not one edge: a **circle is two half arcs** between two seam vertices (loops name their edges since part 1, so two edges between the same two vertices are fine).
- **Curve ids:** the edges of one circle, one drawn arc or one polygon share a `CurveId` (the model counts them like other ids). Picking, moving and erasing take the whole curve; inference treats it as one (centre, quadrants).
- **Exact under transforms:**
  - Move and Rotate move the centre and turn the axis;
  - an even Scale scales the radius;
  - a mirror flips the axis;
  - an uneven Scale of an arc would make an ellipse, so it is refused for selections with arcs, with a message (DECISIONS #32: the segment fallback comes after part 5).
- **Splitting an arc** at a point on it gives two arcs with the same centre, axis and radius.

## Requirements by step

### 2a. Arcs in the core
- `prism-geom`:
  - `Curve::Arc`, with its point at a fraction of the sweep, the sweep angle and length, the tangent at each end, and the closest point to a given point;
  - sampling to a polyline within a chord error.
- `prism-topo`:
  - `Edge` gains `curve_id: Option<CurveId>`;
  - `Model::edge_points(e)` gives an edge's points from one end to the other (128 even steps per full turn for now; adaptive by screen error is part 4);
  - `loop_points(face, k)` gives a loop's points along its edges (the polygon used by point-in-face, interior points and tessellation);
  - `face_area` is exact for arcs (the polygon of the vertices plus each arc's circular segment, signed).
- The model check:
  - an arc's ends lie on its circle;
  - its axis is the face's normal (or the opposite one) for every face that uses it;
  - radius > tolerance.
- Doc command `Command::Circle { centre, normal, radius }` adds two half arcs with one curve id; the plane's faces are rebuilt as for lines, so a circle alone becomes a face, and a circle inside a face (not touching its edges) makes a hole and a face.
- Face finding (`regions`) traces edges, not vertex pairs: half-edges are sorted at a vertex by their tangent there, then by curvature where tangents agree; loops' polygons come from `loop_points`; faces are made with their edges.
- Until 2b, the straight-edge crossing and on-edge tests skip arcs (a line drawn across a circle does not split it yet).
- Transforms keep arcs exact, as above.
- Operations that do not know arcs yet refuse them with a message, from 2a on: Push/Pull and Intersect Faces (part 4), Offset (part 3).
- Drawing:
  - edges are drawn from `edge_points`;
  - faces bounded by arcs are tessellated from `loop_points`;
  - fine enough for now (adaptive tessellation by screen error is part 4).
- Files: geometry layout 3 stores arcs and curve ids, schema 4; layout 1 and 2 files still open.
- MCP: `get_geometry` lists each edge's curve (line, or arc with its centre, axis and radius) and curve id. (Its test comes with 2d, when a tool can draw a circle over MCP.)

### 2b. Crossings
- Exact crossings:
  - line–arc and arc–arc in a plane, by the quadratic;
  - tolerance decides touching (PLAN 1.9);
  - a point landing on an arc splits it.
- A line across a circle splits its face in two; two overlapping circles make three faces; a circle inside a face makes a hole and a face.

### 2c. Picking and inference
- Picking and hover use the drawn points; a click picks the arc, and selecting it takes its whole curve (as with SketchUp curves).
- The Eraser removes the whole curve, and faces it bounded merge as with lines.
- Inference points:
  - arc ends;
  - centre (shown while hovering the arc);
  - midpoint of each arc;
  - quadrants of a circle;
  - "on arc" with the closest point.
- Model Health lists arcs whose ends left their circle, and can repair by moving the ends back onto it.

### 2d. The Circle and Polygon tools
- **Circle (C):**
  - click the centre, then a radius point;
  - the plane comes from the face under the cursor or the axis lock, as with Rectangle;
  - the measurement box takes the radius;
  - a double-click places the same size again (PLAN tool table).
- **Polygon:**
  - the same clicks, with a side count in the measurement box (`6s`), 6 by default;
  - straight edges with one curve id, inscribed.
- Both stay greyed in menus, toolbars and the dock (brief 22), but reach `use_tool` over MCP for testing.
- MCP: no new tools beyond `get_geometry`'s curve fields (2a); the tools reach `use_tool` by name.

## Out of scope
- The Arc tools, tangent inference and fillets (part 3).
- Curved faces, cylinders and adaptive tessellation (part 4).
- Follow Me (part 5).
- Ellipses and the segment fallback (after part 5).

## Tests
- **Geometry:** arc points, tangents, closest point and sampling; circle–line and circle–circle crossings, tangent cases included.
- **Topology:**
  - a circle's two arcs between the same two vertices;
  - splitting an arc;
  - exact area (πr² within tolerance);
  - the check finds an end off its circle.
- **Ops:**
  - a circle alone makes a face;
  - a line across it makes two;
  - two overlapping circles make three;
  - a circle in a face makes a hole;
  - Move, Rotate and an even Scale keep arcs exact (centre and radius checked);
  - an uneven Scale is refused;
  - erase a circle.
- **Files:** layout 3 round trip; layout 1 and 2 golden files still open.
- **Session:** draw a circle and a polygon with the tools; type the radius and side count.
- **Property tests:** random circles and rectangles keep the model check clean.

## Acceptance
- [x] Every step: tests and CI green, NOTES updated, pushed.
- [ ] Circles and polygons drawn over MCP `use_tool` in a build made by hand look round and stay round when moved.
- [x] PLAN 6.0 updated.
