# iobewi-esp

ESP platform capabilities for the [IOBEWI service framework](https://github.com/iobewi/iobewi).

The first adapter opens an HTTPS-only listener on ESP32-S3. It loads a server
identity through an injected capability, accepts TCP, completes the MbedTLS
handshake with a bounded deadline, and passes the authenticated socket to
IOBEWI's portable HTTP dispatcher. Without a valid identity, it opens no
plaintext listener. The existing [espbewi](https://github.com/iobewi/espbewi)
crate continues to own the chip-specific TLS implementation.

**Status:** initial adapter under integration; no hardware validation yet.
Flash, NVS, watchdog, and other platform capabilities will follow.
