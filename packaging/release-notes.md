**New in this build: live fillets, and a switch for Effects.**
- **Fillet and Chamfer inside a group stay live.** Select the group, open Window > Inspector, and change the size under Features. Drawing or push/pulling on the body afterwards re-runs the round. If an edit removes the round's edges, it is marked broken. **Edit > Bake Features** makes it plain geometry. Saved in the file.
- **Effects** in the menu bar is now the kit's switch.
- **Offset** lights up the edge it snaps to.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
