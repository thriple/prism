**Fixed in this build:**
- **Draw > Surface > Patch** fills any ring of curves now, not just four. A side drawn in pieces, or split by a leftover point, counts as one side, and a ring with three corners gets a smooth three-sided fill. A bend gentler than 30° is no longer treated as a corner, which caused the lip (a fold) next to where a curve ran on almost straight. A curve with a piece erased now counts as the part that's left.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
