# Exact curves: what the core needs (2026-11-09)

Research for the Exact Curves chunk (PLAN 6.0, M4). Read before its briefs.

## Where the code is
- `prism-geom`: `Curve` has only `Line`, and `Surface` only `Plane` (DECISIONS #9 said to build lines and planes first, with types ready for more).
- `prism-topo`: a face's loops are **lists of vertices**. The edge between two neighbouring vertices is looked up (`edge_between`). That is used 187 times in 21 files (ops, view, tools, file, app).
- `prism-ops` (about 4,000 lines) assumes straight edges and flat faces everywhere: snapping and splitting (`sticky`), the coplanar rule (`settle`), face finding (`regions`), Push/Pull and solids, transforms, offset, Intersect Faces, Model Health.
- The file has a versioned geometry layout (1) and migrations, so a new layout can read old files.

## The core change: loops name their edges
With arcs, two vertices can be joined by **more than one edge**: a half circle closed by its diameter, a lens of two arcs, two arcs of different radius. So `edge_between(a, b)` can no longer find "the" edge of a loop. Each loop must list its edges (with a direction), as a B-rep does. This touches every place that walks a loop, so it comes first, on its own, with no visible change and every existing test passing.

## How a circle is kept
- An `Arc` edge: centre, axis (normal), radius; its ends are its two vertices; it runs counter-clockwise about the axis from start to end. A full circle is **one closed curve split into arcs at seam vertices**. Two arcs are enough: loops now name edges, so two arcs between the same two vertices are fine.
- The arcs of one circle or one drawn arc share a **curve id**, so they are picked, moved and erased as one, as in SketchUp, and inference treats them as one curve (centre, quadrants).
- Arcs stay exact under moves, rotations and even scales (centre and radius move, scale). Splitting an arc at a point gives two arcs with the same centre and radius.

## Exact intersections in a plane
Lines and circles in one plane meet at points found by a quadratic: line–arc and arc–arc are exact up to the square root, and the tolerance rules (PLAN 1.9) decide touching. That covers drawing lines across circles, circles across circles, and faces bounded by both. Faces stay planar.

## Curved faces
Push/Pull of a face bounded by arcs makes **cylinder** side faces. Follow Me along lines and arcs makes planes, cylinders, **cones**, **spheres** and **tori** (a profile swept round an axis). These are analytic surfaces with exact formulas. They need:
- shading and picking from a tessellation fine enough on screen (PLAN 1.1: adaptive to screen size);
- soft and smooth edges between their pieces (the drawing hides the seams);
- Push/Pull on a cylinder: a new radius.

## What exact curves cannot do yet (decision for the owner)
Some operations make curves that are not lines or arcs:
- **Uneven Scale** of a circle makes an ellipse.
- A cylinder **cut by a slanted plane** (Intersect Faces, a section, a slanted Push/Pull meeting it) makes an ellipse; two cylinders crossing make a 3D curve.
- **Follow Me along a freeform path** makes a general swept surface.

These need NURBS (PLAN 2.3; the kernel comes with the NURBS Spike, after this chunk). Until then each such operation must do one of:
1. **Refuse** with a message ("needs NURBS, coming later").
2. **Fall back to fine straight segments**, marked approximate, which Model Health lists and which NURBS can later replace.
3. **Add ellipses now** as another exact curve type (covers uneven scale and slanted cuts of cylinders, not crossing cylinders or freeform sweeps).

## Parts (proposed)
Exact Curves grows from L to five parts:
1. **Loops of edges:** the topology refactor, file layout 2 with a migration, no visible change.
2. **Arcs and circles in the core:** the `Arc` curve, curve ids, exact line–arc and arc–arc splitting, faces bounded by arcs, display and picking, inference points (centre, arc midpoint, quadrants), files, Model Health, MCP. The Circle and Polygon tools.
3. **The arc tools:** 2-point, 3-point, centre and pie arcs; real tangent inference; Offset of chains with arcs; double-click a corner to fillet it with Arc.
4. **Curved faces:** cylinders from Push/Pull, shading and picking of curved faces, soft and smooth edges, Push/Pull on a cylinder.
5. **Follow Me, fillet and chamfer:** Follow Me along lines and arcs (planes, cylinders, cones, spheres, tori); fillet and chamfer on planar edges.
