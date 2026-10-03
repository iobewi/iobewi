#!/usr/bin/env bash
# Reproducible build of the example native Workloads and their IWNI images.
#
#   examples/workloads/build.sh <out_dir>
#
# source -> `cargo build` (target xtensa-esp32s3-none-elf, intermediate ELF)
#        -> iobewi-workload-pack (ELF -> IWNI image, validated with the loader's own gate)
#        -> SHA-256 (the digest OTM2 verifies) printed next to each image.
#
# Determinism: fixed toolchain (rust-toolchain `esp`), no timestamps, no debug info, the
# source and registry paths remapped, one codegen unit. Two builds from the same source give
# the same bytes (checked by scripts/check-workload-repro.sh).
set -euo pipefail

OUT="${1:?Usage: $0 <out_dir>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"

LAYOUT="$ROOT/workload/sdk/ld"
SYSROOT="$(rustc +esp --print sysroot)"
FLAGS="-C link-arg=-nostartfiles -C link-arg=-nodefaultlibs -C link-arg=-Wl,--gc-sections -C link-arg=-Wl,--build-id=none"
FLAGS="$FLAGS -C link-arg=-Tworkload.ld -C relocation-model=static"
FLAGS="$FLAGS --remap-path-prefix=$HERE=/build/workloads --remap-path-prefix=$ROOT=/build/iobewi"
FLAGS="$FLAGS --remap-path-prefix=$SYSROOT/lib/rustlib/src/rust=/build/rust-src"
[ -n "${CARGO_HOME:-}" ] && FLAGS="$FLAGS --remap-path-prefix=$CARGO_HOME=/build/cargo"
export CARGO_TARGET_XTENSA_ESP32S3_NONE_ELF_RUSTFLAGS="$FLAGS"
export SOURCE_DATE_EPOCH=1767225600

PACK="cargo run -q --manifest-path $ROOT/Cargo.toml -p iobewi-workload-pack --"
ELF="$HERE/hello/target/xtensa-esp32s3-none-elf/release/iobewi-example-workload-hello"

build() { # name features [pack args...]
    local name="$1" features="$2"; shift 2
    ( cd "$HERE/hello" && cargo +esp build --release --locked ${features:+--features "$features"} 2>&1 | grep -E "^(error|warning: unused)" || true )
    $PACK "$ELF" -o "$OUT/$name.iwni" "$@" | sed "s|^|$name: |"
}

# Valid Workloads: A and B differ by identity string and period (observable in the logs).
build native-a ""
build native-b "variant-b"
# Valid image, bad behaviour (fault tests).
build native-a-ignore-stop "fault-ignore-stop"
build native-a-panic "fault-panic"
build native-a-null-jump "fault-null-jump"
# Hostile images: the gate must refuse them before any byte is loaded or executed.
build bad-magic ""         --force-magic XXXX
build bad-format ""        --force-format 9
build wrong-target ""      --force-target 2
build bad-abi ""           --force-abi 7
build bad-entry ""         --force-entry 0x42000000
build future-api ""        --api 9.9
echo "images in $OUT"
