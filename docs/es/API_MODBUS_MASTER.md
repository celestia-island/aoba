# Guía de uso de la API Modbus Master

Este documento describe cómo utilizar la API Modbus Master de Aoba desde aplicaciones Rust en escenarios industriales típicos (monitorización de líneas de producción, control de procesos, monitorización ambiental, etc.), utilizando el crate `examples/api_master` como referencia.

## 1. Visión general

Aoba expone una API Modbus master basada en traits, diseñada para integrarse en otras aplicaciones Rust o software de control de hardware. Los casos de uso típicos incluyen:

- Sondeo periódico de dispositivos Modbus esclavos (RTU sobre puertos serie o virtuales)
- Recopilación de valores de coils / registros hacia tu propia lógica de telemetría o control
- Integración con sistemas existentes de logging / monitorización mediante hooks

El punto de entrada principal es el tipo `ModbusBuilder` de `_main::api::modbus`.

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

> Nota: en los ejemplos, el crate raíz se llama `_main`. En tu propio proyecto, este será generalmente el crate principal `aoba` o el nombre que le des en `Cargo.toml`.

---

## 2. Ciclo de vida básico del master

Un bucle de sondeo mínimo se ve así:

```rust
use anyhow::Result;
use std::time::Duration;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    let master = ModbusBuilder::new_master(1) // station id del esclavo
        .with_port("/dev/ttyUSB0")          // o `/tmp/vcom1` etc.
        .with_register(RegisterMode::Holding, 0, 10)
        .with_timeout(1000)                  // milisegundos
        .build_master()?;

    loop {
        if let Some(resp) = master.recv_timeout(Duration::from_secs(1)) {
            println!("values = {:04X?}", resp.values);
        }
    }
}
```

### Parámetros importantes

- **Puerto**: cualquier puerto serie o virtual que Aoba pueda abrir (`/dev/ttyUSB*`, `/dev/ttyS*` reales, o virtuales `/tmp/vcom*` creados con socat).
- **Station ID**: dirección del esclavo Modbus (normalmente 1–247).
- **Modo de registro**: uno de `RegisterMode::Coils`, `DiscreteInputs`, `Holding`, `Input`.
- **Dirección / longitud de registro**: dirección de inicio y número de elementos a leer, coincidiendo con la tabla de direcciones Modbus de tu dispositivo (por ejemplo, un PLC o pasarela de sensores).
- **Timeout**: tiempo de espera de la solicitud en milisegundos.

El master ejecuta internamente un bucle de sondeo y envía las respuestas a un canal; tu código simplemente llama a `recv_timeout` para obtener los nuevos datos.

---

## 3. Uso de hooks para logging y monitorización

Para sistemas en producción (líneas industriales, equipos de proceso, sensores en campo, etc.) normalmente necesitas:

- Registrar cada respuesta exitosa
- Rastrear errores y timeouts
- Posiblemente enviar datos a un bus de mensajes o base de datos

El trait `ModbusHook` te permite conectar esta lógica de forma centralizada.

```rust
use anyhow::Result;
use std::sync::Arc;
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

struct LoggingHook;

impl ModbusHook for LoggingHook {
    fn on_before_request(&self, port: &str) -> Result<()> {
        log::debug!("sending request on {}", port);
        Ok(())
    }

    fn on_after_response(&self, port: &str, resp: &ModbusResponse) -> Result<()> {
        log::info!(
            "resp {}: station={}, addr=0x{:04X}, values={:04X?}",
            port,
            resp.station_id,
            resp.register_address,
            resp.values,
        );
        Ok(())
    }

    fn on_error(&self, port: &str, err: &anyhow::Error) {
        log::warn!("modbus error on {}: {}", port, err);
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let master = ModbusBuilder::new_master(1)
        .with_port("/tmp/vcom1")
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(Arc::new(LoggingHook))
        .build_master()?;

    // ahora realizar sondeo con recv_timeout como en el ejemplo básico
    # let _ = master;
    Ok(())
}
```

Puedes registrar múltiples hooks (por ejemplo, uno para logging, otro para exportación de métricas).

---

## 4. Patrón de integración para monitorización industrial / de dispositivos

Para escenarios típicos de monitorización industrial (líneas de producción, unidades de proceso, dispositivos de monitorización ambiental, etc.), un patrón común es:

