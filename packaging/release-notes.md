**New in this build: construction planes and the model's own axes.**
- **Construction planes (Tools > Construction Plane).** Click to place a plane to draw on: it lies square to the face or the ground under the pointer, or to an axis the arrow keys lock. Then the pointer lands on it and every drawing tool draws on it, so you can draw on a wall in the air. A face in front of it still wins. Edit > Construction Plane stops using it or deletes it; View > Construction Planes hides them.
- **The model's own axes (Tools > Axes, with nothing open).** Click the new origin, a point on the red axis, then a point that sets green. Everything moves so those become the world's axes, and the view goes with it. Undo puts it all back in one step.
- Also recent: text (Tools > Text, three fonts come with Prism), dimensions (Tools > Dimensions), section planes (Tools > Section Plane).

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
