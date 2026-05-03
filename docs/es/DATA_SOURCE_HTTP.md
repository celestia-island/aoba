# Fuente de datos personalizada — HTTP

## Inicio rápido — ejecutar un receptor CLI sencillo

Inicia la CLI de la aplicación en un modo que actuará como receptor (la CLI alojará un endpoint HTTP y aplicará el JSON recibido). Ejemplo (ejecutar desde la raíz del repositorio):

```bash
# usando cargo (recomendado durante el desarrollo)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source http://8080

# o, si compilaste el binario:
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source http://8080
```

El comando anterior inicia un servidor HTTP vinculado a `127.0.0.1:8080` y acepta solicitudes `POST` a `/` (raíz). Usa el ejemplo con `curl` a continuación para enviar datos vía POST.

## Visión general

Este documento describe la fuente de datos personalizada HTTP utilizada por la aplicación. Muestra el formato esperado de la solicitud, los headers comunes y un ejemplo sencillo con `curl` que puedes usar para validar rápidamente la integración.

## Endpoint

- Método: `POST`
- URL: `http://<host>:<port>/` (ejemplo: `http://localhost:8080/`)
- Content-Type: `application/json`

## Formato de solicitud

El servicio acepta un cuerpo JSON. Un ejemplo mínimo de payload:

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

Notas:

- Usa ISO 8601 para `timestamp` cuando esté disponible.
- El contenido de `payload` es específico de la aplicación; el ejemplo anterior muestra una actualización de estilo de registro común.

## Ejemplo de prueba con curl

Reemplaza `<host>` y `<port>` con tu servidor en ejecución. Este comando `curl` envía el payload JSON anterior:

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

## Comportamiento esperado

- HTTP `200 OK` (o `202 Accepted`) para mensajes aceptados/encolados.
- Si el servidor devuelve un error (4xx/5xx), inspecciona el cuerpo de la respuesta para más detalles.

## Consejos y resolución de problemas

- Asegúrate de que el header `Content-Type: application/json` esté presente.
- Si tu servidor requiere autenticación, añade el header `Authorization` apropiado (ej. `Bearer <token>`).
- Para payloads grandes, considera probar con `--data-binary` e incrementar los timeouts del servidor.

Si necesitas un ejemplo adaptado a un esquema interno, pega un JSON de ejemplo aquí y los desarrolladores adaptarán el handler del endpoint en consecuencia.
Si necesitas un ejemplo adaptado a un esquema interno, pega un JSON de ejemplo aquí y los desarrolladores adaptarán el handler del endpoint en consecuencia.
