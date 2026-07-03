# 커스텀 데이터 소스 — HTTP

## 빠른 시작 — 간단한 CLI 수신기 실행

수신기 역할을 하는 모드로 애플리케이션의 CLI를 시작합니다 (CLI가 HTTP 엔드포인트를 호스팅하고 POST된 JSON을 적용합니다). 예제 (저장소 루트에서 실행):

```bash
# cargo 사용 (개발 중 권장)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source http://8080

# 또는 빌드된 바이너리가 있는 경우:
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source http://8080
```

위 명령은 `127.0.0.1:8080`에 바인딩된 HTTP 서버를 시작하고 `/` (루트) 경로로의 `POST` 요청을 수락합니다. 아래의 `curl` 예제를 사용하여 데이터를 POST할 수 있습니다.

## 개요

이 문서는 애플리케이션에서 사용하는 HTTP 커스텀 데이터 소스를 설명합니다. 예상되는 요청 형식, 공통 헤더 및 통합을 빠르게 검증할 수 있는 간단한 `curl` 예제를 보여줍니다.

## 엔드포인트

- 메서드: `POST`
- URL: `http://<host>:<port>/` (예: `http://localhost:8080/`)
- Content-Type: `application/json`

## 요청 형식

서비스는 JSON 본문을 수락합니다. 최소한의 예제 페이로드는 다음과 같습니다:

```json
{
  "source": "http",
  "timestamp": "2025-11-15T12:34:56Z",
  "port": "/tmp/vcom1",
  "payload": {
    "type": "register_update",
    "registers": [
      {"address": 0, "value": "1234"},
      {"address": 1, "value": "abcd"}
    ]
  }
}
```

참고 사항:

- 가능하면 `timestamp`에 ISO 8601을 사용하십시오.
- `payload` 내용은 애플리케이션별로 다릅니다. 위 예제는 일반적인 레지스터 스타일 업데이트를 보여줍니다.

## curl 테스트 예제

`<host>`와 `<port>`를 실행 중인 서버로 교체하십시오. 이 `curl` 명령은 위의 JSON 페이로드를 전송합니다:

```bash
curl -v -X POST "http://localhost:8080/" \
  -H "Content-Type: application/json" \
  -d '{
    "source":"http",
    "timestamp":"2025-11-15T12:34:56Z",
    "port":"/tmp/vcom1",
    "payload":{
      "type":"register_update",
      "registers":[{"address":0,"value":"1234"}]
    }
  }'
```

## 예상 동작

- 수락/대기열에 들어간 메시지에 대해 HTTP `200 OK` (또는 `202 Accepted`) 응답.
- 서버가 오류(4xx/5xx)를 반환하는 경우, 응답 본문에서 세부 정보를 확인하십시오.

## 팁 및 문제 해결

- `Content-Type: application/json` 헤더가 포함되어 있는지 확인하십시오.
- 서버에 인증이 필요한 경우 적절한 `Authorization` 헤더를 추가하십시오 (예: `Bearer <token>`).
- 대용량 페이로드의 경우 `--data-binary`로 테스트하고 서버 타임아웃을 늘리는 것을 고려하십시오.

내부 스키마에 맞는 맞춤형 예제가 필요한 경우, 샘플 JSON을 제공하면 개발자가 엔드포인트 핸들러를 그에 맞게 조정할 것입니다.
내부 스키마에 맞는 맞춤형 예제가 필요한 경우, 샘플 JSON을 제공하면 개발자가 엔드포인트 핸들러를 그에 맞게 조정할 것입니다.
