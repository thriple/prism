**New in this build: the Inspector.**
- **Window > Inspector** shows what is selected and lets you change it. For a group or component: its name, tag, hidden and locked, and the position and size of its box. Type a length with units or a simple sum, such as `1.5m`, `2*25.4mm` or `(1m+20cm)/2`, and press Enter; each change is one undo step.
- Select several groups and a field they differ on shows "—"; an edit applies to all of them. It also shows faces, edges and volume, and the area of selected faces.
- Editable fillets, chamfers and booleans come in the next parts.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
