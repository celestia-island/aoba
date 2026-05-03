# CLI Modbus 기능

이 문서는 aoba 프로젝트에 추가된 Modbus 작업용 CLI 기능을 설명합니다.

## 기능

### 1. 포트 감지 및 목록

#### 모든 포트 목록

`--list-ports` 명령은 `--json`과 함께 사용할 때 더 상세한 정보를 제공합니다:

```bash
aoba --list-ports --json
```

출력에는 다음이 포함됩니다:

- `path`: 포트 경로 (예: COM1, /dev/ttyUSB0)
- `status`: "Free" 또는 "Occupied"
- `guid`: Windows 장치 GUID (사용 가능한 경우)
- `vid`: USB Vendor ID (사용 가능한 경우)
- `pid`: USB Product ID (사용 가능한 경우)
- `serial`: 일련 번호 (사용 가능한 경우)

예제 출력:

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

#### 단일 포트 점유 상태 확인

`--check-port` 명령은 특정 포트가 점유되어 있는지 감지하는 데 사용됩니다. 스크립트 자동화 및 포트 상태 모니터링에 유용합니다:

```bash
aoba --check-port COM3
```

**종료 코드:**

- `0` - 포트가 비어 있고 사용 가능
- `1` - 포트가 다른 프로그램에 의해 점유됨

**일반 출력:**

```
Port COM3 is free
```

또는

```
Port COM3 is occupied
```

**JSON 형식 출력:**

```bash
aoba --check-port COM3 --json
```

예제 출력:

```json
{"port":"COM3","occupied":false,"status":"Free"}
```

또는

```json
{"port":"COM3","occupied":true,"status":"Occupied"}
```

**사용 예제:**

셸 스크립트에서 사용:

```bash
# Bash 예제
if aoba --check-port /dev/ttyUSB0; then
    echo "Port is free, ready to use"
    # Perform your operations
else
    echo "Port is occupied, please close the program using this port"
    exit 1
fi
```

```powershell
# PowerShell 예제
cargo run --package aoba -- --check-port COM3
if ($LASTEXITCODE -eq 0) {
    Write-Host "Port is free"
} else {
    Write-Host "Port is occupied"
}
```

### 2. 슬레이브 수신 모드

#### 임시 모드

하나의 Modbus 요청을 수신하고 응답한 후 종료합니다:

```bash
aoba --slave-listen /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

단일 JSON 응답을 출력하고 종료합니다.

#### 영구 모드

지속적으로 요청을 수신하고 JSONL을 출력합니다:

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 10 \
  --register-mode holding \
  --baud-rate 9600
```

처리된 요청당 한 줄의 JSON을 출력합니다.

### 3. 마스터 제공 모드

- 임시 모드, 데이터를 한 번 제공하고 종료:

```bash
aoba --master-provide /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

데이터 소스에서 한 줄을 읽어 전송하고 종료합니다.

- 영구 모드, 지속적으로 데이터 제공:

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

데이터 소스에서 줄을 읽어 지속적으로 전송합니다.

### 데이터 소스 형식

마스터 모드의 경우 데이터 소스 파일은 JSONL 형식이어야 합니다:

```json
{"values": [10, 20, 30, 40, 50]}
{"values": [15, 25, 35, 45, 55]}
{"values": [20, 30, 40, 50, 60]}
```

각 줄은 슬레이브에 전송할 업데이트를 나타냅니다.

#### 파일을 데이터 소스로 사용

```bash
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source file:/path/to/data.json \
  --baud-rate 9600
```

#### Unix 명명 파이프를 데이터 소스로 사용

Unix 명명 파이프(FIFO)를 사용하여 실시간 데이터 스트리밍이 가능합니다:

```bash
# 명명 파이프 생성
mkfifo /tmp/modbus_input

# 한 터미널에서 마스터 시작
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_input \
  --baud-rate 9600

# 다른 터미널에서 데이터 쓰기
echo '{"values": [10, 20, 30, 40, 50]}' > /tmp/modbus_input
```

### 출력 대상

슬레이브 모드의 경우 출력 대상을 지정할 수 있습니다:

#### stdout 출력 (기본값)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600
```

#### 파일 출력 (추가 모드)

```bash
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output file:/path/to/output.jsonl
```

#### Unix 명명 파이프 출력

```bash
# 명명 파이프 생성
mkfifo /tmp/modbus_output

# 한 터미널에서 슬레이브 시작
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-address 0 \
  --register-length 5 \
  --register-mode holding \
  --baud-rate 9600 \
  --output pipe:/tmp/modbus_output

# 다른 터미널에서 데이터 읽기
cat /tmp/modbus_output
```

## 데몬 모드 (영구 작동)

