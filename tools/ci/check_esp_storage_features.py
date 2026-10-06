#!/usr/bin/env python3
"""Check each runtime flash consumer's resolved S3 graph, without feature leakage."""
import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONSUMERS = (
    ("iobewi-esp-flash", "esp32s3"),
    ("iobewi-esp-partitions", "esp32s3"),
    ("iobewi-esp-config-space", "esp32s3"),
    ("iobewi-esp-ota", "esp32s3"),
    ("iobewi-esp-ota", "esp32s3,shared-flash"),
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--locked", action="store_true")
    parser.add_argument("--toolchain", default="esp", help="Cargo toolchain (tree only)")
    args = parser.parse_args()
    for package, features in CONSUMERS:
        command = [
            "cargo", f"+{args.toolchain}", "tree", "--manifest-path",
            str(ROOT / "targets/esp32/Cargo.toml"),
            "--target", "xtensa-esp32s3-none-elf", "-p", package,
            "--no-default-features", "--features", features,
            "--edges", "normal,build", "--prefix", "none",
            "--format", "{p}|{f}",
        ]
        if args.locked:
            command.append("--locked")
        result = subprocess.run(command, text=True, capture_output=True)
        if result.returncode:
            sys.stderr.write(result.stderr)
            return result.returncode
        storage = [line for line in result.stdout.splitlines()
                   if line.startswith("esp-storage v")]
        if not storage:
            print(f"FAIL {package} [{features}]: esp-storage absent", file=sys.stderr)
            return 1
        for line in storage:
            enabled = line.split("|", 1)[1].removesuffix(" (*)").split(",")
            print(f"{package} [{features}]: {line}")
            if "critical-section" not in enabled:
                print("FAIL: esp-storage must enable critical-section", file=sys.stderr)
                return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
