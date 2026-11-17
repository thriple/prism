**New in this build:**
- **File > Export > DXF / SVG**, for routers, lasers and CNC programs. Arcs and circles go out as true arcs (so CAM makes smooth G2/G3 moves), splines as splines. Pick a view:
  - **Laid flat** (for sheet parts): each part's largest flat face, or each flat face you select, laid outside up in a row, holes and all;
  - **Top, Front or Side**: the edges seen straight on (hidden lines are not removed yet).
- The file is at its true size in mm, cm or inches. In DXF, a run of lines and arcs is one closed polyline, and each group is its own layer.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
