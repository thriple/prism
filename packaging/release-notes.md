**Faster: Shell / Thicken, booleans and Push/Pull into solids.** Joining two solids no longer runs the whole Model Health check three times or searches every line of the model for each new one, and finds crossings once instead of twice. A shell built face by face that took minutes now takes seconds.

**Shell / Thicken no longer refuses: it builds and warns.** Where a shape cannot be shelled as one piece, it is built anyway (face by face, the pieces joined) and a warning says why and what to check; the parts to check are drawn orange on the preview, so you can see which way will not work before clicking. The warning also shows in the Shell window and the status bar. A wall under 0.1 cm is made with a "Very thin" warning instead of being refused. Faces the tool used to turn round or cut square without saying now say so too. On a body with many faces that cannot be joined into one piece, the pieces are kept side by side and may overlap (Model Health shows where).

**Fixed: an open box with a leaning face would not shell** (a truck cab with a sloped windscreen and no roof: "that thickness turns a face inside out" at any thickness). It shells both ways now.

**New: loose lines show as profiles.** A line drawn on a group's surface from outside it (not part of the group, and with no face of its own) is drawn thick, like the edges of a loose face already are, so you can tell it from the group's own lines.

**Fixed: the line where a box's top meets a cylinder wall** (after Intersect Faces) drew dashed. It draws whole now.

**New in this build: a selected group or component lights up all its lines,** not just the box round it, so you can see the whole thing is selected (a locked one in the locked colour).

**Fixed: a cylinder wall with a hole in it** (after Intersect Faces with a box through it) drew as a fan of wrong triangles across the cylinder. It draws properly now. **A line drawn over a hidden line shows:** a line drawn straight down a cylinder's wall at its seam used to vanish into the hidden seam line.

**Also new: the top of a cylinder can be scaled, moved and turned.** Select the top (or bottom) face and use Scale, Move or Rotate: the side follows. Scaled evenly about its centre it becomes a cone; slid sideways the side leans; scaled one way the top becomes an exact ellipse; turned, the side twists. All exact, not facets. The Scale tool's box now fits circles and curves properly, so a circle has grips.

**Also new: Fillet / Chamfer shows what is waiting.** Edges you click are rounded only when you press Enter; until then the round shows selected (blue), so it no longer looks finished before it is.

**Also new: curves follow the same rules as flat faces.**
- **Curved faces cut and merge like flat ones.** An arc or circle lying on a cylinder, cone or sphere cuts it, as a line cuts a flat face. Where two curved faces overlap on one surface (a wall pushed over an existing wheel well), they become one set of faces instead of two on top of each other. It happens after every edit, not just Push/Pull.
- **Circles no longer vanish.** Moving, copying or pushing anything onto existing geometry could delete every circle's face nearby. Fixed, along with arcs left twice in the same place after a Push/Pull, which made thin slivers and doubled walls.
- **Intersect Faces > With Selection works with groups.** Select groups (and faces): each is cut where the others cross it, in one undo step. With nothing selected it says what to select instead of doing nothing.
- **Reversed faces draw no false outlines.** A purple (reversed) face no longer gets thick profile lines along edges it shares with faces in its own plane.
- **Shell works on shapes with round cut-outs.** A box with half-round wheel wells (or any curved face meeting flat ones) used to be refused ("more than three faces meet at a corner"). It now hollows or thickens properly, with or without open faces.
- **Alt/Option Push/Pull cuts through what it passes.** A plain push still stops at the first face in its way. A new-loop push goes on past, and now cuts the faces it crosses (a floor pushed down past wheel wells), instead of leaving them crossing.
- **Hover lights the whole curved face.** Pushing a wheel well deeper, or pushing a face across onto one, could leave hard lines inside the curved surface, so hovering lit only part of it. Those lines are now soft, as a cylinder's own seams are.
- **The line where two mirrored halves meet** draws as an ordinary line between both halves, not as a heavy outline that could flicker when orbiting low.
- **The camera can orbit under the ground** to look at a model from below.
- **Bend / Twist / Taper (groundwork).** Taper on a group is now a live feature you can change in the Inspector afterwards; the Bend / Twist / Taper tool itself comes next.

