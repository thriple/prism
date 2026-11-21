# Brief 51: Shapes & Placement (plan 2026-11-21)

The chunk is large, so it goes in parts, each one tool or small group of tools that works on its own. The owner chooses the order (this is a proposal, built in this order unless told otherwise). Everything reuses what exists: groups and components with their own axes, `CopyInstances` (copies, each one step further than the last), `Place`, the Inspector's modifier stack for the live parts.

| Part | What | Size |
|---|---|---|
| **1** | **Mirror**: the Mirror tool (click a face: a mirrored copy of the selected groups across its plane; Alt turns them over instead) and Edit > Mirror Copy across the red, green or blue plane through the selection. *Built 2026-11-21.* | S |
| 2 | **Array**: linear (count, spacing, direction), polar (count, angle, axis) and along a path, as copies of the selected groups; first as an Edit command with a small window, then live in the Inspector (one live Array feature per group, count and spacing editable after the fact) | M |
| 3 | **Align and Distribute**, **Center / Align**, **Drop to Face** (rest on whatever is below) for selected groups, working on their boxes and on picked faces | M |
| 4 | **Primitives**: Box, Cylinder, Cone, Sphere, Torus, Capsule from a drag with typed values, each in a group that remembers its parameters (Inspector fields) until edited in a way that breaks them | M |
| 5 | **Trim / Extend** curves and edges to where they meet; **Project Curves** onto a face | M |
| 6 | **Hole** (a counterbore or countersink at a picked point, live) and **Draft Angle** on faces | M |
| 7 | **Bend / Twist / Taper** (a deform of a group about an axis, live while its handle is dragged) | L |
| 8 | **Unfold and Lay Flat**: a face down on the ground; a developable surface or a box flattened to a sheet with fold lines | L |
| later | **Live symmetry mode**: edits to one half mirrored to the other as you model | L |

## Part 1: Mirror (built)

- **Command:** `Change::Mirror { origin, normal }`: a reflection in a plane, usable in `Place` (turn the groups over in place) and `CopyInstances` (a copy across the plane; the command is named "Mirror Copy"). Applied an even number of times it is no change. A group's own coordinates are handled (`change_to_local` moves the normal by the inverse transpose). Mirror images keep their faces facing out (as Flip Along already does).
- **Tool** (`tool.mirror`, `session/mirror.rs`): select groups or components first (a note says so otherwise). Hover a flat face anywhere in the model: a dashed square shows the plane. Click: a mirrored copy appears across it, one undo step; Alt+click turns the selected groups over instead of copying; an arrow key locks a direction and the plane then goes through the snapped point, square to that axis (the Section Plane tool's rules, `section_place`).
- **Menu:** Edit > Mirror Copy > Across X, Y or Z plane (red, green, blue) through the centre of the selected groups together; greyed with no group selected.
- **Loose geometry too (1b, 2026-11-21):** selected faces and edges are copied across the plane (`Command::MirrorCopy`, `transform::mirror_only`: points and arcs move, every loop is turned round so faces still face out, points that land on others weld, then the coplanar rule). Groups and loose geometry can be selected together. Alt turns only groups over; loose geometry is always copied. The Edit > Mirror Copy items are enabled for any selection.
- **Not in part 1:** live symmetry, a typed offset for the plane, and mirror of a group made unique (a copy shares its component, as any copy does).
- Tests: the tool, the menu action, the reflection maths including a slanted plane and local coordinates.
