#!/bin/sh
# Makes a self-signed code-signing certificate, "MingDesk Dev", in this Mac's login keychain (once per Mac).
#
#   sh scripts/dev-cert.sh
#
# Why: macOS ties the Accessibility permission to the app's signature. Unsigned builds get a new identity every
# time, so the permission would have to be granted again after each build. Signing every build with this one
# certificate keeps the identity stable. It costs nothing and needs no Apple Developer account; it only means
# something on the Macs you build or install on (others still see the "Open Anyway" step, PLAN.md §5).
set -eu

NAME="MingDesk Dev"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-certificate -c "$NAME" "$KEYCHAIN" >/dev/null 2>&1; then
  echo "\"$NAME\" is already in the login keychain."
  exit 0
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

cat >"$TMP/cert.cnf" <<EOF
[req]
distinguished_name = dn
prompt = no
x509_extensions = ext
[dn]
CN = $NAME
O = Four Ounces
[ext]
basicConstraints = critical, CA:false
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
EOF

# /usr/bin/openssl (LibreSSL) writes a PKCS#12 file the macOS keychain can import.
/usr/bin/openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -config "$TMP/cert.cnf" \
  -keyout "$TMP/key.pem" -out "$TMP/cert.pem" 2>/dev/null
PASS=$(/usr/bin/openssl rand -hex 16)
/usr/bin/openssl pkcs12 -export -inkey "$TMP/key.pem" -in "$TMP/cert.pem" -name "$NAME" \
  -out "$TMP/cert.p12" -passout "pass:$PASS"
security import "$TMP/cert.p12" -k "$KEYCHAIN" -P "$PASS" -T /usr/bin/codesign >/dev/null

echo "Added \"$NAME\" to the login keychain. npm run app now signs with it."
