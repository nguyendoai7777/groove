#!/usr/bin/env bash
#
# Rebuilds src-tauri/icons/icon.icns with the padding macOS expects.
#
# Every other platform wants the artwork to fill its canvas, and `tauri icon`
# generates it that way for all of them. macOS is the exception: the Dock draws
# icons at their full size and relies on the art itself to carry the margin, so
# a full-bleed icon sits noticeably larger than every neighbour beside it.
#
# Apple's icon grid puts the shape inside 824x824 of a 1024x1024 canvas, with
# the remaining ~100px on each side left transparent. That is what this applies.
#
# Only the .icns is touched. icon.ico and the Square*.png tiles stay full-bleed,
# which is correct for Windows and Linux.
#
# Re-run this after `tauri icon` — that command overwrites icon.icns with an
# unpadded version and would undo this.
#
# Usage:  ./scripts/make-macos-icon.sh [source.png]
#         ICON_SHAPE_SIZE=860 ./scripts/make-macos-icon.sh   # nudge the inset
#
# A circle reads slightly smaller than a squircle of the same width, so bumping
# the size to ~850 is a reasonable optical correction for a round mark.

set -euo pipefail

cd "$(dirname "$0")/.."

SOURCE="${1:-src-tauri/icons/icon.png}"
CANVAS=1024
SHAPE="${ICON_SHAPE_SIZE:-824}"
OUT="src-tauri/icons/icon.icns"

[ -f "$SOURCE" ] || { echo "no such source image: $SOURCE" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
ICONSET="$WORK/icon.iconset"
mkdir -p "$ICONSET"

# Shrink the artwork, then pad back out to the full canvas. sips fills the new
# margin with fully transparent pixels, which is what keeps the Dock from
# drawing a box around the icon.
sips -s format png -Z "$SHAPE" "$SOURCE" --out "$WORK/shape.png" >/dev/null
sips --padToHeightWidth "$CANVAS" "$CANVAS" "$WORK/shape.png" --out "$WORK/master.png" >/dev/null

# Every representation .icns expects. Each is scaled from the padded master so
# the margin stays proportional at all sizes.
for entry in \
  "16 icon_16x16" \
  "32 icon_16x16@2x" \
  "32 icon_32x32" \
  "64 icon_32x32@2x" \
  "128 icon_128x128" \
  "256 icon_128x128@2x" \
  "256 icon_256x256" \
  "512 icon_256x256@2x" \
  "512 icon_512x512" \
  "1024 icon_512x512@2x"
do
  set -- $entry
  sips -s format png -Z "$1" "$WORK/master.png" --out "$ICONSET/$2.png" >/dev/null
done

iconutil -c icns "$ICONSET" -o "$OUT"

echo "wrote $OUT — artwork ${SHAPE}px inside a ${CANVAS}px canvas ($(( SHAPE * 100 / CANVAS ))%)"
