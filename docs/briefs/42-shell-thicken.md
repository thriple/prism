# Shell / Thicken (Tool Pass part 3, 2026-11-16)

**Goal.** One tool that turns faces into a solid of a set thickness. An open sheet (one face, or faces joined at their edges) becomes a slab. A closed body becomes hollow, with the new wall inside or outside it, and faces you choose can be left open, so a box becomes a tray. The result is always watertight.

**Read first:**
- NOTES "Shell / Thicken design" (in "Owner feedback for the next session (2026-11-16)");
- `prism-ops/src/thicken.rs` (Push/Pull of a freeform face: a slab);
- `prism-ops/src/boolean.rs` (`join`, `is_solid`);
- `prism-tools/src/session/round.rs` and `prism-app/src/chrome/fillet.rs` (a tool with a window);
- PLAN section 1.15 (tools) and the window table (Shell / Thicken: thickness, direction, which faces stay open).

## Size
XL: four parts, each pushed and tested on its own.

## How it works
- **Click a face.** The tool takes every face joined to it by edges: its sheet or its body.
- **Move the pointer, or type a thickness.** The thickness is measured along the clicked face's normal. Out of the face makes the new wall outside, and into it makes it inside. For a sheet, the slab grows to the side the pointer is on.
- **Shift+click a face of a closed body** to leave it open, or click it again to close it. Open faces show highlighted.
- **Click, or Enter,** makes it in one undo step. **Esc** steps back.
- **Thickness is exact.** Each face moves along its own normal by the thickness. A corner goes where the moved faces meet, so walls are the same thickness everywhere, not thinner at corners as with a per-point move.
- **The rim.** Where a sheet's edge, or an open face's edge, is not shared, a flat wall joins the face's edge to its moved copy. A wall that cannot be flat is made of two flat triangles.

## Parts
- **Part 3a. The operation, for flat faces with straight edges.**
  - `prism_ops::shelling::shell(model, faces, open, thickness)` and its refusal. `Command::Shell` in prism-doc, one undo step.
  - Refused, with a reason that says what to do:
    - faces that are curved or have arcs (part 3c);
    - faces that face different ways (use Heal);
    - more than two faces at an edge;
    - a corner where more than three faces meet and their moved copies do not meet at one point (part 3d);
    - a thickness that turns a face inside out.
- **Part 3b. The tool.** `tool.shell` is built: click, move, type, Shift+click to leave faces open, Enter or click to finish. A ghost of the result shows while you size it. The hint names the next step. It works inside an open group.
- **Part 3c. Curved faces.** Arcs in flat faces, cylinders, cones, spheres and tori (their offsets are the same kinds of surface), and freeform faces (the offset from `thicken.rs`).
- **Part 3d. The window and harder corners.**
  - The window: thickness, inside or outside, and the open faces.
  - Corners where more than three faces meet, as on a pyramid's tip or a mesh: the moved faces leave a gap there, and a small face fills it.

**MCP:** nothing new. `use_tool` and `run_action` drive the tool, and `Command::Shell` replays in recordings and the log.

## Out of scope
- Shelling lines into pipes or walls (Follow Me does pipes).
- Bodies that cross themselves after the move, beyond the checks named above. The model check (`check::problems`) still reports them.

## Tests
- **3a:**
  - a square face becomes a box of the thickness, on either side;
  - an L of two faces becomes an L-shaped slab with walls the same thickness;
  - a box hollowed inward gives two closed shells, with a volume of the outside less the inside;
  - a box with its top left open becomes a tray with flat rim faces;
  - outward growth;
  - each refusal;
  - results pass `check::problems` and are solid.
- **3b:** click, move and type; Shift+click opens a face; the ghost shows; Esc steps back; it works inside a group.

## Acceptance
- [ ] A box becomes a tray, and a face becomes a slab, the way the owner described.
- [ ] PLAN, NOTES and HANDOFF are updated after each part.
