#!/bin/sh
# Wraps MingDesk.app in a compressed .dmg with an Applications shortcut to drag it onto.
#
#   sh scripts/make-dmg.sh <path/to/MingDesk.app> <out.dmg>
#
# Uses hdiutil directly: Tauri's own DMG step scripts Finder to lay out the window, which opens Finder windows
# on the machine building it.
set -eu
APP="$1"
OUT="$2"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"
hdiutil create -volname "MingDesk" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$OUT" >/dev/null
if security find-certificate -c "MingDesk Dev" >/dev/null 2>&1; then
  codesign --force --sign "MingDesk Dev" "$OUT"
fi
echo "Made $OUT"
