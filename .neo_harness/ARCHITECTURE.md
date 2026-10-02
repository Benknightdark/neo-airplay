# 目前架構

本文件描述目前工作樹中的 Windows x64 桌面接收器；依據為程式碼、專案設定與 [README.md](../README.md)。以下關係依目前程式碼整理，不代表有自動架構檢查。

## 依賴概況

- `NeoAirPlay.App` 是 WPF 啟動與呈現層。`App.xaml.cs` 組裝 Framework 服務、載入設定、建立 `MainViewModel` 與 `MainWindow`。
- `MainWindow` 使用 `MainViewModel`；畫面由 `Views/` 下的 XAML 視圖組成，ViewModel 與 UI 命令留在 App。
- `NeoAirPlay.Framework` 包含 `NeoAirPlay.Models` 與 `NeoAirPlay.Services`，提供設定、記錄、登入啟動註冊、Bonjour 檢查、UxPlay 外部程序生命週期，以及 ADB 裝置列舉和 scrcpy 外部程序生命週期服務。
- App 透過 `IReceiverEngine`、`IAndroidMirrorEngine`、`ISettingsStore`、`IAppLogger`、`IStartupRegistration` 等介面使用服務；啟動時由 App 層的引擎套件準備器在主視窗顯示後檢查並補齊缺少的引擎檔案。
- `NeoAirPlay.App.csproj` 將可散布的 `engine/` 檔案複製到輸出；Bonjour 元件不會複製到 App 輸出。App 從版本固定的 GitHub Release 套件補齊引擎缺件，並把 Bonjour 放在受保護的 `%ProgramFiles%\NeoAirPlay\Bonjour`；套件產生腳本位於 `scripts/Pack-EngineRuntime.ps1`。

## 專案責任

| 專案或元件 | 責任 | 證據 |
| :--- | :--- | :--- |
| App 啟動與視窗 | 組裝服務、啟動主視窗並提供系統匣操作。 | `src/NeoAirPlay.App/App.xaml.cs`、`src/NeoAirPlay.App/MainWindow.xaml.cs` |
| Views 與 ViewModel | Dashboard、Settings、Diagnostics 視圖呈現接收器狀態、設定與記錄；ViewModel 管理畫面狀態及命令。 | `src/NeoAirPlay.App/Views/`、`src/NeoAirPlay.App/ViewModels/MainViewModel.cs` |
| Commands | 提供同步與非同步 UI 命令。 | `src/NeoAirPlay.App/Commands/` |
| Framework Models | 保存接收器設定、選項、狀態及記錄資料。 | `src/NeoAirPlay.Framework/Models/` |
| Framework Services | 管理接收器程序、Bonjour 執行元件檢查、設定、登入啟動註冊及記錄。 | `src/NeoAirPlay.Framework/Services/` |
| Android 投影 | 解析 `adb devices -l` 裝置狀態，依選取序號啟動/停止 scrcpy；不執行無線配對。 | `src/NeoAirPlay.Framework/Services/ScrcpyAndroidMirrorEngine.cs`、`AndroidCommandRunner.cs` |
| Framework 單元測試 | 驗證設定儲存、檔案記錄與接收器程序整合邏輯，並以替身隔離外部邊界。 | `tests/NeoAirPlay.Framework.Test/` |
| App 單元與 UI E2E 測試 | 驗證 ViewModel 行為，並以 UI Automation 驅動 WPF 頁面導覽。 | `tests/NeoAirPlay.App.Test/` |
| 隨附執行元件 | 建置腳本整理 UxPlay、影音執行環境、Bonjour 元件及官方 scrcpy/ADB 套件；App 專案複製可散布的輸出，首次啟動時可下載版本固定的引擎包，Bonjour 安裝至受保護的系統共用目錄。 | `src/NeoAirPlay.App/NeoAirPlay.App.csproj`、`scripts/` |

## 依賴與執行邊界

