# Solids & CAD part 3: cutting freeform faces (2026-11-16)

**Goal.** A freeform (NURBS) face can be cut: lines drawn on it, Intersect Faces, booleans and Push/Pull work with it, as they do with flat faces and cylinders. The kernel's checker also takes spheres, tori and cone tips. Part 3 of Solids & CAD (PLAN 6.0a); the old part 3 (booleans on curved faces through the kernel) was merged into it by the owner on 2026-11-16, since part 2's booleans already handle cylinders, cones, spheres and tori.

**Read first:** NOTES "Freeform NURBS (part 2)" and "... what the whole chunk left for later", `prism-geom/src/patch.rs` (`Patch`, `param`, `grid`), `prism-ops/src/cut.rs` (cutting a turned face in (θ, s): the model for this), `prism-ops/src/crossing.rs`, `prism-view/src/tess.rs` (`patch`, `turned`), `prism-ops/src/check.rs`, `prism-ops/src/health.rs` (`face_volume`), `prism-ops/src/boolean.rs`, `prism-ops/src/bridge.rs`.

## Size
XL: three steps, each pushed and tested on its own.

## Requirements
- **3a. Trimmed freeform faces, and cutting one along edges on it.**
  - A face on a NURBS surface may have any loops of edges (lines, arcs, splines) lying on the surface, not only its patch's four sides. Its loops are laid out in the patch's (u, v) by closest point (`Patch::param`), every edge by its drawn points.
  - A face whose loops are its patch's four sides is drawn and integrated as now. A trimmed one is drawn by triangulating its (u, v) region and refining where the surface bulges (as turned faces are); picking and mesh export use the same triangles.
  - Area and volume (Model Health, Validate, booleans) integrate over the trimmed region, exactly enough to match the untrimmed patch to 1e-9 when the trim is its own sides.
  - The model check accepts a trimmed face whose edges lie on its surface and whose outer loop winds with its normal in (u, v).
  - **Cutting** (`cut::split_patch`): edges that lie on a freeform face and run across it (from its boundary to its boundary, or round a closed loop inside it) split it into faces, each keeping the surface, paint and facing. Used by drawing (sticky geometry) and Intersect Faces.
  - Cage editing refuses a trimmed face in plain words (moving the trims with it is later).
- **3b. Crossings with freeform faces.** A plane, a cylinder, cone, sphere or torus, or another freeform face crossing a freeform face is traced in (u, v) (marching along the zero set, refined onto both surfaces) and added as a spline edge fitted within the tolerance (or fine segments marked approximate where a fit fails, DECISIONS #32). Intersect Faces, sticky geometry and the boolean core use it; the inside test counts ray hits on freeform faces. So Union, Subtract and the rest work with freeform faces.
- **3c. The kernel and Push/Pull.** The bridge sends trimmed freeform faces, spheres, tori and cone tips, so Validate checks every solid through the kernel. Push/Pull of a freeform face offsets it through the kernel; a flat face beside one pulls as beside a cylinder.

**MCP:** `get_geometry` already lists a freeform face's surface; it adds its trims' (u, v) loops for a trimmed face (3a). Nothing else new: Intersect Faces, the Solid tools and Push/Pull are reached by `run_action` and `use_tool`.

## Out of scope
- Moving trims in cage editing; three-sided faces; offsetting a trimmed face's boundary on its surface (Offset on freeform faces).

## Tests
- 3a: a bump patch cut by a spline drawn across it in (u, v): two faces, areas and volumes adding up to the whole; clean model check; drawn and picked; a closed loop inside cuts a hole and an island. A cage edit of a trimmed face is refused.
- 3b: a plane through a bump patch: one crossing, on both surfaces within the tolerance; a box minus a lofted solid is a valid solid with the volume the kernel gives.
- 3c: Validate on a ball goes through the kernel; Push/Pull of a freeform top moves it out by the distance.

## Acceptance
- [ ] A line drawn across a lofted surface splits it; a box subtracted from a lofted solid leaves a clean cut (owner).
- [ ] PLAN, NOTES and HANDOFF are updated after each step.
