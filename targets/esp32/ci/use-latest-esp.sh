#!/usr/bin/env bash
# CI-only compatibility canary for the supported ESP 1.2 generation.
#
# Manifests carry the supported ranges. This script deliberately does not rewrite
# them: the canary differs from the reproducible gate only by resolving without
# the committed Cargo.lock, so it continuously tests the newest versions allowed
# by those ranges (notably the newest esp-hal 1.2.x).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

echo "ESP compatibility canary: manifest ranges are authoritative"
grep -R --include Cargo.toml -E 'esp-(hal|radio|storage|rom-sys|bootloader-esp-idf|alloc|println|metadata-generated).*version' . | sort
