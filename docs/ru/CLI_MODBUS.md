# Возможности CLI для работы с Modbus

Этот документ описывает новые функции CLI для операций Modbus, добавленные в проект aoba.

## Возможности

### 1. Обнаружение и перечисление портов

#### Перечисление всех портов

Команда `--list-ports` теперь предоставляет более подробную информацию при использовании с `--json`:

```bash
aoba --list-ports --json
```

Выходные данные включают:

- `path`: путь к порту (например, COM1, /dev/ttyUSB0)
- `status`: "Free" (свободен) или "Occupied" (занят)
- `guid`: GUID устройства Windows (если доступен)
- `vid`: USB Vendor ID (если доступен)
- `pid`: USB Product ID (если доступен)
- `serial`: серийный номер (если доступен)

Пример выходных данных:

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

#### Проверка занятости отдельного порта

Команда `--check-port` используется для определения, занят ли конкретный порт. Это полезно для автоматизации скриптов и мониторинга состояния портов:

```bash
aoba --check-port COM3
```

**Коды выхода:**

- `0` — порт свободен и доступен
- `1` — порт занят другой программой

**Текстовый вывод:**

```
Port COM3 is free
```

или

```
Port COM3 is occupied
```

**Вывод в формате JSON:**

```bash
aoba --check-port COM3 --json
```

Пример выходных данных:

```json
{"port":"COM3","occupied":false,"status":"Free"}
```

или

```json
{"port":"COM3","occupied":true,"status":"Occupied"}
```

**Примеры использования:**

Использование в скриптах оболочки:

```bash
# Пример для Bash
if aoba --check-port /dev/ttyUSB0; then
    echo "Порт свободен, готов к использованию"
    # Выполните ваши операции
else
    echo "Порт занят, закройте программу, использующую этот порт"
    exit 1
fi
```

```powershell
# Пример для PowerShell
cargo run --package aoba -- --check-port COM3
if ($LASTEXITCODE -eq 0) {
    Write-Host "Порт свободен"
} else {
    Write-Host "Порт занят"
}
```

### 2. Режимы прослушивания slave

#### Временный режим

Принять один запрос Modbus, ответить и завершить работу:

```bash
aoba --slave-listen /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

Выводит один JSON-ответ и завершает работу.

#### Постоянный режим

Непрерывно ожидать запросы и выводить JSONL:

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

Выводит по одной строке JSON на каждый обработанный запрос.

### 3. Режимы предоставления данных master

- Временный режим — предоставить данные один раз и завершить работу:

```bash
aoba --master-provide /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

Считывает одну строку из источника данных, отправляет её и завершает работу.

- Постоянный режим — непрерывное предоставление данных:

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

Считывает строки из источника данных и отправляет их непрерывно.

### Формат источника данных

Для режимов master файл источника данных должен содержать данные в формате JSONL:

```json
{"values": [10, 20, 30, 40, 50]}
{"values": [15, 25, 35, 45, 55]}
{"values": [20, 30, 40, 50, 60]}
```

Каждая строка представляет обновление, которое будет отправлено ведомому устройству.

#### Использование файлов в качестве источника данных

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

#### Использование именованных каналов Unix в качестве источника данных

Именованные каналы Unix (FIFO) можно использовать для потоковой передачи данных в реальном времени:

```bash
# Создание именованного канала
mkfifo /tmp/modbus_input

# Запуск master в одном терминале
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_input \
  --baud-rate 9600

# Запись данных в другом терминале
echo '{"values": [10, 20, 30, 40, 50]}' > /tmp/modbus_input
```

### Выходные назначения

Для режимов slave можно указать назначения вывода:

#### Вывод в stdout (по умолчанию)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600
```

#### Вывод в файл (режим добавления)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output file:/path/to/output.jsonl
```

#### Вывод в именованный канал Unix

```bash
# Создание именованного канала
mkfifo /tmp/modbus_output

# Запуск slave в одном терминале
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output pipe:/tmp/modbus_output

# Чтение данных в другом терминале
cat /tmp/modbus_output
```

