# Modbus Master API 사용 가이드

이 문서는 `examples/api_master` 크레이트를 참조하여, 일반적인 산업 시나리오(생산 라인 모니터링, 공정 제어, 환경 모니터링 등)에서 Rust 애플리케이션 내에서 Aoba의 Modbus Master API를 사용하는 방법을 설명합니다.

## 1. 개요

Aoba는 다른 Rust 애플리케이션이나 하드웨어 제어 소프트웨어에 내장할 수 있도록 트레이트(trait) 기반의 Modbus 마스터 API를 제공합니다. 일반적인 사용 사례는 다음과 같습니다:

- Modbus 슬레이브 장치의 주기적 폴링 (시리얼 또는 가상 포트를 통한 RTU)
- 코일 / 레지스터 값을 자체 원격 측정 또는 제어 로직으로 수집
- 훅(hook)을 통해 기존 로깅 / 모니터링 시스템과 통합

핵심 진입점은 `_main::api::modbus`의 `ModbusBuilder` 타입입니다.

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

> 참고: 예제에서는 크레이트 루트를 `_main`이라고 부릅니다. 실제 프로젝트에서는 보통 `Cargo.toml`에서 지정한 메인 `aoba` 크레이트 또는 원하는 이름이 됩니다.

---

## 2. 기본 마스터 수명 주기

최소한의 마스터 폴링 루프는 다음과 같습니다:

```rust
use anyhow::Result;
use std::time::Duration;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    let master = ModbusBuilder::new_master(1) // station id of the slave
        .with_port("/dev/ttyUSB0")          // or `/tmp/vcom1` etc.
        .with_register(RegisterMode::Holding, 0, 10)
        .with_timeout(1000)                  // milliseconds
        .build_master()?;

    loop {
        if let Some(resp) = master.recv_timeout(Duration::from_secs(1)) {
            println!("values = {:04X?}", resp.values);
        }
    }
}
```

### 주요 파라미터

- **포트(Port)**: Aoba가 열 수 있는 시리얼 또는 가상 포트 (실제 `/dev/ttyUSB*`, `/dev/ttyS*` 또는 socat으로 생성한 가상 `/tmp/vcom*`).
- **국번(Station ID)**: Modbus 슬레이브 주소 (보통 1–247).
- **레지스터 모드(Register mode)**: `RegisterMode::Coils`, `DiscreteInputs`, `Holding`, `Input` 중 하나.
- **레지스터 주소 / 길이**: 시작 주소와 읽을 항목 수로, 장치(예: PLC 또는 센서 게이트웨이)의 Modbus 주소 테이블과 일치해야 합니다.
- **타임아웃(Timeout)**: 요청 타임아웃(밀리초).

마스터는 내부적으로 폴링 루프를 실행하고 채널을 통해 응답을 전달합니다. 사용자 코드는 단순히 `recv_timeout`을 호출하여 새 데이터를 가져옵니다.

---

## 3. 로깅 및 모니터링을 위한 훅 사용

프로덕션 시스템(산업 라인, 공정 장비, 현장 센서 등)에서는 보통 다음이 필요합니다:

- 모든 성공적인 응답 로깅
- 오류 및 타임아웃 추적
- 데이터를 메시지 버스나 데이터베이스로 전송

