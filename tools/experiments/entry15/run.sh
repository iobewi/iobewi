#!/usr/bin/env bash
set -eu
entry15_dir="$(cd "$(dirname "$0")" && pwd)"
entry15_mode="${1:-locked}"
case "$entry15_mode" in
  locked) ;;
  latest) cargo +esp update --manifest-path "$entry15_dir/Cargo.toml" ;;
  *) echo 'expected locked or latest' >&2; exit 2 ;;
esac
cargo +esp build -p entry15-product-proof --release --locked \
  --manifest-path "$entry15_dir/Cargo.toml" --target xtensa-esp32s3-none-elf \
  -Z build-std=core,alloc -j 2
python3 "$entry15_dir/inspect_elf.py" \
  "$entry15_dir/target/xtensa-esp32s3-none-elf/release/entry15-product-proof"
entry15_negative_log="$(mktemp)"
trap 'rm -f "$entry15_negative_log"' EXIT
if cargo +esp check -p entry15-product-proof --release --locked --features async-main \
  --manifest-path "$entry15_dir/Cargo.toml" --target xtensa-esp32s3-none-elf \
  -Z build-std=core,alloc -j 2 > "$entry15_negative_log" 2>&1; then
  echo 'async-main unexpectedly succeeded: re-evaluate the experiment' >&2
  exit 1
fi
python3 - "$entry15_negative_log" <<'PY'
from pathlib import Path
import sys
text = Path(sys.argv[1]).read_text()
assert 'cannot find `embassy_executor` in the crate root' in text, text
assert 'cannot find `esp_rtos` in the crate root' in text, text
print('async-main failed for the expected absolute crate paths')
PY
