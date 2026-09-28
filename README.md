# iobewi-esp

Implémentation ESP des contrats du framework portable [IOBEWI](https://github.com/iobewi/iobewi).
Ce dépôt est l'unique source des composants liés au matériel ESP. L'ancien
workspace `espbewi` a été déplacé ici ; les noms des crates sont désormais
préfixés `iobewi-esp-`.

Chaque composant a son propre `Cargo.toml` :

| Répertoire | Crate | Rôle |
| --- | --- | --- |
| `hardware/` | `iobewi-esp-{flash,nvs,partitions,platform,boot,watchdog}` | Accès au matériel |
| `adapters/` | `iobewi-esp-{ota,config-space}` | Adaptation des contrats FiBeWI et CSM |
| `services/wifi`, `services/tls` | `iobewi-esp-{wifi,tls}` | Réseau et TLS ESP |
| `services/http` | `iobewi-esp-http` | Listener TCP/HTTP sans TLS ; exposition explicite par l’application |
| `services/https` | `iobewi-esp-https` | Handshake TLS ESP et entrée HTTPS vers `iobewi-https` |
| `bootloader/esp/` | `iobewi-esp-bootloader` | Binaire indépendant, avec son propre workspace et lockfile |

Le service HTTP accepte TCP et transmet la connexion au dispatcher portable
`iobewi-http`. Le service HTTPS utilise ce même transport TCP, charge l’identité
serveur via une capacité injectée et termine le handshake MbedTLS avant de
transmettre le socket via `iobewi-https`. L’agent utilise uniquement HTTPS ;
sans identité valide, il n’ouvre aucun port HTTP de repli.

Le code de l'ancien dépôt reste accessible pour l'historique. Les projets
consommateurs doivent désormais pointer vers ce dépôt et utiliser les nouveaux
noms de crates ; il n'existe plus de dépendance de cet adaptateur à `espbewi`.

**État :** intégration et compilation CI ; validation sur carte ESP à faire.
