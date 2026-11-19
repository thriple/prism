**New in this build: edit a live cut's cutter, and a booleans fix.**
- **Inspector > Features:** a live Union, Subtract or Intersect now shows its cutter's position (X, Y, Z) and size (width, depth, height). Type a value and the cut runs again; Undo puts it back.
- **Fix:** a union, subtract or intersect of two boxes that overlap on faces lying in the same plane no longer gives a broken result when the solids are picked in the other order.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
