**New in this build: Exact Curves.** The curve tools are active: Circle (C), Polygon, the Arc tools (A), Fillet / Chamfer and Follow Me. Circles, arcs, cylinders, cones, spheres and tori stay exact: Push/Pull cuts holes and slots through them, stops where it meets them and gives them a new size; lines drawn across them split them; Intersect Faces works on them; smooth curved faces show their outline.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
