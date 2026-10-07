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
ap31_features=()
case "${2:-mode-transition}" in
  mode-transition) ;;
  dormant-apsta)
    : "${AP31_DORMANT_PASSWORD:?set an independent temporary dormant AP secret}"
    ap31_features+=(dormant-apsta)
    ;;
  *) echo 'expected mode-transition or dormant-apsta' >&2; exit 2 ;;
esac
target=xtensa-esp32s3-none-elf
case "${3:-official}" in
  official)
    cargo +esp build ${ap31_features[@]+--features "$(IFS=,; echo "${ap31_features[*]}")"} \
      --manifest-path "$ap31_dir/Cargo.toml" --release --locked \
      --target "$target" -Z build-std=core,alloc
    ;;
  counters)
    # Local diagnostic: a patched COPY of the pinned esp-radio exposes branch
    # counters. Built in a scratch copy of the experiment so the committed
    # Cargo.lock (registry source) is never rewritten. Kept outside the repository tree
    # so documentation tooling does not scan the copied crate. Never a production build.
    ap31_features+=(radio-counters)
    scratch="${AP31_SCRATCH:-${TMPDIR:-/tmp}/ap31-radio-counters}"
    rm -rf "$scratch"
    mkdir -p "$scratch/fw"
    cargo +esp fetch --manifest-path "$ap31_dir/Cargo.toml" --locked
    registry_src="$(ls -d "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/esp-radio-1.0.0-beta.1 | head -n 1)"
    cp -r "$registry_src" "$scratch/esp-radio"
    patch -p1 -d "$scratch/esp-radio" < "$ap31_dir/patches/esp-radio-1.0.0-beta.1-branch-counters.patch"
    cp -r "$ap31_dir/Cargo.toml" "$ap31_dir/Cargo.lock" "$ap31_dir/build.rs" "$ap31_dir/src" "$scratch/fw/"
    cargo +esp build --features "$(IFS=,; echo "${ap31_features[*]}")" \
      --manifest-path "$scratch/fw/Cargo.toml" --release \
      --config "patch.crates-io.esp-radio.path='$scratch/esp-radio'" \
      --target "$target" -Z build-std=core,alloc
    echo "counters build: $scratch/fw/target/$target/release/ap31-qualification (patched esp-radio; never publish)"
    ;;
  *) echo 'expected official or counters' >&2; exit 2 ;;
esac
