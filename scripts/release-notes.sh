#!/bin/sh
# Prints the release notes for a version: its section of CHANGELOG.md, then how to install.
#
#   sh scripts/release-notes.sh 0.2.0
set -eu
cd "$(dirname "$0")/.."
V="$1"
SECTION=$(awk -v v="$V" '
  /^## / { if (found) exit; if (index($0, "## " v " ") == 1 || $0 == "## " v) { found = 1; next } }
  found' CHANGELOG.md)
[ -n "$SECTION" ] || { echo "CHANGELOG.md has no section for $V" >&2; exit 1; }
printf '%s\n' "$SECTION"
cat <<'NOTES'

## Install

1. Download **MingDesk-*-universal.dmg** below. One download for Apple silicon and Intel Macs, macOS 10.14 Mojave
   or later.
2. Open it and drag **MingDesk** onto **Applications**.
3. Open MingDesk from Applications. macOS will say it can't check MingDesk for malicious software: MingDesk is
   signed with Four Ounces' own certificate, not through Apple's paid developer programme (yet).
   - **macOS 15 Sequoia and later:** click **Done**, then open **System Settings → Privacy & Security**, scroll to
     *"MingDesk" was blocked*, click **Open Anyway** and confirm with your password.
   - **macOS 14 and earlier (back to Mojave):** Control-click MingDesk in Applications, choose **Open**, then
     **Open** again.
4. When MingDesk asks, allow it under **Accessibility**, so it can see and focus your windows. Nothing leaves
   your Mac.

Every release is signed with the same certificate, so the Accessibility permission carries over when you
update.
NOTES
