**New in this build: easier cage editing** (Edit Cage, on a selected freeform curve or face).
- **Box select control points:** drag a box with Select. Shift toggles the points inside, Ctrl adds them.
- **Scale has its box and grips:** the box is drawn round the picked control points. Drag a corner to scale evenly, an edge or face grip to scale along its axes; Alt scales about the middle, and you can type a factor.
- **Rotate shows its protractor** about the picked points' middle, with the angle as you turn.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
