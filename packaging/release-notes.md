**New in this build: Offset goes as far as you ask, and keeps working.**
- **Offset** now goes the whole distance you move or type, even past where its sides meet or turn round. The new lines cross the old ones and split them; tidy up what you do not want, as in SketchUp.
- **Fix:** after an Offset taken as far as it could go, Offset no longer refuses every later try (on the thin leftover face, or on a rectangle drawn round it).
- **Recordings** (`Prism_session_*.json`) now start with a short note saying what the file is.
- Also in recent builds: the live boolean's cutter has position and size fields in the Inspector, and a union of boxes overlapping on shared planes is valid in either order.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
