# IOBEWI firmware update (`firmware/update`)

`iobewi-ota` (package name kept for now; the path is the taxonomy) is the portable `no_std` OTA service. It owns resumable writes,
streaming SHA-256 verification, durable firmware identity (`OTM1`), staged
transactions, validation decisions and reconciliation after a restart.
The optional `runtime` feature owns the live upload session, boot
reconciliation, confirmation gate and deadline. The HTTP routes are
in `iobewi-ota-http` (`net/http/ota`).

`OTM1` keeps the on-device bytes and field limits already in use; existing
devices must still be able to read their staged and active image metadata.
The OTA HTTP responses, schemas and `/prepare`, `/write`, `/activate` routes
belong to `iobewi-ota-http`. The application mounts that router under its own
API prefix (for example `/v1alpha1/ota`) and supplies authorization, flash,
storage and a deferred reboot port.

The EWBT `otadata` codec is `iobewi-firmware-boot`, the ESP image validator
`iobewi-firmware-image`, the slot model `iobewi-firmware-slots`; the ESP
adapter (partition lookup, flash execution) and bootloader are in the
provisional `iobewi-esp/` workspace.
