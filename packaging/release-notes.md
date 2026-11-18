**New in this build: live fillets bake themselves when you edit their own faces.**
- **Live fillets:** edit a face next to a fillet (push/pull, move) and the fillet follows. Edit the fillet's own face, edge or corner (erase, split, drag) and that fillet is baked first, in the same undo step, with a red message in the status bar. Undo brings it back.
- Fillet and Chamfer inside a group stay live: change the size under Window > Inspector > Features, or Edit > Bake Features. Saved in the file.
- **Effects** in the menu bar is now the kit's switch. **Offset** lights up the edge it snaps to.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
