# Guide d'utilisation de l'API Modbus Slave

Ce document décrit comment utiliser l'API Modbus Slave d'Aoba depuis des applications Rust pour exposer des données vers des masters Modbus. Les cas d'utilisation typiques incluent les lignes de production industrielles, les systèmes de contrôle de procédé et les bancs de test.

L'exemple de référence est la crate `examples/api_slave`.

## 1. Présentation générale

Aoba fournit une API côté esclave qui reprend le style de l'API master, basée sur un modèle Builder + Hook. Elle est utile lorsque vous souhaitez :

- Transformer votre processus en esclave Modbus, exposant des données de bobines/registres aux masters externes ;
- Construire rapidement un appareil Modbus configurable pour des tests d'intégration ou de simulation ;
- Attacher une chaîne de middlewares (hooks) pour la journalisation, les statistiques, le contrôle d'accès et les alertes.

Le point d'entrée principal est toujours `_main::api::modbus::ModbusBuilder`, mais vous utilisez `new_slave` / `build_slave` :

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

---

## 2. Cycle de vie basique d'un esclave

Une version simplifiée de l'exemple esclave ressemble à ceci :

```rust
use anyhow::Result;
use std::sync::Arc;
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

struct ResponseLoggingHook;

impl ModbusHook for ResponseLoggingHook {
    fn on_before_request(&self, _port: &str) -> Result<()> {
        Ok(())
    }

    fn on_after_response(&self, port: &str, response: &ModbusResponse) -> Result<()> {
        log::info!(
            "sent response on {}: station={}, addr=0x{:04X}, values={:04X?}",
            port,
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(())
    }

    fn on_error(&self, _port: &str, error: &anyhow::Error) {
        log::warn!("error: {}", error);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 { &args[1] } else { "/tmp/vcom2" };

    let hook: Arc<dyn ModbusHook> = Arc::new(ResponseLoggingHook);

    let _slave = ModbusBuilder::new_slave(1)
        .with_port(port)
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(hook)
        .build_slave()?;

    // Keep the slave running and listening for master requests
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

### Paramètres de configuration principaux

- **Port** : même format que pour le master (`/dev/ttyUSB*`, `/dev/ttyS*`, `/tmp/vcom2`, etc.) ;
- **Station ID** : doit correspondre à l'identifiant de station que les masters utiliseront pour communiquer avec cet esclave ;
- **Mode de registre et plage d'adresses** : définit quelle partie de l'espace d'adressage Modbus cet esclave expose ;
- **Timeout** : utilisé en interne pour contrôler les délais d'E/S et de traitement (généralement aligné sur les paramètres du master).

---

## 3. Chaîne de middlewares (hooks)

Côté esclave, vous pouvez également enregistrer plusieurs hooks pour former une chaîne de middlewares. Responsabilités typiques :

- Valider ou inspecter les requêtes entrantes avant leur traitement ;
- Journaliser et post-traiter les réponses après leur envoi ;
- Déclencher des alertes ou mettre à jour les statistiques en cas d'erreur.

La crate `examples/api_slave` illustre trois hooks chaînés :

- `RequestMonitorHook` : surveille les requêtes et journalise/signale les erreurs ;
- `ResponseLoggingHook` : journalise chaque réponse avec l'adresse du registre et les valeurs ;
- `StatisticsHook` : suit le nombre de requêtes.

Ce modèle vous permet de sortir les préoccupations transversales (journalisation, métriques, contrôle d'accès, limitation de débit, etc.) de votre logique métier principale et de les attacher de manière déclarative à une instance esclave.

---

## 4. Cas d'utilisation typiques

Les cas d'utilisation courants de l'API esclave dans les environnements industriels et les configurations de test incluent :

1. **Simulateur d'appareil logiciel**
   - Lorsque les appareils réels ne sont pas encore disponibles, simulez un appareil Modbus en Rust ;
   - Mettez à jour périodiquement les valeurs des registres internes selon vos scénarios de test ;
   - Pilotez des tests d'intégration de bout en bout dans l'IC.
2. **Couche d'adaptation de protocole**
   - Vos appareils réels peuvent utiliser CAN, TCP propriétaire ou un autre bus de terrain, tandis que les systèmes en amont attendent Modbus ;
   - Utilisez l'API esclave pour mapper ces signaux dans un espace de registres/bobines Modbus et présenter une interface Modbus unifiée.
3. **Passerelle edge exposant des données traitées**
   - Collectez et normalisez des données provenant de multiples sources au sein de votre processus ou passerelle ;
   - Utilisez l'API esclave pour exposer les données traitées/agrégées aux systèmes SCADA existants ou à des systèmes tiers via Modbus.

---

## 5. Utilisation conjointe des APIs master et esclave

Les APIs master et esclave partageant le même modèle Builder + Hook, vous pouvez facilement les combiner au sein d'un seul processus :

1. Utiliser l'API master pour interroger plusieurs appareils en amont et construire un modèle de données interne unifié ;
2. Utiliser l'API esclave pour mapper ce modèle de données vers un espace de registres Modbus ;
3. Laisser les systèmes externes traiter votre processus comme un appareil Modbus standard.

Ce modèle est utile pour construire des passerelles de protocole, des nœuds d'agrégation ou des harnais de test.

---

## 6. Exécution de l'exemple esclave

Depuis la racine du dépôt :

```bash
cargo run --package api_slave -- /tmp/vcom2
```

Vous pouvez le coupler avec l'exemple master ou l'interface CLI/TUI d'Aoba pour les tests :

- Démarrez l'exemple esclave en écoute sur `/tmp/vcom2` ;
- Puis utilisez l'exemple master ou l'interface CLI/TUI pour interroger ce port et vérifier le comportement en lecture/écriture.

---

## 7. Documentation connexe

- API côté master : `docs/en/API_MODBUS_MASTER.md` ;
- Utilisation Modbus en ligne de commande : `docs/en/CLI_MODBUS.md` ;
- Capacités de source de données / export (HTTP, MQTT, IPC, etc.) : consultez les documents `DATA_SOURCE_*.md` dans ce répertoire ;
- D'autres exemples de bout en bout se trouvent dans le répertoire `examples`.
