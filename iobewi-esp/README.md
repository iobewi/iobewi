# iobewi-esp

Implémentation ESP des contrats du framework portable [IOBEWI](https://github.com/iobewi/iobewi).
Ce dépôt est l'unique source des composants liés au matériel ESP. L'ancien
workspace `espbewi` a été déplacé ici ; les noms des crates sont désormais
préfixés `iobewi-esp-`.

Chaque composant a son propre `Cargo.toml` :

| Répertoire | Crate | Rôle |
| --- | --- | --- |
| `hardware/` | `iobewi-esp-{flash,nvs,partitions,platform,boot,ota-boot,watchdog}` | Accès au matériel, état EWBT et validation d'image ESP |
| `adapters/` | `iobewi-esp-{ota,config-space}` | Adaptation des services IOBEWI OTA et ConfigSpace |
| `services/tls` | `iobewi-esp-tls` | Transport TLS ESP : instance MbedTLS globale, dialer Embassy, flux de session (`net/io`) |
| `../crypto/mbedtls` | `iobewi-crypto-mbedtls` | Implémentation MbedTLS du contrat crypto (PEM/X.509, identité) |
| `../drivers/net/wifi/esp32` | `iobewi-esp-wifi` | Driver Wi-Fi station ESP (implémente `iobewi-wifi-core::WifiTransport`) |
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
`iobewi-esp-ota-boot` possède le format EWBT et la validation des images ESP.
L’adaptateur `iobewi-esp-ota` fournit aussi au service la persistance
ConfigSpace, le writer flash, la sélection des slots et les effets boot et
watchdog. Le service portable garde les décisions de reprise et de confirmation.

Le code de l'ancien dépôt reste accessible pour l'historique. Les projets
consommateurs doivent désormais pointer vers ce dépôt et utiliser les nouveaux
noms de crates ; il n'existe plus de dépendance de cet adaptateur à `espbewi`.

**État :** intégration et compilation CI ; validation sur carte ESP à faire.
