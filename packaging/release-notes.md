**New in this build:**
- Draw > Surface's Loft, Revolve, Sweep and Patch no longer just refuse when the right curves are not selected: they say what to click, a click adds a curve (no Shift needed), a ghost shows the surface once the curves make it, and Enter makes it. Esc leaves.

**Changed in this build:**
- Solid > Outer Shell is gone: Union of three or more groups does the same.
- Push/Pull, Offset, Fillet and Follow Me say to open a group when you point at one, instead of doing nothing.

**Fixed in this build:**
- Drawing the same lines again over a cut circle or arc no longer splits its curve a second time.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
