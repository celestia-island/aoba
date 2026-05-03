# Fonctionnalités Modbus en ligne de commande (CLI)

Ce document décrit les nouvelles fonctionnalités CLI pour les opérations Modbus ajoutées au projet aoba.

## Fonctionnalités

### 1. Détection et listage des ports

#### Lister tous les ports

La commande `--list-ports` fournit désormais des informations plus détaillées lorsqu'elle est utilisée avec `--json` :

```bash
aoba --list-ports --json
```

La sortie inclut :

- `path` : chemin du port (par ex. COM1, /dev/ttyUSB0)
- `status` : « Free » (libre) ou « Occupied » (occupé)
- `guid` : GUID de l'appareil Windows (si disponible)
- `vid` : Identifiant vendeur USB (si disponible)
- `pid` : Identifiant produit USB (si disponible)
- `serial` : Numéro de série (si disponible)

Exemple de sortie :

```json
[
  {
    "path": "COM1",
    "status": "Free",
    "guid": "{...}",
    "vid": 1234,
    "pid": 5678
  }
]
```

#### Vérifier l'état d'occupation d'un port spécifique

La commande `--check-port` est utilisée pour détecter si un port spécifique est occupé. Elle est utile pour l'automatisation de scripts et la surveillance de l'état des ports :

```bash
aoba --check-port COM3
```

**Codes de sortie :**

- `0` - Le port est libre et disponible
- `1` - Le port est occupé par un autre programme

**Sortie en texte brut :**

```
Port COM3 is free
```

ou

```
Port COM3 is occupied
```

**Sortie au format JSON :**

```bash
aoba --check-port COM3 --json
```

Exemple de sortie :

```json
{"port":"COM3","occupied":false,"status":"Free"}
```

ou

```json
{"port":"COM3","occupied":true,"status":"Occupied"}
```

**Exemples d'utilisation :**

Utilisation dans des scripts shell :

```bash
# Bash example
if aoba --check-port /dev/ttyUSB0; then
    echo "Port is free, ready to use"
    # Perform your operations
else
    echo "Port is occupied, please close the program using this port"
    exit 1
fi
```

```powershell
# PowerShell example
cargo run --package aoba -- --check-port COM3
if ($LASTEXITCODE -eq 0) {
    Write-Host "Port is free"
} else {
    Write-Host "Port is occupied"
}
```

### 2. Modes d'écoute esclave

#### Mode temporaire

Écouter une seule requête Modbus, répondre, puis quitter :

```bash
aoba --slave-listen /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

Affiche une seule réponse JSON puis quitte.

#### Mode persistant

Écouter en continu les requêtes et afficher du JSONL :

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

Affiche une ligne JSON par requête traitée.

### 3. Modes d'émission master

- Mode temporaire, émettre les données une seule fois puis quitter :

```bash
aoba --master-provide /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

Lit une ligne de la source de données, l'envoie, puis quitte.

- Mode persistant, émettre les données en continu :

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

Lit les lignes de la source de données et les envoie en continu.

### Format de la source de données

Pour les modes master, le fichier source de données doit être au format JSONL :

```json
{"values": [10, 20, 30, 40, 50]}
{"values": [15, 25, 35, 45, 55]}
{"values": [20, 30, 40, 50, 60]}
```

Chaque ligne représente une mise à jour à envoyer à l'esclave.

#### Utilisation de fichiers comme source de données

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

#### Utilisation de tubes nommés Unix comme source de données

Les tubes nommés Unix (FIFO) peuvent être utilisés pour la diffusion de données en temps réel :

```bash
# Create named pipe
mkfifo /tmp/modbus_input

# Start master in one terminal
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_input \
  --baud-rate 9600

# Write data in another terminal
echo '{"values": [10, 20, 30, 40, 50]}' > /tmp/modbus_input
```

### Destinations de sortie

Pour les modes esclave, vous pouvez spécifier les destinations de sortie :

#### Sortie vers stdout (par défaut)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600
```

#### Sortie vers un fichier (mode ajout)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output file:/path/to/output.jsonl
```

#### Sortie vers un tube nommé Unix

```bash
# Create named pipe
mkfifo /tmp/modbus_output

# Start slave in one terminal
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output pipe:/tmp/modbus_output

# Read data in another terminal
cat /tmp/modbus_output
```

