# IPC-взаимодействие (пользовательский источник данных)

## Быстрый старт — запуск небольшого CLI-приёмника

Для режима источника данных `ipc:<путь>` CLI считывает JSON-строки из именованного канала (FIFO) или обычного файла. Чтобы запустить небольшой CLI-приёмник, считывающий данные из FIFO, выполните следующее:

```bash
# создание FIFO (однократно)
mkfifo /tmp/aoba_ipc.pipe

# запуск CLI-приёмника (будет считывать строки из пути FIFO)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 --data-source ipc:/tmp/aoba_ipc.pipe \
  --register-mode holding --register-address 0 --register-length 10

# затем из другой оболочки запишите JSON-строку в канал:
echo '{"source":"ipc","type":"downlink","body":{"command":"ping"}}' > /tmp/aoba_ipc.pipe
```

Примечание: репозиторий также использует доменные сокеты Unix / именованные каналы для других видов IPC (TUI↔CLI). Режим источника данных `ipc:<путь>` специально предполагает FIFO/файл, который CLI может открыть и читать построчно.

## Обзор

Этот документ описывает, как приложение принимает пользовательские данные через IPC (межпроцессное взаимодействие). В архитектуре репозитория/приложения приложение выступает в роли слушателя IPC (сервера); сторонние интеграции или вспомогательные программы выступают в роли клиента и отправляют JSON-сообщения в сокет приложения. Ниже приведены примеры клиентов (Rust/Python/Node), демонстрирующие подключение и отправку сообщения.

## Когда использовать IPC

- Локальные интеграции, где накладные расходы сети не требуются
- Быстрое взаимодействие с низкой задержкой между процессами на одном хосте
- Тестовые обвязки и сквозные (E2E) сценарии, запускающие вспомогательные процессы

## Формат сообщения (рекомендуемый)

Используйте JSON для переносимости. Пример сообщения:

```json
{
  "source": "ipc",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": { "command": "write_register", "registers": [{"address":0, "value":"1234"}] }
}
```

## Доменный сокет Unix: пример на Rust (с использованием `interprocess`)

Добавьте зависимость в `Cargo.toml`:

```toml
[dependencies]
interprocess = "*"
```

Приложение ожидает подключений на доменном сокете Unix (например, `/tmp/aoba_ipc.sock`). Следующий пример на Rust показывает, как клиент может подключиться к этому сокету и отправить одно JSON-сообщение.

Клиент (подключение и отправка):

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

Примечания:

- Приложение должно создать и привязать сокет (слушатель). Клиентские программы не должны пытаться привязать тот же путь — они только подключаются.
- Если вы управляете обеими сторонами для тестов, можете запустить небольшой слушатель локально; для промышленной эксплуатации приложение предоставляет путь к сокету.
- На Windows используйте Named Pipes (путь вида `\\.\pipe\aoba_ipc`) или кроссплатформенные API `interprocess`.

## Пример на Python (AF_UNIX)

Приложение создаёт и привязывает доменный сокет Unix; следующий фрагмент на Python показывает, как клиент подключается и отправляет JSON-сообщение по пути к сокету приложения.

Клиент:

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

## Пример на Node.js (ES6) — доменный сокет Unix

Приложение ожидает подключений по пути к сокету; следующий фрагмент на Node.js показывает клиента, который подключается и отправляет JSON-сообщение.

Клиент:

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

## Кроссплатформенные заметки

- На Windows используйте Named Pipes (`\\.\pipe\<имя>`). Node и Python имеют библиотеки для работы с именованными каналами; Rust может использовать `interprocess` для кроссплатформенных каналов.
- Убедитесь, что права доступа к файлу сокета разрешают подключение процессов.

При необходимости можно предоставить небольшую тестовую обвязку, запускающую сервер и клиента и демонстрирующую сквозной цикл JSON на выбранном вами языке программирования.
