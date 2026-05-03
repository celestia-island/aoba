# Fuente de datos personalizada — MQTT

## Inicio rápido — ejecutar un receptor CLI sencillo

Inicia la CLI de la aplicación para que se suscriba a un topic MQTT y actúe como receptor. Ejemplo (ejecutar desde la raíz del repositorio):

```bash
# usando cargo (recomendado durante el desarrollo)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source mqtt://localhost:1883/aoba/data/in

# o, si compilaste el binario:
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source mqtt://localhost:1883/aoba/data/in
```

La URL `mqtt://.../<topic>` incluye la ruta del topic (ej. `aoba/data/in`) y la CLI se suscribirá a ese topic.

## Visión general

Este documento describe cómo publicar mensajes en la fuente de datos personalizada basada en MQTT de la aplicación. Incluye la configuración del broker/conexión, nombres de topics recomendados y un ejemplo de payload con `mosquitto_pub` para realizar un downlink de datos.

## Broker / conexión

- Host: `mqtt.example.com` o `localhost`
- Puerto: `1883` (texto plano) o `8883` (TLS)
- Usuario/contraseña: opcional — si tu broker requiere autenticación, proporciónalos en la configuración del cliente
- TLS: si usas `8883`, proporciona el certificado CA y el certificado/clave del cliente donde sea necesario

## Topics recomendados

- Entrada (hacia la app): `aoba/data/in` — la app se suscribe aquí para recibir datos o comandos ascendentes
- Downlink (hacia dispositivo/vcom): `aoba/data/out/<port>` — la app publica mensajes de downlink procesados dirigidos a un puerto específico (ej. `aoba/data/out/tmp_vcom1`)

## Formato del payload

La aplicación espera payloads JSON. El esquema exacto es flexible, pero el siguiente ejemplo es una estructura práctica tanto para actualizaciones de estado como para comandos de downlink:

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

## Ejemplo: publicar un downlink usando mosquitto_pub

Este ejemplo publica un downlink en el topic de entrada que la aplicación procesará y luego realizará la escritura física en el `port` configurado.

```bash
mosquitto_pub -h localhost -p 1883 -t "aoba/data/in" -u "user" -P "pass" -m '{
  "source":"mqtt",
  "timestamp":"2025-11-15T12:34:56Z",
  "port":"/tmp/vcom1",
  "type":"downlink",
  "body":{ "command":"write_register", "registers":[{"address":0,"value":"1234"}] }
}'
```

## Notas y consejos

- Usa nombres de topics predecibles para simplificar el filtrado y los permisos.
- Cuando dirijas datos a una ruta de puerto serie físico (ej. `/tmp/vcom1`), evita caracteres que puedan causar problemas de parsing en el topic; puedes mapear nombres de puertos a etiquetas seguras para topics en la configuración.
- Si tu broker soporta mensajes retenidos, ten precaución: los mensajes de downlink retenidos pueden reaplicarse al reconectar.

Si deseas una configuración de ejemplo del broker o un arnés de pruebas automatizado (ej. un pequeño script que publique una secuencia de downlinks y espere confirmación de estado del CLI/TUI), indica tus herramientas preferidas y puedo añadirla
