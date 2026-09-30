# 目前架構

本文件描述目前工作樹中的 Windows x64 桌面接收器；依據為程式碼、專案設定與 [README.md](../README.md)。以下關係依目前程式碼整理，不代表有自動架構檢查。

## 依賴概況

- `NeoAirPlay.App` 是 WPF 啟動與呈現層。`App.xaml.cs` 組裝 Framework 服務、載入設定、建立 `MainViewModel` 與 `MainWindow`。
- `MainWindow` 使用 `MainViewModel`；畫面由 `Views/` 下的 XAML 視圖組成，ViewModel 與 UI 命令留在 App。
- `NeoAirPlay.Framework` 包含 `NeoAirPlay.Models` 與 `NeoAirPlay.Services`，提供設定、記錄、登入啟動註冊、Bonjour 檢查及 UxPlay 外部程序生命週期服務。
- App 透過 `IReceiverEngine`、`ISettingsStore`、`IAppLogger`、`IStartupRegistration` 等介面使用服務；專案參考方向為 App → Framework。
- `NeoAirPlay.App.csproj` 將 `engine/` 下的檔案納入建置與發佈輸出；Framework 不負責隨附執行元件。

## 專案責任

| 專案或元件 | 責任 | 證據 |
| :--- | :--- | :--- |
| App 啟動與視窗 | 組裝服務、啟動主視窗並提供系統匣操作。 | `src/NeoAirPlay.App/App.xaml.cs`、`src/NeoAirPlay.App/MainWindow.xaml.cs` |
| Views 與 ViewModel | Dashboard、Settings、Diagnostics 視圖呈現接收器狀態、設定與記錄；ViewModel 管理畫面狀態及命令。 | `src/NeoAirPlay.App/Views/`、`src/NeoAirPlay.App/ViewModels/MainViewModel.cs` |
| Commands | 提供同步與非同步 UI 命令。 | `src/NeoAirPlay.App/Commands/` |
| Framework Models | 保存接收器設定、選項、狀態及記錄資料。 | `src/NeoAirPlay.Framework/Models/` |
| Framework Services | 管理接收器程序、Bonjour 執行元件檢查、設定、登入啟動註冊及記錄。 | `src/NeoAirPlay.Framework/Services/` |
| Framework 單元測試 | 驗證設定儲存、檔案記錄與接收器程序整合邏輯，並以替身隔離外部邊界。 | `tests/NeoAirPlay.Framework.Test/` |
| App 單元與 UI E2E 測試 | 驗證 ViewModel 行為，並以 UI Automation 驅動 WPF 頁面導覽。 | `tests/NeoAirPlay.App.Test/` |
| 隨附執行元件 | 建置腳本整理接收程序、影音執行環境及 Bonjour 元件；App 專案負責複製到輸出。 | `src/NeoAirPlay.App/NeoAirPlay.App.csproj`、`scripts/` |

## 依賴與執行邊界

- Framework 目標為 `net10.0-windows`，不參考 WPF。服務介面與實作放在 Framework；WPF ViewModel、Commands、視窗及啟動組裝放在 App。
- `UxPlayProcessEngine` 以獨立程序執行 `engine/uxplay.exe`，設定其執行環境，並將標準輸出及錯誤寫入應用程式記錄。程序與 Bonjour 狀態可透過注入的介面替身隔離測試。
- 使用者設定存於 `%AppData%\NeoAirPlay\settings.json`；完整記錄存於 `%LocalAppData%\NeoAirPlay\Logs\`；影音執行環境登錄快取存於 `%LocalAppData%\NeoAirPlay\gstreamer-registry.bin`。
- `--e2e-profile <path>` 只供 App UI E2E 測試使用，將設定、記錄、GStreamer 快取及單一執行個體 Mutex 導向測試暫存 profile。正常啟動仍使用既有使用者資料位置與 Mutex。
- `scripts/build-engine.ps1` 會準備建置環境並產生引擎檔案；Bonjour staging、安裝與還原腳本管理 Bonjour 元件及 Windows 服務路徑，不屬於應用程式啟動流程。

## 驗證界線

- `.neo_harness/verify.ps1` 建置方案並執行標記為 `Unit` 的 Framework 與 App 測試；不執行 UI E2E。
- App E2E 需有可互動的 Windows 桌面工作階段，只驗證 WPF 啟動及頁面導覽，不連接 AirPlay 裝置或啟動接收器。
- 目前沒有 CI 工作流程、靜態分析設定或架構邊界檢查；依賴方向仍未由機器強制。
- 本文件所列的依賴方向屬於程式碼觀察，不是架構保證。使用者可見功能、區域網路需求、建置及授權說明以根目錄 README.md 為準。
