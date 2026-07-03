# Modbus 從站 API 使用指南

本文件介紹如何在 Rust 程式中透過 Aoba 提供的 Modbus 從站（Slave） API 暴露現場設備或仿真設備的資料，例如在工業產線、程序控制或測試平台中作為被採集端使用。

範例程式碼參考 `examples/api_slave` crate。

## 1. 總體概覽

Aoba 提供了與主站 API 風格一致的從站 API，透過 Builder + Hook 的形式，便於：

- 將你的程序包裝成一個 Modbus 從站，對外暴露暫存器/線圈資料；
- 在測試/仿真環境中快速搭建一個可配置的 Modbus 設備；
- 透過 Hook 中介軟體鏈路統一處理日誌、統計、告警等邏輯。

核心入口型別同樣是 `_main::api::modbus::ModbusBuilder`，但使用 `new_slave` / `build_slave`：

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

---

## 2. 從站基本生命週期

範例中的從站程式大致結構如下：

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

    // 從站啟動後會持續監聽主站請求
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

### 核心配置項

- **串列埠名稱**：與主站一致，例如 `/dev/ttyUSB0`、`/dev/ttyS1`、`/tmp/vcom2` 等；
- **站點 ID（Station ID）**：需要與主站存取的目標站點一致；
- **暫存器模式與位址區間**：決定從站對外暴露的暫存器/線圈範圍；
- **逾時時間**：從站內部使用，用於控制內部 IO/處理過程的逾時（通常保持與主站設定一致即可）。

---

## 3. Hook 中介軟體鏈路

從站端同樣可以註冊多個 Hook 形成中介軟體鏈路，用於：

- 在請求到達前檢查狀態（例如是否允許當前時段存取）；
- 在回應傳送後記錄日誌、做統計；
- 在出現錯誤時統一告警。

`examples/api_slave` 中的範例展示了 3 個 Hook 串聯的模式：

- `RequestMonitorHook`：監控請求並在出錯時告警；
- `ResponseLoggingHook`：詳細記錄每次回應的暫存器位址和值；
- `StatisticsHook`：統計請求次數等指標。

這種「中介軟體鏈路」的好處是，可以把與業務無關的通用邏輯（日誌、統計、限流、權限校驗等）抽離出來，按需組合掛載在從站實例上。

---

## 4. 典型使用場景

在工業現場或測試平台中，從站 API 典型用法包括：

1. **軟從站仿真設備**：
   - 在沒有真實設備時，用 Rust 程式模擬一個 Modbus 設備；
   - 根據測試需要定期更新內部暫存器值，供上位主站輪詢；
   - 便於在 CI 中做自動化聯調測試。
2. **為其他協議/匯流排提供 Modbus 適配層**：
   - 例如底層是 CAN、乙太網路或自定義協議，上層系統要求透過 Modbus 存取；
   - 可以用從站 API 將這些資料映射到 Modbus 暫存器空間，對外暴露統一介面。
3. **生產線/裝置的邊緣閘道器**：
   - 將現場設備資料先接入你的閘道器程序；
   - 再透過 Modbus 從站 API 暴露給歷史系統或第三方上位機。

---

## 5. 與主站 API 的配合

主站與從站 API 在 Builder 和 Hook 設計上保持一致，方便你在同一程序中：

- 同時作為若干上游設備的主站；
- 又作為其他系統的從站，對外統一暴露整理後的資料。

典型結構：

1. 使用主站 API 輪詢多個現場設備，匯聚並清洗為統一資料模型；
2. 在同一程序內使用從站 API，將這些資料映射到一塊連續的 Modbus 暫存器空間；
3. 其他系統只需要按這塊「匯總暫存器」進行讀取即可。

---

## 6. 執行從站範例

在倉庫根目錄執行：

```bash
cargo run --package api_slave -- /tmp/vcom2
```

可以配合主站範例或 Aoba CLI/TUI 進行聯調：

- 先啟動從站範例監聽 `/tmp/vcom2`；
- 再使用主站範例或 CLI/TUI 輪詢該埠，驗證暫存器讀寫是否符合預期。

---

## 7. 延伸閱讀

- 主站側 API 使用範例：`docs/zht/API_MODBUS_MASTER.md`；
- CLI 等級 Modbus 使用說明：`docs/zht/CLI_MODBUS.md`；
- 資料來源/資料出口能力（HTTP、MQTT、IPC 等）：同目錄下 `DATA_SOURCE_*.md` 文件；
- 更多綜合範例可參考 `examples` 目錄中的其他子專案。
