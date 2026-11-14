**New in this build: round many edges at once (Fillet Joins).**
- With the Fillet / Chamfer tool, **double-click a face** to round all its edges. Corners join cleanly: a mitre where two rounds meet, a ball where three do.
- **Shift-click** edges to gather them, then press **Enter** (or select them first, then click or press Enter).
- A line drawn in pieces is rounded as a whole.
- Type a new size, or change it in the Fillet window, right after a round to change the whole round at once. One undo takes it back.
- Pulling the flat face beside a lofted face now works (last build) and is still in.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
