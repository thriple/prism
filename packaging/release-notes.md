**New in this build: where a shell's walls end.**
- **Shell / Thicken of joined freeform faces (the boat hull) can end its walls flush.** An open edge that lies in a flat plane, such as the hull's centre line, ends in that plane by itself, so two mirrored halves meet exactly. Select the shell with the Shell tool: a diamond sits on each open edge. Click it to switch between a square cut and a flush one; drag it along a straight edge to tilt the cut, and it snaps to nearby planes. One undo step each. The Inspector lists each edge too (Square or Flush, and the tilt in degrees for a straight one), so you can type it instead.
- It works both ways now: where the cut leans over the surface the wall is trimmed back, and where it leans away the surface is carried on past its own edge to meet the plane (the hull thickened outward ends flush along its centre line too). A cut that would need the surface carried on a very long way says so and keeps the square cut. A single freeform surface takes a flush edge as well, and dragging an edge snaps to guide lines and to the faces of other parts beside it.
- **Thin walls.** The boat hull now thickens from 0.1 mm to 5 cm either way (it used to refuse anything under 5 cm inward). Shell / Thicken now refuses a wall under 0.1 cm with a red message; drawing and Push/Pull stay exact to 0.01 cm.
- Also recent: live Union, Subtract and Intersect with a visible cutter, a live Shell with a thickness handle, Fillet and chamfer on selected edges, construction planes and the model's own axes.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
