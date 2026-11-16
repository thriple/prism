**New in this build:**
- Push/Pull works on freeform (NURBS) faces: the face moves out (or in) by the distance everywhere, with sides joining it to where it was. A side that lies flat, as when you pull out a curved wall, is a flat face.

**Fixed in this build:**
- Solid > Validate is much quicker on freeform solids. Where freeform faces meet along very many curve pieces, the kernel's check would take minutes, so Validate checks those by Prism's own rules and says so.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
