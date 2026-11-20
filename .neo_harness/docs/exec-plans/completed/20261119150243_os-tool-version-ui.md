# 依作業系統區別之工具安裝介面與版本管理執行計畫

## 目的與背景

目前 Neo AirPlay 的「工具管理」頁面未依作業系統（macOS / Windows / Linux）區分工具來源、預設版本與注意事項（例如非 Arch Linux 環境仍顯示 `pacman` 系統升級警告），亦無法查詢本機已安裝版本、爬取官方線上版本或手動輸入指定版本／commit 進行安裝。

本計畫依使用者核准方案 **1A + 2A + 3A** 實作跨平台工具版本管理系統：
- **1A**：`UxPlay` 內建預設版強制內建 SHA-256；非預設版提供選填 SHA-256，若未填寫則經由 UI 顯式確認後透過 HTTPS TLS 1.2+ 官方 GitHub 下載編譯（爬取版本優先鎖定 commit SHA）；`scrcpy` 則自動從官方 HTTPS `SHA256SUMS.txt` 驗證雜湊。
- **2A**：開放 `UxPlay` 與 `scrcpy`（含 `ADB`；Linux 非 `x86_64` 架構除外）進行線上版本爬取、手動輸入版本與單一工具切換安裝；`GStreamer` 依各作業系統顯示本機與線上參考版本並由 Homebrew / MSYS2 / 系統套件庫維護。
- **3A**：開啟工具管理頁時先立即展示本機狀態，並於背景非阻塞爬取一次官方版本（快取於記憶體，亦支援手動重新整理）；離線或限流時仍可直接使用內建推薦版或手動輸入版本。

## 目前與預期行為

- **修改前**：
  - `ToolManagementPage.vue` 僅提供全域「升級工具」與「清除並重新安裝」兩顆按鈕，固定使用 `install.sh` / `install.ps1` 硬編碼版本，且無作業系統感知與各工具版本明細。
- **修改後**：
  - 後端提供 `get_tool_catalog`、`fetch_tool_versions`、`install_tool_version`、`reset_tool_versions` 四個 Tauri 命令，由 `presentation/tool_catalog.rs` 透過 `reqwest` 白名單 HTTPS 爬取 GitHub Releases / Atom、`formulae.brew.sh` 與 `packages.msys2.org`，並探測本機 `engine/` 既有工具版本。
  - `install.sh` 與 `install.ps1` 支援 `--only` / `-Only` 單一工具快速安裝及嚴格白名單驗證的版本參數，並將使用者自訂版本偏好安全持久化於 `config_dir/tool-versions.json`。
  - 前端依 OS／架構／套件管理器動態渲染工具卡片、支援「線上版本清單 / 手動輸入」切換、選填 SHA-256、`UxPlay` 未附雜湊確認對話框，以及僅在適用平台顯示之 `pacman` 等警告提示。

## 限制與決策

- **Clean Architecture**：`desktop/src-tauri/src/domain/tool_catalog.rs` 僅含純領域模型、白名單驗證與純文字解析；`ToolCatalogFetcher` Port 置於 `application/ports.rs`，具體 `reqwest` 實作置於 `presentation/tool_catalog.rs`，使 `desktop-test/backend` 維持零網路相依。
- **TLS 選擇**：`desktop/src-tauri/Cargo.toml` 使用 `reqwest = { version = "0.13", default-features = false, features = ["native-tls", "http2", "blocking"] }`，於 macOS / Windows 使用系統原生 TLS，Linux 使用 CI 既有 `libssl-dev`（鎖定 `openssl@0.10.82`，無 OSV 弱點）。
- **CSP 不放寬**：`desktop/src-tauri/tauri.conf.json` CSP 維持不開放前端直連外網，所有網路爬取皆透過 Rust 後端白名單檢查後執行。
- **不變數**：`neo-airplay` 版號維持 `0.2.5`；保留先前的安裝安全強化與套件資安修補變更。

## 工作步驟

