# Chunk 6: Edit Tools (part 2)

**Goal.** Finish the Edit Tools milestone: Offset, Tape Measure and Protractor, and the guides they make. Guides are stored in the model, drawn dashed, take part in inference, and are erased with the Eraser. After this chunk you can model a small building with measured openings.

**Read first:** PLAN.md 1.4 (Offset, Tape Measure, Protractor), 1.7 (their rows), "Tool memory", 1.14 (hidden must be free), DECISIONS #18 and #20, and brief 06 (part 1). The feel test does not have these three tools, so PLAN and SketchUp's behaviour are the reference.

## Requirements
- **Offset (F).**
  - The face under the cursor is tinted. Click it, move, and click to finish; or type a distance and press Enter.
  - The distance is how far the cursor is from the face's outline, measured in the face's plane. Inside the face it offsets inward; outside, outward.
  - Every loop of the face is offset: the outline and its holes.
  - Corners are mitred. The offset stops just before the new outline would cross itself or another loop, so the result is always valid.
  - The new loop splits the face (inward) or adds a ring around it (outward), through the same sticky geometry as drawing.
  - With a face selected, it offsets that face wherever you click.
  - **Double-click** a face repeats the last distance. Typing right after an offset changes it.
  - The result shows live while you move.
- **Guides in the model.**
  - Two kinds: a guide line (infinite, through a point along a direction) and a guide point.
  - Each one is a command, so it has one undo step.
  - Drawn dashed, in the guide colour. Guide points are drawn as small crosses.
  - Inference sees them: "On Guide" along a guide line, "Guide Point" at a guide point, and their crossings with each other, with axis guides and with edges.
  - **View > Guides** shows or hides them. Hidden guides are left out of the scene, so they cost nothing (PLAN 1.14).
  - The **Eraser** erases guides as well as edges, in the same undo step.
  - Out of scope for this chunk: selecting and moving guides.
- **Tape Measure (T).**
  - Click a start point with inference, then move. The distance shows in the measurement box and in a pill.
  - Started on an edge (not at its ends): a guide line parallel to the edge, through the cursor's point. The distance is measured square to the edge.
  - Started on a point: a guide point at the end, with a dashed line back to the start.
  - Click to finish, or type a distance and press Enter.
  - **Ctrl** toggles measure-only (no guide is made).
  - Arrows lock an axis, as in Line.
  - **Double-click** an edge makes a guide line along it at the last offset, on the side of the cursor (on the edge itself if there is no last offset).
- **Protractor.**
  - The full-circle protractor from Rotate, with the same plane rules, and the same Up, Right, Left and Down locks (DECISIONS #18).
  - Click the centre, set the start arm by clicking or by a drag from the centre, then move and click. This makes a guide line through the centre at that angle.
  - The angle snaps softly to 15°. Shift gives 45° steps. Type degrees and press Enter.
  - It has no hotkey (SketchUp has none either).
- **Greyed out until built (DECISIONS #20):** Offset, Tape Measure, Protractor and View > Guides become active now.
- **Feedback:**
  - Offset: the new outline, and the distance in a pill.
  - Tape Measure: a dashed line from start to end, the guide it will make, and the distance.
  - Protractor: as in Rotate, with the guide line it will make.

## Out of scope
Offset of edge chains and curves (that waits for arcs), Tape Measure's resize-the-model, Dimensions and Text (Annotation & Sections), selecting and moving guides, Edit > Delete Guides, and guides inside groups.

## Tests
- `prism-ops`:
  - Offset of a square, an L shape and a face with a hole, inward and outward. Each result has the expected area and passes `check`.
  - The clamp: an offset larger than the face is stopped short, and the result is still valid.
  - Property: a random convex polygon offset inward by a valid distance stays inside the original, and its edges stay parallel to the original edges.
- `prism-doc`: undo and redo of Offset, guide and erase-guide commands are exact.
- `prism-infer`: snapping to a stored guide line, a guide point, a guide crossing an edge, and two stored guides crossing.
- `prism-tools` scenarios:
  - Offset: a typed distance, a mouse offset, double-click repeat, retyping, and an outward offset.
  - Tape Measure: a guide parallel to an edge at a typed distance, a guide point from a corner, measure-only with Ctrl, and a double-click on an edge.
  - Protractor: a typed 30° guide.
  - Eraser: a drag over a guide and an edge erases both in one undo step.
  - Hidden guides are not snapped to.
- A recorded session in `docs/acceptance/sessions/` that offsets a wall face and places a window from tape guides.

## Acceptance
- [ ] Each tool works as described above, natively and on the web, with live previews and one undo step.
- [ ] Guides are drawn, snapped to, hidden and erased.
- [ ] PLAN 6.0 "Where we are" is updated, and NOTES lists what is left.
