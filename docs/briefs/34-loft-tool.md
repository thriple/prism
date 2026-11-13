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
- **Three, five or more sides: split from a centre point** (owner,
  2026-11-13: "whatever creates the cleanest faces"; the methods compared
  in `docs/research/loft-corners.md`). A centre point is joined to the
  midpoint of each side, and each corner is filled with a four-sided
  Coons patch: three sides give three faces, five give five. Every face is
  exact along the boundary, has no point (singularity) to trouble offset,
  shell or booleans later, and needs no trimming. The inner seams are
  made smooth: the centre point and the inner curves are placed so the
  patches share tangent planes across them (G1), checked with zebra
  stripes. Four-sided rings stay one patch.
- Not used: a pole (one side shrunk to a point: simple, but singular, the
  thing Fusion and Rhino users work around), and a trimmed four-sided
  surface (needs trimmed faces; can be added after Solids & CAD).
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
three four-sided faces meeting at a centre point; a square ring stays one patch; two separate curves loft;
an open ring shows the tip and highlights the closing edge; Esc steps
back; one undo removes the result. Ops: three-, four- and five-sided fills
are exact along every side; across the inner seams the normals agree
within a small angle (no crease); the model check stays clean.

## Acceptance
- [ ] The owner's rounded box corner filled with the tool, by clicks.
- [ ] Tests green, NOTES updated, pushed.
