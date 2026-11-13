**New in this build: the Loft Tool** (Draw > Surface > Loft Tool, and on the dock).
- **Pick sides by clicking:** a click on an edge takes the whole side (the edges that run on from it without a sharp turn; the threshold is in the Loft window). Shift adds or removes one edge. Enter (or Next Side) finishes a side; each side shows in its own colour with its number.
- **Fill a ring:** sides joined end to end are filled. Four sides make one face; three, five or more are filled from a centre point with four-sided faces that meet smoothly, so a rounded box corner (three arcs) fills cleanly.
- **Loft through sections:** sides that stand apart are lofted through, in the order you picked them.
- **Help along the way:** if the ring is almost closed, the status bar says so and the edges that would close it light up orange; a click takes them. The surface shows as a live preview before you make it.
- **Make it:** Enter or Loft. One undo removes it. Esc steps back one side. Right after, Smooth or Straight and Rows in the window change the result in place.

**Also:** curved faces with many details draw their outlines much faster.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
