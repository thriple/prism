**New in this build:**
- **File > Import of STEP files** (`.step`, `.stp`, or drop one on the window): solids and surfaces from other CAD programs come in exact (flat, round and freeform faces stay what they are). Each part is a group named after it, a part used more than once is a component, and colours become materials. The window shows what is in the file and its size before you import, and one Undo takes it all away. Anything Prism cannot take is counted, with why.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
