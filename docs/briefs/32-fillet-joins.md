# Brief 32: Fillet Joins

PLAN 6.0 row "Fillet Joins" (M5, size M), right after Freeform NURBS.
Owner request, 2026-11-12 (NOTES "Fillet on many edges"). Outcome: a
box's top face double-clicked and filleted, with clean corners.

**Read first:** NOTES "Fillet on many edges", "The Fillet window" and
"Freeform NURBS: what the whole chunk left for later"; brief 26 (step 5a,
the single-edge fillet) and brief 29 (the Fillet window);
`docs/research/nurbs-spike.md` ("Risks": two models, convert at the
boundary); `crates/prism-ops/src/round.rs`; `crates/prism-kernel`.

## What the owner asked for
1. Fillet / Chamfer on **many edges at once**.
2. **Double-click a face** (or select faces) and round **all its edges**;
   the face itself is left alone (it shrinks, it is not removed).
3. Edges that **meet at angles join cleanly**: a mitre where two rounded
   edges meet, a ball corner where three do, a blend where faces meet at
   uneven angles.
4. A **line made of segments on one axis is rounded as a whole**, unless
   single segments were picked by hand before choosing the tool.

## How it is built
Today's `round.rs` rounds one straight edge between two flat faces with
plain corners, and refuses everything else. Corners where rounded edges
meet need pieces of their own. arris builds these in closed form
(cylinders, tori, spheres, mitres; the spike's "fillet all 12 cube edges"
case), so this chunk is the first to send **solids** through the kernel.

- **The bridge** (`prism-kernel`): a closed region of Prism's model
  becomes an arris solid, the kernel works on it, and the result comes
  back as Prism faces with exact surfaces. The `Kernel` trait gains
  `fillet` and `chamfer` (edges of a solid, one size). Only this crate
  names arris types. Faces, edges and vertices the kernel did not touch
  keep their Prism ids, paint and texture placement; new faces take the
  paint of the face they were cut from (DECISIONS #36), and blend faces
  are Default.
- **What comes back** is planes, cylinders, cones, spheres and tori,
  which Prism already stores, bounded by lines, arcs and ellipses. An
  ellipse (a mitre between two fillets, a fillet's slanted end) is stored
  **exactly as a NURBS curve** (Freeform NURBS part 1), replacing the
  approximate segments of DECISIONS #32 for these cases.
- **One edit, one command:** `Command::RoundEdges { edges, size, fillet }`.
  A refused edit changes nothing and says why; the kernel's typed
  refusals are turned into Prism's plain sentences in one place.
- Lines between a blend and the faces it is tangent to are soft and
  smooth, as today.

## Steps (each tested, documented and pushed on its own)
- **A. The bridge.** Prism closed solid to arris and back, with no edit:
  the same faces, ids, paint and volume return. Loose edges and faces
  that are not part of the solid stay untouched. Tests: the spike's solids
  (box, box with a hole, cylinder, a pulled circle-in-rectangle, an
  L-shaped prism), each round trip clean under the model check. arris's
  solid crates get their DEPENDENCIES row; the web build's size is noted.
- **B. Many edges through the kernel.** `RoundEdges` for fillet and
  chamfer on any set of edges of one solid. Corners: two edges (a mitre),
  three (a ball corner or a triangle), a chain through a tangent vertex.
  Tests against volumes worked by hand: one edge (equal to today's
  result), a box's four top edges, all twelve edges (the rounded cube's
  volume, from the spike), a prism with a 60° corner, an edge beside a
  cylinder. Each refusal has a test. Undo and redo. Property test: random
  edge sets on boxes keep the model check clean and undo restores the
  model.
- **C. Picking, the way the owner asked.**
  - With the Fillet / Chamfer tool: a click on an edge rounds it (as
    today); **a double-click on a face rounds all its edges**; with
    **edges or faces selected before** choosing the tool, the first click
    (or Enter) rounds the selection. Shift-click adds an edge to a set,
    and Enter rounds the set.
  - **Whole lines:** a click on a segment takes every segment in the
    same straight run (end to end, same direction, same two faces'
    planes). Segments selected by hand before choosing the tool are taken
    as picked, no more.
  - The **Fillet window** works as before on the set: kind, size, Fit
    (the largest size that fits every picked edge), the live preview of
    the whole set on hover, why not before the click, and changing the
    last round in place.
  - Hover highlights everything a click would round.
- **D. Finish.** Rounding inside an open group; MCP (`use_tool` with
  several picks, and a `round_edges` check in `get_geometry`'s output);
  Planned Features updated; golden file `rounded.prism`; the release.

## Decisions to make (defaults are used until the owner says otherwise)
- **#37 One edge: old code or the kernel?** Default: every round goes
  through the kernel once step B's "one edge" test shows the same result
  as `round.rs`; `round.rs` then stays only for the flat corner of a
  loose face (the Arc tool's double-click), which is not a solid.
- **#38 Ellipses.** Default: exact NURBS curves, as above. Model Health's
  "approximate" row stops counting fillet ends made this way.
- **#39 Waiting.** arris's analytic blends took milliseconds in the spike
  (11 ms for twelve edges), so step B runs in place, with no progress
  window. Running kernel calls off the interface thread waits for the
  first slow operation (freeform booleans, Solids & CAD).

## Refused, with a reason
An edge that is not between two faces of one closed solid (a loose face
has no solid to round); a size too large for a face beside it or for a
neighbouring blend; an edge beside a freeform face (the kernel's blends
are closed-form for planes, cylinders and cones); edges of two different
solids in one go.

## Out of scope
Variable radius; setbacks and other corner styles; blends on freeform
faces; dragging to set the size (PLAN 1.4) and editing a fillet afterwards
(Inspector & Features); rounding open shells.

## Risks
- **The bridge is new and everything after it uses it** (Solids & CAD's
  booleans and shell). Keep it small, with its own tests, and keep ids
  stable by mapping, not by matching positions.
- **Tolerance:** Prism's is relative to the model's size and arris has its
  own; set arris's from the model's `Tol` at the boundary, in one place.
- **A face with an ellipse on a cylinder** is new for the unrolled
  tessellation and for picking: step B checks it by a screenshot test.
- arris is pre-1.0: the spike's fillet cases become tests here, so a
  version change shows.

## Acceptance
- [ ] Every step: tests and CI green (Rust updated first, see HANDOFF),
  NOTES updated, pushed.
- [ ] In a build made by hand: a box's top face double-clicked with the
  Fillet tool, its four edges rounded with clean corners; then all twelve
  edges; a chamfer of the same.
- [ ] PLAN 6.0 and DECISIONS updated.
