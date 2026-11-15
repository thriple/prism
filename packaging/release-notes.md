**New in this build: Validate and Heal.**
- **Solid > Validate and Heal** opens the Solids window: each solid in the open group says Valid with its volume, or Not a solid with the reasons (holes, faces pointing inward, faces crossing, points almost touching).
- Click a solid's name to select and zoom to it.
- **Heal** fixes what it safely can in one undo step: welds points that almost touch, fills holes whose edges lie flat, turns faces to face out, and removes faces left inside and stray edges. It says what it did.
- Fixed: Model Health no longer lists every cone, sphere and torus face as "Degenerate".

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
