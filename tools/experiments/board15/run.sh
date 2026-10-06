#!/usr/bin/env bash
set -eu
board15_dir="$(cd "$(dirname "$0")" && pwd)"
case "${1:-locked}" in
  locked) ;;
  latest) cargo +esp update --manifest-path "$board15_dir/Cargo.toml" ;;
  *) echo 'expected locked or latest' >&2; exit 2 ;;
esac
# Panic/console must not introduce a physical USB/JTAG sink through features.
board15_graph="$(cargo +esp tree --locked --manifest-path "$board15_dir/Cargo.toml" -p board15-product-proof --edges normal,build)"
if printf '%s\n' "$board15_graph" | grep -Eq 'esp-(println|backtrace)'; then
  echo 'unexpected physical console/panic dependency' >&2; exit 1
fi
for board15_features in '' 'mass-storage'; do
  cargo +esp build --manifest-path "$board15_dir/Cargo.toml" -p board15-product-proof \
    --release --locked --features "$board15_features" --target xtensa-esp32s3-none-elf \
    -Z build-std=core,alloc -j 2
  echo "USB branch: ${board15_features:-provisioning}"
  python3 "$board15_dir/inspect_elf.py" \
    "$board15_dir/target/xtensa-esp32s3-none-elf/release/board15-product-proof"
done