## Режим демона (постоянная работа)

CLI поддерживает непрерывную работу в режиме демона через **постоянные режимы**:

- **Демон slave**: используйте `--slave-listen-persist` для непрерывного прослушивания и ответа
- **Демон master**: используйте `--master-provide-persist` для непрерывной передачи данных

Эти режимы работают бесконечно до прерывания (Ctrl+C) и выводят JSONL (по одному JSON-объекту на строку) для каждой операции. Они подходят для:

- Длительно работающих приложений мониторинга
- Систем логирования данных
- Интеграции с другими инструментами через каналы или файлы
- Взаимодействия с подпроцессами TUI (в сочетании с `--ipc-channel`)

Пример использования в режиме демона:

```bash
# Запуск демона slave с логированием в файл
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --output file:/var/log/modbus-slave.jsonl

# Запуск демона master с вводом из канала
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_data
```

**Примечание**: Режим TUI внутренне использует эти постоянные режимы с параметром `--ipc-channel` для двунаправленного взаимодействия с подпроцессами CLI.

## Параметры

| Параметр | Описание | Значение по умолчанию |
|-----------|----------|----------------------|
| `--station-id` | Идентификатор станции Modbus (адрес ведомого устройства) | 1 |
| `--register-address` | Начальный адрес регистра | 0 |
| `--register-length` | Количество регистров | 10 |
| `--register-mode` | Тип регистров: holding, input, coils, discrete | holding |
| `--data-source` | Источник данных: `file:<путь>` или `pipe:<имя>` | - |
| `--output` | Назначение вывода: `file:<путь>` или `pipe:<имя>` (по умолчанию: stdout) | stdout |
| `--baud-rate` | Скорость передачи последовательного порта | 9600 |
| `--debounce-seconds` | Окно подавления дублирующего JSON-вывода (секунды, float) | 1.0 |
| `--ipc-channel` | UUID канала IPC для взаимодействия с TUI (внутреннее использование) | - |

## Режимы регистров

- `holding`: Holding Registers (чтение/запись)
- `input`: Input Registers (только чтение)
- `coils`: Coils (чтение/запись, битовые)
- `discrete`: Discrete Inputs (только чтение, битовые)

## Интеграционные тесты

Интеграционные тесты доступны в каталоге `examples/cli_e2e/`. Запустите их командой:

```bash
cd examples/cli_e2e
cargo run
```

### Запуск тестов в циклическом режиме

Для тестирования стабильности и отладки можно запустить тесты несколько раз с использованием аргумента командной строки `--loop-count`:

```bash
# Запуск тестов 5 раз подряд
cargo run --example cli_e2e -- --loop-count 5

# Запуск тестов 10 раз для проверки очистки портов и стабильности
cargo run --example cli_e2e -- --loop-count 10
```

Это полезно для:

- Проверки очистки портов между запусками тестов
- Тестирования стабильности и повторяемости
- Отладки периодических проблем
- Проверки корректности сброса виртуальных портов socat

Тесты проверяют:

- Расширенное перечисление портов с указанием состояния
- Временный режим прослушивания slave
- Постоянный режим прослушивания slave
- Временный режим предоставления данных master
- Постоянный режим предоставления данных master
- Тест непрерывного соединения (файловый источник данных и файловый вывод)
- Тест непрерывного соединения (источник данных и вывод через каналы Unix)

### Тесты непрерывного соединения

Тесты непрерывного соединения проверяют длительную передачу данных между master и slave:

1. **Файлы в качестве источника данных и вывода**: master считывает данные из файла и отправляет, slave принимает и добавляет в файл
2. **Каналы Unix в качестве источника данных и вывода**: master считывает данные в реальном времени из именованного канала, slave выводит в именованный канал
3. **Генерация случайных данных**: каждый запуск теста генерирует различные случайные данные для обеспечения надёжности тестирования

## Планируемые улучшения

- Тесты обмена данными Modbus в реальном времени с виртуальными последовательными портами
- Поддержка дополнительных режимов регистров
