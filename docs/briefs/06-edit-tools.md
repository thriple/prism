# Chunk 6: Edit Tools (part 1)

**Goal.** The feel test's editing tools, natively, on top of the inference engine: Push/Pull, Move and copy with arrays, Rotate with the full-circle protractor, Scale and the Eraser. Offset and Tape Measure follow in part 2, because each brings new geometry (polygon offsetting, and guides stored in the model).

**Read first:** PLAN.md 1.4 (Push/Pull, Move, Rotate, Scale, Eraser), 1.7 (their rows), the "Standard modifier meanings" and "Tool memory" lists, DECISIONS #14, #15, #18, #20, #21. The feel test's `toolDown(` (push, move, rotate, scale, erase branches), `preview(`, `applyPush(`, `moveApply(`, `rotApply(`, `scaleApply(`, `commit*(` and `drawProtractor(`.

## Requirements
- **Push/Pull (P).**
  - The face under the cursor is tinted. Click it, move to pull, and click to finish; or type a distance and press Enter.
  - While pulling, the distance snaps:
    - to the height of any endpoint within its radius ("Match Endpoint", with a magenta guide from that point);
    - to the ground;
    - to zero within 6 px.
  - **Alt** toggles "new loop" (PLAN 1.4, DECISIONS #14).
  - **Double-click** a face repeats the last distance.
  - Typing right after a pull changes it.
  - The result shows live as you pull.
- **Move (M).**
  - Click a start point (with inference, and with the moving points excluded from snapping), move, and click to finish. With nothing selected, it moves the item under the cursor.
  - Locks and typed distances work as in Line.
  - **Ctrl** toggles copy.
  - After a copy, `x5` makes an array of five copies at the same step, and `/5` places five evenly between.
  - Typing right after a move changes it.
- **Rotate (Q).**
  - The full-circle protractor (DECISIONS #18) shows at the cursor, on the plane of the face under it, the ground, or the view's main plane.
  - Click the centre. Then either drag and release to set the start arm, or move and click to set it. Move and click to finish.
  - **Up** locks the protractor to the plane of the face under it, after which the protractor can be moved anywhere. With no face under the cursor, Up locks it to blue. **Right** and **Left** lock it to red and green.
  - The angle snaps softly to 15°. **Shift** held gives 45° steps.
  - **Ctrl** toggles copy.
  - Type degrees and press Enter. Typing right after a rotation changes it.
  - With nothing selected, it rotates what is connected to the item under the cursor.
- **Scale (S).**
  - Scales uniformly about the selection's box centre, by how far the cursor is from it compared with where you clicked. The factor is clamped between 0.05 and 20, and snaps to 1 within 2%.
  - Type a factor and press Enter. Typing right after a scale changes it.
  - Uniform only; per-axis grips come later.
- **Eraser (E).**
  - Press and drag over edges. They turn red and are erased on release, one undo step (DECISIONS #15).
  - Shift (hide) and Ctrl (soften) are not built: hiding and soft edges do not exist yet.
- **One undo step each.** A copy, an array, a rotated copy and an erase are each one command. Add a `Copy` command that duplicates and transforms in one step.
- **Live previews** use a preview model that is drawn instead of the document's until the step is finished or cancelled. Esc cancels and leaves the tool active.
- **Greyed out until built (DECISIONS #20):** these five tools become active only now.
- **Feedback:**
  - Push/Pull and Move: a dashed line from start to end, with the distance in a pill.
  - Rotate: the protractor with ticks every 15° and longer ticks every 90°, its centre, the start and current arms, the swept wedge and the angle.
  - Scale: a line from the centre, with the factor.
  - Eraser: the doomed edges in red.

## Out of scope
Offset, Tape Measure and Protractor (part 2), Follow Me, auto-fold, Move's Alt, per-axis scale, hide and soften in the Eraser, and Smart Guides (they wait for groups).

## Tests
- Scenario tests in `prism-tools` for each tool:
  - Push/Pull: pull a rectangle into a box, a typed distance, Alt new loop, double-click repeat, match endpoint height, and retyping.
  - Move: an exact typed move, a copy, a `x3` array and a `/4` division, each one undo step.
  - Rotate: a typed 90° and 15° snap, Up locking to a face's plane and then moving the protractor elsewhere, a drag-set start arm, and a copy.
  - Scale: a typed factor.
  - Eraser: a drag that erases both faces beside an edge.
- Every result passes `prism_ops::check`.
- Undo and redo of each new command are exact (`prism-doc` tests).
- A recorded session in `docs/acceptance/sessions/` that models a small building: a box, a pulled roof edge, and a copied window.

## Acceptance
- [ ] Each tool works as in the feel test, natively and on the web, with live previews and one undo step.
- [ ] Typed values, retyping, arrays, Alt, Ctrl, Shift and Up do what is listed above.
- [ ] PLAN 6.0 "Where we are" is updated, and NOTES lists what is left.
