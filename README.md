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
| `services/` | `iobewi-esp-{wifi,tls}` | Réseau et TLS ESP |
| `.` | `iobewi-esp` | Listener HTTPS reliant TLS ESP à `iobewi-http` |
| `bootloader/esp/` | `iobewi-esp-bootloader` | Binaire indépendant, avec son propre workspace et lockfile |

Le listener charge l'identité serveur via une capacité injectée, accepte TCP,
effectue le handshake MbedTLS puis transmet le socket au serveur portable
`iobewi-http`. Sans identité valide, aucun port HTTP non chiffré n'est ouvert.

Le code de l'ancien dépôt reste accessible pour l'historique. Les projets
consommateurs doivent désormais pointer vers ce dépôt et utiliser les nouveaux
noms de crates ; il n'existe plus de dépendance de cet adaptateur à `espbewi`.

**État :** intégration et compilation CI ; validation sur carte ESP à faire.
