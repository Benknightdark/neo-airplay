# 全面安全性與外部工具安裝／執行強化執行計畫

## 目的與背景

在先前的安裝流程強化與依賴弱點修補基礎上，對 Neo AirPlay 全專案（Rust 後端、Tauri WebView/IPC、Vue 前端、外部工具執行期使用、跨平台安裝腳本 `install.sh`／`install.ps1`、CI/CD 供應鏈）進行全面逐行安全盤點後，發現 13 項高／中／低風險項目（涵蓋環境變數與 `.uxplayrc` 劫持、提權與 PATH 劫持、Tar/Zip Slip 與符號連結預檢、下載重導向白名單與大小上限、本機持久化與日誌權限及防 TOCTOU、AirPlay/scrcpy 區網與剪貼簿保護、TOFU 雜湊鎖定、Tauri CSP／最小 capability、CI/CD `cargo-audit` 與發佈 Provenance 等）。

本計畫已依使用者核准之決策（**Q1 選項 A、Q2 選項 A、Q3 選項 A**）完成全部安全強化並通過完整驗證。

## 目前與預期行為

- **修改前**：
  - 外部工具執行期未鎖定 `scrcpy` 的 `ADB` / `SCRCPY_SERVER_PATH` 與 `uxplay` 的 `UXPLAYRC`，且未淨化 `LD_PRELOAD`、`DYLD_INSERT_LIBRARIES`、`ADB_SERVER_SOCKET` 等環境變數；子程序輸出讀取與日誌無界；`ReceiverName` 與 ADB 序號缺乏長度及控制字元驗證；AirPlay 區網無驗證、scrcpy 預設自動同步剪貼簿。
  - `persistence.rs` 寫入 `settings.json` 使用可預測 `.json.tmp`，`load_settings` 與 `AppLogger` 未拒絕符號連結或設 `0o600` 權限；目錄建立未全面使用 `ensure_safe_directory(..., true)`；`read_bounded_regular_file` 與 `read_log_tail` 仍有 TOCTOU 視窗。
  - `install.sh` / `install.ps1` 下載缺少重導向主機白名單與大小上限、解壓缺少 Tar/Zip Slip 與符號連結預檢、`install.sh` 將 `~/.local/bin` 置於 `PATH` 前端且提權呼叫裸名稱、產物複製過度（含 `uxplay-beacon.py` 與無關 Homebrew 目錄）、`install.ps1` WinGet 模組未強制 `AllUsers` 與 `%ProgramFiles%`、MSYS2 `SigLevel` 修復不完整；自訂 UxPlay 持久化 `allowUnverified: true`。
  - `tauri.conf.json` 缺乏嚴格 CSP 指令與 `freezePrototype`、`capabilities/default.json` 開放整包 `core:default`、CI 缺少自動化 `cargo-audit`、CD 缺少 `SHA256SUMS.txt` 與 Build Provenance。
- **修改後**：
  - 本機設定、日誌與受控目錄均套用防符號連結、Unix `0o700`/`0o600` 權限、inode 比對防 TOCTOU 與大小上限。
  - 外部工具全面執行環境淨化、鎖定 `UXPLAYRC` / `ADB` / `SCRCPY_SERVER_PATH`、驗證實體目錄鏈與非符號連結、強制 `--serial=<serial>` 與有界逐行輸出讀取；提供 AirPlay PIN 保護與 Android 停用剪貼簿同步（預設開啟）／唯讀模式。
  - `install.sh` / `install.ps1` 完成安全 `PATH` 與絕對系統路徑提權、官方主機下載白名單與大小上限、Tar/Zip Slip 與連結預檢、最小化原子部署、MSYS2 `SigLevel` 全面防護及自訂 UxPlay TOFU SHA-256 自動鎖定。
  - Tauri WebView 啟用嚴格 CSP、`freezePrototype` 與最小 capability（`permissions: []`）；CI 加入 `rustsec/audit-check`（搭配 `desktop/src-tauri/.cargo/audit.toml`）與 `persist-credentials: false`；CD 加入 `-- --locked`、`SHA256SUMS.txt` 與 Build Provenance Attestation（`actions/attest`）。

## 限制與決策

