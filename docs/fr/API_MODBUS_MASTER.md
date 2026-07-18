# Guide d'utilisation de l'API Modbus Master

Ce document décrit comment utiliser l'API Modbus Master d'Aoba depuis des applications Rust dans des scénarios industriels typiques (surveillance de ligne de production, contrôle de procédé, surveillance environnementale, etc.), en s'appuyant sur la crate `examples/api_master` comme référence.

## 1. Présentation générale

Aoba expose une API Modbus master basée sur des traits, conçue pour être intégrée dans d'autres applications Rust ou des logiciels de contrôle matériel. Les cas d'utilisation typiques incluent :

- Le poll périodique d'esclaves Modbus (RTU sur ports série ou virtuels)
- La collecte de valeurs de bobines (coils) / registres dans votre propre logique de télémétrie ou de contrôle
- L'intégration avec des systèmes de journalisation / surveillance existants via des hooks

Le point d'entrée principal est le type `ModbusBuilder` du module `_main::api::modbus`.

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

> Remarque : dans les exemples, la racine de la crate est appelée `_main`. Dans votre propre projet, il s'agira généralement de la crate principale `aoba` ou du nom que vous lui donnez dans `Cargo.toml`.

---

## 2. Cycle de vie basique d'un master

Une boucle de poll minimale ressemble à ceci :

```rust
use anyhow::Result;
use std::time::Duration;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    let master = ModbusBuilder::new_master(1) // station id of the slave
        .with_port("/dev/ttyUSB0")          // or `/tmp/vcom1` etc.
        .with_register(RegisterMode::Holding, 0, 10)
        .with_timeout(1000)                  // milliseconds
        .build_master()?;

    loop {
        if let Some(resp) = master.recv_timeout(Duration::from_secs(1)) {
            println!("values = {:04X?}", resp.values);
        }
    }
}
```

### Paramètres importants

- **Port** : tout port série ou virtuel qu'Aoba peut ouvrir (véritable `/dev/ttyUSB*`, `/dev/ttyS*`, ou virtuel `/tmp/vcom*` créé par socat).
- **Station ID** : adresse de l'esclave Modbus (généralement 1–247).
- **Mode de registre** : parmi `RegisterMode::Coils`, `DiscreteInputs`, `Holding`, `Input`.
- **Adresse / longueur de registre** : adresse de départ et nombre d'éléments à lire, correspondant à la table d'adresses Modbus de votre appareil (par exemple, un API ou une passerelle de capteurs).
- **Timeout** : délai d'attente de la requête en millisecondes.

Le master exécute en interne une boucle de poll et alimente un canal ; votre code appelle simplement `recv_timeout` pour obtenir les nouvelles données.

---

## 3. Utilisation des hooks pour la journalisation et la surveillance

Pour les systèmes de production (lignes industrielles, équipements de procédé, capteurs sur site, etc.), vous souhaitez généralement :

- Journaliser chaque réponse réussie
- Suivre les erreurs et les dépassements de délai
- Éventuellement pousser les données vers un bus de messages ou une base de données

Le trait `ModbusHook` vous permet d'intégrer cette logique de manière centralisée.

```rust
use anyhow::Result;
use std::sync::Arc;
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

struct LoggingHook;

impl ModbusHook for LoggingHook {
    fn on_before_request(&self, port: &str) -> Result<()> {
        log::debug!("sending request on {}", port);
        Ok(())
    }

    fn on_after_response(&self, port: &str, resp: &ModbusResponse) -> Result<()> {
        log::info!(
            "resp {}: station={}, addr=0x{:04X}, values={:04X?}",
            port,
            resp.station_id,
            resp.register_address,
            resp.values,
        );
        Ok(())
    }

    fn on_error(&self, port: &str, err: &anyhow::Error) {
        log::warn!("modbus error on {}: {}", port, err);
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let master = ModbusBuilder::new_master(1)
        .with_port("/tmp/vcom1")
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(Arc::new(LoggingHook))
        .build_master()?;

    // now poll with recv_timeout as in the basic example
    # let _ = master;
    Ok(())
}
```

