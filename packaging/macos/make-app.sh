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

# The app icon (owner's image, 2026-11-17): a full-bleed square with no
# border, so macOS 26 fills its rounded shape with it instead of putting it
# on a grey plate. Made from the owner's 2048 px image with Pillow (16 to
# 1024 px); Info.plist names it.
mkdir -p "$APP/Contents/Resources"
cp "$HERE/Prism.icns" "$APP/Contents/Resources/Prism.icns"

# Ad-hoc signature for the whole bundle. Without it, Apple Silicon Macs call a
# downloaded app "damaged" and refuse to open it. This is not a Developer ID
# signature, so macOS still warns the first time (see release-notes.md).
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

# ditto keeps the bundle structure, permissions and signature intact.
ditto -c -k --keepParent "$APP" "$OUT"
echo "Made $OUT"
