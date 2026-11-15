# Group Axes: set a group's or component's own axes (owner request, 2026-11-15)

**Goal.** A group or component gets axes the owner chooses: an origin and a direction for red and green. Nothing moves in the world. The axes are where the object turns from, what Scale and Flip use, and the pivot a game engine gets in the GLB export (brief 35).

**Read first:** `crates/prism-doc/src/assembly.rs` (`Instance`, `make_group`), `crates/prism-doc/src/lib.rs` (`Command`), `crates/prism-tools/src/session/groups.rs` and `draw.rs` (the Rotated Rectangle's three clicks), brief 35.

## Size
S–M: one command, one tool, two menu items.

## Requirements
- **Change Axes command.** `Command::ChangeAxes { instance, axes }`, with `axes` the new axes in the definition's own coordinates. The definition's geometry and its own groups move by the inverse, and every instance of the definition anywhere moves by `axes`, so nothing moves in the world, every copy of a component included. One undo step. Locked groups refuse it. No file format change: axes are already part of an instance's placement.
- **The Axes tool** (Draw > Axes, and the Construct toolbar; greyed until now). Inside an open group: click the origin, click a point on the red axis, click a point that sets green (made square to red), all with inference. The tool draws the three axes as it goes. Esc cancels. At the top level it says to open a group first (moving the model's own axes is part of Annotation & Sections).
- **Context menu and Edit > Axes,** for the selected groups and components (inside an open group, the Axes tool is the way):
  - **Change Axes:** opens it and starts the Axes tool.
  - **Reset Axes:** axes square with the parent's again, origin at the low corner of the contents.
  - **Axes to Centre:** origin at the centre of the contents' box, directions kept.
- The axes are kept as rigid motions, so curved geometry stays exact. A group scaled unevenly keeps its directions on Reset (only its origin moves).

## Out of scope
- The model's own axes (Annotation & Sections).
- Typed values for the Axes tool.

## Tests
- Change Axes on a group and on a component used three times: every vertex stays where it was in the world; the instance's transform is the old one times `axes`. Undo puts it back.
- Reset Axes after a rotation: the transform has no rotation and the origin is the contents' low corner. Axes to Centre: the origin is the box centre.
- The Axes tool's three clicks in a session give the expected transform; at the top level nothing happens.
- A property test: random rigid axes keep every world vertex in place.
- The GLB export's node translation follows the new origin.

## Acceptance
- [ ] A turret's axes are set at its hinge; it rotates about it in Prism, and exports with that pivot (owner).
- [ ] PLAN and NOTES are updated.
