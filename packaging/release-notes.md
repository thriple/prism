**New in this build:**
- **Shell / Thicken** (Shapes): click a face and move the pointer, or type a thickness and press Enter. A sheet of faces becomes a slab. A closed body becomes hollow: move out of the face for a new wall outside, into it for one inside. Shift+click faces to leave them open, so a box becomes a tray. A ghost shows the result before you click. It takes flat faces with straight edges for now; curved faces come next.

**Fixed in this build:**
- Erasing faces now also erases the hidden (soft) lines that were only there between them, such as a loft's rows. Left behind, they could not be seen, yet a new line drawn there was cut where they ended.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
