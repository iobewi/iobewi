#!/usr/bin/env bash
set -eu
ap31_dir="$(cd "$(dirname "$0")" && pwd)"
case "${1:-locked}" in
  locked) ;;
  latest) cargo +esp update --manifest-path "$ap31_dir/Cargo.toml" ;;
  *) echo 'expected locked or latest' >&2; exit 2 ;;
esac
: "${AP31_STA_SSID:?set the test station SSID}"
: "${AP31_STA_PASSWORD:?set the test station WPA2 password}"
: "${AP31_AP_PASSWORD:?set a unique test AP WPA2 password}"
cargo +esp build --manifest-path "$ap31_dir/Cargo.toml" --release --locked \
    --target xtensa-esp32s3-none-elf -Z build-std=core,alloc
