# 目前架構

本文件描述目前工作樹中的 Tauri 桌面應用程式。關係依程式、設定和根目錄 [README.md](../README.md) 整理，不代表有自動架構檢查。

## 依賴概況

- `desktop/src/` 是 Vue 3 + TypeScript 介面。`App.vue` 組合總覽、設定、診斷及工具管理頁；`useAppState` 管理載入狀態、動作與 1.5 秒狀態輪詢。
- `desktop/src/api.ts` 透過 Tauri `invoke` 呼叫 Rust 命令。前端不直接操作程序、檔案或作業系統啟動設定。
- `desktop/src-tauri/src/lib.rs` 註冊單一執行個體、命令、登入啟動插件、系統匣與視窗關閉行為，並建立 `AppState`。
- `desktop/src-tauri/src/models.rs` 定義設定、引擎狀態、Android 裝置、記錄與總覽資料結構。
- `desktop/src-tauri/src/state.rs` 保存 JSON 設定、每日 JSONL 記錄、列舉 Android 裝置及管理 UxPlay/scrcpy 子程序。
- `desktop/src-tauri/src/dependencies.rs` 在背景檢查、升級及重新安裝投影工具；`DependencyPanel.vue` 顯示啟動檢查狀態，`ToolManagementPage.vue` 顯示維護狀態並發出手動維護命令。
- `desktop/scripts/install.sh` 與 `install.ps1` 保存共用平台安裝腳本。`setup:runtime` 與 Rust 背景檢查共用這些腳本；Rust 以 `include_str!` 將對應腳本編入正式執行檔。
- Windows 安裝器以 `install-progress.json` 回報階段，並在完成結果保留可選的階段與錯誤摘要；提權程序接收原使用者路徑與相同記錄目的地。Rust 外部程序與 PowerShell 執行階段安裝命令使用不建立主控台的程序設定，UAC 授權仍由作業系統處理。
- `desktop/scripts/run-tauri.mjs` 在直接呼叫 Tauri CLI 前執行平台 setup；`launch.sh`／`launch.ps1` 可在首次 checkout、Bun 尚未安裝時先準備環境並執行 Debug、Release 或 bundle。
- `desktop/src-tauri/capabilities/default.json` 只開放 Tauri 核心預設 IPC 權限。WDIO 權限只存在 E2E 組態與 `wdio` Rust feature。
- `desktop/engine/` 是本機引擎目錄，只有 `.GITKEEP` 納入版控；Tauri 從這裡打包工具。Debug 使用此目錄，Release 將套件內的 `engine/` 種入使用者資料目錄後再執行。

## 專案責任

| 元件 | 責任 | 證據 |
| :--- | :--- | :--- |
| Vue 應用程式 | 顯示總覽、設定、診斷記錄及工具管理，將使用者動作送至 API 模組。 | `desktop/src/App.vue`、`desktop/src/components/` |
| 前端狀態與 IPC | 讀取總覽、呼叫 Tauri commands、顯示錯誤並輪詢引擎程序狀態。 | `desktop/src/composables/useAppState.ts`、`desktop/src/api.ts` |
| Rust 命令入口 | 限制單一執行個體、註冊 Dashboard、設定、工具維護與引擎啟停命令；設定登入啟動並建立系統匣。 | `desktop/src-tauri/src/lib.rs` |
| 模型 | 定義跨 IPC 傳輸的序列化結構、設定預設值及 renderer 選項。 | `desktop/src-tauri/src/models.rs` |
| 執行狀態、程序與記錄 | 管理設定 JSON、每日 JSONL、ADB 裝置清單及兩個分離的子程序。 | `desktop/src-tauri/src/state.rs` |
| 依賴準備 | 共用平台安裝腳本、背景檢查、升級及重新安裝；工具維護期間會序列化引擎存取。 | `desktop/src-tauri/src/dependencies.rs`、`desktop/scripts/install.sh`、`desktop/scripts/install.ps1`、`desktop/src/components/DependencyPanel.vue`、`desktop/src/components/ToolManagementPage.vue` |
| 獨立測試專案 | 使用獨立 Bun 與 Cargo 專案執行前端、後端單元測試及桌面 E2E；測試直接載入桌面程式原始碼。 | `desktop-test/package.json`、`desktop-test/backend/Cargo.toml` |
| 前端單元測試 | 驗證狀態標籤、時間格式、總覽操作、設定表單與錯誤狀態。 | `desktop-test/frontend/` |
| Rust 單元測試 | 驗證設定正規化、ADB 輸出解析、UxPlay 參數、renderer allowlist 與引擎程序生命週期；測試 crate 以路徑載入桌面後端模組。 | `desktop-test/backend/`、`desktop/src-tauri/src/` |
| 桌面 E2E | 以 WebdriverIO 啟動實際 Tauri 程式，檢查第二次啟動、頁面導覽、設定儲存及兩個假引擎程序的並行啟停。 | `desktop-test/e2e/` |
| 正式應用程式啟動檢查 | `smoke:bundle` 可啟動目前平台正式 bundle 執行檔 8 秒，使用隔離的暫存使用者資料目錄；CI/CD 工作流程不執行此檢查。 | `desktop/scripts/smoke-bundle.mjs` |
| CI | `develop` 有新提交時，於原生 Windows、macOS、Linux runner 執行驗證及桌面 E2E；三平台成功後，`develop` 相對 `main` 的差異不只版本檔時才建立或沿用 promotion PR。 | `.github/workflows/ci.yml` |
| CD 與安裝套件 | PR 合併到 `main` 後檢查四處一致的穩定版本及 tag；tag 撞號時先建立版本修正 PR，否則於三平台建置安裝檔並發布 GitHub Release，成功後建立下一個 patch 版本的 `develop` PR。 | `.github/workflows/cd.yml`、`desktop/scripts/project-version.mjs` |

