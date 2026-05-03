# Funcionalidades Modbus de la CLI

Este documento describe las nuevas funcionalidades de la CLI para operaciones Modbus añadidas al proyecto aoba.

## Funcionalidades

### 1. Detección y listado de puertos

#### Listar todos los puertos

El comando `--list-ports` ahora proporciona información más detallada cuando se usa con `--json`:

```bash
aoba --list-ports --json
```

La salida incluye:

- `path`: Ruta del puerto (ej., COM1, /dev/ttyUSB0)
- `status`: "Free" o "Occupied"
- `guid`: GUID del dispositivo Windows (si está disponible)
- `vid`: USB Vendor ID (si está disponible)
- `pid`: USB Product ID (si está disponible)
- `serial`: Número de serie (si está disponible)

Ejemplo de salida:

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

#### Verificar el estado de ocupación de un puerto individual

El comando `--check-port` se usa para detectar si un puerto específico está ocupado. Esto es útil para automatización de scripts y monitorización del estado de puertos:

```bash
aoba --check-port COM3
```

**Códigos de salida:**

- `0` - El puerto está libre y disponible
- `1` - El puerto está ocupado por otro programa

**Salida en texto plano:**

```
Port COM3 is free
```

o

```
Port COM3 is occupied
```

**Salida en formato JSON:**

```bash
aoba --check-port COM3 --json
```

Ejemplo de salida:

```json
{"port":"COM3","occupied":false,"status":"Free"}
```

o

```json
{"port":"COM3","occupied":true,"status":"Occupied"}
```

**Ejemplos de uso:**

Uso en scripts de shell:

```bash
# Ejemplo en Bash
if aoba --check-port /dev/ttyUSB0; then
    echo "Port is free, ready to use"
    # Realiza tus operaciones
else
    echo "Port is occupied, please close the program using this port"
    exit 1
fi
```

```powershell
# Ejemplo en PowerShell
cargo run --package aoba -- --check-port COM3
if ($LASTEXITCODE -eq 0) {
    Write-Host "Port is free"
} else {
    Write-Host "Port is occupied"
}
```

### 2. Modos de escucha del esclavo

#### Modo temporal

Escuchar una solicitud Modbus, responder y salir:

```bash
aoba --slave-listen /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

Genera una única respuesta JSON y termina.

#### Modo persistente

Escuchar solicitudes continuamente y generar JSONL:

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

Genera una línea JSON por cada solicitud procesada.

### 3. Modos de provisión del master

- Modo temporal, proveer datos una vez y salir:

```bash
aoba --master-provide /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

Lee una línea de la fuente de datos, la envía y termina.

- Modo persistente, proveer datos continuamente:

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

Lee líneas de la fuente de datos y las envía continuamente.

### Formato de fuente de datos

Para los modos master, el archivo de fuente de datos debe contener formato JSONL:

```json
{"values": [10, 20, 30, 40, 50]}
{"values": [15, 25, 35, 45, 55]}
{"values": [20, 30, 40, 50, 60]}
```

Cada línea representa una actualización que se enviará al esclavo.

#### Uso de archivos como fuente de datos

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

#### Uso de tuberías con nombre (named pipes) Unix como fuente de datos

Las tuberías con nombre Unix (FIFOs) se pueden usar para transmisión de datos en tiempo real:

```bash
# Crear tubería con nombre
mkfifo /tmp/modbus_input

# Iniciar el master en una terminal
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_input \
  --baud-rate 9600

# Escribir datos en otra terminal
echo '{"values": [10, 20, 30, 40, 50]}' > /tmp/modbus_input
```

### Destinos de salida

Para los modos esclavo, puedes especificar destinos de salida:

#### Salida a stdout (predeterminado)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600
```

#### Salida a archivo (modo adición)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output file:/path/to/output.jsonl
```

#### Salida a tubería con nombre Unix

```bash
# Crear tubería con nombre
mkfifo /tmp/modbus_output

# Iniciar el esclavo en una terminal
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output pipe:/tmp/modbus_output

# Leer datos en otra terminal
cat /tmp/modbus_output
```

## Modo demonio (operación persistente)

