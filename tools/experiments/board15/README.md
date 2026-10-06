# Milestone 3 downstream Board proof

This standalone firmware links the actual S3 Board adapter through a renamed
entry facade. The product source uses only portable Board/config/device APIs.
Run `bash tools/experiments/board15/run.sh locked` after loading the ESP toolchain.
CI also runs `latest` within the supported manifest ranges.

The script links provisioning and mass-storage variants, checks the downstream
image descriptor and chip metadata, extracts future sizes, and rejects physical
console/backtrace dependencies in the normal/build graph. It does not flash a
board, execute NVS operations or implement persisted boot-mode product policy.

Observed on the committed dependency baseline and the installed ESP compiler:

| Evidence | Both variants |
| --- | --- |
| Product image name/version | board15-product-proof / 7.8.9 |
| Chip | ESP32-S3 |
| Generic product future | 328 bytes, alignment 8 |
| Entry task future | 376 bytes, alignment 8 |
| Configured heap | 98,304 bytes |
| Linker stack reservation | 202,828 bytes |

The reservation is a linker difference, not runtime free stack. These are fixture
measurements, not StreamBeWI measurements, and compiler/dependency updates can
change them. The inspector reports sizes rather than fixing them as golden values.
The Xtensa linker reports an RWX LOAD segment; the gate does not assert memory
isolation or alter upstream linker permissions.

Hardware qualification still needs named/missing NVS partitions, startup-fatal
UART behavior, actual button polarity, both USB modes, absence of JTAG writes
including panic, and RTC reset from provisioning into OTG. Real StreamBeWI's
joined services and heap/stack high-water measurements belong to its migration.
See [product fixture](product/README.md) and ADR-0015.
