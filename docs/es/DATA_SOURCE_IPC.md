# Comunicación IPC (Fuente de datos personalizada)

## Inicio rápido — ejecutar un receptor CLI sencillo

Para el modo de fuente de datos `ipc:<ruta>`, la CLI lee líneas JSON desde una tubería con nombre (FIFO) o un archivo regular. Para iniciar un receptor CLI sencillo que lee desde una FIFO, haz lo siguiente:

```bash
# crear una FIFO (una sola vez)
mkfifo /tmp/aoba_ipc.pipe

# iniciar el receptor CLI (leerá líneas desde la ruta FIFO)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 --data-source ipc:/tmp/aoba_ipc.pipe \
  --register-mode holding --register-address 0 --register-length 10

# luego, desde otra terminal, escribe una línea JSON en la tubería:
echo '{"source":"ipc","type":"downlink","body":{"command":"ping"}}' > /tmp/aoba_ipc.pipe
```

Nota: el repositorio también usa sockets de dominio Unix / tuberías con nombre para otra IPC (TUI↔CLI). El modo de fuente de datos `ipc:<ruta>` espera específicamente una ruta FIFO/archivo que la CLI pueda abrir y leer línea por línea.

## Visión general

Este documento describe cómo la aplicación acepta datos personalizados vía IPC (comunicación entre procesos). En el diseño del repositorio/aplicación, la aplicación actúa como el listener IPC (servidor); las integraciones de terceros o programas auxiliares deben actuar como clientes y enviar mensajes JSON al socket de la aplicación. A continuación se presentan ejemplos solo de cliente (Rust/Python/Node) mostrando cómo conectarse y enviar un mensaje.

## Cuándo usar IPC

- Integraciones locales donde la sobrecarga de red es innecesaria
- Comunicación rápida y de baja latencia entre procesos en el mismo host
- Arneses de prueba y configuraciones E2E que lanzan procesos auxiliares

## Estructura del mensaje (recomendada)

Usa JSON para portabilidad. Ejemplo de mensaje:

```json
{
  "source": "ipc",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": { "command": "write_register", "registers": [{"address":0, "value":"1234"}] }
}
```

## Socket de dominio Unix: ejemplo en Rust (usando `interprocess`)

Añade la dependencia en `Cargo.toml`:

```toml
[dependencies]
interprocess = "*"
```

La aplicación escucha en un socket de dominio Unix (por ejemplo `/tmp/aoba_ipc.sock`). El siguiente ejemplo en Rust muestra cómo un cliente puede conectarse a ese socket y enviar un único mensaje JSON.

Cliente (conectar y enviar):

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

Notas:

- Se espera que la aplicación cree y vincule el socket (el listener). Los programas cliente no deben intentar vincular la misma ruta — solo se conectan.
- Si controlas ambos lados para pruebas, puedes ejecutar un pequeño listener localmente; para producción, la aplicación proporciona la ruta del socket.
- En Windows usa Named Pipes (ruta como `\\.\pipe\aoba_ipc`) o usa las APIs multiplataforma de `interprocess`.

## Ejemplo en Python (AF_UNIX)

La aplicación crea y vincula el socket de dominio Unix; el siguiente fragmento en Python muestra cómo un cliente se conecta y envía un mensaje JSON a la ruta del socket de la aplicación.

Cliente:

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

## Ejemplo en Node.js (ES6) — socket de dominio UNIX

La aplicación escucha en la ruta del socket; el siguiente fragmento en Node.js muestra un cliente que se conecta y envía un mensaje JSON.

Cliente:

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

## Notas multiplataforma

- En Windows usa Named Pipes (`\\.\pipe\<nombre>`). Tanto Node como Python tienen librerías para trabajar con named pipes; Rust puede usar `interprocess` para tuberías multiplataforma.
- Asegúrate de que los permisos del archivo de socket permitan que los procesos se conecten.

Si lo deseas, puedo proporcionar un pequeño arnés de prueba que lance el servidor y el cliente y demuestre un recorrido completo de JSON de extremo a extremo en tu lenguaje preferido.
