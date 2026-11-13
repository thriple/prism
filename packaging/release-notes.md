**New in this build: camera and clipboard.**
- **Copy and paste between windows:** Copy (or Cut) in one Prism window, then Paste in another. The copy goes through the system clipboard.
- **Walk with the arrow keys:** up and down step forward and back, left and right turn. **Type an eye height** and press Enter to stand at that height.
- **The Camera menu ticks** Perspective or Parallel Projection, whichever is in use.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
