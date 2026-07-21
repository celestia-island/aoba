# カスタムデータソース — MQTT

## クイックスタート — 小規模な CLI レシーバーの実行

アプリケーションの CLI を起動し、MQTT トピックをサブスクライブしてレシーバーとして動作させます。例（リポジトリルートから実行）:

```bash
# using cargo (recommended during development)
cargo run --bin aoba -- --master-provide-persist /tmp/vcom1 \
  --register-mode holding --register-address 0 --register-length 10 \
  --data-source mqtt://localhost:1883/aoba/data/in

# or, if you built the binary:
./target/debug/aoba --master-provide-persist /tmp/vcom1 --data-source mqtt://localhost:1883/aoba/data/in
```

`mqtt://.../<topic>` URL にはトピックパス（例: `aoba/data/in`）が含まれており、CLI はそのトピックをサブスクライブします。

## 概要

本ドキュメントでは、アプリケーションの MQTT ベースのカスタムデータソースにメッセージをパブリッシュする方法について説明します。ブローカー/接続設定、推奨トピック名、データダウンリンクを実行するための `mosquitto_pub` ペイロード例を含みます。

## ブローカー / 接続

- ホスト: `mqtt.example.com` または `localhost`
- ポート: `1883`（平文）または `8883`（TLS）
- ユーザー名/パスワード: オプション — ブローカーが認証を要求する場合は、クライアント設定で指定してください
- TLS: `8883` を使用する場合は、必要に応じて CA 証明書とクライアント証明書/キーを提供してください

## 推奨トピック

- 受信（アプリ向け）: `aoba/data/in` — アプリはここをサブスクライブして上流データやコマンドを受信します
- ダウンリンク（デバイス/vcom向け）: `aoba/data/out/<port>` — アプリは特定のポート向けに処理されたダウンリンクメッセージをパブリッシュします（例: `aoba/data/out/tmp_vcom1`）

## ペイロード形式

アプリケーションは JSON ペイロードを想定しています。正確なスキーマは柔軟ですが、以下の例はステータス更新とダウンリンクコマンドの両方に実用的な形式です。

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

## 例: mosquitto_pub を使用したダウンリンクのパブリッシュ

この例では、アプリが処理して設定された `port` に物理的な書き込みを実行する、受信トピックへのダウンリンクをパブリッシュします。

```bash
mosquitto_pub -h localhost -p 1883 -t "aoba/data/in" -u "user" -P "pass" -m '{
  "source":"mqtt",
  "timestamp":"2025-11-15T12:34:56Z",
  "port":"/tmp/vcom1",
  "type":"downlink",
  "body":{ "command":"write_register", "registers":[{"address":0,"value":"1234"}] }
}'
```

## 注記とヒント

- フィルタリングとパーミッション管理を簡素化するため、予測可能なトピック名を使用してください。
- 物理シリアルポートパス（例: `/tmp/vcom1`）を対象とする場合、トピックの解析に問題を引き起こす可能性のある文字は避けてください。設定でポート名をトピックセーフなラベルにマッピングできます。
- ブローカーが保持メッセージ（retained messages）をサポートしている場合は注意が必要です。保持されたダウンリンクメッセージは再接続時に再適用される可能性があります。

ブローカー設定のサンプルや自動テストハーネス（下りリンクのシーケンスをパブリッシュし、CLI/TUI のステータス確認を待つ小さなスクリプトなど）が必要な場合は、お好みのツールをお知らせください。追加できます。
