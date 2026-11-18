**New in this build: Effects.**
- When you finish an edit, the shape answers with a short line animation in the accent colour: a new shape flashes and echoes outward, a line or arc is traced along, a Push/Pull outline rises from where the face was, and things you erase fade where they were.
- Lofts, revolves and other curved faces roll their surface grid across them. Moved, rotated and scaled things travel from where they were. Follow Me carries its profile along the path, Offset grows from the face's edge, a new group's box draws in, and new section planes, construction planes and dimensions trace themselves.
- Turn it on or off with **Effects** in the menu bar (after Help), **View > Effects**, or **Edit > Preferences > Effects**, which also sets how long it lasts (1 second by default).
- **Snap to origin:** where the red, green and blue axes cross is now a snap point ("Origin") while the axes are shown.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
