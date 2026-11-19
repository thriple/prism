**New in this build: see and change a live cut.**
- **Live Union, Subtract and Intersect:** when you open the body and select the cut, a dashed outline shows the cutter. In Inspector > Features you can change it between Union, Subtract and Intersect, and type its position and size. Undo puts each change back.
- **Offset** goes as far as you ask, even past where its sides meet (the new lines cross and split the old ones), and no longer refuses to start again after a maximum offset.
- **Fix:** a union of boxes that overlap on shared planes is valid whichever you pick first.
- **Recordings** (`Prism_session_*.json`) start with a short note saying what the file is.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
