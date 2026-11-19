# 前後端依賴套件資安修復執行計畫

## 目的與背景

針對 Neo AirPlay 四組依賴專案（`desktop/`、`desktop-test/`、`desktop/src-tauri/`、`desktop-test/backend/`）執行 `bun audit` 與 OSV（GHSA + RustSec）交叉盤點後，發現下列弱點與風險：

- **Rust 後端（`desktop/src-tauri/Cargo.lock`）**：
  - `tokio@1.53.2`（`RUSTSEC-2026-0386`，Unsound / 違反 I/O Safety）
  - `rustix@1.1.5`（`RUSTSEC-2026-0486` / `GHSA-jh2m-fhpj-3vr7`，Unsound / `xattr` 未初始化記憶體）
  - `chrono@0.4.45`（`RUSTSEC-2026-0543`，Unmaintained，RustSec 建議改用 `jiff` 或 `time`）
  - Linux 限定上游受阻項目：`glib@0.18.5`（`RUSTSEC-2024-0429`）與 `proc-macro-error@1.0.4`（`RUSTSEC-2024-0370`）受限於 `tauri@2.x` Linux GTK3 / webkit2gtk；`zvariant` / `zbus_names`（`RUSTSEC-2026-0370/0371/0372`）受限於 `tauri-plugin-single-instance@2.x` 的 `zbus@^5.9`
- **Rust 後端測試（`desktop-test/backend/Cargo.lock`）**：`chrono@0.4.45`（`RUSTSEC-2026-0543`，Unmaintained）
- **前端與 E2E 測試套件（`desktop-test/bun.lock`）**：11 個弱點（`basic-ftp@5.3.1`、`deepmerge-ts@7.1.6`、`serialize-javascript@6.0.2`、`glob@10.5.0/13.0.7`、`minimatch@9.0.9/10.2.6`、`extract-zip@2.0.1`）
- **前端主程式（`desktop/bun.lock`）**：0 弱點

本計畫依使用者核准之方案 **1A + 2A + 3A** 執行修復、遷移、CI 守門與安全文件更新，並完成全量驗證。

## 目前與預期行為

- **修改前**：
  - `desktop/src-tauri/Cargo.lock` 鎖定存在 Unsound 弱點的 `tokio@1.53.2` 與 `rustix@1.1.5`，且前後端 Rust 皆直接依賴停止維護的 `chrono@0.4.45`。
  - `desktop-test/bun.lock` 含有 11 個 npm 弱點（5 High、5 Moderate、1 Low）。
  - `.github/workflows/ci.yml` 尚未對正式前端 `desktop/` 執行 `bun audit`；`.neo_harness/SECURITY.md` 尚未記錄依賴掃描現況與上游受阻評估。
- **修改後**：
  - `desktop/src-tauri/Cargo.lock` 更新 `tokio@1.54.1` 與 `rustix@1.1.6`（並對齊 `tauri-plugin-wdio*` 至 `1.5.1`），完全消除已知可修復 Unsound 弱點。
  - `chrono` 從程式碼、兩份 `Cargo.toml` 與 `desktop-test/backend/Cargo.lock` 完全移除並遷移至 `jiff@0.2.46`（`Timestamp` 維持 RFC 3339 `Z` 輸出並向下相容讀取舊 `.jsonl` `+08:00` 偏移量格式；`Zoned::now()` / `to_zoned(TimeZone::system())` 維持本機日期分檔）。
  - `desktop-test/` 升級穩定版 WDIO `9.33.1`、`@wdio/tauri-*@1.5.1`、`@vue/test-utils@^2.6.0` 與 scoped `overrides`（含 `readdir-glob@^3>minimatch`），消除 9/11 弱點；不可達且上游無修補之 `extract-zip@2.0.1`（`driverProvider: "embedded"` 不觸發 `@puppeteer/browsers` 解壓）記錄於 `.neo_harness/SECURITY.md`。
  - `.github/workflows/ci.yml` 新增 `desktop/` `bun audit` 檢查；`.neo_harness/SECURITY.md` 更新供應鏈稽核與上游受阻項目說明。

## 限制與決策

- **決策 1A（`chrono` → `jiff`）**：`chrono` 僅用於 `desktop/src-tauri/src/domain/diagnostics.rs` 與 `desktop/src-tauri/src/infrastructure/persistence.rs`。實作確認使用 `jiff::Timestamp`（而非會輸出 RFC 9557 `[Asia/Taipei]` 後綴的 `jiff::Zoned`），確保 JS `new Date(...)` 與既有 `.jsonl` 記錄向下相容。
- **決策 2A（穩定版 WDIO 9 + Scoped Overrides）**：於 `desktop-test/package.json` 採用限定父套件作用域的 Bun `overrides`，包含補充新增的 `"readdir-glob@^3>minimatch": "^11.0.9"`（處理 `webdriverio@9.33.1` → `archiver@8.0.0` → `readdir-glob@3.0.0` 引入的 `minimatch@10.2.6`），同時保持 `mocha@10.8.2` 與 `readdir-glob@1.1.3` 所依賴之 `glob@8.1.0`、`minimatch@5.1.9`、`3.1.5` 不變。
- **決策 3A（安全文件 + `desktop/` CI `bun audit`）**：於 CI 加入正式前端 `bun audit` 並於 `.neo_harness/SECURITY.md` 記錄盤點與上游受阻清單。
- **不變數**：保持 `neo-airplay` 套件版本號 `0.2.5` 一致；保留工作樹中先前已完成的安裝工具安全強化未提交變更；遵守 Clean Architecture 依賴邊界。

