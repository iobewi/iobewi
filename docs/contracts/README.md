# Canonical contracts

This index points to the canonical contract documents. It deliberately does not duplicate
their specifications.

| Contract | Canonical document | Owner |
| --- | --- | --- |
| Owned Board and boot I/O capabilities | [Board contract](../../board/README.md) | board; platform adapters implement it |
| Dual Agent/Workload update model | [dual-ota](../dual-ota.md) | firmware/model + update layers |
| OTM2 persistent Workload metadata | [otm2](../otm2.md) | workload/update |
| Native Workload image IWNI v1 | [native-workload-image](../native-workload-image.md) | workload/image |
| Workload ABI / Runtime API | [workload-runtime-api](../workload-runtime-api.md) | workload/abi + sdk |
| Workload OTA HTTP API | [workload-ota-http](../workload-ota-http.md) | workload/http |
| Workload supervisor/runtime lifecycle | [workload-supervisor](../workload-supervisor.md) | workload/update + native |
| ESP32 Workload partition layout | [esp32-workload-layout](../esp32-workload-layout.md) | platform storage |
| Virtual FAT16 volume | [virtual FAT16](../../fs/fat16/README.md) | fs/fat16 |
| Read-only USB MSC supported wire behaviour | [USB MSC](../../drivers/usb/msc/README.md) | drivers/usb/msc |
| Persisted log policy YAML v1 | [log/config](../../log/config/README.md) | log/config |
| Log WebSocket JSON origin metadata | [log/stream](../../log/stream/README.md) | log/stream |