La CLI soporta operación continua tipo demonio a través de los **modos persistentes**:

- **Demonio esclavo**: Usa `--slave-listen-persist` para escucha y respuesta continua
- **Demonio master**: Usa `--master-provide-persist` para provisión continua de datos

Estos modos se ejecutan indefinidamente hasta ser interrumpidos (Ctrl+C) y generan JSONL (un objeto JSON por línea) por cada operación. Son ideales para:

- Aplicaciones de monitorización de larga duración
- Sistemas de registro de datos
- Integración con otras herramientas mediante tuberías o archivos
- Comunicación con subprocesos TUI (cuando se combina con `--ipc-channel`)

Ejemplo de uso como demonio:

```bash
# Ejecutar como demonio esclavo con registro en archivo
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --output file:/var/log/modbus-slave.jsonl

# Ejecutar como demonio master con entrada por tubería
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_data
```

**Nota**: El modo TUI utiliza estos modos persistentes internamente con `--ipc-channel` para comunicación bidireccional con subprocesos CLI.

## Parámetros

| Parámetro | Descripción | Predeterminado |
|-----------|-------------|----------------|
| `--station-id` | ID de estación Modbus (dirección del esclavo) | 1 |
| `--register-address` | Dirección de inicio del registro | 0 |
| `--register-length` | Número de registros | 10 |
| `--register-mode` | Tipo de registro: holding, input, coils, discrete | holding |
| `--data-source` | Fuente de datos: `file:<ruta>` o `pipe:<nombre>` | - |
| `--output` | Destino de salida: `file:<ruta>` o `pipe:<nombre>` (predeterminado: stdout) | stdout |
| `--baud-rate` | Velocidad en baudios del puerto serie | 9600 |
| `--debounce-seconds` | Ventana de debounce para salida JSON duplicada (segundos, decimal) | 1.0 |
| `--ipc-channel` | UUID del canal IPC para comunicación con TUI (uso interno) | - |

## Modos de registro

- `holding`: Registros Holding (lectura/escritura)
- `input`: Registros Input (solo lectura)
- `coils`: Coils (bits de lectura/escritura)
- `discrete`: Discrete Inputs (bits de solo lectura)

## Pruebas de integración

Las pruebas de integración están disponibles en `examples/cli_e2e/`. Ejecútalas con:

```bash
cd examples/cli_e2e
cargo run
```

### Ejecución de pruebas en modo bucle

Para pruebas de estabilidad y depuración, puedes ejecutar las pruebas múltiples veces usando el argumento de línea de comandos `--loop-count`:

```bash
# Ejecutar pruebas 5 veces consecutivas
cargo run --example cli_e2e -- --loop-count 5

# Ejecutar pruebas 10 veces para verificar limpieza de puertos y estabilidad
cargo run --example cli_e2e -- --loop-count 10
```

Esto es útil para:

- Verificar la limpieza de puertos entre ejecuciones de pruebas
- Probar la estabilidad y repetibilidad
- Depurar problemas intermitentes
- Asegurar que el reinicio de puertos virtuales socat funcione correctamente

Las pruebas verifican:

- Listado mejorado de puertos con estado
- Modo temporal de escucha esclava
- Modo persistente de escucha esclava
- Modo temporal de provisión master
- Modo persistente de provisión master
- Prueba de conexión continua (fuente de datos por archivo y salida a archivo)
- Prueba de conexión continua (fuente de datos por tubería Unix y salida a tubería)

### Pruebas de conexión continua

Las pruebas de conexión continua verifican la transmisión de datos de larga duración entre master y esclavo:

1. **Archivos como fuente de datos y salida**: El master lee datos de un archivo y los envía, el esclavo los recibe y los añade a un archivo
2. **Tuberías Unix como fuente de datos y salida**: El master lee datos en tiempo real de una tubería con nombre, el esclavo envía la salida a una tubería con nombre
3. **Generación de datos aleatorios**: Cada ejecución de prueba genera datos aleatorios diferentes para asegurar la fiabilidad de las pruebas

## Mejoras futuras

- Pruebas de comunicación Modbus en tiempo real con puertos serie virtuales
- Soporte adicional para modos de registro
