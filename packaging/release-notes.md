**New in this build: Angle Guides.**
- **Rotate** now matches any angle a neighbour's edges make, not only the way it faces or a quarter turn from it. Turn a group, component or whole body near the angle of a neighbour's edge: a dashed teal guide shows along both edges, labelled Parallel, and the turn snaps softly to it.
- Typing an angle, Shift (45° steps) and snapping to a point win over it, as with the other guides.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
