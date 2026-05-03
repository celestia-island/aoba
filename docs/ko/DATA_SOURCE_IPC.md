# IPC 통신 (커스텀 데이터 소스)

## 빠른 시작 — 간단한 CLI 수신기 실행

데이터 소스 모드 `ipc:<path>`의 경우 CLI는 명명 파이프(FIFO) 또는 일반 파일에서 JSON 라인을 읽습니다. FIFO에서 읽는 간단한 CLI 수신기를 시작하려면 다음을 수행합니다:

```bash
# FIFO 생성 (최초 1회)
mkfifo /tmp/aoba_ipc.pipe

# CLI 수신기 시작 (FIFO 경로에서 라인을 읽음)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 --data-source ipc:/tmp/aoba_ipc.pipe \
  --register-mode holding --register-address 0 --register-length 10

# 그런 다음 다른 셸에서 파이프에 JSON 라인을 작성합니다:
echo '{"source":"ipc","type":"downlink","body":{"command":"ping"}}' > /tmp/aoba_ipc.pipe
```

참고: 저장소는 다른 IPC(TUI↔CLI)에도 Unix 도메인 소켓 / 명명 파이프를 사용합니다. `ipc:<path>` 데이터 소스 모드는 특히 CLI가 열고 줄 단위로 읽을 수 있는 FIFO/파일 경로를 기대합니다.

## 개요

이 문서는 애플리케이션이 IPC(프로세스 간 통신)를 통해 커스텀 데이터를 수락하는 방법을 설명합니다. 저장소/애플리케이션 설계에서 애플리케이션은 IPC 리스너(서버) 역할을 하며, 타사 통합 또는 헬퍼 프로그램이 클라이언트 역할을 하여 애플리케이션의 소켓으로 JSON 메시지를 보내야 합니다. 아래에는 연결 및 메시지 전송 방법을 보여주는 클라이언트 전용 예제(Rust/Python/Node)가 있습니다.

## IPC 사용 시기

- 네트워크 오버헤드가 불필요한 로컬 통합
- 동일 호스트의 프로세스 간 빠르고 저지연 통신
- 헬퍼 프로세스를 생성하는 테스트 하네스 및 E2E 설정

## 메시지 형식 (권장)

이식성을 위해 JSON을 사용합니다. 예제 메시지:

```json
{
  "source": "ipc",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": { "command": "write_register", "registers": [{"address":0, "value":"1234"}] }
}
```

## Unix 도메인 소켓: Rust 예제 (`interprocess` 사용)

`Cargo.toml`에 종속성을 추가합니다:

```toml
[dependencies]
interprocess = "*"
```

애플리케이션은 Unix 도메인 소켓(예: `/tmp/aoba_ipc.sock`)에서 수신합니다. 다음 Rust 예제는 클라이언트가 해당 소켓에 연결하여 단일 JSON 메시지를 전송하는 방법을 보여줍니다.

클라이언트 (연결 및 전송):

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

참고 사항:

- 애플리케이션이 소켓을 생성하고 바인딩할 것으로 예상됩니다(리스너). 클라이언트 프로그램은 동일한 경로에 바인딩을 시도하지 말고 연결만 해야 합니다.
- 테스트를 위해 양쪽을 모두 제어하는 경우 로컬에서 작은 리스너를 실행할 수 있습니다. 프로덕션에서는 애플리케이션이 소켓 경로를 제공합니다.
- Windows에서는 명명 파이프(`\\.\pipe\aoba_ipc`와 같은 경로)를 사용하거나 `interprocess`의 크로스 플랫폼 API를 사용하십시오.

## Python 예제 (AF_UNIX)

애플리케이션이 Unix 도메인 소켓을 생성하고 바인딩합니다. 다음 Python 스니펫은 클라이언트가 애플리케이션의 소켓 경로에 연결하여 JSON 메시지를 전송하는 방법을 보여줍니다.

클라이언트:

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

## Node.js (ES6) 예제 — Unix 도메인 소켓

애플리케이션이 소켓 경로에서 수신합니다. 다음 Node.js 스니펫은 클라이언트가 연결하여 JSON 메시지를 전송하는 방법을 보여줍니다.

클라이언트:

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

## 크로스 플랫폼 참고 사항

- Windows에서는 명명 파이프(`\\.\pipe\<name>`)를 사용하십시오. Node와 Python 모두 명명 파이프 작업을 위한 라이브러리가 있습니다. Rust에서는 크로스 플랫폼 파이프를 위해 `interprocess`를 사용할 수 있습니다.
- 소켓 파일 권한이 프로세스의 연결을 허용하는지 확인하십시오.
