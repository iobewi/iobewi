# IOBEWI OTA

`iobewi-ota` is the portable `no_std` OTA service. It owns resumable writes,
streaming SHA-256 verification, durable firmware identity (`OTM1`), staged
transactions, validation decisions and reconciliation after a restart.
The optional `runtime` feature owns the live upload session, boot
reconciliation, confirmation gate and deadline. The optional `http` feature
exposes the streaming OTA upload handler through a platform-supplied writer
and caller-supplied authorization policy.

`OTM1` keeps the on-device bytes and field limits already in use; existing
devices must still be able to read their staged and active image metadata.
The OTA HTTP responses, schemas and `/prepare`, `/write`, `/activate` routes
belong to this service. The application mounts that router under its own
API prefix (for example `/v1alpha1/ota`) and supplies authorization, flash,
storage and a deferred reboot port.

The ESP EWBT `otadata` codec and ESP image validator live in
`iobewi-esp-ota-boot`, alongside the ESP adapter and bootloader in
[`iobewi-esp`](https://github.com/iobewi/iobewi-esp).
