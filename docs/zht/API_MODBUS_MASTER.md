# Modbus 主站 API 使用指南

本文件介紹如何在 Rust 程式中透過 Aoba 提供的 Modbus 主站（Master） API 採集和監控工業現場設備資料，例如生產線設備狀態監控、環境監測、程序控制等場景。

範例程式碼參考 `examples/api_master` crate。

## 1. 總體概覽

Aoba 暴露了一套基於 trait 的 Modbus 主站 API，適合嵌入到你的工控 / 監控應用中，用於：

- 週期性輪詢 Modbus 從設備（串列埠 / 虛擬串列埠）
- 採集線圈 / 暫存器資料並轉換為工程量（壓力、溫度、閥門狀態等）
- 透過 Hook 機制統一做日誌、告警、統計等

核心入口型別是 `_main::api::modbus::ModbusBuilder`：

```rust
use _main::api::modbus::{ModbusBuilder, ModbusHook, ModbusResponse, RegisterMode};
```

> 說明：範例專案中 crate 根叫 `_main`，在你自己的專案中通常是 `aoba` 或在 `Cargo.toml` 裡設定的別名。

---

## 2. 主站基本生命週期

一個最小可用的主站輪詢程式大致如下：

```rust
use anyhow::Result;
use std::time::Duration;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    // 建立主站實例
    let master = ModbusBuilder::new_master(1) // 從站位址（站號）
        .with_port("/dev/ttyUSB0")          // 或 `/tmp/vcom1` 等
        .with_register(RegisterMode::Holding, 0, 10)
        .with_timeout(1000)                  // 逾時：毫秒
        .build_master()?;

    // 簡單輪詢讀取
    loop {
        if let Some(resp) = master.recv_timeout(Duration::from_secs(1)) {
            println!("values = {:04X?}", resp.values);
        }
    }
}
```

### 關鍵參數說明

- **串列埠名稱**：
  - 真實串列埠：如 `/dev/ttyUSB0`、`/dev/ttyS1` 等；
  - 虛擬串列埠：如 `/tmp/vcom1`，可用 `socat` 或系統工具建立，用於聯調 / 仿真。
- **站點 ID（Station ID）**：Modbus 從站位址，通常在 1–247 範圍內，需要與現場設備配置一致。
- **暫存器模式（RegisterMode）**：
  - `Coils`（01 線圈）
  - `DiscreteInputs`（02 離散量輸入，可寫模式在部分場景會特殊處理）
  - `Holding`（03 保持暫存器）
  - `Input`（04 輸入暫存器，可寫模式同上）
- **暫存器位址 / 長度**：起始位址和讀取長度，對應現場協議文件（例如某生產線 PLC 的 Modbus 位址表）。
- **逾時時間**：單次請求的逾時（毫秒），建議根據匯流排長度和鮑率適當放寬。

主站內部維護請求傳送邏輯並將回應推入內部通道，你的程式碼只需要定期呼叫 `recv_timeout` 即可獲得最新資料。

---

## 3. 使用 Hook 做日誌和監控

在大多數工業監控或程序控制場景中，通常需要：

- 記錄每次成功的 Modbus 回應（便於追溯）
- 統計錯誤和逾時次數，觸發告警
- 將資料推送到外部系統（MQTT、HTTP、資料庫等）

