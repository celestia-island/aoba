# Modbus Slave API 사용 가이드

이 문서는 Rust 애플리케이션에서 Aoba의 Modbus Slave API를 사용하여 Modbus 마스터에 데이터를 노출하는 방법을 설명합니다. 일반적인 사용 사례로는 산업 생산 라인, 공정 제어 시스템 및 테스트 벤치가 있습니다.

참조 예제는 `examples/api_slave` 크레이트입니다.

## 1. 개요

Aoba는 마스터 API 스타일을 반영하는 슬레이브 측 API를 제공하며, Builder + Hook 패턴을 기반으로 합니다. 다음과 같은 경우에 유용합니다:

- 프로세스를 Modbus 슬레이브로 전환하여 외부 마스터에 코일/레지스터 데이터를 노출
- 통합 테스트 또는 시뮬레이션을 위한 구성 가능한 Modbus 장치를 빠르게 구축
- 로깅, 통계, 접근 제어 및 알림을 위한 미들웨어 체인 연결

메인 진입점은 여전히 `_main::api::modbus::ModbusBuilder`이지만, `new_slave` / `build_slave`를 사용합니다:

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

---

## 2. 기본 슬레이브 수명 주기

간소화된 예제 슬레이브는 다음과 같습니다:

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

    // Keep the slave running and listening for master requests
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

### 핵심 구성 파라미터

- **포트(Port)**: 마스터와 동일한 형식 (`/dev/ttyUSB*`, `/dev/ttyS*`, `/tmp/vcom2` 등);
- **국번(Station ID)**: 마스터가 이 슬레이브와 통신할 때 사용할 국번과 일치해야 합니다;
- **레지스터 모드 및 주소 범위**: 이 슬레이브가 노출할 Modbus 주소 공간의 범위를 정의합니다;
- **타임아웃(Timeout)**: 내부적으로 IO/처리 타임아웃을 제어하는 데 사용됩니다 (보통 마스터 설정과 일치시킵니다).

---

## 3. 훅 미들웨어 체인

슬레이브 측에서도 여러 훅을 등록하여 미들웨어 체인을 구성할 수 있습니다. 일반적인 역할은 다음과 같습니다:

- 요청이 처리되기 전에 검증 또는 검사;
- 응답이 전송된 후 로깅 및 후처리;
- 오류 발생 시 알림 발생 또는 통계 업데이트.

`examples/api_slave` 크레이트는 세 개의 체인된 훅을 보여줍니다:

- `RequestMonitorHook`: 요청을 모니터링하고 오류 발생 시 로깅/알림;
- `ResponseLoggingHook`: 모든 응답을 레지스터 주소 및 값과 함께 로깅;
- `StatisticsHook`: 요청 수를 추적.

이 패턴을 사용하면 횡단 관심사(로깅, 메트릭, 접근 제어, 속도 제한 등)를 핵심 비즈니스 로직에서 분리하고 슬레이브 인스턴스에 선언적으로 연결할 수 있습니다.

---

## 4. 일반적인 사용 사례

산업 환경 및 테스트 환경에서 슬레이브 API의 일반적인 사용 사례는 다음과 같습니다:

1. **소프트웨어 기반 장치 시뮬레이터**
   - 실제 장치를 아직 사용할 수 없을 때 Rust에서 Modbus 장치를 시뮬레이션합니다;
   - 테스트 시나리오에 따라 내부 레지스터 값을 주기적으로 업데이트합니다;
   - CI에서 엔드투엔드 통합 테스트를 수행합니다.
2. **프로토콜 적응 레이어**
   - 실제 장치는 CAN, 독점 TCP 또는 다른 필드버스를 사용할 수 있지만, 상위 시스템은 Modbus를 기대합니다;
   - 슬레이브 API를 사용하여 해당 신호를 Modbus 레지스터/코일 공간에 매핑하고 통합 Modbus 인터페이스를 제공합니다.
3. **처리된 데이터를 노출하는 엣지 게이트웨이**
   - 프로세스나 게이트웨이 내부의 여러 소스에서 데이터를 수집하고 정규화합니다;
   - 슬레이브 API를 사용하여 처리/집계된 데이터를 레거시 SCADA 또는 타사 시스템에 Modbus를 통해 노출합니다.

---

## 5. 마스터 및 슬레이브 API 함께 사용하기

마스터와 슬레이브 API는 동일한 Builder + Hook 설계를 공유하므로, 단일 프로세스 내에서 쉽게 결합할 수 있습니다:

1. 마스터 API를 사용하여 여러 상위 장치를 폴링하고 통합 내부 데이터 모델을 구축합니다;
2. 슬레이브 API를 사용하여 해당 데이터 모델을 Modbus 레지스터 공간에 매핑합니다;
3. 외부 시스템이 프로세스를 표준 Modbus 장치로 취급하도록 합니다.

이 패턴은 프로토콜 게이트웨이, 집계 노드 또는 테스트 하네스를 구축하는 데 유용합니다.

---

## 6. 슬레이브 예제 실행

저장소 루트에서:

```bash
cargo run --package api_slave -- /tmp/vcom2
```

마스터 예제 또는 Aoba CLI/TUI와 함께 테스트할 수 있습니다:

- `/tmp/vcom2`에서 수신하도록 슬레이브 예제를 시작합니다;
- 그런 다음 마스터 예제 또는 CLI/TUI를 사용하여 해당 포트를 폴링하고 읽기/쓰기 동작을 확인합니다.

---

## 7. 관련 문서

- 마스터 측 API: `docs/ko/API_MODBUS_MASTER.md`;
- CLI 수준 Modbus 사용법: `docs/ko/CLI_MODBUS.md`;
- 데이터 소스 / 내보내기 기능 (HTTP, MQTT, IPC 등): 이 디렉토리의 `DATA_SOURCE_*.md` 문서를 참조하십시오;
- 더 많은 엔드투엔드 예제는 `examples` 디렉토리에 있습니다.
