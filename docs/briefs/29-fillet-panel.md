# Fillet panel

**Goal.** The Fillet / Chamfer tool shows how to use it. Picking the tool
opens a **Fillet** window in the right-hand column (with the other side
windows, not floating). The window sets the kind and the size, and the
edge under the cursor shows the result before you click.

**Read first:** PLAN 1.4 (Fillet / Chamfer in the tool table), NOTES
"Fillet / Chamfer" (the owner's request), brief 26 step 5a (the tool as
built). Owner, 2026-11-12: "give it a right-side window instead of
floating".

## Requirements
- **The window.** A side card titled "Fillet", in the same column as
  Entity Info and Materials. It opens, unfolded, whenever the Fillet tool
  is picked, and is shown only while that tool is active (it is the
  tool's settings). The × closes it until the tool is picked again.
- **Kind:** two buttons, Fillet (round) and Chamfer (cut flat). Holding
  Ctrl swaps the kind while it is held, as before.
- **Size:** a field labelled Radius (fillet) or Distance (chamfer). Empty
  means "fit each edge" (the size the tool picks for that edge, shown as
  the field's hint while an edge is hovered). Enter or leaving the field
  sets it. Typing a size into the measurement box still works and fills
  the field.
- **Changing the last one.** Right after a round, a new kind or size in
  the window (or typed) changes that round in place, one undo step, as
  typing did before.
- **Live preview.** Hovering an edge shows the model with that edge
  rounded at the window's kind and size; hovering near a flat corner shows
  the corner rounded. If the edge cannot be rounded, nothing is previewed
  and the status line says why before you click. The preview follows Ctrl.
  Moving off the edge, leaving the tool or Esc removes it.
- **Hints:** the window repeats the short how-to (click an edge; Ctrl
  swaps; near a flat corner rounds the corner).

## Not in this step
- Dragging to set the size (PLAN 1.4) and rounding several edges in one
  click: later, with Inspector & Features (editable fillets after the
  fact).

## Tests
- Session: hovering previews without changing the model; the preview
  follows the kind and Ctrl; a refused edge says why and shows nothing;
  the window's size and kind change the last round in one undo step;
  leaving the tool clears the preview.
- App: picking the tool opens the window unfolded; the card is shown only
  for the Fillet tool.