**Also recent: where a shell's walls end.**
- **Shell / Thicken of joined freeform faces (the boat hull) can end its walls flush.** An open edge that lies in a flat plane, such as the hull's centre line, ends in that plane by itself, so two mirrored halves meet exactly. Select the shell with the Shell tool: a diamond sits on each open edge. Click it to switch between a square cut and a flush one; drag it along a straight edge to tilt the cut, and it snaps to nearby planes. One undo step each. The Inspector lists each edge too (Square or Flush, and the tilt in degrees for a straight one), so you can type it instead.
- It works both ways now: where the cut leans over the surface the wall is trimmed back, and where it leans away the surface is carried on past its own edge to meet the plane (the hull thickened outward ends flush along its centre line too). A cut that would need the surface carried on a very long way says so and keeps the square cut. A single freeform surface takes a flush edge as well, and dragging an edge snaps to guide lines and to the faces of other parts beside it.
- **Thin walls.** The boat hull now thickens from 0.1 mm to 5 cm either way (it used to refuse anything under 5 cm inward). Shell / Thicken now refuses a wall under 0.1 cm with a red message; drawing and Push/Pull stay exact to 0.01 cm.
- **A new component asks for its name.** Make Component opens the Inspector with the Name field ready: type a name and press Enter (Esc keeps "Component"). The naming is part of making it, so one undo takes both away. A name another component already has is refused.
- **Tags you can type, and several per object.** In the Inspector or Entity Info, type tag names with commas between (`Walls, breakable`) and press Enter: new names make the tags, existing ones are used, None clears. Click a tag's button to put it on or take it off. In the Tags window, Add tag opens a name field ready to type (Enter makes it, Esc drops it); a name already used is refused in red. A **Beverly** list in the Tags window has the words Beverly acts on (collider, breakable, loose, stone, door...): click one to tag the selection. An object is hidden if any of its tags is hidden. Game-engine GLB files list each node's tags in `extras.prism.tags`, and Prism reads them back.
- **Mirror.** Select groups, components or loose faces and edges, pick the Mirror tool and click a flat face: a mirrored copy appears across that face's plane (Alt turns selected groups over instead of copying; an arrow key locks the plane's direction). Edit > Mirror Copy does the red, green or blue plane through the middle of the selection.
- **Array tool.** Select groups, components, faces or edges, pick Array and set it in the window: Linear (count, spacing, red, green or blue direction) or Polar (count, angle, axis). A ghost shows the result; Make Array does it in one undo step.
- **Duplicate Again (Ctrl+D), and Ctrl before you click.** After you copy something with Move or Rotate, Ctrl+D makes the next copy the same distance on (or the same angle round the same centre), and again each time you press it: a chain for layouts. It works for groups and loose geometry, and carries on after an `x5` array. Tapping Ctrl now also works before the first click of a Move or Rotate (the hint and a label at the cursor say Copy is on), as well as while moving.
- **Revolve makes a proper solid.** Turning a closed outline (a rectangle, hexagon, circle, a shape with an arc side, even one with a hole) now leaves every face looking out of the solid; before, some came out inside-out. The hint says any outline or curve works. A shape touching the axis is still refused.
- **Align, Distribute, Drop to Face.** Select groups or components, then Edit > Align (left, right, centre, front, back, bottom, top), Edit > Distribute (equal gaps along red, green or blue) or Edit > Drop to Face (each one falls until it rests on what is below it, or the ground). Edit > Center on Origin puts the selection round the zero lines. One undo step each.
- **Align and Distribute windows.** The dock's Copy & place group has Align (a 3 x 3 grid: wedges for corners, bars for edges, a hollow square for the middle; pick Front, Side or Top for the two directions), Distribute (red, green or blue), Drop to Face and Center on Origin. Right-click a selection for Align, Distribute, Drop to Face and Center on Origin too. Every one has a hotkey slot (none set by default).
- **Primitives.** Draw > Primitives (or the Primitives button): Box, Cylinder, Cone, Sphere, Torus and Capsule as exact solids. Pick the kind and type its sizes in the window, then click to set it down or press and drag (a box from corner to corner, the round ones from the middle out). Each one is a group that remembers its sizes: select it and the Inspector shows them, and changing one makes it again in place. Edit its faces or scale it and it becomes plain geometry.
- **Trim / Extend and Project Curves.** Trim / Extend: click a piece of a loose line or arc to take it away (edges of faces are left to the Eraser); hold Alt and click near a free end of a straight edge to carry it on to the first edge or face in its way. Project Curves: select lines, then click a flat face to drop them straight onto it, where they cut it.
- **Draft Angle.** Select flat faces, set the angle in the Draft Angle window and click the neutral face (the base the part comes out from): the faces lean about where they meet it and the corners follow, so the part is exactly valid. Positive narrows the part away from the base, negative widens it.
- **Holes.** The Hole tool makes a plain, counterbored or countersunk hole where you click a flat face of a solid group (set the kind, sizes and Through in its window). It is a live subtract, so it can be taken out or baked from the Inspector, and one undo removes it.
- Also recent: live Union, Subtract and Intersect with a visible cutter, a live Shell with a thickness handle, Fillet and chamfer on selected edges, construction planes and the model's own axes.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
