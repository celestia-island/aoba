# カスタムデータソース — HTTP

## クイックスタート — 小規模な CLI レシーバーの実行

アプリケーションの CLI をレシーバーとして動作するモードで起動します（CLI が HTTP エンドポイントをホストし、POST された JSON を適用します）。例（リポジトリルートから実行）:

```bash
# using cargo (recommended during development)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source http://8080

# or, if you built the binary:
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source http://8080
```

上記のコマンドは `127.0.0.1:8080` にバインドされた HTTP サーバーを起動し、`/`（ルート）への `POST` リクエストを受け付けます。下記の `curl` 例を使用してデータを POST してください。

## 概要

本ドキュメントでは、アプリケーションが使用する HTTP カスタムデータソースについて説明します。期待されるリクエスト形式、一般的なヘッダー、統合を素早く検証するためのシンプルな `curl` 例を示します。

## エンドポイント

- メソッド: `POST`
- URL: `http://<host>:<port>/`（例: `http://localhost:8080/`）
- Content-Type: `application/json`

## リクエスト形式

サービスは JSON ボディを受け付けます。最小限のペイロード例は以下の通りです。

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

注:

- `timestamp` には可能な限り ISO 8601 を使用してください。
- `payload` の内容はアプリケーション固有です。上記の例は一般的なレジスタスタイルの更新を示しています。

## curl テスト例

`<host>` と `<port>` を稼働中のサーバーに置き換えてください。この `curl` コマンドは上記の JSON ペイロードを送信します。

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

## 期待される動作

- 受け入れられた/キューに入れられたメッセージに対しては HTTP `200 OK`（または `202 Accepted`）が返されます。
- サーバーがエラー（4xx/5xx）を返した場合、レスポンスボディで詳細を確認してください。

## ヒントとトラブルシューティング

- `Content-Type: application/json` ヘッダーが存在することを確認してください。
- サーバーが認証を必要とする場合は、適切な `Authorization` ヘッダー（例: `Bearer <token>`）を追加してください。
- 大きなペイロードの場合は、`--data-binary` でのテストやサーバー側のタイムアウト増加を検討してください。

内部スキーマに合わせたカスタマイズ例が必要な場合は、サンプル JSON を提示してください。開発者がエンドポイントハンドラーを適宜調整します。
内部スキーマに合わせたカスタマイズ例が必要な場合は、サンプル JSON を提示してください。開発者がエンドポイントハンドラーを適宜調整します。
