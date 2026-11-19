**New in this build: fillets and chamfers stay live on loose geometry, and go with their solid.**
- **A fillet or chamfer is live wherever you make it**, not only inside a group. Make Group and Make Component take the live fillets of the solids you group. Explode gives them back.
- Window > Inspector > Features (nothing selected) edits the top level's or the open group's fillets. Edit > Bake Features bakes them.
- Edit the fillet's own faces and that fillet is baked first, in the same undo step, with a red message. Undo brings it back.
- **Effects** in the menu bar is now the kit's switch. **Offset** lights up the edge it snaps to.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