Vous pouvez enregistrer plusieurs hooks (par exemple, un pour la journalisation, un pour l'export de métriques).

---

## 4. Modèle d'intégration pour la surveillance industrielle / d'équipements

Pour les scénarios typiques de surveillance industrielle (lignes de production, unités de procédé, dispositifs de surveillance environnementale, etc.), un modèle courant est :

1. **Configurer les ports et les stations** via l'interface TUI ou CLI d'Aoba, ou les coder en dur dans votre application.
2. **Créer un master par port physique/virtuel** en utilisant `ModbusBuilder::new_master`.
3. **Lancer une tâche Tokio par master** qui :
   - appelle `recv_timeout` dans une boucle
   - analyse `ModbusResponse::values` en unités de génie (pression, température, état de vanne, etc.)
   - transmet les données traitées à votre backend de surveillance (MQTT, HTTP, base de données, etc.).
4. Utiliser `ModbusHook` pour centraliser la journalisation, la mesure de latence et le comptage des erreurs.

Aoba étant construit sur `tokio`, l'API master est conçue pour être utilisée dans un environnement d'exécution asynchrone mais expose un `recv_timeout` de style bloquant pour plus de commodité dans les tâches.

---

## 5. Gestion des erreurs et des timeouts

- `build_master()` renvoie une `anyhow::Error` si le port ne peut pas être ouvert ou si la configuration est invalide.
- `recv_timeout()` renvoie `None` en cas de timeout ; ce n'est pas une erreur en soi.
- Les erreurs au niveau du protocole (CRC, codes d'exception, erreurs d'E/S) sont signalées via `ModbusHook::on_error`.

Un modèle recommandé :

- Considérer les timeouts occasionnels comme normaux dans les environnements série instables.
- Utiliser un compteur glissant dans votre hook ; si les erreurs consécutives dépassent un seuil, déclencher une alarme.

---

## 6. Exécution de l'exemple

Depuis la racine du dépôt :

```bash
cargo run --package api_master -- /tmp/vcom1
```

Dans un banc de test de type production (tel qu'un banc de stockage d'hydrogène), vous procédez généralement ainsi :

- Utiliser l'interface CLI/TUI d'Aoba ou `examples/modbus_slave` pour simuler le côté esclave.
- Puis exécuter l'exemple `api_master` pour vérifier que votre câblage Modbus et votre logique applicative se comportent comme prévu.

---

## 7. Master en mode manuel (poll_once / opérations d'écriture)

Pour les scénarios nécessitant un contrôle fin du timing de poll (machines à états, stratégies adaptatives ou opérations d'écriture), utilisez `build_master_manual()` :

```rust
use anyhow::Result;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    let master = ModbusBuilder::new_master(1)
        .with_port("/dev/ttyUSB0")
        .with_baud_rate(9600)
        .with_timeout(5000)
        .build_master_manual()?;

    // Manual single-shot poll
    let response = master.poll_once(RegisterMode::Holding, 0x00, 10)?;
    println!("Values: {:?}", response.values);

    // Write a single holding register (fc 0x06)
    master.write_holding(0x00, 0x1234)?;

    // Write multiple holding registers (fc 0x10)
    master.write_registers(0x00, &[0x1234, 0x5678, 0x9ABC])?;

    // Write coils (fc 0x0F)
    master.write_coils(0x00, &[true, false, true, true])?;

    Ok(())
}
```

### Quand utiliser le mode manuel

| Scénario | Mode recommandé |
|----------|-----------------|
| Surveillance continue / collecte de données | `build_master()` (automatique) |
| Boucles de contrôle lecture-modification-écriture | `build_master_manual()` |
| Machines à états / poll piloté par événements | `build_master_manual()` |
| Poll adaptatif basé sur la latence de réponse | `build_master_manual()` |
| Diagnostics ou configuration ponctuels | `build_master_manual()` |

### Détails des opérations d'écriture

- **`write_holding(address, value)`** — écrit un seul registre de maintien en utilisant le code fonction 0x06. Idéal pour écrire des paramètres de configuration individuels.
- **`write_registers(address, values)`** — écrit plusieurs registres de maintien consécutifs en utilisant le code fonction 0x10. Idéal pour les écritures de paramètres par lot.
- **`write_coils(address, values)`** — écrit plusieurs bobines en utilisant le code fonction 0x0F. Inclut un échange automatique d'octets pour les écritures de 11 bobines (requis par certains matériels).
- Toutes les méthodes d'écriture bloquent jusqu'à ce que l'esclave accuse réception ou qu'une erreur se produise.

---

## 8. Prochaines étapes

- Pour les APIs côté esclave, voir `examples/api_slave`.
- Pour l'utilisation Modbus en ligne de commande, voir `docs/en/CLI_MODBUS.md`.
- Pour l'export de données via HTTP / MQTT / IPC, consultez les documentations `DATA_SOURCE_*.md` dans ce répertoire.