- Framework 目標為 `net10.0-windows`，不參考 WPF。服務介面與實作放在 Framework；WPF ViewModel、Commands、視窗及啟動組裝放在 App。
- `UxPlayProcessEngine` 以獨立程序執行 `engine/uxplay.exe`，設定其執行環境，並將標準輸出及錯誤寫入應用程式記錄。程序與 Bonjour 狀態可透過注入的介面替身隔離測試。
- `ScrcpyAndroidMirrorEngine` 以隨附的 ADB 執行 `devices -l`，將 `device`、`unauthorized`、`offline` 分別呈現給使用者，再以獨立程序啟動 `engine/android/scrcpy.exe --serial <序號>`。USB 裝置與已透過 ADB 配對的 TCP/IP 裝置共用此流程。
- App 建立兩個獨立引擎與狀態，Android 啟停不會停止 AirPlay；關閉 App 時兩個程序都會停止。
- App 在主視窗顯示後背景檢查引擎檔案；缺件時從版本固定的 GitHub Release 下載套件，驗證外層 SHA-256、內部逐檔 SHA-256 和安全路徑後才安裝。準備期間會暫停啟動／刷新命令；失敗時記錄狀態並於下次 App 啟動重試。E2E profile 不執行下載或管理員操作。
- 若 Bonjour Service 已存在但尚未指向受保護的 `%ProgramFiles%\NeoAirPlay\Bonjour`，App 先顯示服務變更說明，再透過 Windows UAC 啟動 elevated helper 執行 Bonjour staging 與安裝腳本；安裝腳本重設目錄 ACL，限制修改者為 Administrators，並授予必要的讀取權。若本機沒有 Bonjour，顯示人工安裝資訊，Android 不受影響。
- 使用者設定存於 `%AppData%\NeoAirPlay\settings.json`；完整記錄存於 `%LocalAppData%\NeoAirPlay\Logs\`；影音執行環境登錄快取存於 `%LocalAppData%\NeoAirPlay\gstreamer-registry.bin`。
- `--e2e-profile <path>` 只供 App UI E2E 測試使用，將設定、記錄、GStreamer 快取及單一執行個體 Mutex 導向測試暫存 profile。正常啟動仍使用既有使用者資料位置與 Mutex。
- `scripts/build-engine.ps1` 會準備建置環境並產生引擎檔案；`scripts/Pack-EngineRuntime.ps1` 產生不含 Bonjour 的 Release 引擎包與 App SHA 設定。Bonjour staging、安裝與還原腳本可由人工執行；App 也會在使用者確認並通過 UAC 後呼叫 staging／安裝腳本。

## 驗證界線

- `.neo_harness/verify.ps1` 建置方案並執行標記為 `Unit` 的 Framework 與 App 測試；不執行 UI E2E。
- App UI E2E 需有可互動的 Windows 桌面工作階段，驗證 WPF 啟動、頁面導覽及 Android 控制項，不連接裝置或啟動接收器。
- `Category=HardwareE2E` 會啟動本機 UxPlay 與真實 scrcpy Android 投影；一項測試驗證 Framework 引擎並行，另一項透過 WPF 總覽啟動 USB 投影。測試要求互動式桌面、`%ProgramFiles%\NeoAirPlay\Bonjour` 中的 Bonjour Service 正在執行及 USB Android 裝置已授權。
- `.github/workflows/ci.yml` 在 develop push 與 main PR 執行 Harness；develop CI 成功後由 `.github/workflows/promote-develop.yml` 建立／更新 PR 並設定 auto-merge。main push 由 `.github/workflows/release.yml` 在標記為 `neo-airplay-release` 的 Windows x64 runner 建置及發佈版本化套件。
- CI 目前執行 build 與 Unit tests；不執行 UI E2E、硬體 E2E 或靜態架構檢查。依賴方向仍未由機器強制。
- 本文件所列的依賴方向屬於程式碼觀察，不是架構保證。使用者可見功能、區域網路需求、建置及授權說明以根目錄 README.md 為準。
