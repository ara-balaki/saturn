#!/bin/sh
# Signs a locally built Saturn binary (`pnpm dev`, `pnpm demo`) as the app is signed: with the
# publisher's "Saturn Code Signing" certificate and the app's identifier. Every build then has the
# same identity as the installed app, and macOS's "Always Allow" for Saturn's Keychain items
# covers it too; an unsigned build looks new each time and asks again. Without the certificate it
# does nothing. Usage: sign-local.sh <binary>
set -eu
identity=$(security find-identity -p codesigning 2>/dev/null | awk '/"Saturn Code Signing"/ { print $2; exit }')
if [ -n "$identity" ]; then
  codesign --force --sign "$identity" --identifier local.saturn.workbench "$1" 2>/dev/null
fi
