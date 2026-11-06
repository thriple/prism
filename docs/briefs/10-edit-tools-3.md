# Chunk 6: Edit Tools (part 3)

**Goal.** Faces that pass through each other are known to the program (DECISIONS #29). Snapping finds where they cross, and Edit > Intersect Faces turns the crossings into edges. Then a shape pushed through another can be cut out to leave a tunnel.

**Read first:**
- DECISIONS #29 and #15;
- PLAN 1.6 (Intersect Faces) and 1.10 (Model Health's crossing-faces check);
- `crates/prism-infer/src/lib.rs` (how guides and their crossings work).

## Requirements
- **Crossings (`prism-ops`):** a pure function gives the segments where two planar faces cross.
  - It intersects the two planes, clips that line to each face (outline and holes) in the face's own plane, and keeps where both clips overlap.
  - A segment that lies on both faces' outlines is their shared edge, so it is not a crossing.
  - A segment on one face's outline but inside the other face is a crossing. For example, a wall standing on a floor that has no edge under it.
  - It works whether or not the faces are connected.
- **Snapping (`prism-infer`):**
  - With the cursor over a face, the crossings of that face with every other face become guides. They are dashed, labelled "Face Intersection", and can be snapped along within their length.
  - Where a crossing meets an edge or another guide, the usual Intersection and Guide Cross snaps appear, so its ends and the points where edges pass through the face can be snapped to.
  - Hidden faces are not in the scene, so they cost nothing.
  - It stays fast on the Large Scene: only the hovered face is checked against the others, with a bounding-box test first.
- **Edit > Intersect Faces:**
  - **With Model:** every crossing in the model becomes an edge.
  - **With Selection:** only crossings that involve a selected face.
  - Each runs as one command, so one undo step. The new edges go through sticky geometry, so the faces split along them. Afterwards the middle of a shape pushed through another can be selected and erased, or pushed through.
- **Greyed out until built (DECISIONS #20):** the Intersect Faces actions become active.

## Out of scope
- Curved faces (they wait for the kernel).
- Intersect inside a group (it waits for groups).
- Model Health's crossing-faces list (the Model Health chunk). It reuses the same function.

## Tests
- `prism-ops`:
  - two crossing squares give one segment;
  - faces that share an edge give none;
  - a face with a hole gives two segments;
  - a wall standing on a floor gives the line under it;
  - parallel faces give none;
  - all of these at three scales.
- Intersect Faces on the owner's example:
  - a box pushed through another box, still joined to it, gets the crossing loops on the other box's faces;
  - the result passes `check`;
  - erasing the faces inside then leaves a tunnel;
  - one undo puts it back exactly.
- `prism-infer`: over a face that another box passes through, the crossing guide is offered, and its end where it meets an edge snaps as an Intersection.
- Large Scene: the time for an inference with crossings, recorded in NOTES.

## Acceptance
- [ ] The owner's tunnel example can be traced by snapping, and cut with Intersect Faces.
- [ ] PLAN 6.0 is updated, and NOTES lists what is left.
