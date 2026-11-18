**New in this build: sections, dimensions and construction planes are easier to edit.**
- **Alt + click** picks a section plane, a dimension or a construction plane (plain clicks never touch them, so they cannot get in the way of your model). Alt + click on the picked one picks it up: it follows the pointer and the next click sets it. **Type a distance and press Enter** to place it exactly (a plane moves that far along its normal; a dimension line stands that far from what it measures). **Delete** removes the picked one. **Right-click** near one opens its menu.
- Construction planes can now be moved after you place them, and **Shift + click** in the Construction Plane tool makes a plane through three points.
- **Dimensions follow the model:** a dimension drawn to corners stays attached to them when the geometry or the group moves.
- **Text lands on the working plane** when the pointer is on it. Section planes no longer offer guides from faces they have cut away.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
