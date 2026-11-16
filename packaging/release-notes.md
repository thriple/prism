**Fixed in this build:**
- The Loft tool picks a ring's sides in any order, and a click on an edge that joins the side you are picking adds it to that side.
- Enter makes a ring as soon as it closes, even while you are still picking its last side.
- Rows shows live: the preview draws the faces the loft will be cut into, and so does the loft you just made while Rows can still change it.
- Smooth / Straight is greyed where it changes nothing (a ring, or two sections). It no longer remakes the loft, so Undo no longer takes the whole loft away.
- The Eraser no longer shows Move's "a face with arcs would fold" message.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
