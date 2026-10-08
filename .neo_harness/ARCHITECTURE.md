# 目前架構

本文件描述目前工作樹中的 Tauri 桌面應用程式。關係依程式、設定和根目錄 [README.md](../README.md) 整理，不代表有自動架構檢查。

## 依賴概況

- `desktop/src/` 是 Vue 3 + TypeScript 介面。`App.vue` 組合總覽、設定及診斷頁；`useAppState` 管理載入狀態、動作與 1.5 秒狀態輪詢。
- `desktop/src/api.ts` 透過 Tauri `invoke` 呼叫 Rust 命令。前端不直接操作程序、檔案或作業系統啟動設定。
- `desktop/src-tauri/src/lib.rs` 註冊單一執行個體、命令、登入啟動插件、系統匣與視窗關閉行為，並建立 `AppState`。
- `desktop/src-tauri/src/models.rs` 定義設定、引擎狀態、Android 裝置、記錄與總覽資料結構。
- `desktop/src-tauri/src/state.rs` 保存 JSON 設定、每日 JSONL 記錄、列舉 Android 裝置及管理 UxPlay/scrcpy 子程序。
- `desktop/src-tauri/capabilities/default.json` 只開放 Tauri 核心預設 IPC 權限。WDIO 權限只存在 E2E 組態與 `wdio` Rust feature。
- `desktop/src-tauri/resources/engine/` 是選擇性第三方引擎資源位置；目前只含說明檔，預設套件不附帶引擎二進位檔。

## 專案責任

| 元件 | 責任 | 證據 |
| :--- | :--- | :--- |
| Vue 應用程式 | 顯示總覽、設定及診斷記錄，將使用者動作送至 API 模組。 | `desktop/src/App.vue`、`desktop/src/components/` |
| 前端狀態與 IPC | 讀取總覽、呼叫 Tauri commands、顯示錯誤並輪詢引擎程序狀態。 | `desktop/src/composables/useAppState.ts`、`desktop/src/api.ts` |
| Rust 命令入口 | 限制單一執行個體、註冊 Dashboard、設定與引擎啟停命令；設定登入啟動並建立系統匣。 | `desktop/src-tauri/src/lib.rs` |
| 模型 | 定義跨 IPC 傳輸的序列化結構、設定預設值及 renderer 選項。 | `desktop/src-tauri/src/models.rs` |
| 執行狀態、程序與記錄 | 管理設定 JSON、每日 JSONL、ADB 裝置清單及兩個分離的子程序。 | `desktop/src-tauri/src/state.rs` |
| 前端單元測試 | 驗證狀態標籤、時間格式、總覽操作、設定表單與錯誤狀態。 | `desktop/src/lib/status.test.ts`、`desktop/src/components/DashboardPage.test.ts`、`desktop/src/components/SettingsPage.test.ts`、`desktop/src/composables/useAppState.test.ts` |
| Rust 單元測試 | 驗證設定正規化、ADB 輸出解析、UxPlay 參數、renderer allowlist 與引擎程序生命週期。 | `desktop/src-tauri/src/models.rs`、`desktop/src-tauri/src/state.rs` |
| 桌面 E2E | 以 WebdriverIO 啟動實際 Tauri 程式，檢查第二次啟動、頁面導覽、設定儲存及兩個假引擎程序的並行啟停。 | `desktop/e2e/main.spec.ts`、`desktop/e2e/fake-engine.rs`、`desktop/wdio.conf.ts` |
| 正式應用程式啟動檢查 | 啟動目前平台的正式 bundle 執行檔 8 秒，並使用隔離的暫存使用者資料目錄。 | `desktop/scripts/smoke-bundle.mjs`、`.github/workflows/ci.yml` |
| CI 與安裝套件 | 於原生 Windows、macOS、Linux runner 執行檢查和 E2E，產生並上傳平台安裝套件。 | `.github/workflows/ci.yml`、`desktop/src-tauri/tauri.conf.json` |

## 程序與執行元件

- AirPlay 與 Android 分別由 `EngineController` 持有程序及狀態；停止或故障其中一個不會呼叫另一個引擎。
- 單一執行個體插件先於其他插件註冊；第二次啟動會喚醒並顯示既有主視窗，不會建立另一組引擎控制器。
- AirPlay 啟動 UxPlay，使用獨立引數傳遞接收器名稱、全螢幕與 allowlist 中的 renderer。Android 只接受目前 ADB 清單中狀態為 `device` 的序號，並以 `--serial` 獨立引數啟動 scrcpy。
- App 啟動時使用固定 revision 的 `fix-path-env` 讀取登入 shell 的 `PATH`，再從 Tauri resource directory 的 `engine/` 子目錄與作業系統 `PATH` 尋找引擎。Android 另外搜尋 `engine/android/`；macOS 與 Linux 也會檢查常見系統 binary 目錄。
- App 會讀取子程序標準輸出和錯誤並寫入診斷記錄。Android ADB 列舉逾時為 15 秒，兩條輸出管道由背景讀取器同步排空。
- 關閉主視窗會隱藏視窗並保留程式；系統匣選單可顯示視窗或結束程式。結束時 Rust state 會停止仍持有的 UxPlay/scrcpy 子程序。
- 設定與記錄目錄由 Tauri path resolver 提供，分別使用 `app_config_dir()`、`app_log_dir()`。登入啟動由 Tauri autostart plugin 管理，支援值在啟動時同步至作業系統註冊項目。
- 預設 renderer 選項依編譯平台提供：Windows 使用 D3D，macOS 使用 osxvideosink/glimagesink，Linux 使用 glimagesink/ximagesink/waylandsink。
- UxPlay、GStreamer、ADB、scrcpy 及 DNS-SD 服務探索是外部執行環境。此 repository 預設不散布這些二進位檔；README 說明安裝方式與授權界線。

## 驗證界線

- `.neo_harness/verify.ps1` 呼叫 `bun run verify`，執行 Vue/TypeScript 型別檢查、前端單元測試、Rust 格式檢查、Rust 單元測試、Clippy 與 Vite 建置。
- `bun run test:e2e:ci` 額外建置 `wdio` 測試版本與假引擎，並啟動桌面 E2E。測試會驗證第二次啟動退出並由既有執行個體處理；假執行檔只放在隔離的 E2E profile。Linux CI 以 D-Bus session 和 Xvfb 啟動測試。
- GitHub Actions 對 `ubuntu-24.04`、`windows-latest`、`macos-latest` 執行 verify、桌面 E2E 與原生安裝套件建置。各平台實際結果須以 workflow run 為準。
- 安裝套件建置後會啟動該平台正式執行檔 8 秒；此 smoke check 確認程式能啟動，不驗證頁面操作或投影引擎串流。
- E2E 會以假執行檔驗證兩個子程序並行啟停，不會連接實際 Android 裝置或啟動真實 UxPlay/scrcpy。AirPlay 網路探索、視訊、音訊及硬體加速仍需在目標 OS、裝置與區域網路人工驗收。
