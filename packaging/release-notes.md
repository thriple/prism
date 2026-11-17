**New in this build:**
- **File > Export > STEP (machining, CAM):** your solids go out as a STEP file with exact curves and surfaces, which Fusion CAM and other CAD and CAM programs open as solids. Each group and component is a named part (a component is one part, placed at each copy), colours go with them, and hidden groups are left out. Pick the unit, or export only the selected faces. The status line says what went out and what was left out, and why.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