## 程序與執行元件

- AirPlay 與 Android 分別由 `EngineController` 持有程序及狀態；停止或故障其中一個不會呼叫另一個引擎。
- 單一執行個體插件先於其他插件註冊；第二次啟動會喚醒並顯示既有主視窗，不會建立另一組引擎控制器。
- AirPlay 啟動 UxPlay，使用獨立引數傳遞接收器名稱、全螢幕與 allowlist 中的 renderer。Android 只接受目前 ADB 清單中狀態為 `device` 的序號，並以 `--serial` 獨立引數啟動 scrcpy。
- macOS 將 GStreamer registry 放在 Tauri app cache；依賴檢查成功後以 UxPlay 相同的外掛、scanner、函式庫及 registry 路徑執行一次最佳努力預熱。GStreamer 子程序的函式庫路徑使用 engine `lib`、Homebrew prefix `lib` 及繼承路徑，不枚舉所有 Homebrew formula。AirPlay 只有在系統 `dns-sd` 找到設定的 `_airplay._tcp` 名稱後才回報 `Running`。
- App 只從目前模式使用的 `engine/` 目錄尋找 UxPlay、ADB、scrcpy 與 GStreamer 檢查工具；Debug 指向 `desktop/engine/`，Release 指向使用者資料目錄的 `engine/`。執行程序的 `PATH` 只包含該目錄及其 Android 子目錄。Unix 執行檔必須有執行權限。
- App 會讀取子程序標準輸出和錯誤並寫入診斷記錄。Android ADB 列舉逾時為 15 秒，兩條輸出管道由背景讀取器同步排空。
- 關閉主視窗會隱藏視窗並保留程式；系統匣選單可顯示視窗或結束程式。結束時 Rust state 會停止仍持有的 UxPlay/scrcpy 子程序。
- 設定與記錄目錄由 Tauri path resolver 提供，分別使用 `app_config_dir()`、`app_log_dir()`。登入啟動由 Tauri autostart plugin 管理，支援值在啟動時同步至作業系統註冊項目。
- 設定 command 的 transaction mutex 涵蓋 OS 登入啟動變更、設定寫入及失敗復原；state 的設定 mutex 涵蓋暫存檔寫入和記憶體更新。ADB 查詢也會序列化，前端忽略過期輪詢結果和重複動作。
- 工具維護使用共用鎖阻擋引擎啟動與 ADB 刷新；升級或重裝開始前會停止 AirPlay 和 Android 引擎。升級會清除指定的 GStreamer registry 檔案；重裝另外清除 App 的 `engine/` 內容。兩種操作都保留設定、診斷記錄及系統套件。
- 預設 renderer 選項依編譯平台提供：Windows 使用 D3D，macOS 使用 osxvideosink/glimagesink，Linux 使用 glimagesink/ximagesink/waylandsink。
- UxPlay、GStreamer、ADB、scrcpy 及 DNS-SD 服務探索是外部執行環境。正常 App 每次啟動都會在背景檢查並補備缺少的依賴；CI、E2E 及 smoke check 明確停用補備。執行檔只從 `engine/` 目錄啟動；平台安裝腳本仍使用 OS 套件管理器取得系統函式庫及建置工具。

## 驗證界線

- `desktop/scripts/verify.sh` 與 `desktop/scripts/verify.ps1` 分別提供 macOS／Linux 與 Windows 的統一驗證入口，兩者都呼叫 `desktop` 的 `verify`；前端與後端單元測試命令轉接至 `desktop-test`，其餘流程執行版本工具與安裝流程測試、Rust 格式檢查、Clippy 及 Vite 建置。
- `desktop-test` 的 `test:e2e:ci` 建置 `wdio` 測試版本與假引擎，並啟動桌面 E2E。測試會驗證第二次啟動退出並由既有執行個體處理；假執行檔只放在隔離的 E2E profile。Linux CI 以 D-Bus session 和 Xvfb 啟動測試。
- CI 在 `develop` 更新時對 `ubuntu-24.04`、`windows-latest`、`macos-latest` 執行 verify 與桌面 E2E；若 `develop` 相對 `main` 的完整差異只有版本檔，仍驗證但不建立 promotion PR。其他提交三平台成功後建立或沿用 `develop` 到 `main` 的 PR。
- Release 版本由 `desktop/scripts/project-version.mjs` 讀取 `desktop/package.json`、`desktop/src-tauri/tauri.conf.json`、`desktop/src-tauri/Cargo.toml` 與 `Cargo.lock`；四處必須一致且採用穩定 `X.Y.Z` 格式。若相同 tag 指向其他提交，CD 建立下一個未使用 patch 版本的 `main` 修正 PR，合併後再執行發布；發布成功後，CD 建立或更新下一個未使用 patch 版本的 `develop` PR。CI/CD 不執行正式 bundle 啟動 smoke check。
- E2E 會以假執行檔驗證兩個子程序並行啟停，不會連接實際 Android 裝置或啟動真實 UxPlay/scrcpy。AirPlay 網路探索、視訊、音訊及硬體加速仍需在目標 OS、裝置與區域網路人工驗收。
