# 架構與依賴

本文件依目前程式碼、設定與 [README.md](../README.md) 記錄桌面應用程式架構與驗證界線。

## 元件與依賴邊界（Clean Architecture）

前後端均採 `Domain` ← `Application` ← `Infrastructure` / `Presentation` 向內依賴分層：`Domain` 與 `Application` 層不直接依賴框架（Vue / Tauri）或外部 I/O（檔案系統、子程序）。

| 元件 | 職責與依賴限制 | 證據 |
| :--- | :--- | :--- |
| 前端 Domain 層 | 定義純 TypeScript 實體、值物件規則、狀態轉換、`Result<T>` 與錯誤正規化，零 Vue / Tauri 依賴。 | `desktop/src/domain/` |
| 前端 Application 層 | 定義 `ReceiverGateway`、`DependencyGateway` Port 介面與 CQRS Use Cases，僅依賴 `domain/`。 | `desktop/src/application/` |
| 前端 Infrastructure 層 | 以 Tauri `invoke` 封裝（`api.ts`）實作前端 Gateway 介面。 | `desktop/src/infrastructure/tauri/`、`desktop/src/api.ts`、`desktop/src-tauri/capabilities/default.json` |
| 前端 Presentation 層 | Vue SFCs 與 Composables（`useAppState`、`useDependencyState`、`useInstallLogViewer`），透過 `presentation/di.ts` 解析用例；保留 `types.ts`、`lib/status.ts` 相容轉出。 | `desktop/src/App.vue`、`desktop/src/components/`、`desktop/src/composables/`、`desktop/src/presentation/di.ts` |
| 後端 Domain 層 | 定義 `ReceiverName`、`VideoRenderer`、`ReceiverSettings`、`EngineKind`、`AndroidDevice`、`DependencySnapshot`、`AppLogEntry` 與 `DomainError`，不依賴 `tauri` 或 I/O。 | `desktop/src-tauri/src/domain/` |
| 後端 Application 層 | 定義 `ReceiverRuntime`、`DependencyRuntime`、`SettingsRepository`、`AutostartGateway`、`LogRepository`、`EngineAccessGuard` 等 Port Traits 與 Command/Query 用例（含 `SaveSettingsUseCase` 交易與回滾）。 | `desktop/src-tauri/src/application/` |
| 後端 Infrastructure 層 | 實作 `settings.json` 原子讀寫、每日 JSONL 記錄、`AppState`／`EngineController` 雙引擎控制、ADB 列舉、與 `DependencyManager` 背景工具檢查／安裝。 | `desktop/src-tauri/src/infrastructure/`、`desktop/scripts/` |
| 後端 Presentation 層 | 註冊單一執行個體、`autostart`、系統匣、視窗關閉隱藏及薄型 `#[tauri::command]` 入口；組裝 `AppState` 與 `DependencyManager` 注入應用層。 | `desktop/src-tauri/src/lib.rs`、`desktop/src-tauri/src/presentation/` |
| 本機引擎目錄 | 僅 `.GITKEEP` 入版控；Debug 直接使用 `desktop/engine/`，Release 將打包資源寫入使用者資料目錄的 `engine/` 後執行。 | `desktop/engine/` |
| 獨立測試專案 | 直接載入 `desktop` 與 `src-tauri` 模組，執行架構向內依賴檢查、Domain/Application/Infrastructure/Presentation 分層單元測試，以及帶 `wdio` feature 與假引擎的 WebdriverIO 桌面 E2E。 | `desktop-test/package.json`、`desktop-test/frontend/`、`desktop-test/backend/`、`desktop-test/e2e/` |
| CI / CD 工作流程 | CI 驗證 `develop` 提交並維護 `develop -> main` PR；CD 在 `main` PR 合併後驗證四處版本與 tag、打包三平台安裝檔並發布 Release。 | `.github/workflows/ci.yml`、`.github/workflows/cd.yml`、`desktop/scripts/project-version.mjs` |

## 核心架構不變量

- **執行路徑隔離**：App 只從目前模式的 `engine/` 啟動 `uxplay`、`adb`、`scrcpy` 與 GStreamer 檢查工具，子程序 `PATH` 僅含該目錄與其 Android 子目錄，不回退系統 `PATH`。
- **引擎與鎖分離**：AirPlay 與 Android 各自持有程序與狀態，互不影響；設定變更（含 OS 登入啟動 transaction 鎖）、ADB 列舉與工具維護（阻擋引擎啟動與 ADB 刷新）各自以 Mutex 序列化。
- **macOS 專屬管線**：GStreamer registry 存放於 Tauri app cache，函式庫路徑僅含 engine `lib`、Homebrew prefix `lib` 與繼承路徑；AirPlay 需經系統 `dns-sd` 找到設定的 `_airplay._tcp` 名稱才回報 `Running`。

## 驗證與 CI/CD 界線

- **本機統一驗證**：`desktop/scripts/verify.sh`（macOS／Linux）與 `desktop/scripts/verify.ps1`（Windows）執行前後端型別檢查、單元測試、安裝與版本腳本測試、`cargo fmt`、Clippy 及 Vite 建置。
- **桌面 E2E 與 Smoke**：`desktop-test` 的 `test:e2e:ci` 以隔離 `--e2e-profile` 驗證單一執行個體、導覽、設定與雙假引擎並行啟停（Linux 使用 D-Bus + Xvfb）；`smoke:bundle` 隔離啟動正式套件 8 秒（僅手動執行）。真實裝置投影、mDNS 與硬體加速需於目標環境人工驗收。
- **CI（`.github/workflows/ci.yml`）**：`develop` 新提交時（排除合併自 `automation/next-version` 分支的 PR）於 Ubuntu、Windows、macOS 執行 verify 與桌面 E2E；三平台通過且相對 `main` 不只變更版本檔時，建立或沿用 `develop -> main` PR。
- **CD（`.github/workflows/cd.yml`）**：PR 合併至 `main` 後檢查 `desktop/package.json`、`tauri.conf.json`、`Cargo.toml` 與 `Cargo.lock` 四處一致的穩定 `X.Y.Z` 版本與 tag；若 tag 已指向其他提交則建立下一個 patch 的 `main` 修正 PR，否則三平台打包發布 GitHub Release，成功後建立或更新 `automation/next-version -> develop` PR。
