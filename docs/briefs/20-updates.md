# Updates (owner request, 2026-11-09)

**Goal.** Help > Check for Updates brings the newest release build down from GitHub Releases and puts it in place of the running one, after asking. The new build runs from the next start (PLAN 3.1, decision 8).

**Read first:** PLAN 3.1 decision 8; `.github/workflows/release.yml`; `packaging/macos/make-app.sh`; `crates/prism-app/src/overlay.rs` (`BUILD`).

## Size
S: one part.

## Requirements
- **What is published:** the release workflow adds `update.json` to each release: the build number, the commit, and the SHA-256 of each zip. Prism reads it from the fixed "latest" link, so no GitHub API (and no rate limit) is involved.
- **Check:** compare the build number in `update.json` with the running build's (`PRISM_BUILD`, "build N"). Builds made by hand ("local") are not updated: the window says so.
- **Ask, then install:** a window shows "Build M is available (this is build N)" with **Download and Install**. While downloading it shows progress. The zip's SHA-256 must match `update.json`, or nothing is changed.
- **Swap:**
  - **Windows:** the running `Prism.exe` cannot be overwritten, but it can be renamed: it becomes `Prism.old.exe`, and the new one takes its name. `Prism.old.exe` is deleted on the next start.
  - **macOS:** the new `Prism.app` is unpacked beside the old one (keeping file permissions), the old bundle is moved aside and the new one put in its place; the leftovers are deleted. A Prism run from App Translocation (not moved out of Downloads) is told to move itself first.
  - If a step fails, the old build is put back and the window says why (for example, a folder Prism may not write to).
- **After:** "Installed build M. Restart Prism to use it."
- **Network:** a plain HTTPS download with the system's TLS and certificates (Schannel on Windows, Security framework on macOS), so it behaves like a browser download behind firewalls. No admin rights needed when Prism is in a folder the user owns. The download runs on a thread; the UI never waits on it.
- **Web build:** the window says the web version updates on every push (reload the page).
- **One action:** `help.update` in the Help menu, through the registry.

## Out of scope
- Checking automatically at start, or on a schedule.
- A "Restart now" button (it needs the quit path that asks to save documents).
- Signature checks (decision 5: once builds are signed).
- Going back to an older build from the app (older builds stay on the Releases page).

## Tests
- `update.json` parses; the build number in "build N" parses, and "local" gives none.
- A newer build is offered, the same or an older one is not.
- The checksum check accepts the right bytes and refuses changed ones.
- Unpacking a zip keeps folders and (on Unix) the executable bit, and refuses entries that point outside the folder.
- The swap: the new file or folder takes the old one's place and the old one is kept aside; if putting the new one in place fails, the old one is back where it was. Leftovers are cleaned up.
- These run on Windows and macOS too (CI's native jobs).
- The action is in the Help menu and built.

## Acceptance
- [ ] Check for Updates finds a newer build, downloads it, checks it, and the next start runs it (owner, on Windows).
- [x] PLAN and NOTES are updated.
