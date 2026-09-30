# 目前架構

本文件描述目前工作樹中的 Windows x64 桌面接收器；依據為程式碼、專案設定與 [README.md](../README.md)。以下關係依目前程式碼整理，不代表有自動架構檢查。

## 依賴概況

- App.xaml.cs 組裝日誌、設定儲存、登入啟動註冊、接收器引擎及 MainViewModel，然後建立 MainWindow。
- MainWindow 使用 MainViewModel；畫面由 Views/ 下的 XAML 視圖組成。
- MainViewModel 經 IReceiverEngine、ISettingsStore 與 IAppLogger 等服務介面管理接收器、設定及診斷記錄。
- UxPlayProcessEngine 透過 BonjourServiceInspector 檢查執行元件，並啟動 engine/ 下的外部接收程序。
- NeoAirPlay.csproj 將 engine/ 下的檔案納入建置與發佈輸出。

## 元件責任

| 元件 | 目前責任 | 證據 |
| :--- | :--- | :--- |
| 啟動與視窗 | App.xaml.cs 建立服務、載入設定並建立主視窗。MainWindow 提供系統匣操作。 | src/NeoAirPlay/App.xaml.cs、src/NeoAirPlay/MainWindow.xaml.cs |
| Views | Dashboard、Settings、Diagnostics XAML 視圖呈現接收器狀態、設定與記錄。 | src/NeoAirPlay/Views/ |
| ViewModel | MainViewModel 管理畫面狀態、接收器命令、設定與診斷記錄，並呼叫服務。 | src/NeoAirPlay/ViewModels/MainViewModel.cs |
| Models 與 Commands | Models 保存接收器設定、選項、狀態及記錄資料；Commands 提供同步與非同步 UI 命令。 | src/NeoAirPlay/Models/、src/NeoAirPlay/Commands/ |
| 接收器引擎 | IReceiverEngine 定義非同步啟停介面。UxPlayProcessEngine 檢查隨附元件、啟動外部程序、讀取輸出與錯誤，並回報生命週期狀態。 | src/NeoAirPlay/Services/IReceiverEngine.cs、src/NeoAirPlay/Services/UxPlayProcessEngine.cs |
| Bonjour 檢查 | BonjourServiceInspector 檢查專案內的服務執行元件及可用狀態。 | src/NeoAirPlay/Services/BonjourServiceInspector.cs |
| 設定與登入啟動 | JsonSettingsStore 讀寫使用者設定；StartupRegistration 管理目前使用者登入啟動設定。 | src/NeoAirPlay/Services/JsonSettingsStore.cs、src/NeoAirPlay/Services/StartupRegistration.cs |
| 記錄 | FileAppLogger 將記錄寫到使用者 LocalAppData，並提供記錄事件給介面。 | src/NeoAirPlay/Services/FileAppLogger.cs |
| 隨附執行元件 | 專案設定將 engine/ 下的內容複製到建置與發佈輸出。建置腳本整理接收程序、影音執行環境及 Bonjour 元件。 | src/NeoAirPlay/NeoAirPlay.csproj、scripts/ |

## 依賴與執行邊界

- App 啟動時組裝具體服務；MainWindow 使用 MainViewModel；ViewModel 經 IReceiverEngine 與 ISettingsStore 等介面呼叫服務。這是目前程式的依賴方向，尚無測試或靜態工具強制該方向。
- UxPlayProcessEngine 以獨立程序執行 engine/uxplay.exe，設定其執行環境，並將標準輸出及錯誤寫入應用程式記錄。
- 使用者設定存於 %AppData%\NeoAirPlay\settings.json；完整記錄存於 %LocalAppData%\NeoAirPlay\Logs\。README 另列出影音執行環境登錄快取位置。
- scripts/build-engine.ps1 會準備建置環境並產生引擎檔案；scripts/Stage-BonjourRuntime.ps1、Install-BonjourService.ps1 與 Restore-BonjourService.ps1 管理 Bonjour 元件及 Windows 服務路徑。這些操作不是應用程式啟動流程的一部分。

## 已知限制

- 目前只找到一個應用程式專案；沒有找到測試專案、CI 工作流程、靜態分析設定或架構邊界檢查。
- 本文件所列的依賴方向屬於程式碼觀察，不是機器強制的不變量。
- 使用者可見功能、區域網路需求、建置及授權說明以根目錄 README.md 為準。
