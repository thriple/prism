**New in this build:**
- Intersect Faces and the Solid tools (Union, Subtract, Intersect, Trim, Split) now work with freeform (NURBS) faces, such as lofts: where they cross, Prism traces an exact-enough curve on both.

**Fixed in this build:**
- A flat face with a curved (NURBS) side now has its exact area and volume.
- A mirrored freeform face no longer faces the wrong way.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
