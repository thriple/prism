**New in this build: the last of the smaller gaps.**
- **Sweep turns an open profile with its path:** sweep a profile along a curved path and it turns with the path, as a pipe's circle does (along a straight path it slides, as before).
- **Freehand in 3D:** hold Shift as you start a Freehand stroke to draw over faces at any height, not just on one plane.
- **Preferences > Reset Units** puts new documents back to metres.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