- **Q1（AirPlay 驗證）**：`AirplayAuthMode`（`none`［預設］、`randomPin`、`fixedPin`），透過受控 `0o600` `config_dir/uxplay.rc` 與 `config_dir/uxplay.register` 傳遞 `pin`／`pin XXXX`／`reg`，不將固定 PIN 暴露於命令列參數或一般日誌（於異常輸入時 fail-closed 退回 `randomPin`）。
- **Q2（Android 安全選項）**：`androidNoClipboardSync` 預設 `true`（`--no-clipboard-autosync`），`androidReadOnly` 預設 `false`（`--no-control`）。
- **Q3（自訂 UxPlay TOFU）**：保留介面確認下載機制，成功後由安裝腳本回報 `resolvedSha256` 並自動鎖定寫入 `tool-versions.json`、清除 `allowUnverified`。
- 不新增第三方 Cargo 或 npm 套件依賴；維持四處專案版本一致性（`0.2.7`）與 Clean Architecture 向內依賴邊界。

## 工作步驟

0. 建立本持久執行計畫。
1. 強化底層檔案安全原語、環境淨化、有界串流與本機持久化／日誌（`process.rs`、`persistence.rs`、`bootstrap.rs`、`state.rs`）。
2. 強化領域模型驗證、AirPlay PIN 與 Android `scrcpy` 選項（`domain/settings.rs`、`domain/android.rs`、`domain/diagnostics.rs`、`domain/tool_catalog.rs`、`domain/dependency.rs`）。
3. 強化外部工具執行期隔離、macOS Terminal `launch.sh`、TOFU 鎖定與爬蟲併發鎖（`infrastructure/state.rs`、`infrastructure/dependencies.rs`）。
4. 強化 `install.sh`、`install.ps1` 與 `smoke-bundle.mjs`。
5. 收緊 `tauri.conf.json`、`capabilities/default.json`並更新前端領域與設定／總覽 UI。
6. 配置 `desktop/src-tauri/.cargo/audit.toml`、`build-app.ts`、`.github/workflows/ci.yml` 與 `.github/workflows/cd.yml`。
7. 補齊前後端、安裝腳本與 E2E 測試，更新 `.neo_harness/SECURITY.md`，執行全量驗證並歸檔計畫。

## 進度與發現

- [x] 建立持久執行計畫。
- [x] 步驟 1：底層檔案安全原語、環境淨化、有界串流與持久化／日誌。
- [x] 步驟 2：領域模型驗證與 AirPlay/Android 安全設定。
- [x] 步驟 3：外部工具執行期隔離與 TOFU 鎖定。
- [x] 步驟 4：外部工具安裝與輔助腳本強化。
- [x] 步驟 5：Tauri CSP、最小 Capability 與前端 UI／Domain。
- [x] 步驟 6：CI/CD 供應鏈安全與可驗證發佈。
- [x] 步驟 7：測試補齊、安全文件更新與全量驗證。

## 驗證方式與結果

全部驗證命令皆於最後一次程式碼修改完成後實測通過（退出碼 0）：
- `bash desktop/scripts/verify.sh`：
  - `vue-tsc --noEmit` 與 Vite 正式建置通過（50 個模組，`dist/` 建置完成）。
  - Vitest 前端單元與 Clean Architecture 測試：18 個測試檔、57 項測試全部通過（0 失敗）。
  - `desktop/scripts/test-setup.mjs`：52 項測試項目執行完成、0 失敗（於 macOS 主機上 3 項 Windows／無 Homebrew 專屬情境依設計略過）。
  - `project-version.test.mjs` 通過；`cargo fmt --check` 與 `cargo clippy --locked --all-targets --all-features -- -D warnings` 0 警告。
  - `cargo test --manifest-path backend/Cargo.toml --locked`：78 項後端單元測試全部通過（0 失敗）。
- `pwsh -NoProfile -NonInteractive -File desktop/scripts/test-windows-setup.ps1 -InstallerPath desktop/scripts/install.ps1`：`26 installer scenarios passed.`
- `cd desktop && bun audit`：`No vulnerabilities found (checked 128 packages)`。
- `bash desktop/scripts/install.sh --check`：`Runtime dependencies are available.`
- `cd desktop && bun run --silent test:e2e:ci`：WebdriverIO 桌面 E2E 測試於嚴格 CSP、`freezePrototype: true` 與空 `permissions: []` 設定下 `5 passing`。
- Windows 原生安裝執行與 Linux GitHub Actions CI/CD 實際 runner 步驟未於本機 macOS 環境執行，已於 `.github/workflows/` 對應配置並保留現有測試覆蓋。

## 復原方式

可透過 Git 還原 `desktop/`、`desktop-test/`、`.github/` 與 `.neo_harness/` 的所有變更。

## 最終結果

完成跨後端 Rust、外部工具安裝與執行期隔離、Tauri WebView/IPC 邊界、前端設定介面、CI/CD 供應鏈與回歸測試之全面安全性強化，並完成文件更新與計畫歸檔。
