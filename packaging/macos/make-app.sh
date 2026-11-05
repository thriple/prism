#!/usr/bin/env bash
# Build Prism.app from one or more compiled binaries, sign it ad hoc, check the
# signature, and zip it.
# Usage: make-app.sh VERSION OUT.zip BINARY [BINARY...]
# Several binaries (one per architecture) are joined into a universal app.
set -euo pipefail

VERSION="$1"; OUT="$2"; shift 2
HERE="$(cd "$(dirname "$0")" && pwd)"
WORK="$(mktemp -d)"
APP="$WORK/Prism.app"

mkdir -p "$APP/Contents/MacOS"
lipo -create -output "$APP/Contents/MacOS/Prism" "$@"
sed "s/__VERSION__/$VERSION/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"

# Ad-hoc signature for the whole bundle. Without it, Apple Silicon Macs call a
# downloaded app "damaged" and refuse to open it. This is not a Developer ID
# signature, so macOS still warns the first time (see release-notes.md).
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

# ditto keeps the bundle structure, permissions and signature intact.
ditto -c -k --keepParent "$APP" "$OUT"
echo "Made $OUT"