1. 領域層新增 `tool_catalog.rs`、更新 `dependency.rs` 與 `mod.rs`。
2. 應用層擴充 `ports.rs`、`queries.rs`、`commands.rs` 與 `mod.rs`。
3. 基礎設施層與表現層加入 `reqwest`、`presentation/tool_catalog.rs`、`dependencies.rs` 版本探測／持久化／爬取／單工具安裝，並註冊四個新 Tauri 命令。
4. 擴充 `install.sh` 與 `install.ps1` 支援單一工具安裝、自訂版本白名單驗證與官方 `scrcpy` `SHA256SUMS.txt` 自動校驗。
5. 擴充前端 Domain / Application / Infrastructure / Composable (`useToolCatalog.ts`) 與重構 `ToolManagementPage.vue` 呈現 OS 專屬介面。
6. 補齊前後端、安裝腳本與 E2E 測試，更新 `.neo_harness/SECURITY.md` 與 `.neo_harness/ARCHITECTURE.md`，執行 `bun audit`、OSV 檢查、`verify.sh`、`verify.ps1` 與 `test:e2e:ci`。

## 進度與發現

- [x] 建立進行中執行計畫。
- [x] 步驟 1：Rust 領域層模型、驗證與解析器。
- [x] 步驟 2：Rust 應用層埠與使用案例（抽出共用 `stop_engines_for_maintenance`）。
- [x] 步驟 3：Rust 基礎設施與表現層（`reqwest`、`dependencies.rs`、`process.rs`、`commands.rs`、`bootstrap.rs`）；注意 E2E 停用模式（`!enabled`）略過二進位探測以免 `fake-engine` 休眠阻塞，且 `launch.sh` 採位置參數轉傳消除腳本內插。
- [x] 步驟 4：跨平台安裝腳本（`install.sh`、`install.ps1`）並新增 `winarm64` 支援與嚴格白名單校驗。
- [x] 步驟 5：前端分層與 OS 差異化 `ToolManagementPage.vue`（共用於 `useDependencyState` 世代保護）。
- [x] 步驟 6：測試、安全與架構文件、全量驗證與歸檔。

## 驗證方式與結果

- `cd desktop && bun audit`：**通過**（exit code 0，`No vulnerabilities found (checked 128 packages)`）。
- OSV `Cargo.lock` 掃描（518 crates）：新增 `reqwest` + `native-tls` 相關 34 個 crate 後 **0 新增弱點**（`openssl@0.10.82` 等皆無弱點）。
- `bash desktop/scripts/verify.sh`：**通過**（exit code 0；涵蓋 `vue-tsc`、Vitest 18 files / 55 tests、`test:setup`（含 `test-windows-setup.ps1` 與 `install.sh` 驗證測試）、`test:version`、`cargo fmt --check`、Rust `--locked` 55 tests、`cargo clippy --locked --all-targets --all-features -- -D warnings` 與 Vite build）。
- `pwsh desktop/scripts/verify.ps1`：**通過**（exit code 0）。
- `cd desktop-test && bun run --silent test:e2e:ci`：**通過**（exit code 0；macOS WebKit 實測 `5 passing (1.3s)`，並於 `nav-tools` 驗證 OS 徽章與工具目錄清單）。
- 真實線上版本爬取端點實機連線：於規劃期間直接透過 Node `fetch` 實測確認 GitHub Releases / Atom、`formulae.brew.sh` 與 `packages.msys2.org` 回應狀態與格式相容；UI 點擊實裝線上安裝真實 UxPlay/scrcpy 則留待目標作業系統實機環境驗證。

## 復原方式

可透過 Git 還原本次新增與修改的前後端原始碼、安裝腳本、測試與文件，並自 `Cargo.toml` / `Cargo.lock` 移除 `reqwest` 相依；不影響先前提交前既有安全強化變更。

## 最終結果

已完成依作業系統差異化之工具安裝 UI、官方來源版本爬取、手動版本／雜湊指定安裝與全量自動化驗證並歸檔。
