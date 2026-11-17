**New in this build:**
- **File > Import of DXF and SVG drawings** (or drop one on the window). Lines, arcs, circles, ellipses, polylines and splines come in on the ground as a group named after the file, at their true size, and closed loops become faces, ready to Push/Pull. Blocks in a DXF come in where they are placed. Arcs stay true arcs and curves stay exact. Text, dimensions and hatches are not imported yet; the window and the status line say how many were left out.
- Also new recently: **File > Export > DXF / SVG** for routers, lasers and CNC programs (laid flat, or seen from the top, front or side, with true arcs).

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
