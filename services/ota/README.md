# IOBEWI OTA

`iobewi-ota` is the portable `no_std` OTA service. It owns resumable writes,
streaming SHA-256 verification, durable firmware identity (`OTM1`), staged
transactions, validation decisions and reconciliation after a restart.

`OTM1` keeps the on-device bytes and field limits already in use; existing
devices must still be able to read their staged and active image metadata.
HTTP response formatting belongs to the application-facing transport.

The ESP EWBT `otadata` codec and ESP image validator live in
`iobewi-esp-ota-boot`, alongside the ESP adapter and bootloader in
[`iobewi-esp`](https://github.com/iobewi/iobewi-esp).
