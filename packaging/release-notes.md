**New in this build: Smart Guides in Scale and Rotate.**
- **Scale:** drag a grip on a group, component or whole object and its side lines up with a neighbour's side, and its width, depth or height snaps to a neighbour's. Both show an orange measurement, such as `Height 4.000 m`.
- **Rotate:** turn an object near the angle a neighbour faces and it lines up with it exactly, or square to it (`Parallel`, `Square`).
- Ctrl+U still turns all Smart Guides off.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
