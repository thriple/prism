**New in this build: where a shell's walls end.**
- **Shell / Thicken of joined freeform faces (the boat hull) can end its walls flush.** An open edge that lies in a flat plane, such as the hull's centre line, ends in that plane by itself, so two mirrored halves meet exactly. Select the shell with the Shell tool: a diamond sits on each open edge. Click it to switch between a square cut and a flush one; drag it along a straight edge to tilt the cut, and it snaps to nearby planes. One undo step each. The Inspector lists each edge too (Square or Flush, and the tilt in degrees for a straight one), so you can type it instead.
- It works both ways now: where the cut leans over the surface the wall is trimmed back, and where it leans away the surface is carried on past its own edge to meet the plane (the hull thickened outward ends flush along its centre line too). A cut that would need the surface carried on a very long way says so and keeps the square cut. A single freeform surface takes a flush edge as well, and dragging an edge snaps to guide lines and to the faces of other parts beside it.
- **Thin walls.** The boat hull now thickens from 0.1 mm to 5 cm either way (it used to refuse anything under 5 cm inward). Shell / Thicken now refuses a wall under 0.1 cm with a red message; drawing and Push/Pull stay exact to 0.01 cm.
- **A new component asks for its name.** Make Component opens the Inspector with the Name field ready: type a name and press Enter (Esc keeps "Component"). The naming is part of making it, so one undo takes both away. A name another component already has is refused.
- **Tags you can type, and several per object.** In the Inspector or Entity Info, type tag names with commas between (`Walls, breakable`) and press Enter: new names make the tags, existing ones are used, None clears. Click a tag's button to put it on or take it off. In the Tags window, Add tag opens a name field ready to type (Enter makes it, Esc drops it); a name already used is refused in red. A **Beverly** list in the Tags window has the words Beverly acts on (collider, breakable, loose, stone, door...): click one to tag the selection. An object is hidden if any of its tags is hidden. Game-engine GLB files list each node's tags in `extras.prism.tags`, and Prism reads them back.
- **Mirror.** Select groups, components or loose faces and edges, pick the Mirror tool and click a flat face: a mirrored copy appears across that face's plane (Alt turns selected groups over instead of copying; an arrow key locks the plane's direction). Edit > Mirror Copy does the red, green or blue plane through the middle of the selection.
- Also recent: live Union, Subtract and Intersect with a visible cutter, a live Shell with a thickness handle, Fillet and chamfer on selected edges, construction planes and the model's own axes.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
