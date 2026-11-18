**New in this build: dimensions.**
- **Dimensions (Tools > Dimensions).** Click two points, then click where the dimension line goes: Prism draws the extension lines, arrowheads and the length in your document's unit. Click a dimension's line to move it; Edit > Dimension > Delete Dimension removes one; View > Dimensions hides them all. They are saved in the file and undo like any edit.
- **Fixed:** section planes were drawn only while the Section Plane tool was active; they now stay drawn with any tool.
- Also recent: section planes (Tools > Section Plane), the program's icon on Windows, and Alt tapped with Ctrl or Cmd held no longer turns on New Loop.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
