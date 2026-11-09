#!/bin/sh
# Builds MingDesk.app (release) and, on a Mac with the "MingDesk Dev" certificate (scripts/dev-cert.sh), signs it
# so the Accessibility permission survives rebuilds.
#
#   npm run app            build
#   npm run app -- --open  build, quit any running copy, and open the new one
set -eu
cd "$(dirname "$0")/.."

NAME="MingDesk Dev"
if [ "$(uname)" = Darwin ] && security find-certificate -c "$NAME" >/dev/null 2>&1; then
  export APPLE_SIGNING_IDENTITY="$NAME"
  echo "Signing with \"$NAME\"."
else
  echo "No \"$NAME\" certificate: building unsigned (run sh scripts/dev-cert.sh once to fix)."
fi

npx tauri build --bundles app

APP="src-tauri/target/release/bundle/macos/MingDesk.app"
if [ "$(uname)" = Darwin ]; then
  codesign -dv "$APP" 2>&1 | grep -E '^(Identifier|Authority|Signature)=' || true
  if [ "${1:-}" = "--open" ]; then
    osascript -e 'tell application "MingDesk" to quit' >/dev/null 2>&1 || true
    sleep 1
    open "$APP"
  fi
fi
echo "Built $APP"