`ModbusHook` trait 提供了統一的插樁入口：

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
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let master = ModbusBuilder::new_master(1)
        .with_port("/tmp/vcom1")
        .with_register(RegisterMode::Holding, 0, 5)
        .with_timeout(1000)
        .add_hook(Arc::new(LoggingHook))
        .build_master()?;

    // 後續使用 recv_timeout 輪詢即可
    # let _ = master;
    Ok(())
}
```

你可以註冊多個 Hook，例如：

- 一個負責寫日誌；
- 一個負責將資料轉成 JSON 並推送到 MQTT；
- 一個負責統計錯誤率並觸發告警。

---

## 4. 面向工業現場 / 程序設備的推薦整合模式

結合一般工業現場（如產線設備、程序裝置、環境監測裝置等）的實踐經驗，一個比較穩妥的整合模式是：

1. **確定通訊映射**：
   - 明確每台設備對應的串列埠（或虛擬串列埠）；
   - 明確每台設備的站點 ID 和暫存器位址表（參考設備提供的 Modbus 通訊協議文件）。
2. **每個物理/虛擬串列埠建立一個主站實例**：
   - 使用 `ModbusBuilder::new_master(station_id)`；
   - 或在外層維護一個 `(port, station_id, register_region)` 的表，循環建立多個主站。
3. **在 Tokio 執行時中為每個主站起一個任務**：
   - 在任務中使用 `recv_timeout` 輪詢資料；
   - 將 `ModbusResponse::values` 按協議文件換算為工程量，寫入記憶體共享狀態或通道；
   - 再由其他任務負責資料匯聚（MQTT/HTTP 上報、歷史記錄等）。
4. **透過 Hook 統一監控健康度**：
   - 在 `on_error` 中維護錯誤計數；
   - 如果連續錯誤超過閾值（如 10 次），更新全域「設備離線」狀態；
   - 可在 `on_after_response` 中記錄回應延遲，做效能監控。

這種結構下，Modbus 主站層只關心通訊本身，而業務層則只面對清洗後的工程量資料，職責分離清晰，便於後續擴展（增加資料來源、切換上報通道等）。

---

## 5. 錯誤與逾時處理建議

- `build_master()` 失敗通常表示：
  - 串列埠不存在或權限不足；
  - 配置參數不合法（例如暫存器長度為 0）。
- `recv_timeout()` 回傳 `None` 表示在指定時間內沒有拿到回應，**不一定是致命錯誤**，在現場環境（線纜較長、電磁干擾、大量設備共享匯流排）下偶發逾時是常態。
- 協議級錯誤（CRC 錯誤、異常碼、IO 錯誤等）會透過 `ModbusHook::on_error` 回調暴露出來。

實踐建議：

- **區分偶發錯誤與持續錯誤**：
  - 使用 Hook 裡的計數器統計連續失敗次數；
  - 連續失敗超過 N 次（比如 5～10）再標記「設備疑似離線」。
- **注意節流日誌**：
  - 在高頻輪詢（< 1s 週期）下，避免對每次請求都列印大量日誌，建議使用 Info 打點 + Debug 等級詳細日誌的方式。

---

## 6. 執行範例程式

在倉庫根目錄執行：

```bash
cargo run --package api_master -- /tmp/vcom1
```

其中 `/tmp/vcom1` 可以是：

- 真實的串列埠設備（在 Linux/WSL 下需要用實際設備名替換）；
- 或透過 `socat` 建立的虛擬串列埠對，用於與 `examples/modbus_slave` 或現場設備仿真程式聯動。

在典型的工業聯調過程中，常見步驟是：

1. 先用 Aoba CLI/TUI 或 `examples/modbus_slave` 搭一個從站仿真；
2. 再執行 `api_master`，確認 Modbus 配置和協議映射都正確；
3. 在此基礎上，將範例程式碼中的採集邏輯改造成自己的業務服務（寫資料庫、推 MQTT 等）。

---

## 7. 手動模式主站（poll_once / 寫操作）

在需要精細控制輪詢時序的場景（狀態機、自適應策略、寫操作等），使用 `build_master_manual()`：

```rust
use anyhow::Result;
use _main::api::modbus::{ModbusBuilder, RegisterMode};

fn main() -> Result<()> {
    let master = ModbusBuilder::new_master(1)
        .with_port("/dev/ttyUSB0")
        .with_baud_rate(9600)
        .with_timeout(5000)
        .build_master_manual()?;

    // 單次輪詢
    let response = master.poll_once(RegisterMode::Holding, 0x00, 10)?;
    println!("值: {:?}", response.values);

    // 寫入單個保持暫存器 (fc 0x06)
    master.write_holding(0x00, 0x1234)?;

    // 寫入多個保持暫存器 (fc 0x10)
    master.write_registers(0x00, &[0x1234, 0x5678, 0x9ABC])?;

    // 寫入線圈 (fc 0x0F)
    master.write_coils(0x00, &[true, false, true, true])?;

    Ok(())
}
```

### 何時使用手動模式

| 場景 | 推薦模式 |
|------|---------|
| 持續監控 / 資料採集 | `build_master()`（自動模式） |
| 讀-改-寫控制迴圈 | `build_master_manual()` |
| 狀態機 / 事件驅動輪詢 | `build_master_manual()` |
| 根據回應延遲自適應輪詢 | `build_master_manual()` |
| 一次性診斷或配置 | `build_master_manual()` |

### 寫操作詳解

- **`write_holding(address, value)`** — 使用功能碼 0x06 寫入單個保持暫存器，適合寫入單個配置參數。
- **`write_registers(address, values)`** — 使用功能碼 0x10 寫入多個連續保持暫存器，適合批次參數寫入。
- **`write_coils(address, values)`** — 使用功能碼 0x0F 寫入多個線圈。對於 11 線圈寫入會自動執行位元組交換（特定硬體需要）。
- 所有寫方法會阻塞直到從站確認回應或發生錯誤。

---

## 8. 延伸閱讀

- 從站側 API 使用範例：`examples/api_slave`；
- CLI 等級的 Modbus 使用說明：`docs/zht/CLI_MODBUS.md`；
- HTTP / MQTT / IPC 等資料來源對接文件：同目錄下 `DATA_SOURCE_*.md`；
- 若要結合 TUI 做 E2E 測試，請參考 `examples/tui_e2e` 及根目錄下 `CLAUDE.md` 中關於 IPC 與狀態監控的介紹。
