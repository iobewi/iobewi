#!/usr/bin/env bash
# Build the firmware. The first argument selects the lockfile policy (`locked`, default, or
# `latest`), the second the hardware feature (`esp32s3`, the only one so far).
set -eu
dir="$(cd "$(dirname "$0")" && pwd)"
case "${1:-locked}" in
  locked) ;;
  latest) cargo +esp update --manifest-path "$dir/Cargo.toml" ;;
  *) echo 'expected locked or latest' >&2; exit 2 ;;
esac
case "${2:-esp32s3}" in
  esp32s3) target=xtensa-esp32s3-none-elf ;;
  *) echo 'expected esp32s3' >&2; exit 2 ;;
esac
cargo +esp build --manifest-path "$dir/Cargo.toml" --release --locked \
  --features "${2:-esp32s3}" --target "$target" -Z build-std=core,alloc
