**New in this build:**
- **Solid > Convert > Faces ↔ Smooth Patch:** a grid of four-sided faces (rows and columns, such as a loft's rows) becomes one smooth freeform face through all their points; a freeform face becomes a grid of flat faces with soft edges between them.
- **Solid > Convert > Curve Chain ↔ NURBS Curve:** a selected chain of lines and arcs becomes one smooth-editable NURBS curve, exactly (corners kept); one NURBS curve becomes lines and arcs again (a free curve as fine marked segments).
- **Edit > Weld:** select edges joined end to end and Weld them into one curve, picked, moved and erased as one.

**Changed in this build:**
- A line that only touches one of a curve's own points (a circle's point, the corner of a welded run) no longer splits the curve. A line drawn across a curve still parts it.

**Fixed in this build:**
- **Rectangle:** a second corner snapped to a point off the first corner's plane (such as straight above a corner on the ground) now stands the rectangle up toward it, instead of making nothing. A click that would make no rectangle keeps the first corner, as the other drawing tools do.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
