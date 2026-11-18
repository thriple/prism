**New in this build: Snap to Grid, sections in DXF and SVG, and easier editing of planes and dimensions.**
- **View > Snap to Grid** (off by default): on the ground and on a construction plane, the pointer goes to the nearest grid point when it is close. Object snaps still win.
- **File > Export > DXF / SVG has a Section view:** where your cutting section plane meets the model, drawn exactly (circles and arcs stay true arcs) for a router, laser or CNC program.
- **Alt + click** picks a section plane, a dimension or a construction plane (plain clicks never touch them). Alt + click on the picked one picks it up; type a distance and Enter to place it exactly; Delete removes it; right-click near one opens its menu.
- Construction planes can be moved after placing, and Shift + click in the Construction Plane tool makes one through three points. Dimensions follow the corners they were drawn to. Text lands on the working plane.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
