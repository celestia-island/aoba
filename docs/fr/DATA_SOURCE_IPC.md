# Communication IPC (Source de données personnalisée)

## Démarrage rapide — exécuter un petit récepteur CLI

Pour le mode de source de données `ipc:<path>`, la CLI lit des lignes JSON depuis un tube nommé (FIFO) ou un fichier régulier. Pour démarrer un petit récepteur CLI qui lit depuis un FIFO, procédez comme suit :

```bash
# create a FIFO (one-time)
mkfifo /tmp/aoba_ipc.pipe

# start the CLI receiver (it will read lines from the FIFO path)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 --data-source ipc:/tmp/aoba_ipc.pipe \
  --register-mode holding --register-address 0 --register-length 10

# then, from another shell, write a JSON line into the pipe:
echo '{"source":"ipc","type":"downlink","body":{"command":"ping"}}' > /tmp/aoba_ipc.pipe
```

Remarque : le dépôt utilise également des sockets de domaine Unix / tubes nommés pour d'autres communications IPC (TUI↔CLI). Le mode de source de données `ipc:<path>` attend spécifiquement un chemin FIFO/fichier que la CLI peut ouvrir et lire ligne par ligne.

## Présentation générale

Ce document décrit comment l'application accepte des données personnalisées via IPC (communication inter-processus). Dans la conception du dépôt/de l'application, l'application agit comme l'écouteur IPC (serveur) ; les intégrations tierces ou les programmes auxiliaires agissent comme client et envoient des messages JSON au socket de l'application. Vous trouverez ci-dessous des exemples côté client uniquement (Rust/Python/Node) montrant comment se connecter et envoyer un message.

## Quand utiliser l'IPC

- Intégrations locales où la surcharge réseau est inutile
- Communication rapide et à faible latence entre processus sur le même hôte
- Harnais de test et configurations de bout en bout qui lancent des processus auxiliaires

## Format de message (recommandé)

Utilisez JSON pour la portabilité. Exemple de message :

```json
{
  "source": "ipc",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": { "command": "write_register", "registers": [{"address":0, "value":"1234"}] }
}
```

## Socket de domaine Unix : exemple Rust (avec `interprocess`)

Ajoutez la dépendance dans `Cargo.toml` :

```toml
[dependencies]
interprocess = "*"
```

L'application écoute sur un socket de domaine Unix (par exemple `/tmp/aoba_ipc.sock`). L'exemple Rust suivant montre comment un client peut se connecter à ce socket et envoyer un seul message JSON.

Client (connexion et envoi) :

```rust
use std::io::{Read, Write};
use interprocess::local_socket::LocalSocketStream;

fn main() -> std::io::Result<()> {
    let mut stream = LocalSocketStream::connect("/tmp/aoba_ipc.sock")?;
    let msg = r#"{"source":"ipc","type":"downlink","body":{"command":"ping"}}"#;
    stream.write_all(msg.as_bytes())?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp)?;
    println!("Response: {}", resp);
    Ok(())
}
```

Remarques :

- L'application est censée créer et lier le socket (l'écouteur). Les programmes clients ne doivent pas essayer de lier le même chemin — ils se contentent de se connecter.
- Si vous contrôlez les deux côtés pour les tests, vous pouvez exécuter un petit écouteur localement ; en production, l'application fournit le chemin du socket.
- Sous Windows, utilisez des tubes nommés (chemin comme `\\.\pipe\aoba_ipc`) ou les APIs multi-plateformes de `interprocess`.

## Exemple Python (AF_UNIX)

L'application crée et lie le socket de domaine Unix ; l'extrait Python suivant montre comment un client se connecte et envoie un message JSON au chemin du socket de l'application.

Client :

```python
import socket
PATH = '/tmp/aoba_ipc.sock'
cli = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
cli.connect(PATH)
cli.sendall(b'{"source":"ipc","type":"downlink","body":{"command":"ping"}}')
resp = cli.recv(65536)
print('Response:', resp)
cli.close()
```

## Exemple Node.js (ES6) — Socket de domaine Unix

L'application écoute sur le chemin du socket ; l'extrait Node.js suivant montre un client qui se connecte et envoie un message JSON.

Client :

```javascript
import net from 'net';

const PATH = '/tmp/aoba_ipc.sock';
const client = net.createConnection({ path: PATH }, () => {
  client.write(JSON.stringify({ source: 'ipc', type: 'downlink', body: { command: 'ping' } }));
});

client.on('data', (data) => {
  console.log('Response:', data.toString());
  client.end();
});
```

## Notes multi-plateformes

- Sous Windows, utilisez des tubes nommés (`\\.\pipe\<name>`). Node et Python disposent tous deux de bibliothèques pour travailler avec les tubes nommés ; Rust peut utiliser `interprocess` pour les tubes multi-plateformes.
- Assurez-vous que les permissions du fichier socket permettent aux processus de se connecter.
