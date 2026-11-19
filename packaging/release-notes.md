**New in this build: Union, Subtract and Intersect stay live.**
- **Solid > Union, Subtract and Intersect** of two groups now keep the first solid inside the result as a live feature (like fillets): Edit > Bake Features makes it plain, Remove takes it off, and pulling a wall the cut does not touch runs the cut again underneath. Trim and Split are unchanged. Files made with this build need this build or later.

These builds are not signed with a paid Apple or Microsoft certificate yet, so both systems warn the first time.

**macOS** (universal: Apple Silicon and Intel)
1. Unzip and move `Prism.app` to Applications.
2. Double-click it. macOS says it cannot check the app for malware. Click **Done**.
3. Open **System Settings > Privacy & Security**, scroll down to the line about Prism, and click **Open Anyway**. Confirm with your password.

After that it opens normally. Alternatively, run this once in Terminal: `xattr -dr com.apple.quarantine /Applications/Prism.app`

**Windows**
Unzip and run `Prism.exe`. If SmartScreen warns, choose **More info**, then **Run anyway**.
