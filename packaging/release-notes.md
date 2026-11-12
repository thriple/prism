**New in this build: the Fillet window.** Picking Fillet / Chamfer opens a window on the right: choose Fillet or Chamfer and a size (empty fits each edge), and the edge under the cursor shows the result before you click. Right after a round, a new kind or size changes it. Ctrl swaps the kind while held.

**Also: colours (Basic Materials, part 1).** Window > Materials holds the model's materials and 16 plain colours: click one to paint with it, and edit its name and colour there. The Paint Bucket (B) paints the side of the face you click (a curved side as a whole, a group as a whole); Ctrl fills joined faces of the same material, Shift replaces it everywhere, Ctrl+Shift in the open group, Alt samples. Hovering shows the paint before you click. Entity Info names the materials, right-click > Select > All with the Same Material picks them, and Model Health purges unused ones. Exact Curves' circles, arcs, cylinders, cones, spheres and tori are in too.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
