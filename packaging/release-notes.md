**New in this build: Solid tools.**
- **Solid > Union, Subtract, Intersect, Trim, Split and Outer Shell** work on groups and components that are solids. The result is a group, exact (flat faces, cylinders, cones, spheres and tori), with no seams left on flat faces; Undo brings the solids back.
- Select two or more solids and pick Union, Intersect, Split or Outer Shell: it runs at once. Or pick a command and click one solid, then the other.
- **Subtract and Trim:** click the cutter, then the solid to cut. Subtract removes the cutter; Trim keeps it. Esc starts over.
- A box with a box removed from inside it now counts as one valid solid with a hollow; Model Health no longer calls the hollow's faces reversed.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
