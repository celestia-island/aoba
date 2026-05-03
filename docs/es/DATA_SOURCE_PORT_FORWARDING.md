# Reenvío de puertos (Modo de reenvío transparente de puertos)

## Visión general

El modo de reenvío de puertos permite reenviar datos Modbus de forma transparente de un puerto a otro dentro de la TUI. Esta funcionalidad habilita casos de uso avanzados como:

1. **Reenvío de datos**: Convertir una estación esclava de un puerto en una estación master en otro puerto
2. **Replicación de datos**: Duplicar datos de una estación master a múltiples puertos para monitorización o pruebas
3. **Puente de protocolo**: Tender un puente de datos entre diferentes puertos físicos o conexiones virtuales

## Cuándo usar el reenvío de puertos

- **Escenarios multi-puerto**: Cuando tienes múltiples puertos serie y necesitas compartir datos entre ellos
- **Pruebas**: Crear configuraciones de prueba donde un puerto simula el comportamiento de otro
- **Monitorización**: Replicar datos de un puerto activo a un puerto de monitorización sin interrumpir la conexión original
- **Agregación de datos**: Combinar datos de múltiples fuentes reenviando desde diferentes puertos

## Configuración en la TUI

### Paso 1: Asegurarse de que el puerto fuente esté en ejecución

Antes de configurar el reenvío de puertos, asegúrate de que el puerto fuente ya esté configurado y en ejecución:

1. Navega al puerto fuente en la página Entry
2. Configura sus estaciones Modbus (modo master o esclavo)
3. Guarda la configuración con `Ctrl+S` para habilitar el puerto
4. Verifica que el puerto muestre el estado "Running ●"

### Paso 2: Configurar el puerto destino con reenvío de puertos

1. Navega al puerto destino (el que reenviará los datos)
2. Pulsa `Enter` para entrar al Panel de Configuración
3. Navega hacia abajo hasta "Enter Business Configuration" y pulsa `Enter`
4. Navega hacia abajo hasta el campo "Data Source"
5. Pulsa `Enter` para editar la fuente de datos
6. Usa las teclas de flecha (`←` / `→`) para recorrer las opciones hasta llegar a "Port Forwarding"
7. Pulsa `Enter` para confirmar

### Paso 3: Seleccionar el puerto fuente

Después de seleccionar "Port Forwarding" como fuente de datos:

1. Navega hacia abajo hasta el campo "Source Port"
2. Pulsa `Enter` para abrir el selector de puertos
3. Usa las teclas de flecha (`←` / `→`) para navegar entre los puertos disponibles
4. Pulsa `Enter` para seleccionar el puerto fuente deseado
5. Pulsa `Ctrl+S` para guardar y habilitar el reenvío

**Nota**: Si solo existe un puerto (el puerto actual), el campo "Source Port" mostrará un indicador atenuado "No other ports available" y pulsar `Enter` no hará nada.

### Paso 4: Configurar la estación

Incluso con el reenvío de puertos habilitado, aún necesitas configurar al menos una estación en el puerto destino:

1. Navega a "Create Station"
2. Configura Station ID, Register Type, Start Address y Register Count
3. Los valores de los registros se rellenarán automáticamente con los datos del puerto fuente

### Paso 5: Guardar y habilitar

1. Pulsa `Ctrl+S` para guardar la configuración
2. El puerto comenzará a ejecutarse con el estado "Running ●"
3. Los datos del puerto fuente se reenviarán periódicamente a este puerto

## Cómo funciona

Cuando el reenvío de puertos está habilitado:

1. **Demonio en segundo plano**: La TUI lanza un hilo en segundo plano dedicado a este puerto
2. **Lectura periódica**: El demonio lee periódicamente los valores de los registros desde el estado global del puerto fuente
3. **Sincronización de estado**: El demonio actualiza los valores de los registros del puerto destino vía IPC interna
4. **Actualizaciones automáticas**: Los cambios en el puerto fuente se reflejan automáticamente en el puerto destino

El reenvío ocurre completamente dentro del proceso de la TUI, sin requerir comunicación de red o serie externa.

## Caso de uso de ejemplo: Configuración multi-master

Supongamos que tienes:

- `/tmp/vcom1`: Conectado a un dispositivo Modbus físico como esclavo
- `/tmp/vcom2`: Deseas que actúe como master leyendo desde vcom1

Configuración:

1. Configurar `/tmp/vcom1`:
   - Modo: Slave
   - Configurar estaciones esclavas para responder a solicitudes Modbus

2. Configurar `/tmp/vcom2`:
   - Modo: Master
   - Data Source: Port Forwarding
   - Source Port: `/tmp/vcom1`
   - Configurar estaciones master

Resultado: `/tmp/vcom2` actuará como master, pero sus datos provienen de las respuestas esclavas de `/tmp/vcom1`, reenviando efectivamente los datos.

## Caso de uso de ejemplo: Replicación de datos

Supongamos que tienes:

- `/tmp/vcom1`: Puerto principal que lee de una fuente de datos IPC externa
- `/tmp/vcom2`: Puerto de monitorización que necesita replicar los datos de vcom1

Configuración:

1. Configurar `/tmp/vcom1`:
   - Modo: Master
   - Data Source: IPC Pipe (ej., `/tmp/data_feed`)
   - Configurar estaciones

2. Configurar `/tmp/vcom2`:
   - Modo: Master
   - Data Source: Port Forwarding
   - Source Port: `/tmp/vcom1`
   - Configurar estaciones con el mismo diseño de registros

Resultado: Ambos puertos muestran los mismos datos, con vcom2 replicando los valores de registros de vcom1.

## Limitaciones

- **El puerto fuente debe estar en ejecución**: El puerto fuente debe estar habilitado antes de que el reenvío pueda funcionar
- **Sin auto-reenvío**: Un puerto no puede reenviar desde sí mismo
- **Solo modo Master**: El reenvío de puertos solo está disponible para estaciones master
- **Solo interno**: El reenvío ocurre dentro de la TUI; los procesos externos no pueden reenviar puertos directamente

## Resolución de problemas

### Mensaje "No other ports available"

Esto aparece cuando:

- Solo existe un puerto en el sistema (no hay puerto fuente del cual reenviar)
- El puerto actual es el único puerto
- **Solución**: Añade otro puerto primero, configúralo y habilítalo, luego configura el reenvío

### Los datos no se actualizan

Verifica:

- El puerto fuente está en ejecución (muestra el estado "Running ●")
- El puerto fuente tiene estaciones configuradas
- El puerto destino está en ejecución (muestra el estado "Running ●")
- Ambos puertos usan tipos de registros y rangos de direcciones compatibles

### El reenvío de puertos no aparece en las opciones

Asegúrate de:

- Estar configurando una estación master (no esclavo)
- Estar en el panel del Dashboard Modbus
- Haber navegado hasta el campo "Data Source"

## Avanzado: Cadenas de reenvío múltiples

Puedes crear cadenas de reenvío:

> Puerto A → Puerto B → Puerto C

Sin embargo, ten precaución:

- Cada eslabón en la cadena introduce latencia
- El reenvío circular (A → B → A) está prevenido por la interfaz
- Monitoriza el rendimiento si usas múltiples niveles de reenvío

## Ver también

- [Fuente de datos IPC](DATA_SOURCE_IPC.md) - Para integración de datos externos
- [Fuente de datos HTTP](DATA_SOURCE_HTTP.md) - Para fuentes de datos basadas en HTTP
- [Fuente de datos MQTT](DATA_SOURCE_MQTT.md) - Para integración MQTT
