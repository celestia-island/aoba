# Modbus Slave API 使用ガイド

本ドキュメントでは、Aoba の Modbus Slave API を使用して、Rust アプリケーションから Modbus マスターにデータを公開する方法について説明します。典型的なユースケースは、産業生産ライン、プロセス制御システム、テストベンチなどです。

参考例は `examples/api_slave` クレートです。

## 1. 概要

Aoba はマスター API と同様のスタイルに基づく、Builder + Hook パターンのスレーブ側 API を提供しています。以下のような用途に便利です。

- プロセスを Modbus スレーブに変換し、外部マスターにコイル/レジスタデータを公開する
- 統合テストやシミュレーション用に設定可能な Modbus デバイスを迅速に構築する
- ロギング、統計、アクセス制御、アラート用のフックミドルウェアチェーンを接続する

メインのエントリポイントは `_main::api::modbus::ModbusBuilder` のままですが、`new_slave` / `build_slave` を使用します。

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

---

## 2. 基本的なスレーブのライフサイクル

例のスレーブを簡略化したバージョンは以下のようになります。

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

### 主要な設定パラメータ

- **ポート**: マスターと同じ形式（`/dev/ttyUSB*`、`/dev/ttyS*`、`/tmp/vcom2` など）。
- **ステーション ID**: マスターがこのスレーブと通信する際に使用するステーション ID と一致させる必要があります。
- **レジスタモードとアドレス範囲**: このスレーブが公開する Modbus アドレス空間の範囲を定義します。
- **タイムアウト**: 内部的な I/O / 処理タイムアウトの制御に使用されます（通常はマスター設定と合わせます）。

---

## 3. フックミドルウェアチェーン

スレーブ側でも複数のフックを登録してミドルウェアチェーンを構成できます。主な役割は以下の通りです。

- リクエストが処理される前に検証や検査を行う
- レスポンス送信後のログ記録と後処理
- エラー発生時のアラート発報や統計の更新

`examples/api_slave` クレートでは、3つのチェーンされたフックがデモンストレーションされています。

- `RequestMonitorHook`: リクエストを監視し、エラー時にログ/アラートを出します。
- `ResponseLoggingHook`: レジスタアドレスと値を含むすべてのレスポンスをログに記録します。
- `StatisticsHook`: リクエスト数を追跡します。

このパターンにより、横断的関心事（ロギング、メトリクス、アクセス制御、レート制限など）をコアビジネスロジックから分離し、スレーブインスタンスに宣言的にアタッチできます。

---

## 4. 典型的なユースケース

産業環境やテストセットアップにおけるスレーブ API の一般的なユースケースは以下の通りです。

1. **ソフトウェアベースのデバイスシミュレータ**
   - 実デバイスがまだ利用できない場合、Rust で Modbus デバイスをシミュレートします。
   - テストシナリオに従って内部レジスタ値を定期的に更新します。
   - CI でのエンドツーエンド統合テストを実行します。
2. **プロトコル適応レイヤ**
   - 実際のデバイスは CAN、独自 TCP、または他のフィールドバスを使用し、上位システムは Modbus を期待する場合があります。
   - スレーブ API を使用して、それらの信号を Modbus レジスタ/コイル空間にマッピングし、統一的な Modbus インターフェースを提供します。
3. **処理済みデータを公開するエッジゲートウェイ**
   - プロセスやゲートウェイ内で複数ソースからのデータを収集・正規化します。
   - スレーブ API を使用して、処理/集計されたデータをレガシー SCADA やサードパーティシステムに Modbus 経由で公開します。

---

## 5. マスター API とスレーブ API の併用

マスター API とスレーブ API は同じ Builder + Hook 設計を共有しているため、単一プロセス内で簡単に組み合わせることができます。

1. マスター API を使用して複数の上位デバイスをポーリングし、統一的な内部データモデルを構築します。
2. スレーブ API を使用して、そのデータモデルを Modbus レジスタ空間にマッピングします。
3. 外部システムは、プロセスを標準的な Modbus デバイスとして扱えるようになります。

このパターンは、プロトコルゲートウェイ、集約ノード、テストハーネスの構築に有用です。

---

## 6. スレーブ例の実行

リポジトリルートから:

```bash
cargo run --package api_slave -- /tmp/vcom2
```

マスター例や Aoba CLI/TUI と組み合わせてテストできます。

- スレーブ例を `/tmp/vcom2` でリッスンを開始します。
- その後、マスター例または CLI/TUI を使用してそのポートをポーリングし、読み取り/書き込み動作を確認します。

---

## 7. 関連ドキュメント

- マスター側 API: `docs/ja/API_MODBUS_MASTER.md`
- CLI レベルの Modbus 利用: `docs/ja/CLI_MODBUS.md`
- データソース / エクスポート機能（HTTP、MQTT、IPC など）: このディレクトリ内の `DATA_SOURCE_*.md` ドキュメントを参照してください。
- その他のエンドツーエンドの例は `examples` ディレクトリにあります。
