#!/usr/bin/env bash
# Build the provisioning proof for the ESP32-S3. `locked` (default) uses the committed
# lockfile; `latest` first re-resolves this independent workspace.
set -eu
dir="$(cd "$(dirname "$0")" && pwd)"
case "${1:-locked}" in
  locked) ;;
  latest) cargo +esp update --manifest-path "$dir/Cargo.toml" ;;
  *) echo 'expected locked or latest' >&2; exit 2 ;;
esac
cargo +esp build --manifest-path "$dir/Cargo.toml" --release --locked \
  --target xtensa-esp32s3-none-elf -Z build-std=core,alloc
