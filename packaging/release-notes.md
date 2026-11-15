**New in this build: export for game engines.**
- **File > Export > Game engine (Beverly, Godot, Unity)** writes a GLB where every group and component is its own named object, nested as in Prism, turning about its own origin. A component used many times is stored once and shared.
- GLB from the media preset keeps groups too; tick **Merge into one mesh** for the old single mesh.
- Rounding many edges at once (Fillet Joins) is in from the last builds.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