`ModbusHook` 트레이트를 사용하면 이 로직을 중앙에서 연결할 수 있습니다.

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

    // now poll with recv_timeout as in the basic example
    # let _ = master;
    Ok(())
}
```

여러 훅을 등록할 수 있습니다 (예: 로깅용 하나, 메트릭 내보내기용 하나).

---

## 4. 산업 / 장치 모니터링 통합 패턴

일반적인 산업 모니터링 시나리오(생산 라인, 공정 유닛, 환경 모니터링 장치 등)에서의 일반적인 패턴은 다음과 같습니다:

1. Aoba TUI 또는 CLI를 통해 **포트와 국번을 구성**하거나, 애플리케이션에 하드코딩합니다.
2. `ModbusBuilder::new_master`를 사용하여 **물리적/가상 포트당 하나의 마스터를 생성**합니다.
3. **마스터당 하나의 Tokio 태스크를 생성**하여:
   - 루프 내에서 `recv_timeout`을 호출
   - `ModbusResponse::values`를 공학 단위(압력, 온도, 밸브 상태 등)로 파싱
   - 처리된 데이터를 모니터링 백엔드(MQTT, HTTP, 데이터베이스 등)로 전달
4. `ModbusHook`을 사용하여 로깅, 지연 시간 측정, 오류 카운팅을 중앙화합니다.

Aoba는 `tokio` 기반으로 구축되었으므로, 마스터 API는 비동기 런타임 내에서 사용하도록 설계되었지만 편의상 간단한 블로킹 스타일의 `recv_timeout`을 제공합니다.

---

## 5. 오류 처리 및 타임아웃

- `build_master()`는 포트를 열 수 없거나 구성이 유효하지 않은 경우 `anyhow::Error`를 반환합니다.
- `recv_timeout()`은 타임아웃 시 `None`을 반환하며, 이 자체로는 오류가 아닙니다.
- 프로토콜 수준의 오류(CRC, 예외 코드, IO 오류)는 `ModbusHook::on_error`를 통해 보고됩니다.

권장 패턴:

- 불안정한 시리얼 환경에서는 간헐적인 타임아웃을 정상으로 간주합니다.
- 훅에 롤링 카운터를 사용하고, 연속 오류가 임계값을 초과하면 알람을 발생시킵니다.

---

## 6. 예제 실행

저장소 루트에서:

```bash
cargo run --package api_master -- /tmp/vcom1
```

프로덕션 유사 테스트베드(예: 수소 저장 탱크 벤치)에서는 일반적으로:

- Aoba CLI/TUI 또는 `examples/modbus_slave`를 사용하여 슬레이브 측을 시뮬레이션합니다.
- 그런 다음 `api_master` 예제를 실행하여 Modbus 배선 및 애플리케이션 수준 로직이 예상대로 작동하는지 확인합니다.

---

## 7. 수동 모드 마스터 (poll_once / 쓰기 작업)

폴링 타이밍에 대한 세밀한 제어가 필요한 시나리오(상태 기계, 적응형 전략 또는 쓰기 작업)에서는 `build_master_manual()`을 사용합니다:

```rust
use anyhow::Result;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    let master = ModbusBuilder::new_master(1)
        .with_port("/dev/ttyUSB0")
        .with_baud_rate(9600)
        .with_timeout(5000)
        .build_master_manual()?;

    // Manual single-shot poll
    let response = master.poll_once(RegisterMode::Holding, 0x00, 10)?;
    println!("Values: {:?}", response.values);

    // Write a single holding register (fc 0x06)
    master.write_holding(0x00, 0x1234)?;

    // Write multiple holding registers (fc 0x10)
    master.write_registers(0x00, &[0x1234, 0x5678, 0x9ABC])?;

    // Write coils (fc 0x0F)
    master.write_coils(0x00, &[true, false, true, true])?;

    Ok(())
}
```

### 수동 모드 사용 시기

| 시나리오 | 권장 모드 |
|----------|----------|
| 연속 모니터링 / 데이터 수집 | `build_master()` (자동) |
| 읽기-수정-쓰기 제어 루프 | `build_master_manual()` |
| 상태 기계 / 이벤트 기반 폴링 | `build_master_manual()` |
| 응답 지연 시간에 따른 적응형 폴링 | `build_master_manual()` |
| 일회성 진단 또는 구성 | `build_master_manual()` |

### 쓰기 작업 상세

- **`write_holding(address, value)`** — 함수 코드 0x06을 사용하여 단일 홀딩 레지스터에 씁니다. 개별 구성 파라미터 작성에 적합합니다.
- **`write_registers(address, values)`** — 함수 코드 0x10을 사용하여 여러 연속 홀딩 레지스터에 씁니다. 배치 파라미터 작성에 적합합니다.
- **`write_coils(address, values)`** — 함수 코드 0x0F를 사용하여 여러 코일에 씁니다. 특정 하드웨어에서 요구하는 11코일 쓰기 시 자동 바이트 스왑이 포함됩니다.
- 모든 쓰기 메서드는 슬레이브가 응답하거나 오류가 발생할 때까지 블로킹됩니다.

---

## 8. 다음 단계

- 슬레이브 측 API는 `examples/api_slave`를 참조하십시오.
<<<<<<< HEAD
- CLI 수준의 Modbus 사용법은 `docs/en/CLI_MODBUS.md`를 참조하십시오.
=======
- CLI 수준의 Modbus 사용법은 `docs/ko/CLI_MODBUS.md`를 참조하십시오.
>>>>>>> origin/dev
- HTTP / MQTT / IPC를 통한 데이터 내보내기는 이 디렉토리의 `DATA_SOURCE_*.md` 문서를 참조하십시오.