## Mode démon (fonctionnement persistant)

La CLI prend en charge un fonctionnement continu de type démon via les **modes persistants** :

- **Démon esclave** : Utilisez `--slave-listen-persist` pour une écoute et une réponse continues
- **Démon master** : Utilisez `--master-provide-persist` pour une émission continue de données

Ces modes s'exécutent indéfiniment jusqu'à interruption (Ctrl+C) et produisent du JSONL (un objet JSON par ligne) pour chaque opération. Ils sont idéaux pour :

- Les applications de surveillance longue durée
- Les systèmes de journalisation de données
- L'intégration avec d'autres outils via des tubes ou des fichiers
- La communication avec les sous-processus TUI (lorsqu'ils sont combinés avec `--ipc-channel`)

Exemple d'utilisation en mode démon :

```bash
# Run as slave daemon with file output logging
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --output file:/var/log/modbus-slave.jsonl

# Run as master daemon with pipe input
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_data
```

**Remarque** : Le mode TUI utilise ces modes persistants en interne avec `--ipc-channel` pour la communication bidirectionnelle avec les sous-processus CLI.

## Paramètres

| Paramètre | Description | Valeur par défaut |
|-----------|-------------|-------------------|
| `--station-id` | Identifiant de station Modbus (adresse esclave) | 1 |
| `--register-address` | Adresse de registre de départ | 0 |
| `--register-length` | Nombre de registres | 10 |
| `--register-mode` | Type de registre : holding, input, coils, discrete | holding |
| `--data-source` | Source de données : `file:<path>` ou `pipe:<name>` | - |
| `--output` | Destination de sortie : `file:<path>` ou `pipe:<name>` (par défaut : stdout) | stdout |
| `--baud-rate` | Débit baud du port série | 9600 |
| `--debounce-seconds` | Fenêtre de dédoublonnage pour la sortie JSON (secondes, flottant) | 1.0 |
| `--ipc-channel` | UUID du canal IPC pour la communication TUI (usage interne) | - |

## Modes de registre

- `holding` : Registres de maintien (lecture/écriture)
- `input` : Registres d'entrée (lecture seule)
- `coils` : Bobines (lecture/écriture, bits)
- `discrete` : Entrées discrètes (lecture seule, bits)

## Tests d'intégration

Les tests d'intégration sont disponibles dans `examples/cli_e2e/`. Exécutez-les avec :

```bash
cd examples/cli_e2e
cargo run
```

### Exécution des tests en boucle

Pour les tests de stabilité et le débogage, vous pouvez exécuter les tests plusieurs fois en utilisant l'argument `--loop-count` :

```bash
# Run tests 5 times consecutively
cargo run --example cli_e2e -- --loop-count 5

# Run tests 10 times to verify port cleanup and stability
cargo run --example cli_e2e -- --loop-count 10
```

Cela est utile pour :

- Vérifier le nettoyage des ports entre les exécutions de test
- Tester la stabilité et la répétabilité
- Déboguer les problèmes intermittents
- S'assurer que la réinitialisation des ports virtuels socat fonctionne correctement

Les tests vérifient :

- Le listage amélioré des ports avec statut
- Le mode d'écoute esclave temporaire
- Le mode d'écoute esclave persistant
- Le mode d'émission master temporaire
- Le mode d'émission master persistant
- Le test de connexion continue (fichier comme source de données et sortie vers fichier)
- Le test de connexion continue (tube Unix comme source de données et sortie vers tube)

### Tests de connexion continue

Les tests de connexion continue vérifient la transmission de données longue durée entre master et esclave :

1. **Fichiers comme source de données et sortie** : Le master lit les données depuis un fichier et les envoie, l'esclave les reçoit et les ajoute à un fichier
2. **Tubes Unix comme source de données et sortie** : Le master lit les données en temps réel depuis un tube nommé, l'esclave sort vers un tube nommé
3. **Génération de données aléatoires** : Chaque exécution de test génère des données aléatoires différentes pour garantir la fiabilité des tests

## Améliorations futures

- Tests de communication Modbus en temps réel avec des ports série virtuels
- Prise en charge de modes de registre supplémentaires
