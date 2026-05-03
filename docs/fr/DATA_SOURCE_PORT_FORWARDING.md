# Transfert de port (Mode de transfert de port transparent)

## Présentation générale

Le mode de transfert de port permet de transférer de manière transparente des données Modbus d'un port vers un autre au sein de l'interface TUI. Cette fonctionnalité permet des cas d'utilisation avancés tels que :

1. **Transfert de données** : Convertir une station esclave d'un port en une station master sur un autre port
2. **Réplication de données** : Dupliquer les données d'une station master vers plusieurs ports pour la surveillance ou les tests
3. **Pont de protocole** : Relayer des données entre différents ports physiques ou connexions virtuelles

## Quand utiliser le transfert de port

- **Scénarios multi-ports** : Lorsque vous avez plusieurs ports série et devez partager des données entre eux
- **Tests** : Créer des configurations de test où un port simule le comportement d'un autre port
- **Surveillance** : Répliquer les données d'un port actif vers un port de surveillance sans perturber la connexion d'origine
- **Agrégation de données** : Combiner des données de sources multiples en transférant depuis différents ports

## Configuration dans l'interface TUI

### Étape 1 : S'assurer que le port source est en cours d'exécution

Avant de configurer le transfert de port, assurez-vous que le port source est déjà configuré et en cours d'exécution :

1. Accédez au port source dans la page Entry
2. Configurez ses stations Modbus (mode master ou esclave)
3. Sauvegardez la configuration avec `Ctrl+S` pour activer le port
4. Vérifiez que le port affiche le statut « Running ● »

### Étape 2 : Configurer le port cible avec le transfert de port

1. Accédez au port cible (celui qui transférera les données)
2. Appuyez sur `Enter` pour ouvrir le panneau de configuration
3. Descendez jusqu'à « Enter Business Configuration » et appuyez sur `Enter`
4. Descendez jusqu'au champ « Data Source »
5. Appuyez sur `Enter` pour modifier la source de données
6. Utilisez les touches fléchées (`←` / `→`) pour parcourir les options jusqu'à atteindre « Port Forwarding »
7. Appuyez sur `Enter` pour confirmer

### Étape 3 : Sélectionner le port source

Après avoir sélectionné « Port Forwarding » comme source de données :

1. Descendez jusqu'au champ « Source Port »
2. Appuyez sur `Enter` pour ouvrir le sélecteur de port
3. Utilisez les touches fléchées (`←` / `→`) pour parcourir les ports disponibles
4. Appuyez sur `Enter` pour sélectionner le port source souhaité
5. Appuyez sur `Ctrl+S` pour sauvegarder et activer le transfert

**Remarque** : S'il n'existe qu'un seul port (le port courant), le champ « Source Port » affichera un indice grisé « No other ports available » et appuyer sur `Enter` n'aura aucun effet.

### Étape 4 : Configurer la station

Même avec le transfert de port activé, vous devez toujours configurer au moins une station sur le port cible :

1. Accédez à « Create Station »
2. Configurez l'ID de station, le type de registre, l'adresse de départ et le nombre de registres
3. Les valeurs des registres seront automatiquement renseignées à partir des données du port source

### Étape 5 : Sauvegarder et activer

1. Appuyez sur `Ctrl+S` pour sauvegarder la configuration
2. Le port démarrera avec le statut « Running ● »
3. Les données du port source seront périodiquement transférées vers ce port

## Fonctionnement

Lorsque le transfert de port est activé :

1. **Démon en arrière-plan** : L'interface TUI lance un thread dédié à ce port
2. **Lecture périodique** : Le démon lit périodiquement les valeurs des registres depuis l'état global du port source
3. **Synchronisation d'état** : Le démon met à jour les valeurs des registres du port cible via l'IPC interne
4. **Mises à jour automatiques** : Les modifications sur le port source sont automatiquement reflétées sur le port cible

Le transfert s'effectue entièrement au sein du processus TUI, sans communication réseau ou série externe requise.

## Exemple de cas d'utilisation : Configuration multi-master

Supposons que vous avez :

- `/tmp/vcom1` : Connecté à un appareil Modbus physique en tant qu'esclave
- `/tmp/vcom2` : Vous souhaitez qu'il agisse comme un master lisant depuis vcom1

Configuration :

1. Configurez `/tmp/vcom1` :
   - Mode : Esclave
   - Configurez les stations esclaves pour répondre aux requêtes Modbus

2. Configurez `/tmp/vcom2` :
   - Mode : Master
   - Data Source : Port Forwarding
   - Source Port : `/tmp/vcom1`
   - Configurez les stations master

Résultat : `/tmp/vcom2` agira comme un master, mais ses données proviennent des réponses esclaves de `/tmp/vcom1`, transférant effectivement les données.

## Exemple de cas d'utilisation : Réplication de données

Supposons que vous avez :

- `/tmp/vcom1` : Port principal lisant depuis une source de données IPC externe
- `/tmp/vcom2` : Port de surveillance devant refléter les données de vcom1

Configuration :

1. Configurez `/tmp/vcom1` :
   - Mode : Master
   - Data Source : IPC Pipe (par ex., `/tmp/data_feed`)
   - Configurez les stations

2. Configurez `/tmp/vcom2` :
   - Mode : Master
   - Data Source : Port Forwarding
   - Source Port : `/tmp/vcom1`
   - Configurez les stations avec la même disposition de registres

Résultat : Les deux ports affichent les mêmes données, vcom2 reflétant les valeurs de registres de vcom1.

## Limites

- **Le port source doit être en cours d'exécution** : Le port source doit être activé avant que le transfert puisse fonctionner
- **Pas d'auto-transfert** : Un port ne peut pas se transférer vers lui-même
- **Mode master uniquement** : Le transfert de port n'est disponible que pour les stations master
- **Interne uniquement** : Le transfert s'effectue au sein de l'interface TUI ; les processus externes ne peuvent pas transférer directement des ports

## Dépannage

### Message « No other ports available »

Cela apparaît lorsque :

- Il n'existe qu'un seul port dans le système (aucun port source vers lequel transférer)
- Le port courant est le seul port
- **Solution** : Ajoutez d'abord un autre port, configurez-le et activez-le, puis configurez le transfert

### Les données ne se mettent pas à jour

Vérifiez :

- Le port source est en cours d'exécution (affiche le statut « Running ● »)
- Le port source possède des stations configurées
- Le port cible est en cours d'exécution (affiche le statut « Running ● »)
- Les deux ports utilisent des types de registres et des plages d'adresses compatibles

### Le transfert de port n'apparaît pas dans les options

Assurez-vous :

- Que vous configurez une station master (et non esclave)
- Que vous êtes dans le panneau Modbus Dashboard
- Que vous avez navigué jusqu'au champ « Data Source »

## Avancé : Chaînes de transfert multiples

Vous pouvez créer des chaînes de transfert :

> Port A → Port B → Port C

Cependant, soyez prudent :

- Chaque maillon de la chaîne introduit de la latence
- Le transfert circulaire (A → B → A) est empêché par l'interface
- Surveillez les performances si vous utilisez plusieurs niveaux de transfert

## Voir aussi

- [Source de données IPC](DATA_SOURCE_IPC.md) - Pour l'intégration de données externes
- [Source de données HTTP](DATA_SOURCE_HTTP.md) - Pour les sources de données HTTP
- [Source de données MQTT](DATA_SOURCE_MQTT.md) - Pour l'intégration MQTT
