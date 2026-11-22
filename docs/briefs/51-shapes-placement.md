# Brief 51: Shapes & Placement (plan 2026-11-21)

The chunk is large, so it goes in parts, each one tool or small group of tools that works on its own. The owner chooses the order (this is a proposal, built in this order unless told otherwise). Everything reuses what exists: groups and components with their own axes, `CopyInstances` (copies, each one step further than the last), `Place`, the Inspector's modifier stack for the live parts.

| Part | What | Size |
|---|---|---|
| **1** | **Mirror**: the Mirror tool (click a face: a mirrored copy of the selected groups across its plane; Alt turns them over instead) and Edit > Mirror Copy across the red, green or blue plane through the selection. *Built 2026-11-21.* | S |
| 2 | **Array**: the Array tool with its window: linear (count, spacing, red/green/blue direction) and polar (count, angle, axis), for groups, components and loose faces and edges, with a ghost. *Built 2026-11-21.* Along a path, and live in the Inspector (one live Array feature per group, count and spacing editable after the fact), come later | M |
| 3 | **Align and Distribute**, **Center / Align**, **Drop to Face** (rest on whatever is below) for selected groups, working on their boxes. *Built 2026-11-22.* | M |
| 4 | **Primitives**: Box, Cylinder, Cone, Sphere, Torus, Capsule from a drag with typed values, each in a group that remembers its parameters (Inspector fields) until edited in a way that breaks them. *Built 2026-11-22* (click or drag; the window holds the sizes; on a face and a typed size after the drag come later) | M |
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

## Part 2: Array (built)

- **Tool** `tool.array` (`session/arraying.rs`, window `chrome/arraying.rs`): the window docks in the side column while the tool is active (like Text and Shell). Kind **Linear** (count, spacing in the document's unit, direction red, green or blue) or **Polar** (count, angle in degrees, axis through the middle of the selection; 360 spreads the copies round a full circle, less spreads them over that angle). Count is the number in all, the original included (2 to 501). A **ghost** shows the result while the window has it; **Make Array** makes it in one undo step (nothing selected says so).
- It reuses `CopyInstances` (groups and components) and `Copy` (loose faces and edges), whose step is a move or a turn: no new document command. Move and Rotate with Ctrl and `x5` already made arrays by hand; this adds exact numbers and the preview.
- **Left for later:** along a path; the Array as a live feature in the Inspector's stack (count and spacing editable afterwards); a spacing given as the gap between copies or the total length; the ghost does not follow a selection changed while the tool waits (press a window field to refresh).

## Part 3: Align, Distribute, Drop to Face, Center (built)

- **Where:** the Edit menu (and, for Drop to Face, the context menu). All work on the selected groups and components, by the smallest red/green/blue box that holds each one's blue box (so a turned group is lined up by its real extent). Each is one undo step (one `Place` per group that has to move, in one batch); nothing to do says so ("Already there").
- **Align** (`align.x|y|z.low|middle|high`): two or more groups. Low or high sides, or centres, are lined up to the whole selection's extent: Left/Right (red), Front/Back (green), Bottom/Top (blue), and Centre across each.
- **Distribute** (`dist.x|y|z`): three or more groups, equal gaps between the boxes along red, green or blue; the two outermost stay put.
- **Drop to Face** (`edit.drop`): each group falls straight down until it rests on the highest thing under it: another group's top (boxes that overlap it in plan), a flat face looking up, or the ground (a group already below the ground is left on its side of it). The lowest falls first, so a selected stack stays a stack.
- **Center / Align** (`edit.center`, now named **Center on Origin**): the selected groups together, their box middle on the red and green zero lines; the height stays.
- **Left out:** working on picked faces (align to a face you click), loose faces and edges (these work on groups only), and Drop to Face using loose faces inside an open group or sloped faces; hidden groups never count as a support.

### Part 3b: the Align and Distribute windows, toolbar buttons, context menu (2026-11-22)

- **Align window** (`win.align`, side card; Window menu and the dock): a 3 x 3 grid like an anchor picker: a right-angle wedge in each corner, a short bar on each edge, a hollow square in the middle. Corners line up two ways, edges one way (the side the cell is on), the middle centres both ways. **Seen from** Front / Side / Top picks the two axes the grid means (red across and blue up; green across and blue up; red across and green up, the back being up). Greyed with fewer than two groups selected. One undo step per click (`Session::align_cell`).
- **Distribute window** (`win.distribute`): three buttons, red, green and blue, each drawn as three evenly spaced marks. Greyed with fewer than three groups.
- **Dock** ("Copy & place" group): Align window, Distribute window, Drop to Face and Center on Origin (the last two act at once). The old placeholder buttons `tool.align` and `tool.drop` are gone (Drop to Face is `edit.drop`).
- **Right-click** (with a selection): Align (the nine one-way lines), Distribute, Drop to Face and Center on Origin.
- **Hotkeys:** every function is an action with a hotkey slot and no key by default: `align.{x,y,z}.{low,middle,high}`, `dist.{x,y,z}`, `edit.drop`, `edit.center`, `win.align`, `win.distribute`. The grid's two-way corners are made of two of the one-way lines (there is no action for each corner in each plane), so a key on a corner is a pair: bind the two lines you want, or use the window.

## Part 4: Primitives (built 2026-11-22)

- **Solids** (`prism_ops::primitives`): `Primitive::{Box, Cylinder, Cone, Sphere, Torus, Capsule}` with their sizes; `build()` makes an exact closed solid (planes for the box, cylinder, cone, sphere and torus surfaces, a capsule of a cylinder and two half spheres with poles) standing on the ground with its middle on the z axis, every face turned to look out. All six pass Solid > Validate with the right volume. The cylinder, cone, sphere and torus builders are the ones the test scenes had (now in `prism-ops`; `scenes` re-exports them). A size is at least 0.01 mm; a torus's tube must be thinner than its ring.
- **The group remembers:** `Definition.primitive` (saved in the file as `primitive` on the definition, no schema bump). It counts only while the group's geometry is exactly what that primitive makes and the group is not scaled (`Assembly::live_primitive`): edit a face, add a feature, scale it, and the sizes are gone and it is plain geometry; undo and they are back. Copies of a group keep it too.
- **Commands:** `AddPrimitive { primitive, at }` (a new group named for the kind, base middle at `at`) and `SetPrimitive { instance, primitive }` (made again in place, same position), each one undo step.
- **Tool** (`tool.prim`, `session/primitives.rs`, window `chrome/primitives.rs`): the Primitives menu items (`prim.box`, `prim.cyl`, `prim.cone`, `prim.sphere`, `prim.torus`, `prim.capsule`) pick the kind and the tool. The window has the six kinds and the chosen one's sizes as typed fields. A ghost shows it where the pointer is. **Click** to set it down on the snapped point; **press and drag**: a box takes width and depth from the two corners (the height is the window's), the round ones their radius (a torus its ring radius) from the middle out. The new group is selected and the window keeps the sizes.
- **Inspector:** a selected primitive group has a **Primitive** section with its sizes (`prim.<key>`); typing one makes it again in place.
- **Not in part 4:** standing on a face's plane (they stand on the ground direction, Z up, at the clicked point); a typed size right after the drag in the measurement box; a Smart-Guides-style snap of sizes; a live "primitive" feature that survives edits.
