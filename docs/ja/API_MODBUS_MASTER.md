# Modbus Master API 使用ガイド

本ドキュメントでは、`examples/api_master` クレートを参考に、Aoba の Modbus Master API を Rust アプリケーションから利用する方法について、典型的な産業用途（生産ライン監視、プロセス制御、環境モニタリングなど）を例に説明します。

## 1. 概要

Aoba は、他の Rust アプリケーションやハードウェア制御ソフトウェアへの組み込みを想定した、トレイトベースの Modbus マスター API を提供しています。主なユースケースは以下の通りです。

- Modbus スレーブデバイスの定期ポーリング（シリアルポートまたは仮想ポート経由の RTU）
- コイル / レジスタ値の収集とテレメトリや制御ロジックへの組み込み
- フックを介した既存のロギング / 監視システムとの統合

コアのエントリポイントは `_main::api::modbus` の `ModbusBuilder` 型です。

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

> 注: 例ではクレートルートが `_main` となっています。実際のプロジェクトでは、`Cargo.toml` で指定したメイン `aoba` クレート名などに置き換えてください。

---

## 2. 基本的なマスターのライフサイクル

最小限のマスターポーリングループは以下のようになります。

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

### 主要なパラメータ

- **ポート**: Aoba がオープンできるシリアルポートまたは仮想ポート（実ポート `/dev/ttyUSB*`、`/dev/ttyS*`、または socat で作成した仮想ポート `/tmp/vcom*`）。
- **ステーション ID**: Modbus スレーブアドレス（通常 1–247）。
- **レジスタモード**: `RegisterMode::Coils`、`DiscreteInputs`、`Holding`、`Input` のいずれか。
- **レジスタアドレス / 長さ**: 開始アドレスと読み取り項目数。デバイス（PLC やセンサーゲートウェイなど）の Modbus アドレステーブルに合わせて設定します。
- **タイムアウト**: リクエストタイムアウト（ミリ秒）。

マスターは内部的にポーリングループを実行し、チャネルにレスポンスを流し込みます。コード側は `recv_timeout` を呼び出すだけで新しいデータを取得できます。

---

## 3. ロギング・監視用フックの使用

本番システム（産業ライン、プロセス機器、現場センサーなど）では、通常以下を行いたいと考えます。

- 成功したレスポンスのすべてをログに記録する
- エラーやタイムアウトを追跡する
- データをメッセージバスやデータベースにプッシュする

`ModbusHook` トレイトを使用すると、このロジックを一元管理できます。

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

複数のフックを登録できます（例: ロギング用とメトリクス書き出し用）。

---

## 4. 産業・デバイス監視のための統合パターン

典型的な産業監視シナリオ（生産ライン、プロセスユニット、環境監視デバイスなど）では、一般的なパターンは以下の通りです。

1. Aoba TUI または CLI で**ポートとステーションを設定**するか、アプリケーション内にハードコードします。
2. `ModbusBuilder::new_master` を使用して**物理/仮想ポートごとに1つのマスターを作成**します。
3. **マスターごとに Tokio タスクを起動**し、以下を行います。
   - ループ内で `recv_timeout` を呼び出す
   - `ModbusResponse::values` を工業単位（圧力、温度、バルブステータスなど）に変換する
   - 処理済みデータを監視バックエンド（MQTT、HTTP、データベースなど）に転送する
4. `ModbusHook` を使用して、ロギング、レイテンシ測定、エラーカウントを一元管理します。

Aoba は `tokio` 上に構築されているため、マスター API は非同期ランタイム内で使用されることを前提としつつ、タスク内での利便性のためシンプルなブロッキングスタイルの `recv_timeout` を提供しています。

---

## 5. エラー処理とタイムアウト

- `build_master()` はポートが開けない場合や設定が無効な場合、`anyhow::Error` を返します。
- `recv_timeout()` はタイムアウト時に `None` を返します。これ自体はエラーではありません。
- プロトコルレベルのエラー（CRC、例外コード、I/O エラー）は `ModbusHook::on_error` を通じて報告されます。

推奨パターン:

- 不安定なシリアル環境では、時折のタイムアウトは正常とみなします。
- フック内でローリングカウンターを使用し、連続エラーが閾値を超えた場合にアラームを発します。

---

## 6. 例の実行

リポジトリルートから:

```bash
cargo run --package api_master -- /tmp/vcom1
```

本番に近いテストベッド（水素貯蔵タンクベンチなど）では、通常以下のように行います。

- Aoba CLI/TUI または `examples/modbus_slave` を使用してスレーブ側をシミュレートします。
- その後、`api_master` の例を実行して、Modbus 配線とアプリケーションレベルのロジックが期待通りに動作することを確認します。

---

## 7. マニュアルモードマスター（poll_once / 書き込み操作）

ポーリングのタイミングを細かく制御する必要がある場面（ステートマシン、適応型戦略、書き込み操作など）では、`build_master_manual()` を使用してください。

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

### マニュアルモードの使いどころ

| シナリオ | 推奨モード |
|----------|-----------|
| 継続監視 / データ収集 | `build_master()`（自動） |
| Read-Modify-Write 制御ループ | `build_master_manual()` |
| ステートマシン / イベント駆動ポーリング | `build_master_manual()` |
| レスポンスレイテンシに基づく適応ポーリング | `build_master_manual()` |
| 単発診断や設定 | `build_master_manual()` |

### 書き込み操作の詳細

- **`write_holding(address, value)`** — ファンクションコード 0x06 を使用して単一のホールディングレジスタに書き込みます。個別の設定パラメータの書き込みに最適です。
- **`write_registers(address, values)`** — ファンクションコード 0x10 を使用して複数の連続ホールディングレジスタに書き込みます。バッチパラメータ書き込みに最適です。
- **`write_coils(address, values)`** — ファンクションコード 0x0F を使用して複数のコイルに書き込みます。11コイル書き込み時の自動バイトスワップが含まれます（特定ハードウェアで必要）。
- すべての書き込みメソッドは、スレーブが応答を確認するかエラーが発生するまでブロックします。

---

## 8. 次のステップ

- スレーブ側 API については、`examples/api_slave` を参照してください。
<<<<<<< HEAD
- CLI レベルの Modbus 利用については、`docs/en/CLI_MODBUS.md` を参照してください。
=======
- CLI レベルの Modbus 利用については、`docs/ja/CLI_MODBUS.md` を参照してください。
>>>>>>> origin/dev
- HTTP / MQTT / IPC 経由のデータエクスポートについては、このディレクトリ内の `DATA_SOURCE_*.md` ドキュメントを参照してください。
