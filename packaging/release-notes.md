**New in this build: fixes from the cleanup pass.**
- **Push/Pull no longer leaves the old face inside a box by surprise.** Tapping Alt while Ctrl (or Cmd) is held no longer turns on New Loop; only Alt (Option on a Mac) tapped on its own does.
- **New Loop shows at the cursor:** while it is on, a "New loop" label follows the pointer, so it can't be missed before a pull.
- Also recent: arc tangent guides at any edge, both ways, and Model Health finds and merges sliver faces; DXF and SVG import and export.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
