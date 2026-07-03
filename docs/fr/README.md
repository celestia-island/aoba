<p align="center">
  <img src="../../res/logo.webp" alt="Aoba Logo" width="240" />
</p>

<p align="center">
  <h1 align="center">Aoba</h1>
</p>

<p align="center">
  <a href="https://github.com/celestia-island/aoba/actions/workflows/checks.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/checks.yml/badge.svg?branch=master" alt="Checks status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-tui.yml/badge.svg?branch=master" alt="E2E TUI status" />
  </a>
  <a href="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml">
    <img src="https://github.com/celestia-island/aoba/actions/workflows/e2e-tests-cli.yml/badge.svg?branch=master" alt="E2E CLI status" />
  </a>
  <a href="../../LICENSE">
    <img src="https://img.shields.io/badge/license-SySL%201.0-blue" alt="License: SySL" />
  </a>
  <a href="https://github.com/celestia-island/aoba/releases/latest">
    <img src="https://img.shields.io/github/v/tag/celestia-island/aoba?label=version&sort=semver" alt="Latest Version" />
  </a>
</p>

<p align="center">
  <a href="../ar/README.md">AR</a> | <a href="../en/README.md">EN</a> | <a href="../es/README.md">ES</a> | FR | <a href="../ja/README.md">JA</a> | <a href="../ko/README.md">KO</a> | <a href="../ru/README.md">RU</a> | <a href="../zhs/README.md">ZH</a> | <a href="../zht/README.md">ZHT</a>
</p>

Outil de debogage et de simulation multi-protocole pour Modbus RTU, compatible avec les ports serie physiques et les ports rediriges via le reseau. Fournit des interfaces CLI et TUI.

## Fonctionnalites

- Debogage et simulation Modbus RTU (maitre/esclave) ; prend en charge quatre types de registres : holding, input, coils et discrete.
- CLI complet : decouverte et verification de ports (`--list-ports` / `--check-port`), operations maitre/esclave (`--master-provide` / `--slave-listen`) et modes persistants (`--*-persist`). Sorties en JSON/JSONL adaptees aux scripts et au CI.
- TUI interactif : configuration des ports, stations et registres via l'interface terminal ; prise en charge de la sauvegarde/chargement (`Ctrl+S` sauvegarde et active automatiquement les ports) et integration IPC avec CLI pour les tests et l'automatisation.
- Multiples sources de donnees et protocoles : ports serie physiques/virtuels (geres via `socat`), HTTP, MQTT, IPC (sockets Unix / tubes nommes), fichiers et FIFOs.
- Redirection de ports : configuration des ports source et destination dans le TUI pour la replication, la surveillance ou le pontage des donnees.
- Mode demon : execution sans interface en utilisant une configuration TUI sauvegardee pour demarrer tous les ports/stations configures (adapte aux deploiements embarques/CI).
- Outils de port virtuel et de test : inclut `scripts/socat_init.sh` pour les ports serie virtuels et des tests d'exemple dans `examples/cli_e2e` et `examples/tui_e2e`.

> Remarque : utilisez `--no-config-cache` pour desactiver la sauvegarde/chargement du TUI ; `--config-file <FILE>` et `--no-config-cache` sont mutuellement exclusifs.

## Demarrage rapide

1. Installer la chaine d'outils Rust
2. Cloner le depot et entrer dans le repertoire
3. Installer :
   - Compiler depuis les sources : `cargo install aoba`
   - Ou installer une version pre-compilee par CI (si disponible) avec `cargo-binstall` :
     - Exemple : `cargo binstall --manifest-path ./Cargo.toml --version <version>`
     - Utilisez `--target <triple>` pour choisir une plateforme specifique (ex. `x86_64-unknown-linux-gnu`)
4. Lancer `aoba` pour demarrer le TUI par defaut

## Documentation

- [API Modbus Master](API_MODBUS_MASTER.md)
- [API Modbus Slave](API_MODBUS_SLAVE.md)
- [CLI Modbus](CLI_MODBUS.md)
- [Source de donnees HTTP](DATA_SOURCE_HTTP.md)
- [Source de donnees IPC](DATA_SOURCE_IPC.md)
- [Source de donnees MQTT](DATA_SOURCE_MQTT.md)
- [Redirection de ports](DATA_SOURCE_PORT_FORWARDING.md)

## Langues

| Langue | Lien |
|--------|------|
| العربية | [AR](../ar/README.md) |
| English | [EN](../en/README.md) |
| Espanol | [ES](../es/README.md) |
| Francais | [FR](README.md) |
| 日本語 | [JA](../ja/README.md) |
| 한국어 | [KO](../ko/README.md) |
| Русский | [RU](../ru/README.md) |
| 简体中文 | [ZH](../zhs/README.md) |
| 繁體中文 | [ZHT](../zht/README.md) |

## Licence

Sous licence [Synthetic Source License (SySL), Version 1.0](../../LICENSE).