## 工作步驟

1. 更新 Rust `tokio`、`rustix`、`tauri-plugin-wdio`、`tauri-plugin-wdio-webdriver`。
2. 將 `desktop/src-tauri` 與 `desktop-test/backend` 之 `chrono` 遷移至 `jiff`，更新兩個 `Cargo.lock` 並補齊單元測試。
3. 同步 `desktop/` 之 `@wdio/tauri-plugin` 至 `^1.5.1`，並更新 `desktop-test/` 依賴與 scoped `overrides` 後重產 `bun.lock`。
4. 更新 `.github/workflows/ci.yml` 與 `.neo_harness/SECURITY.md`。
5. 執行 `bun install --frozen-lockfile`、`bun audit`、OSV Rust 鎖定檔掃描、`bash desktop/scripts/verify.sh` 與 `cd desktop-test && bun run --silent test:e2e:ci`。
6. 填寫驗證結果並將本計畫移至 `.neo_harness/docs/exec-plans/completed/`。

## 進度與發現

- [x] 建立進行中執行計畫。
- [x] 步驟 1：更新 `tokio`（`1.53.2 -> 1.54.1`）、`rustix`（`1.1.5 -> 1.1.6`）、`tauri-plugin-wdio*`（`1.5.0 -> 1.5.1`）。
- [x] 步驟 2：遷移 `chrono` → `jiff`（`0.2.46`）並補齊 `models.rs` 與 `state.rs` 單元測試；**發現**：`chrono@0.4.45` 仍以 `tauri-utils@2.10.1` → `serde_with@3.24.0` 未啟用可選功能形式留存於 `desktop/src-tauri/Cargo.lock`，經 `cargo tree --all-features --target all --edges all -i chrono` 確認為零編譯邊，且已於 `desktop-test/backend/Cargo.lock` 完全移除。
- [x] 步驟 3：更新 `desktop/`（`@wdio/tauri-plugin` → `^1.5.1`）與 `desktop-test/` 依賴；**發現**：升級 `webdriverio@9.33.1` 後 `archiver@8.0.0` → `readdir-glob@3.0.0` 引入 `minimatch@10.2.6`，經重返規劃模式補充 `"readdir-glob@^3>minimatch": "^11.0.9"` 後徹底清除。
- [x] 步驟 4：更新 `.github/workflows/ci.yml` 與 `.neo_harness/SECURITY.md`。
- [x] 步驟 5：執行全量驗證並歸檔。

## 驗證方式與結果

- `cd desktop && bun install --frozen-lockfile && bun audit`：**通過**（exit code 0，`No vulnerabilities found (checked 128 packages)`）。
- `cd desktop-test && bun install --frozen-lockfile`：**通過**（exit code 0，`Checked 624 installs across 595 packages`）。
- `cd desktop-test && bun audit`：11 項弱點降至 **2 項（`extract-zip@2.0.1`，2 high）**，其餘 9 項（`basic-ftp`、`deepmerge-ts`、`serialize-javascript`、`glob`、`minimatch`）全部消除。
- Rust OSV（`api.osv.dev/v1/querybatch`）與 `cargo tree` 檢查：
  - `desktop-test/backend/Cargo.lock`（22 crates）：**0 弱點**（`[]`），且無 `chrono` 項目。
  - `desktop/src-tauri/Cargo.lock`（484 crates）：`RUSTSEC-2026-0386`（`tokio`）與 `RUSTSEC-2026-0486`（`rustix`）已消除；`cargo tree --manifest-path desktop/src-tauri/Cargo.toml --all-features --target all --edges all -i chrono` 回傳空（零編譯邊）；僅餘文件記錄之上游受阻 Linux 項目（`glib@0.18.5`、`proc-macro-error@1.0.4`、`zbus_names@4.3.4`、`zvariant@5.15.0`、`zvariant_derive@5.15.0`）與 `serde_with` 未啟用之可選 `chrono@0.4.45` 鎖定項。
- `bash desktop/scripts/verify.sh`：**通過**（exit code 0；含 `vue-tsc`、Vitest 18 files / 51 tests、`test:setup`、`test:version`、`cargo fmt --check`、Rust 50 tests `--locked`、`cargo clippy --locked --all-targets --all-features -- -D warnings` 與 Vite build）。
- `cd desktop-test && bun run --silent test:e2e:ci`：**通過**（exit code 0；macOS WebKit `5 passing (1.2s)`）。

## 復原方式

若須回退，可透過 Git 還原 `desktop/package.json`、`desktop/bun.lock`、`desktop-test/package.json`、`desktop-test/bun.lock`、`desktop/src-tauri/Cargo.toml`、`desktop/src-tauri/Cargo.lock`、`desktop/src-tauri/src/domain/diagnostics.rs`、`desktop/src-tauri/src/infrastructure/persistence.rs`、`desktop-test/backend/Cargo.toml`、`desktop-test/backend/Cargo.lock`、`desktop-test/backend/unit-tests/models.rs`、`desktop-test/backend/unit-tests/state.rs`、`.github/workflows/ci.yml` 與 `.neo_harness/SECURITY.md`，不影響其他變更。

## 最終結果

已完成 Neo AirPlay 前後端套件資安盤點、依決策 `1A + 2A + 3A` 完成修復與 CI／文件強化，並通過完整靜態檢查、前後端單元測試與桌面 E2E 驗證。
