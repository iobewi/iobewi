# iobewi-esp

Implémentation ESP des contrats du framework portable [IOBEWI](https://github.com/iobewi/iobewi).
Ce dépôt est l'unique source des composants liés au matériel ESP. L'ancien
workspace `espbewi` a été déplacé ici ; les noms des crates sont désormais
préfixés `iobewi-esp-`.

Chaque composant a son propre `Cargo.toml` :

| Répertoire | Crate | Rôle |
| --- | --- | --- |
| (déplacé) | `iobewi-esp-{flash,partitions,nvs,config-space,ota}` | `drivers/flash/{esp32,partitions-esp32}`, `fs/nvs/{esp32,config-esp32}`, `firmware/esp32` ; platform/boot -> `arch/esp32`, watchdog -> `drivers/watchdog/esp32` |
| `services/tls` | `iobewi-esp-tls` | Transport TLS ESP : instance MbedTLS globale, dialer Embassy, flux de session (`net/io`) |
| `../crypto/mbedtls` | `iobewi-crypto-mbedtls` | Implémentation MbedTLS du contrat crypto (PEM/X.509, identité) |
| `../drivers/net/wifi/esp32` | `iobewi-esp-wifi` | Driver Wi-Fi station ESP (implémente `iobewi-wifi-core::WifiTransport`) |
| (déplacé en S11) | `iobewi-esp-{device,indicator,runtime}` | `drivers/device/esp32`, `drivers/indicator/esp32`, `arch/esp32/runtime` ; contrats portables dans `drivers/{device,indicator,diagnostics}/core` ; `iobewi-esp-reboot` supprimé (orchestration dans l'agent, primitive dans `arch/esp32/reset`) |
| `services/http` | `iobewi-esp-http` | Listener TCP/HTTP sans TLS ; exposition explicite par l’application |
| `services/https` | `iobewi-esp-https` | Listener TLS ESP (`TlsListener`) ; HTTPS = serveur HTTP + ce listener |
| `bootloader/esp/` | `iobewi-esp-bootloader` | Binaire indépendant, avec son propre workspace et lockfile |

Le service HTTP accepte TCP et transmet la connexion au dispatcher portable
`iobewi-http-server`. Le service HTTPS utilise ce même transport TCP, charge l’identité
serveur via une capacité injectée et termine le handshake MbedTLS avant de
transmettre la connexion au serveur HTTP (`serve_forever_tls`). L’agent utilise uniquement HTTPS ;
sans identité valide, il n’ouvre aucun port HTTP de repli.
Le backend NVS `iobewi-esp-config-space` implémente le contrat portable
`iobewi-config-space` depuis le workspace IOBEWI.
Le service portable `iobewi-ota` possède les transactions et leurs métadonnées ;
`iobewi-firmware-boot` (EWBT) et `iobewi-firmware-image` (validation des images ESP) vivent dans l'arbre racine `firmware/`.
L’adaptateur `iobewi-esp-ota` fournit aussi au service la persistance
ConfigSpace, le writer flash, la sélection des slots et les effets boot et
watchdog. Le service portable garde les décisions de reprise et de confirmation.

Le code de l'ancien dépôt reste accessible pour l'historique. Les projets
consommateurs doivent désormais pointer vers ce dépôt et utiliser les nouveaux
noms de crates ; il n'existe plus de dépendance de cet adaptateur à `espbewi`.

**État :** intégration et compilation CI ; validation sur carte ESP à faire.
