# Brief 31: Freeform NURBS

PLAN 6.0 row "Freeform NURBS" (M5, size XL), six parts (PLAN 6.0a).
Owner, 2026-11-12: build the whole chunk. Outcome: a loft you can pull
into shape.

**Read first:** PLAN 1.1 (it looks like a mesh, it is NURBS underneath),
2.3 (own the data model, borrow the hard maths), `docs/research/nurbs-spike.md`
(arris is the kernel), briefs 23 to 27 (how arcs and curved faces were
added: splines and freeform faces follow the same pattern).

## How it is stored
- **A NURBS curve** (`prism-geom`): degree, knots, control points,
  weights; closed ones are periodic (smooth at the seam). Prism owns the
  type, its evaluation and its file layout.
- **An edge on a curve** is `Curve::Spline`: the shared curve and the part
  of it the edge runs along (two parameters). Like an arc's circle, a split
  edge's halves share the curve.
- **A freeform face** is `Surface::Nurbs`: a whole NURBS patch, bounded by
  its own border curves. Cutting a patch (lines drawn on it, overlap
  splits, booleans) needs the kernel's surface intersection and comes with
  Solids & CAD; until then such an edit is refused with a reason.
- **The kernel** (`prism-kernel`, arris behind the `Kernel` trait) does
  what is hard: skinning (loft), revolution, fitting. It arrives with
  part 4, when it is first needed.

## Parts (each tested, documented and pushed on its own)
1. **NURBS curves.** The Curve tool: click control points, double-click or
   Enter to end, click the start to close; Ctrl switches to a curve through
   the clicked points. Display, picking and inference on curves (ends, on
   the curve, tangent). Lines and arcs that cross a curve split it and are
   split; a closed flat curve makes a face. Move, Rotate, Scale (uneven
   too: a NURBS curve stays exact), copies, files, MCP.
2. **Surfaces.** `Surface::Nurbs` in the core, files, the model check,
   transforms, adaptive tessellation (finer where it bends), shading,
   picking. Push/Pull of a flat face with spline edges makes exact
   freeform side faces.
3. **Cage editing.** A selected curve or freeform face shows its control
   points; Edit Cage (double-click, or the context menu) makes them
   pickable; Move, Rotate and Scale act on the picked points with the
   normal inference. Add control points (knot insertion) keeps the shape.
4. **Loft and revolve.** Loft: select two or more curves, run Loft.
   Revolve: Follow Me turns spline profiles about an arc's axis.
5. **Sweep and patch.** Follow Me along a spline path (a fitted sweep);
   Patch fills a closed ring of two to four curves.
6. **Curvature display.** Zebra stripes (a view toggle) and curvature
   combs on selected curves.

## Rules
- Every edit is a command; refusals come through `Command::refusal`.
- Every new action has a menu path; tools use the inference engine.
- A curve's ends must move together (as arcs): the shape changes through
  its cage (part 3).
- Tolerances come from the model's `Tol`.

## Out of scope
Trimmed patches and anything that cuts a patch; "Smooth" of a flat region
into a patch; SubD; the Freehand tool; STEP and 3DM (Solids & CAD).

## Tests
Geometry: evaluation against known curves (a quarter circle as a rational
quadratic), knot insertion keeps the shape, closest point, transforms.
Ops and session: each tool by clicks, undo and redo, file round trip,
refusals. Property tests: random curves split and transformed keep the
model check clean.

## Acceptance
- [ ] Every part: tests and CI green, NOTES updated, pushed.
- [ ] A loft between three curves, a cage point pulled, zebra on.
- [ ] PLAN 6.0 updated.
