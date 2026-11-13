**New in this build: curves fixed up.**
- **A curve that crosses itself is cut at the crossing.** Draw a loop or a figure eight with the Curve tool: the crossing becomes a corner you can snap to, and a flat loop fills with a face (a figure eight makes two).
- **Midpoint snap on curves:** hover the middle of a curve to snap halfway along it.
- **Lofts no longer twist:** circles and other closed curves drawn from different start points now loft straight, in both the Loft command and the Loft Tool.
- **Fixed:** a line drawn across a curve that doubles back close to itself could miss a crossing and make overlapping faces.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
