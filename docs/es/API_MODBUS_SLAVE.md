# Guía de uso de la API Modbus Slave

Este documento describe cómo utilizar la API Modbus Slave de Aoba desde aplicaciones Rust para exponer datos a masters Modbus. Los casos de uso típicos incluyen líneas de producción industrial, sistemas de control de procesos y bancos de pruebas.

El ejemplo de referencia es el crate `examples/api_slave`.

## 1. Visión general

Aoba proporciona una API del lado esclavo que refleja el estilo de la API master, basada en un patrón Builder + Hook. Es útil cuando deseas:

- Convertir tu proceso en un esclavo Modbus, exponiendo datos de coils/registros a masters externos;
- Construir rápidamente un dispositivo Modbus configurable para pruebas de integración o simulación;
- Adjuntar una cadena de middleware de hooks para logging, estadísticas, control de acceso y alertas.

El punto de entrada principal sigue siendo `_main::api::modbus::ModbusBuilder`, pero utilizas `new_slave` / `build_slave`:

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

---

## 2. Ciclo de vida básico del esclavo

Una versión simplificada del ejemplo esclavo se ve así:

```rust
use anyhow::Result;
use std::sync::Arc;
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};

struct ResponseLoggingHook;

impl ModbusHook for ResponseLoggingHook {
    fn on_before_request(&self, _port: &str) -> Result<()> {
        Ok(())
    }

    fn on_after_response(&self, port: &str, response: &ModbusResponse) -> Result<()> {
        log::info!(
            "sent response on {}: station={}, addr=0x{:04X}, values={:04X?}",
            port,
            response.station_id,
            response.register_address,
            response.values
        );
        Ok(())
    }

    fn on_error(&self, _port: &str, error: &anyhow::Error) {
        log::warn!("error: {}", error);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args: Vec<String> = std::env::args().collect();
    let port = if args.len() > 1 { &args[1] } else { "/tmp/vcom2" };

    let hook: Arc<dyn ModbusHook> = Arc::new(ResponseLoggingHook);

    let _slave = ModbusBuilder::new_slave(1)
        .with_port(port)
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(hook)
        .build_slave()?;

    // Mantener el esclavo ejecutándose y escuchando solicitudes del master
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

### Parámetros de configuración principales

- **Puerto**: mismo formato que para el master (`/dev/ttyUSB*`, `/dev/ttyS*`, `/tmp/vcom2`, etc.);
- **Station ID**: debe coincidir con el station id que los masters usarán al comunicarse con este esclavo;
- **Modo de registro y rango de direcciones**: define qué parte del espacio de direcciones Modbus expone este esclavo;
- **Timeout**: se usa internamente para controlar los timeouts de E/S/procesamiento (normalmente alineado con la configuración del master).

---

## 3. Cadena de middleware de hooks

Del lado esclavo también puedes registrar múltiples hooks para formar una cadena de middleware. Responsabilidades típicas:

- Validar o inspeccionar solicitudes entrantes antes de que se procesen;
- Registrar y post-procesar respuestas después de enviarlas;
- Generar alertas o actualizar estadísticas cuando se producen errores.

El crate `examples/api_slave` demuestra tres hooks encadenados:

- `RequestMonitorHook`: monitoriza solicitudes y genera logs/alertas en caso de errores;
- `ResponseLoggingHook`: registra cada respuesta con dirección de registro y valores;
- `StatisticsHook`: rastrea conteos de solicitudes.

Este patrón te permite mantener las preocupaciones transversales (logging, métricas, control de acceso, limitación de tasa, etc.) fuera de tu lógica de negocio principal y adjuntarlas declarativamente a una instancia esclava.

---

## 4. Casos de uso típicos

Los casos de uso comunes para la API esclava en entornos industriales y configuraciones de pruebas incluyen:

1. **Simulador de dispositivos basado en software**
   - Cuando los dispositivos reales aún no están disponibles, simula un dispositivo Modbus en Rust;
   - Actualiza periódicamente los valores internos de los registros según tus escenarios de prueba;
   - Ejecuta pruebas de integración de extremo a extremo en CI.
2. **Capa de adaptación de protocolo**
   - Tus dispositivos reales pueden usar CAN, TCP propietario u otro bus de campo, mientras que los sistemas superiores esperan Modbus;
   - Usa la API esclava para mapear esas señales a un espacio de registros/coils Modbus y presentar una interfaz Modbus unificada.
3. **Pasarela edge que expone datos procesados**
   - Recopila y normaliza datos de múltiples fuentes dentro de tu proceso o pasarela;
   - Usa la API esclava para exponer los datos procesados/agregados a sistemas SCADA heredados o de terceros vía Modbus.

---

## 5. Uso conjunto de las APIs master y esclava

Como las APIs master y esclava comparten el mismo diseño Builder + Hook, puedes combinarlas fácilmente dentro de un único proceso:

1. Usa la API master para sondear varios dispositivos aguas arriba y construir un modelo de datos interno unificado;
2. Usa la API esclava para mapear ese modelo de datos a un espacio de registros Modbus;
3. Permite que sistemas externos traten tu proceso como un dispositivo Modbus estándar.

Este patrón es útil para construir pasarelas de protocolo, nodos de agregación o arneses de prueba.

---

## 6. Ejecución del ejemplo esclavo

Desde la raíz del repositorio:

```bash
cargo run --package api_slave -- /tmp/vcom2
```

Puedes combinar esto con el ejemplo master o la CLI/TUI de Aoba para pruebas:

- Inicia el ejemplo esclavo escuchando en `/tmp/vcom2`;
- Luego usa el ejemplo master o la CLI/TUI para sondear ese puerto y verificar el comportamiento de lectura/escritura.

---

## 7. Documentación relacionada

<<<<<<< HEAD
- API del lado master: `docs/en/API_MODBUS_MASTER.md`;
- Uso de Modbus a nivel de CLI: `docs/en/CLI_MODBUS.md`;
=======
- API del lado master: `docs/es/API_MODBUS_MASTER.md`;
- Uso de Modbus a nivel de CLI: `docs/es/CLI_MODBUS.md`;
>>>>>>> origin/dev
- Capacidades de fuente de datos / exportación (HTTP, MQTT, IPC, etc.): consulta los documentos `DATA_SOURCE_*.md` en este directorio;
- Más ejemplos de extremo a extremo se encuentran en el directorio `examples`.
