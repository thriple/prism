# Chunk 9: Model Health (part 1: checks, panel and fixes)

**Goal.** Window > Model Health lists what probably should not be in the model, each with a count. A click selects the items and zooms to them, and the common problems can be fixed with one undo step (PLAN 1.10).

**Read first:**
- PLAN 1.10, 1.9 and 1.3 (watertight);
- DECISIONS #29, #30 and #31;
- `crates/prism-ops/src/check.rs`, `solid.rs` and `intersect.rs`.

## Parts of this chunk
1. **Checks, panel and fixes** (this brief).
2. **Problems drawn in the viewport** (problem edges coloured, reversed faces tinted), checks run live in the background, and a check before export.

## Requirements
- **Checks:** pure functions in `prism-ops` (a new `health` module), run on the open group's model. Each gives the faces, edges and points it found.
  - **Duplicates:** points closer than the tolerance (and near-misses under ten tolerances), edges with the same ends, faces with the same loops.
  - **Reversed faces:** faces whose loop runs the same way as a neighbour's across a shared edge (they disagree), and faces of a closed shell whose normal points into the solid.
  - **Open edges:** edges with one face (holes in a shell), and edges with more than two faces (non-manifold). Each shell is reported as watertight or not.
  - **Interior faces:** faces of closed shells with the solid on both sides.
  - **Degenerate:** faces of near-zero area, slivers (area tiny for their outline's length), edges shorter than the tolerance, and loose edges that belong to no face.
  - **Crossing faces:** faces that pass through each other with no edge along the crossing (DECISIONS #29).
  - **Nesting:** how many groups and components there are, the deepest nesting, and definitions shared by several instances (the whole document).
  - **Scale:** geometry far from the origin compared with its size, and parts smaller than the tolerance.
- **Fixes, each one undo step:**
  - **Orient Faces:** turn faces round so each shell agrees, facing out of closed shells.
  - **Clean Envelope:** remove interior faces in the selection, or the whole model if nothing is selected (PLAN 1.10).
  - **Intersect Faces:** the existing command, offered on the crossing row.
  - **Remove Stray Edges:** delete loose edges and points.
- **Panel:**
  - one row per check, with its count, greyed at zero;
  - a click selects the items and zooms to them;
  - a fix button where there is one;
  - "Run again" re-runs the checks (they also run each time the panel opens and after each edit while it is open);
  - Nesting and Scale are information only.
- **Menus:** Window > Model Health; Edit > Reverse Faces and Orient Faces become active where built (DECISIONS #20).

## Out of scope
Drawing problems in the viewport, running in the background while closed, and checking before export (part 2). Healing gaps (Solids & CAD).

## Tests
- Each check on a small hand-made case that has the problem, and on a clean box that has none.
- Orient Faces turns a flipped face back, and a box built inside out faces out after.
- Clean Envelope removes a wall left inside a box and keeps the outside.
- Remove Stray Edges.
- The panel's rows are computed from the session (pure); a session test selects a row's items.

## Acceptance
- [ ] A model with each problem shows it in the panel, and the fixes work with undo.
- [ ] PLAN 6.0 is updated, and NOTES lists what is left.
