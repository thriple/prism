# Brief 34: the Loft tool

Owner request, 2026-11-13, after a recorded test: three arcs round a cut
box corner could not be filled. The owner is used to Curviloft (a
SketchUp plugin) and wants a hands-on tool rather than a command run on
a selection. This brief is the owner's description, turned into steps.
Its place in PLAN 6.0 is the owner's to choose.

**Read first:** PLAN 5 (feel), NOTES "Freeform NURBS (part 4)" and
"(part 5)" (Loft, Patch, `lay`), NOTES "Owner test, 2026-11-13".

## How it works
1. Pick the Loft tool. Click an edge: it selects the whole **side**, that
   is, the edges joined to it that run on without a sharp turn. That
   means an unbroken curve, or edges meeting under an angle threshold
   (default 30°, set in the tool's window). Shift adds or removes single
   edges.
2. Press **Next Side** (or Enter) when the side is complete. It is drawn
   in its own colour and numbered. Repeat. Sides can be curves, arcs,
   lines or chains of them.
3. With two or more sides, the tool works out what they make:
   - **A closed ring** (each side's ends meet the next one's): a
     boundary to fill. It needs 3 or more sides.
   - **Separate sections** that do not touch: a loft through them.
4. If the sides almost make a ring, an unobtrusive tip by the cursor says
   which end is open, and the edge (or chain) that would most likely
   close it is highlighted. A click takes it as the next side.
5. When the sides are valid, the new surface is shown as a live preview.
   **Loft** (a button in the window, or Enter) makes it. Esc steps back
   one side; Esc with no sides leaves the tool.
6. Afterwards: presets in the window change the result while it is still
   the last thing made, like retyping a value. Presets are smooth or
   straight between sections, and how many rows. The cage handles (Freeform
   NURBS part 3) shape it further.

## What has to be built underneath
- **Side chains:** follow edges from a picked one while the turn at each
  vertex is under the threshold, or while they share one curve.
- **Three sides: a pole.** A three-sided ring is a four-sided surface
  with one side shrunk to a point, as Curviloft does it (owner,
  2026-11-13). Prism's surface is exact NURBS, not a mesh: the Coons patch
  is built with one side a single repeated point. What has to change is
  the face. Freeform faces are four-sided rectangles of their surface
  today, so a **three-sided freeform face** (one side a pole) is needed.
  Tessellation fans the last row into plain triangles at the pole (the
  owner's "single triangle at the corner"), with the normal there taken
  from the rows beside it, so it shades cleanly. Picking, the model check,
  cage editing (the pole's control points move as one) and the file
  format accept it. The same face unlocks Revolve of a curve touching its
  axis, and lofting to a point.
- **Five or more sides: down to four.** Neighbouring sides are joined
  until there are four, at the flattest corners (where the turn is
  least), as Curviloft does. The joined sides become one side of the
  patch.
- **Loft through sides** reuses `loft::lofted`, with the sides in the
  order picked, not guessed.
- **Preview:** the surface tessellated into the overlay (as Paste's edges
  are), not into the model, until Loft is pressed.
- One command, one undo. The model check stays clean.

## Window
A side window like Fillet's, shown while the tool is active: the
threshold, the list of sides (each removable), Next Side, Loft, and the
presets.

## Tests
Session: the owner's recording (three arcs round a corner) fills with
one three-sided face; a square ring stays one patch; two separate curves loft;
an open ring shows the tip and highlights the closing edge; Esc steps
back; one undo removes the result. Ops: three-, four- and five-sided fills
are exact along every side; the pole shades without a crease (normals
near it agree within a small angle); a curve touching Revolve's axis
turns into a closed dome.

## Acceptance
- [ ] The owner's rounded box corner filled with the tool, by clicks.
- [ ] Tests green, NOTES updated, pushed.
