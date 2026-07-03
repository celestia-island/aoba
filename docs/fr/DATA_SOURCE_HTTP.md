# Source de données personnalisée — HTTP

## Démarrage rapide — exécuter un petit récepteur CLI

Démarrez l'interface CLI de l'application dans un mode où elle agira comme récepteur (la CLI hébergera un point de terminaison HTTP et appliquera le JSON posté). Exemple (à exécuter depuis la racine du dépôt) :

```bash
# using cargo (recommended during development)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source http://8080

# or, if you built the binary:
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source http://8080
```

La commande ci-dessus démarre un serveur HTTP lié à `127.0.0.1:8080` et accepte les requêtes `POST` vers `/` (racine). Utilisez l'exemple `curl` ci-dessous pour poster des données.

## Présentation générale

Ce document décrit la source de données personnalisée HTTP utilisée par l'application. Il présente le format de requête attendu, les en-têtes courants et un exemple `curl` simple que vous pouvez utiliser pour valider rapidement l'intégration.

## Point de terminaison

- Méthode : `POST`
- URL : `http://<host>:<port>/` (exemple : `http://localhost:8080/`)
- Content-Type : `application/json`

## Format de requête

Le service accepte un corps JSON. Un exemple minimal de charge utile ressemble à ceci :

```json
{
  "source": "http",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "payload": {
    "type": "register_update",
    "registers": [
      {"address": 0, "value": "1234"},
      {"address": 1, "value": "abcd"}
    ]
  }
}
```

Remarques :

- Utilisez le format ISO 8601 pour `timestamp` lorsque cela est possible.
- Le contenu de `payload` est spécifique à l'application ; l'exemple ci-dessus montre une mise à jour typique de type registre.

## Exemple de test avec curl

Remplacez `<host>` et `<port>` par ceux de votre serveur en cours d'exécution. Cette commande `curl` envoie la charge utile JSON ci-dessus :

```bash
curl -v -X POST "http://localhost:8080/" \
  -H "Content-Type: application/json" \
  -d '{
    "source":"http",
    "timestamp":"2025-11-15T12:34:56Z",
    "port":"/tmp/vcom1",
    "payload":{
      "type":"register_update",
      "registers":[{"address":0,"value":"1234"}]
    }
  }'
```

## Comportement attendu

- HTTP `200 OK` (ou `202 Accepted`) pour les messages acceptés/mis en file d'attente.
- Si le serveur renvoie une erreur (4xx/5xx), inspectez le corps de la réponse pour plus de détails.

## Conseils et dépannage

- Assurez-vous que l'en-tête `Content-Type: application/json` est présent.
- Si votre serveur nécessite une authentification, ajoutez l'en-tête `Authorization` approprié (par ex. `Bearer <token>`).
- Pour les charges utiles volumineuses, envisagez de tester avec `--data-binary` et d'augmenter les timeouts du serveur.

Si vous avez besoin d'un exemple adapté à un schéma interne, collez un exemple JSON ici et les développeurs adapteront le gestionnaire de point de terminaison en conséquence.
Si vous avez besoin d'un exemple adapté à un schéma interne, collez un exemple JSON ici et les développeurs adapteront le gestionnaire de point de terminaison en conséquence.