1. **Configurar puertos y estaciones** mediante la TUI o CLI de Aoba, o codificarlos directamente en tu aplicación.
2. **Crear un master por cada puerto físico/virtual** usando `ModbusBuilder::new_master`.
3. **Lanzar una tarea Tokio por cada master** que:
   - llame a `recv_timeout` en un bucle
   - analice `ModbusResponse::values` y los convierta a unidades de ingeniería (presión, temperatura, estado de válvulas, etc.)
   - reenvíe los datos procesados a tu backend de monitorización (MQTT, HTTP, base de datos, etc.).
4. Usar `ModbusHook` para centralizar logging, medición de latencia y conteo de errores.

Como Aoba está construido sobre `tokio`, la API master está diseñada para usarse dentro de un runtime asíncrono, pero expone un `recv_timeout` de estilo bloqueante por conveniencia en las tareas.

---

## 5. Manejo de errores y timeouts

- `build_master()` devuelve `anyhow::Error` si el puerto no se puede abrir o la configuración es inválida.
- `recv_timeout()` devuelve `None` en caso de timeout; esto no es un error en sí mismo.
- Los errores a nivel de protocolo (CRC, códigos de excepción, errores de E/S) se informan a través de `ModbusHook::on_error`.

Un patrón recomendado:

- Tratar los timeouts ocasionales como normales en entornos serie inestables.
- Usar un contador acumulativo en tu hook; si los errores consecutivos superan un umbral, generar una alarma.

---

## 6. Ejecución del ejemplo

Desde la raíz del repositorio:

```bash
cargo run --package api_master -- /tmp/vcom1
```

En un entorno de prueba similar a producción (como un banco de pruebas de almacenamiento de hidrógeno), normalmente:

- Usas la CLI/TUI de Aoba o `examples/modbus_slave` para simular el lado esclavo.
- Luego ejecutas el ejemplo `api_master` para verificar que el cableado Modbus y la lógica a nivel de aplicación funcionan correctamente.

---

## 7. Master en modo manual (poll_once / operaciones de escritura)

Para escenarios donde necesitas un control detallado del tiempo de sondeo (máquinas de estados, estrategias adaptativas u operaciones de escritura), usa `build_master_manual()`:

```rust
use anyhow::Result;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    let master = ModbusBuilder::new_master(1)
        .with_port("/dev/ttyUSB0")
        .with_baud_rate(9600)
        .with_timeout(5000)
        .build_master_manual()?;

    // Sondeo manual único
    let response = master.poll_once(RegisterMode::Holding, 0x00, 10)?;
    println!("Values: {:?}", response.values);

    // Escribir un solo registro holding (fc 0x06)
    master.write_holding(0x00, 0x1234)?;

    // Escribir múltiples registros holding (fc 0x10)
    master.write_registers(0x00, &[0x1234, 0x5678, 0x9ABC])?;

    // Escribir coils (fc 0x0F)
    master.write_coils(0x00, &[true, false, true, true])?;

    Ok(())
}
```

### Cuándo usar el modo manual

| Escenario | Modo recomendado |
|----------|-----------------|
| Monitorización continua / recopilación de datos | `build_master()` (automático) |
| Bucles de control leer-modificar-escribir | `build_master_manual()` |
| Máquina de estados / sondeo basado en eventos | `build_master_manual()` |
| Sondeo adaptativo basado en latencia de respuesta | `build_master_manual()` |
| Diagnósticos puntuales o configuración | `build_master_manual()` |

### Detalles de las operaciones de escritura

- **`write_holding(address, value)`** — escribe un solo registro holding usando el código de función 0x06. Ideal para escribir parámetros de configuración individuales.
- **`write_registers(address, values)`** — escribe múltiples registros holding consecutivos usando el código de función 0x10. Ideal para escritura de parámetros en lote.
- **`write_coils(address, values)`** — escribe múltiples coils usando el código de función 0x0F. Incluye intercambio de bytes automático para escrituras de 11 coils (requerido por cierto hardware).
- Todos los métodos de escritura se bloquean hasta que el esclavo reconoce o se produce un error.

---

## 8. Próximos pasos

- Para las APIs del lado esclavo, consulta `examples/api_slave`.
<<<<<<< HEAD
- Para el uso de Modbus a nivel de CLI, consulta `docs/en/CLI_MODBUS.md`.
=======
- Para el uso de Modbus a nivel de CLI, consulta `docs/es/CLI_MODBUS.md`.
>>>>>>> origin/dev
- Para la exportación de datos vía HTTP / MQTT / IPC, consulta los documentos `DATA_SOURCE_*.md` en este directorio.
