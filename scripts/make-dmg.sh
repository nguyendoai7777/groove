#!/usr/bin/env bash
#
# Builds the macOS .dmg, replacing Tauri's own DMG bundler.
#
# Tauri ships a copy of create-dmg's bundle_dmg.sh, which drives Finder over
# AppleScript to lay the disk image window out. On macOS 26 that fails:
#
#   Finder got an error: Can't set statusbar visible of
#   container window of disk "dmg.XXXXXX" to false. (-10006)
#
# Finder no longer exposes that property, the script treats it as fatal, and the
# whole build fails after the .app was already produced. The script does have a
# --skip-jenkins flag that would skip the AppleScript entirely, but Tauri calls
# it with a fixed argument list and rewrites the file on every build, so there
# is nothing to patch.
#
# hdiutil alone is enough for a disk image people can install from — Finder is
# only ever needed for the cosmetic window layout. What is lost is the custom
# icon positions and window size; what is kept is the app, the drag-to-install
# Applications link, and compression.
#
# tauri.conf.json therefore lists "app" as the macOS bundle target rather than
# "dmg", and this runs afterwards. Called by the mac scripts in package.json.
#
# Usage:  ./scripts/make-dmg.sh [target-triple]
#
# The triple matters: a .app from an earlier build for a different target is
# still sitting in the tree, and packaging that too would drop a stale installer
# into ./release next to the fresh one. Passing the triple that was just built
# keeps it to the one that is current. With no argument, every .app found is
# packaged.

set -euo pipefail

cd "$(dirname "$0")/.."

PRODUCT_NAME=$(node -p "require('./src-tauri/tauri.conf.json').productName")
VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")

TRIPLE_FILTER="${1:-*}"

shopt -s nullglob
APPS=(src-tauri/target/$TRIPLE_FILTER/release/bundle/macos/*.app)
shopt -u nullglob

if [ ${#APPS[@]} -eq 0 ]; then
  echo "no .app found under src-tauri/target/$TRIPLE_FILTER/release/bundle/macos — build first" >&2
  exit 1
fi

for APP in "${APPS[@]}"; do
  # src-tauri/target/<triple>/release/bundle/macos/x.app -> <triple>
  TRIPLE=$(echo "$APP" | cut -d/ -f3)
  case "$TRIPLE" in
    aarch64-apple-darwin) ARCH=aarch64 ;;
    x86_64-apple-darwin) ARCH=x64 ;;
    universal-apple-darwin) ARCH=universal ;;
    *) ARCH="$TRIPLE" ;;
  esac

  OUT_DIR="src-tauri/target/$TRIPLE/release/bundle/dmg"
  OUT="$OUT_DIR/${PRODUCT_NAME}_${VERSION}_${ARCH}.dmg"
  mkdir -p "$OUT_DIR"

  # Staging directory, so the image holds exactly the app and the shortcut and
  # nothing else from the bundle directory.
  STAGE="$(mktemp -d)"
  trap 'rm -rf "$STAGE"' EXIT
  cp -R "$APP" "$STAGE/"
  ln -s /Applications "$STAGE/Applications"

  rm -f "$OUT"
  hdiutil create \
    -volname "$PRODUCT_NAME" \
    -srcfolder "$STAGE" \
    -fs HFS+ \
    -format UDZO \
    -ov \
    "$OUT" >/dev/null

  rm -rf "$STAGE"
  trap - EXIT

  echo "built $OUT"
done
