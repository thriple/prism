**New in this build: section planes, and the program's icon on Windows.**
- **Section planes (Tools > Section Plane).** Click a face to place a plane, slide it through the model, click to set it: everything on one side is cut away so you can look inside, and closed solids get a flat cap where they are cut. Edit > Section Plane flips it, turns its cutting off or on, or deletes it. View > Section Display and Section Planes hide the cut and the outlines. The model itself never changes, and the planes are saved in the file.
- **Windows:** Prism.exe and its window now carry the program's icon (File Explorer, the taskbar, the title bar). Windows can keep showing the old icon for a while; moving or renaming the file refreshes it.
- Also recent: Alt tapped with Ctrl or Cmd held no longer turns on New Loop, and New Loop shows a label at the cursor.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