CLI는 **영구 모드**를 통해 데몬과 같은 연속 작동을 지원합니다:

- **슬레이브 데몬**: 지속적인 수신 및 응답을 위해 `--slave-listen-persist`를 사용합니다
- **마스터 데몬**: 지속적인 데이터 제공을 위해 `--master-provide-persist`를 사용합니다

이 모드들은 중단될 때까지(Ctrl+C) 무기한 실행되며, 각 작업에 대해 JSONL(줄당 하나의 JSON 객체)을 출력합니다. 다음에 적합합니다:

- 장기 실행 모니터링 애플리케이션
- 데이터 로깅 시스템
- 파이프나 파일을 통한 다른 도구와의 통합
- TUI 서브프로세스 통신 (`--ipc-channel`과 결합 시)

데몬 사용 예제:

```bash
# 파일 출력 로깅과 함께 슬레이브 데몬으로 실행
aoba --slave-listen-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --output file:/var/log/modbus-slave.jsonl

# 파이프 입력과 함께 마스터 데몬으로 실행
aoba --master-provide-persist /dev/ttyUSB0 \
  --station-id 1 \
  --register-mode holding \
  --data-source pipe:/tmp/modbus_data
```

**참고**: TUI 모드는 CLI 서브프로세스와의 양방향 통신을 위해 내부적으로 `--ipc-channel`과 함께 이 영구 모드들을 사용합니다.

## 파라미터

| 파라미터 | 설명 | 기본값 |
|-----------|------|--------|
| `--station-id` | Modbus 국번 (슬레이브 주소) | 1 |
| `--register-address` | 시작 레지스터 주소 | 0 |
| `--register-length` | 레지스터 수 | 10 |
| `--register-mode` | 레지스터 유형: holding, input, coils, discrete | holding |
| `--data-source` | 데이터 소스: `file:<path>` 또는 `pipe:<name>` | - |
| `--output` | 출력 대상: `file:<path>` 또는 `pipe:<name>` (기본값: stdout) | stdout |
| `--baud-rate` | 시리얼 포트 보드레이트 | 9600 |
| `--debounce-seconds` | 중복 JSON 출력에 대한 디바운스 윈도우 (초, 부동소수점) | 1.0 |
| `--ipc-channel` | TUI 통신용 IPC 채널 UUID (내부 사용) | - |

## 레지스터 모드

- `holding`: 홀딩 레지스터 (읽기/쓰기)
- `input`: 입력 레지스터 (읽기 전용)
- `coils`: 코일 (읽기/쓰기 비트)
- `discrete`: 개별 입력 (읽기 전용 비트)

## 통합 테스트

통합 테스트는 `examples/cli_e2e/`에서 사용할 수 있습니다. 다음 명령으로 실행합니다:

```bash
cd examples/cli_e2e
cargo run
```

### 루프 모드에서 테스트 실행

안정성 테스트 및 디버깅을 위해 `--loop-count` 명령줄 인수를 사용하여 테스트를 여러 번 실행할 수 있습니다:

```bash
# 테스트를 5회 연속 실행
cargo run --example cli_e2e -- --loop-count 5

# 포트 정리 및 안정성 확인을 위해 10회 실행
cargo run --example cli_e2e -- --loop-count 10
```

다음에 유용합니다:

- 테스트 실행 간 포트 정리 확인
- 안정성 및 재현성 테스트
- 간헐적 문제 디버깅
- socat 가상 포트 재설정이 올바르게 작동하는지 확인

테스트가 확인하는 항목:

- 상태가 포함된 향상된 포트 목록
- 슬레이브 수신 임시 모드
- 슬레이브 수신 영구 모드
- 마스터 제공 임시 모드
- 마스터 제공 영구 모드
- 연속 연결 테스트 (파일 데이터 소스 및 파일 출력)
- 연속 연결 테스트 (Unix 파이프 데이터 소스 및 파이프 출력)

### 연속 연결 테스트

연속 연결 테스트는 마스터와 슬레이브 간의 장기 실행 데이터 전송을 확인합니다:

1. **파일을 데이터 소스 및 출력으로 사용**: 마스터가 파일에서 데이터를 읽어 전송하고, 슬레이브가 수신하여 파일에 추가합니다
2. **Unix 파이프를 데이터 소스 및 출력으로 사용**: 마스터가 명명 파이프에서 실시간 데이터를 읽고, 슬레이브가 명명 파이프로 출력합니다
3. **랜덤 데이터 생성**: 각 테스트 실행 시 다른 랜덤 데이터를 생성하여 테스트 신뢰성을 보장합니다

## 향후 개선 사항

- 가상 시리얼 포트를 이용한 실시간 Modbus 통신 테스트
- 추가 레지스터 모드 지원
