**New in this build: group axes.**
- **Set a group's or component's own axes** (its origin and directions). Nothing moves; only the axes change, for every copy of a component.
- **Draw > Axes:** open a group, click the new origin, a point on the red axis, then a point that sets green.
- **Right-click a group > Axes:** Change Axes (opens it with the Axes tool), Reset Axes (square again, origin at the corner), Axes to Centre.
- Exporting for game engines (File > Export > Game engine) uses these axes as each object's pivot.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
