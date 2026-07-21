# Source de données personnalisée — MQTT

## Démarrage rapide — exécuter un petit récepteur CLI

Démarrez l'interface CLI de l'application pour qu'elle s'abonne à un topic MQTT et agisse comme récepteur. Exemple (à exécuter depuis la racine du dépôt) :

```bash
# using cargo (recommended during development)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source mqtt://localhost:1883/aoba/data/in

# or, if you built the binary:
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source mqtt://localhost:1883/aoba/data/in
```

L'URL `mqtt://.../<topic>` inclut le chemin du topic (par ex. `aoba/data/in`) et la CLI s'abonnera à ce topic.

## Présentation générale

Ce document décrit comment publier des messages vers la source de données personnalisée MQTT de l'application. Il couvre la configuration du broker/de la connexion, les noms de topics recommandés et un exemple de charge utile `mosquitto_pub` pour effectuer une liaison descendante de données.

## Broker / connexion

- Hôte : `mqtt.example.com` ou `localhost`
- Port : `1883` (en clair) ou `8883` (TLS)
- Nom d'utilisateur/mot de passe : optionnel — si votre broker nécessite une authentification, fournissez-les dans la configuration du client
- TLS : si vous utilisez `8883`, fournissez le certificat CA et le certificat/clé client si nécessaire

## Topics recommandés

- Entrant (vers l'application) : `aoba/data/in` — l'application s'abonne ici pour recevoir les données amont ou les commandes
- Liaison descendante (vers l'appareil/vcom) : `aoba/data/out/<port>` — l'application publie les messages de liaison descendante traités ciblant un port spécifique (par ex. `aoba/data/out/tmp_vcom1`)

## Format de charge utile

L'application attend des charges utiles JSON. Le schéma exact est flexible, mais l'exemple suivant représente un format pratique pour les mises à jour de statut et les commandes de liaison descendante :

```json
{
  "source": "mqtt",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": {
    "command": "write_register",
    "registers": [{"address":0, "value": "1234"}]
  }
}
```

## Exemple : publier une liaison descendante avec mosquitto_pub

Cet exemple publie une liaison descendante vers le topic entrant que l'application traitera, puis effectuera l'écriture physique vers le `port` configuré.

```bash
mosquitto_pub -h localhost -p 1883 -t "aoba/data/in" -u "user" -P "pass" -m '{
  "source":"mqtt",
  "timestamp":"2025-11-15T12:34:56Z",
  "port":"/tmp/vcom1",
  "type":"downlink",
  "body":{ "command":"write_register", "registers":[{"address":0,"value":"1234"}] }
}'
```

## Notes et conseils

- Utilisez des noms de topics prévisibles pour simplifier le filtrage et les permissions.
- Lorsque vous ciblez un chemin de port série physique (par ex. `/tmp/vcom1`), évitez les caractères pouvant poser problème lors de l'analyse du topic ; vous pouvez mapper les noms de ports vers des étiquettes sûres pour les topics dans la configuration.
- Si votre broker prend en charge les messages retenus (retained), soyez prudent : les messages de liaison descendante retenus peuvent être réappliqués lors d'une reconnexion.

Si vous souhaitez un exemple de configuration de broker ou un outil de test automatisé (par exemple, un petit script qui publie une séquence de messages descendants et attend la confirmation d'état du CLI/TUI), indiquez vos outils préférés et je peux l'ajouter.
