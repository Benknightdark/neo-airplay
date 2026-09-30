# NeoAirPlay Framework 抽離與測試

## 目的與背景

將目前單一 WPF 專案中的模型與服務抽離至 `NeoAirPlay.Framework`，將應用專案改名為 `NeoAirPlay.App`，並新增 `NeoAirPlay.Framework.Test`、`NeoAirPlay.App.Test`。目前方案只有 `src/NeoAirPlay/NeoAirPlay.csproj`，沒有測試專案。Framework 需要容納 Windows Registry、Service Control Manager 和外部程序整合，因此目標為 `net10.0-windows`，不啟用 WPF。

## 目前與預期行為

- 目前 `App.xaml.cs` 組裝日誌、設定儲存、登入啟動註冊及 UxPlay 引擎；`MainViewModel`、模型和服務都位於單一 WPF 專案。
- 預期 App 只依賴 Framework；WPF ViewModel、命令、視窗及啟動組裝仍留在 App。Framework 包含既有模型、服務介面和服務實作。
- 保留 `NeoAirPlay.exe`、`NeoAirPlay` 命名空間、使用者資料路徑、Mutex 及登入啟動登錄值。引擎檔案仍由 App 專案複製至輸出。
- Framework 與 App 各有單元測試；App 測試另包含透過 UI Automation 驅動實際 WPF 程序的 E2E。E2E 使用暫存 profile，不啟動接收引擎或儲存設定。

## 限制與決策

- 依賴方向為 App → Framework；Framework 不參考 WPF。
- 維持既有 `NeoAirPlay.Models`、`NeoAirPlay.Services` 命名空間，新增 `IStartupRegistration` 供 ViewModel 依賴抽象。
- 以內部可注入替身隔離程序與 Bonjour/Windows 狀態；單元測試不得連接真實 UxPlay、Bonjour Service 或系統登錄。
- 測試採 xUnit v3、VSTest 與 FlaUI UIA3。Harness 預設執行建置及單元測試；E2E 使用獨立命令並要求互動式 Windows 桌面。
- 不新增 CI 工作流程；目前專案沒有 CI 設定。

## 工作順序與進度

1. [x] 新增 Framework 專案並遷移 Models、Services；接上 App 的專案參考與服務組裝。
2. [x] 將 WPF 專案資料夾及 csproj 改名為 `NeoAirPlay.App`，更新方案與 README 發佈路徑。
3. [x] 建立 Framework 單元測試及必要的程序/Windows 邊界替身。
4. [x] 建立 App 單元與 UI E2E 測試，加入隔離 profile 和穩定的 UI Automation ID。
5. [x] 更新 Harness 驗證入口與架構/可靠性/命令文件。
6. [x] 執行方案建置、單元測試、UI E2E 和 Release publish；核對文件連結及輸出內容。

## 發現

- `MainViewModel` 使用 WPF Dispatcher、Clipboard、`ICommand`，應留在 App。
- `UxPlayProcessEngine` 依賴 `AppContext.BaseDirectory\engine`；Framework 呼叫時仍以 App 程序為基準，engine Content 項目需留在 App csproj。
- `BonjourServiceInspector` 從 engine 路徑尋找根目錄的 `NeoAirPlay.slnx`；方案檔名不變，該偵測方式可繼續使用。
- `MainWindow` 關閉時會隱藏至系統匣；測試 profile 需允許視窗關閉以便自動化測試結束程序。

## 驗證

- SDK：`dotnet --version` 回報 `10.0.401`。
- `& .\\.neo_harness\\verify.ps1`：建置方案成功，0 警告、0 錯誤；Framework 7/7 與 App 4/4 單元測試通過。
- `dotnet test tests/NeoAirPlay.App.Test/NeoAirPlay.App.Test.csproj --filter "Category=E2E"`：互動式桌面 UI E2E 1/1 通過。
- `dotnet publish src/NeoAirPlay.App/NeoAirPlay.App.csproj -c Release -o dist/NeoAirPlay-Release`：成功，輸出包含 `NeoAirPlay.exe` 與 365 個 engine 檔案。
- 初次使用 xUnit v3 預設 MTP 套件執行舊 VSTest 模式時，.NET 10 SDK 拒絕執行；測試專案改用 `xunit.v3.mtp-off` 4.0.1 保留 VSTest，之後 Harness 與兩組測試皆通過。
- 已檢查本次更新的 README、AGENTS 及 Harness 文件相對連結，並確認命令表路徑及發佈輸出內容。

## 復原方式

以 Git 還原本計畫涵蓋的專案/文件變更，恢復單一 `src/NeoAirPlay/NeoAirPlay.csproj` 及原方案參考。測試 profile 僅在暫存目錄產生資料；不改動使用者設定或 Windows 啟動登錄值。

## 結果

已完成專案分層、測試專案、隔離測試替身、Harness 驗證入口及文件更新。`NeoAirPlay.App` 的輸出組件名稱仍為 `NeoAirPlay`，使用者資料位置與單一執行個體識別也保持原值。

E2E 只涵蓋 WPF 啟動與頁面導覽；它不驗證真實 AirPlay 網路探索、Bonjour 服務或影音播放。Release publish 僅確認本機輸出完整，不代表可將本機 Bonjour 執行元件對外散布。
