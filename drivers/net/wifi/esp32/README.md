# iobewi-esp-wifi

Reusable `no_std` ESP Wi-Fi station transport built on `esp-radio` and
`embassy-net`.

It owns radio/network mechanics only:

- lazy station initialization;
- scanning and strongest-BSSID selection per SSID;
- association;
- DHCP;
- Embassy network runner;
- access to the configured IP stack.

It deliberately does **not** own credential persistence, NVS layout,
provisioning protocols, TLS, HTTP, heartbeat/log services, OTA policy or
application supervision.

The caller supplies `StackResources<N>`, so socket-set sizing remains an
application decision rather than being hidden inside the transport crate.

## Features

- `esp32c3`
- `esp32s3`

No chip is selected by default.

## Status

Pre-stable. Extracted from the hardware-tested Wi-Fi path in
`embewi-agent-esp`. Integration and hardware gates are required before the
API is considered stable.

## License

MIT.
