#!/usr/bin/env bash
# Builds every example image twice from a clean target directory and compares the bytes.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
A="$(mktemp -d)"; B="$(mktemp -d)"
rm -rf "$HERE/hello/target"; "$HERE/build.sh" "$A" >/dev/null
rm -rf "$HERE/hello/target"; "$HERE/build.sh" "$B" >/dev/null
fail=0
for f in "$A"/*.iwni; do
    n="$(basename "$f")"
    if cmp -s "$f" "$B/$n"; then echo "same      $n $(sha256sum "$f" | cut -c1-16)"; else echo "DIFFERENT $n"; fail=1; fi
done
exit $fail
