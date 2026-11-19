**New in this build: fillets and chamfers are things you can select.**
- **Fixed:** a dialog opened again after you had clicked outside it (a second file dragged in to import) showed dimmed and closed on any click. Dialogs now keep the card above the dimmed backdrop every time.
- **Click a fillet or chamfer with Select** to pick the feature. It shows in Window > Inspector (size, Explode, Remove) and lights up. Right-click it for Explode Fillet / Chamfer, Remove, or Bake Features.
- **Fillet tool + a selected fillet or chamfer** shows a square handle: drag it to change the size (live), let go to set it. Nothing shows under other tools.
- **Window > Outliner** lists each body's fillets and chamfers under a "Features" row; click one to select it.
- Fillets and chamfers stay live on loose geometry as well as in groups, go with their solid when you group it, and are saved in the file (loose geometry's were not saved before).
- Editing a fillet's own face bakes it first, with a red message; Undo brings it back. **Effects** in the menu bar is the kit's switch. **Offset** lights up the edge it snaps to.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
