**New in this build: Offset matches, better recordings.**
- **Offset:** while you drag, snapping to a corner, midpoint or guide (also in another plane, such as the edge of a part beside it) now sets the distance from the edge you clicked near, so offsets on different faces line up. When a face cannot be offset (a spline edge, a curved face), the tool says why.
- **Record Session** now saves every group and component, which group was open and what was selected, so a recording made inside a component can be replayed. Files are larger by the size of the model.
- **Inspector** (Window > Inspector): see and change a group's name, tag, hidden and locked, and the position and size of its box, with units and sums like `2*25.4mm`.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
