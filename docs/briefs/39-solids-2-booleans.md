# Solids & CAD part 2: booleans (2026-11-16)

**Goal.** The Solid menu's Union, Subtract, Intersect, Trim, Split and Outer Shell work on groups and components that are solids, exactly, with the result a group. Part 2 of Solids & CAD (PLAN 6.0a). Part 1's Validate says which groups are solids.

**Read first:** `prism-ops/src/intersect.rs` (`intersect_with`), `prism-ops/src/shell.rs` (`ShellFace`, `winding`, `interior`), `prism-ops/src/solid.rs` (the solid rule, `keep_boundary`), `prism-ops/src/select.rs` (`merge_across`), `prism-ops/src/validate.rs`, `prism-doc/src/assembly.rs` (`make_group`, `explode`).

## Size
L: two steps. A: the core. B: the command, menus, the tool and MCP.

## Requirements
- **The core** (`prism_ops::boolean`): two solids in one frame, each a model whose faces close (Validate's rule: no open edges, no edge with three or more faces).
  1. Each is cut where the other crosses it (`intersect_with`, exact for flat faces and arcs, and for cylinders, cones, spheres and tori as Intersect Faces is).
  2. Each piece is placed against the other solid by the two-sided inside test: **inside**, **outside**, or **on** its boundary, facing **the same way** or **the opposite way**.
  3. Pieces are kept by the operation (A is the first solid, B the second):

     | Operation | A's pieces | B's pieces |
     |---|---|---|
     | Union | outside, on-same | outside |
     | Intersect | inside, on-same | inside |
     | Subtract (A − B) | outside, on-opposite | inside, turned round |

     Trim is A − B keeping B; Split gives A − B, B − A and A ∩ B; Outer Shell is the union of many, one after another.
  4. The kept pieces are joined (welded), and edges left between two faces on one plane facing the same way are erased (the eraser's merge), so the result has no seams.
  - The result is checked: a closed solid. A result with nothing left (Intersect of solids that do not touch) is empty, not an error.
  - Refused, in plain words: an input that is not a solid ("Refused: … is not a solid. Solid > Validate and Heal says why."), freeform (NURBS) faces (part 4).
- **The command** (`Command::Boolean { op, a, b }` and `Command::OuterShell { instances }`, one undo step, in the open group): the inputs are groups or components in it; each is used from its own geometry, placed. The result is a new group placed at A's axes; the inputs go (Trim keeps B; Split leaves three groups). Locked groups refuse it. A component used elsewhere stays as it is elsewhere.
- **The UI:**
  - Union, Intersect, Outer Shell and Split run on the selected groups when two or more are selected (Split exactly two). Otherwise each picks the tool.
  - **Subtract** and **Trim** are a tool, as in SketchUp: click the cutter, then the solid to cut. Hovering shows which is picked; Esc starts over. The other operations also work as this tool (click, click) when nothing is selected.
  - A refusal shows in the status bar.
- **MCP:** `run_action` reaches the menu actions on a selection; the tool is reached by `use_tool` (two clicks on the solids). Validate (`validate_solids`) reads the result. No new MCP tool.

## Out of scope
- Booleans through the kernel (part 3), freeform faces (part 4); loose geometry as an input (make it a group first); keeping the inputs (a Preference later).

## Tests
- Two overlapping boxes: Union, Intersect, Subtract (both ways), each a valid solid with the exact volume, no seams on shared planes.
- Boxes touching face to face: Union is one box; Intersect is empty. One box inside another: Subtract leaves a hollow (two shells).
- A box minus a cylinder (a hole): exact volume.
- Not a solid: refused. Split gives three solids with volumes adding up.
- The command: one undo step, the result a group at A's axes; a locked group refuses.
- The tool: two clicks run Subtract; Esc starts over. Union on two selected groups.

## Acceptance
- [ ] A box with a cylinder subtracted shows a clean round hole; Undo brings both back (owner).
- [ ] PLAN, NOTES and HANDOFF are updated.
