#!/usr/bin/env bash
# CI-only compatibility canary for the current ESP Rust generation.
#
# This mutates Cargo.toml files only in the ephemeral CI checkout. The committed
# manifests and targets/esp32/Cargo.lock remain the reproducible reference.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

mapfile -t manifests < <(find . -name Cargo.toml -not -path './target/*' -print)

replace_all() {
  local from="$1"
  local to="$2"
  for file in "${manifests[@]}"; do
    sed -i "s/${from}/${to}/g" "$file"
  done
}

# esp-hal 1.x is the supported API line we want to continuously probe.
replace_all 'esp-hal = { version = "=1\.1\.2"' 'esp-hal = { version = "1"'
replace_all 'esp-hal = { version = "~1\.1\.0"' 'esp-hal = { version = "1"'

# Move the companion ESP crates to the generation paired with esp-hal 1.2+.
# These are compatibility-probe constraints, not release pins.
replace_all 'esp-radio = { version = "0\.18\.0"' 'esp-radio = { version = "1.0.0-beta.1"'
replace_all 'esp-storage = { version = "0\.9\.0"' 'esp-storage = { version = "0.10"'
replace_all 'esp-rom-sys = { version = "=0\.1\.4"' 'esp-rom-sys = { version = "0.1.5"'
replace_all 'esp-bootloader-esp-idf = { version = "=0\.5\.0"' 'esp-bootloader-esp-idf = { version = "0.6"'
replace_all 'esp-alloc = { version = "0\.10\.0"' 'esp-alloc = { version = "0.11"'
replace_all 'esp-println = { version = "0\.17\.0"' 'esp-println = { version = "0.18"'
replace_all 'esp-metadata-generated = "0\.4\.0"' 'esp-metadata-generated = "0.5"'

echo "CI canary dependency overrides:"
grep -R --include Cargo.toml -E 'esp-(hal|radio|storage|rom-sys|bootloader-esp-idf|alloc|println|metadata-generated).*version' . | sort
