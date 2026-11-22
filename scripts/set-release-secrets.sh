#!/bin/sh
# Gives the Release workflow the "MingDesk Dev" certificate, so downloads are signed like local builds and the
# Accessibility permission carries over between versions (PLAN.md §5, decided 2026-11-22).
#
# 1. In Keychain Access, find "MingDesk Dev" under login → My Certificates, Control-click it, Export…, save as
#    a .p12 with a password.
# 2. gh auth switch -u kratner
# 3. sh scripts/set-release-secrets.sh ~/Desktop/MingDesk\ Dev.p12
#    It asks for the .p12's password, stores both as GitHub Actions secrets on kratner/mingdesk, then deletes the
#    .p12 file. The key stays in your keychain; GitHub keeps the secret encrypted and never shows it again.
set -eu
P12="$1"
[ -f "$P12" ] || { echo "No file at $P12" >&2; exit 1; }
printf 'Password for %s: ' "$P12"
stty -echo
read -r PASS
stty echo
echo
/usr/bin/openssl pkcs12 -in "$P12" -passin "pass:$PASS" -nokeys -noout 2>/dev/null \
  || { echo "That password doesn't open the file." >&2; exit 1; }
base64 -i "$P12" | gh secret set MINGDESK_DEV_P12 --repo kratner/mingdesk
printf '%s' "$PASS" | gh secret set MINGDESK_DEV_P12_PASSWORD --repo kratner/mingdesk
rm -P "$P12" 2>/dev/null || rm -f "$P12"
echo "Secrets set on kratner/mingdesk; $P12 deleted."
