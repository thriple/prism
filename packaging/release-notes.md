**New in this build: Everyday Commands.**
- **Cut, Copy and Paste** (Ctrl+X, C, V) work on loose geometry, groups and components, across open tabs; the paste follows the cursor and snaps like Move. **Paste in Place** (Ctrl+Shift+V) puts it back where it was. **Lock** and **Unlock** (Edit > Hide & Lock) keep a group from being changed. **Intersect Faces > With Context** works inside an open group.
- **Smart Guides** (View > Smart Guides, Ctrl+U): while you move or paste a group, a component or a whole body, it lines up with its neighbours (teal lines) and snaps to gaps equal to theirs (orange arrows with the gap, `d: 1.000 m`). A snapped point still wins.
- **Camera:** standard views (Top, Front, Iso and the rest), Parallel Projection, Field of View (type the degrees), Zoom Selection, Zoom Window, Walk and Look Around.
- **Drawing:** the **Rotated Rectangle** (three clicks, any plane) and **Freehand** (drag; it becomes one smooth curve, flat on the face you draw on).
- **Windows:** Edit > **Preferences** (units for new documents, the snap feel, Smart Guides, the keyboard preset), Help > **About**, and Help > **Tool Hints** (the steps of the tool you are using).

**Also since the last notes:** textures and the Position Texture tool, materials in OBJ, glTF and 3MF; freeform NURBS curves and surfaces (the Curve tool, cage editing, Loft, Revolve, Sweep, Patch, zebra stripes and curvature combs); an arc's plane now follows its second click, and Patch takes arcs as sides.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
