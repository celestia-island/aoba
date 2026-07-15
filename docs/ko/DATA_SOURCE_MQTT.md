# 커스텀 데이터 소스 — MQTT

## 빠른 시작 — 간단한 CLI 수신기 실행

애플리케이션의 CLI를 시작하여 MQTT 토픽을 구독하고 수신기 역할을 하도록 합니다. 예제 (저장소 루트에서 실행):

```bash
# cargo 사용 (개발 중 권장)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source mqtt://localhost:1883/aoba/data/in

# 또는 빌드된 바이너리가 있는 경우:
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source mqtt://localhost:1883/aoba/data/in
```

`mqtt://.../<topic>` URL에 토픽 경로(예: `aoba/data/in`)가 포함되어 있으며, CLI가 해당 토픽을 구독합니다.

## 개요

이 문서는 애플리케이션의 MQTT 기반 커스텀 데이터 소스에 메시지를 게시하는 방법을 설명합니다. 브로커/연결 구성, 권장 토픽 이름 및 데이터 다운링크를 수행하기 위한 `mosquitto_pub` 페이로드 예제가 포함되어 있습니다.

## 브로커 / 연결

- 호스트: `mqtt.example.com` 또는 `localhost`
- 포트: `1883` (평문) 또는 `8883` (TLS)
- 사용자 이름/비밀번호: 선택 사항 — 브로커에 인증이 필요한 경우 클라이언트 구성에서 제공하십시오
- TLS: `8883`을 사용하는 경우 필요에 따라 CA 인증서 및 클라이언트 인증서/키를 제공하십시오

## 권장 토픽

- 인바운드 (앱으로): `aoba/data/in` — 앱이 여기서 구독하여 업스트림 데이터 또는 명령을 수신합니다
- 다운링크 (장치/vcom으로): `aoba/data/out/<port>` — 앱이 특정 포트를 대상으로 처리된 다운링크 메시지를 게시합니다 (예: `aoba/data/out/tmp_vcom1`)

## 페이로드 형식

애플리케이션은 JSON 페이로드를 기대합니다. 정확한 스키마는 유연하지만, 다음 예제는 상태 업데이트 및 다운링크 명령 모두에 실용적인 형태입니다:

```json
{
  "source": "mqtt",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "type": "downlink",
  "body": {
    "command": "write_register",
    "registers": [{"address":0, "value": "1234"}]
  }
}
```

## 예제: mosquitto_pub을 사용한 다운링크 게시

이 예제는 앱이 처리하고 구성된 `port`에 물리적 쓰기를 수행할 인바운드 토픽으로 다운링크를 게시합니다.

```bash
mosquitto_pub -h localhost -p 1883 -t "aoba/data/in" -u "user" -P "pass" -m '{
  "source":"mqtt",
  "timestamp":"2025-11-15T12:34:56Z",
  "port":"/tmp/vcom1",
  "type":"downlink",
  "body":{ "command":"write_register", "registers":[{"address":0,"value":"1234"}] }
}'
```

## 참고 사항 및 팁

- 필터링 및 권한 관리를 단순화하기 위해 예측 가능한 토픽 이름을 사용하십시오.
- 물리적 시리얼 포트 경로(예: `/tmp/vcom1`)를 대상으로 지정할 때 토픽 파싱 문제를 일으킬 수 있는 문자를 피하십시오. 구성에서 포트 이름을 토픽 안전 레이블로 매핑할 수 있습니다.
- 브로커가 보유 메시지(retained messages)를 지원하는 경우 주의하십시오: 보유된 다운링크 메시지는 재연결 시 다시 적용될 수 있습니다.
<<<<<<< HEAD
=======

브로커 구성 샘플이나 자동화된 테스트 하네스(다운링크 시퀀스를 게시하고 CLI/TUI 상태 확인을 기다리는 작은 스크립트 등)가 필요하시면 선호하는 도구를 알려주시면 추가해 드리겠습니다.
>>>>>>> origin/dev
