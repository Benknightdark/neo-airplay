# 程序生命週期與復原

本文件依據 `src/NeoAirPlay.Framework/Services/UxPlayProcessEngine.cs`、`BonjourServiceInspector.cs`、`src/NeoAirPlay.App/App.xaml.cs` 與 README.md 描述目前可觀察的行為。

## 啟動條件與失敗狀態

- 引擎啟動前檢查 `engine/uxplay.exe`、受保護的系統共用 Bonjour 執行元件，以及 GStreamer `runtime/bin` 與 `runtime/lib/gstreamer-1.0` 目錄。
- 缺少必要元件時狀態設為 `DependencyMissing`，並寫入錯誤記錄；程序啟動失敗時狀態設為 `Faulted`。
- 引擎使用非同步生命週期鎖避免啟停操作重疊。若程序已在執行，重複啟動要求會直接返回。
- App 顯示主視窗後背景檢查 AirPlay 與 Android 引擎檔案；缺少時下載固定版本套件並驗證後安裝。下載或安裝失敗會顯示在總覽及記錄，不會關閉 App，且下次啟動會再檢查。
- 引擎準備期間停用啟動 AirPlay、刷新 Android 裝置及啟動 Android 投影命令。E2E profile 會略過引擎準備。
- 一般 App 啟動時，Bonjour 檢查先於引擎套件準備，因此引擎下載失敗不會略過服務處理。服務停止、路徑不符或項目不存在時，App 分別從可信任 Program Files / Program Files (x86) 尋找 Bonjour responder，以及從 64 位元 Program Files / Windows System32 尋找相同版本的 x64 DNS-SD DLL；通過檢查後顯示簡短原因並要求 UAC，成功後重新檢查服務狀態。
- 找不到 responder 時，App 提供 Bonjour 安裝資訊；若找到 responder 但缺少同版本 x64 DLL，會明確指出缺少的 DNS-SD 元件。UAC 取消、服務停用或安裝失敗時，AirPlay 不啟動，Android 仍可用。E2E profile 不執行 Windows 服務操作；`--start` 會等 Bonjour 與引擎都就緒才啟動 AirPlay。

## 停止與意外結束

- 停止時先嘗試關閉程序主視窗；若無法正常關閉，會結束程序樹。
- 正常關閉最多等待五秒；逾時後結束程序樹並等待程序結束。
- 程序意外結束時，結束碼 0 回報 `Stopped`，非 0 回報 `Faulted`，並記錄結果。程式碼中沒有自動重新啟動流程。
- 應用程式結束時會呼叫引擎 Dispose，嘗試停止接收程序。

## Android 投影

- `ScrcpyAndroidMirrorEngine` 先確認 `engine/android` 的 ADB、scrcpy、server、必要 DLL 與授權文件，再呼叫 `adb devices -l`；ADB 查詢最長等待 15 秒，失敗時回報錯誤，不會自動連線或配對。
- 裝置狀態 `device` 可選擇投影；`unauthorized` 顯示需在手機接受 USB 偵錯授權；`offline` 提示重新連接或檢查無線偵錯連線。
- scrcpy 以獨立程序開啟投影視窗。使用者關閉視窗時，程序退出狀態會反映到 Android 區塊；App 關閉時也會停止 scrcpy。
- 停止流程先關閉視窗，最多等待五秒，再結束程序樹。非零結束碼會記錄為錯誤。Android 與 UxPlay 的生命週期鎖和 UI 狀態彼此分開。
- Android 11 以上支援 scrcpy 音訊轉送；更舊 Android 版本的畫面仍可投影，但不支援這條音訊轉送路徑。
- scripts/scrcpy-runtime-files.txt 是唯一的 scrcpy 必要檔案清單；staging 腳本讀取此清單，Framework 將同一份清單嵌入組件供啟動檢查使用。
- 投影執行中若 ADB 刷新失敗或執行元件檢查失敗，狀態仍維持 Running，讓停止控制保持可用；刷新恢復後會繼續顯示投影中。

## 診斷與人工復原

- 接收器引擎狀態、啟動失敗原因與子程序輸出可由應用程式診斷頁及記錄檔查看。完整記錄位置為 `%LocalAppData%\NeoAirPlay\Logs\`；介面保留最近 300 筆。
- 若狀態顯示缺少元件，先依 README.md 檢查建置與 Bonjour staging 步驟，不要在 Harness 驗證中自動執行建置環境或系統服務安裝腳本。
- README 記錄 Bonjour Service 安裝與還原命令；服務建立或路徑切換需要系統管理員權限，操作前後應確認服務狀態與路徑。有效備份可還原原路徑，或在服務原先不存在時移除新建服務；舊備份無效時只在目前服務路徑可信任或服務不存在的情況下依現況更新，否則停止安裝。

## 驗證界線

- `.neo_harness/verify.ps1` 建置 `NeoAirPlay.slnx` 並執行標記為 `Unit` 的 Framework 與 App 單元測試；不會啟動外部 UxPlay 程序或執行 Bonjour 服務管理腳本。
- `.github/workflows/ci.yml` 在 develop push 與 main PR 執行同一驗證入口；develop 驗證成功後，`promote-develop.yml` 建立／更新到 main 的 PR 並設定 auto-merge。main push 由 `release.yml` 重跑驗證，再建立版本化 Release。
- Release workflow 以觸發該 run 的 main commit 建置。版本取目前 App 版本及既有穩定 Release tags 中較大的值後升 patch；若另一個工作流程先建立了相同 tag，發佈腳本會停止而不覆寫別次建置，應重跑失敗的 Release workflow 以取得下一版。
- `NeoAirPlay.App.Test` 的 E2E 測試需可互動的 Windows 桌面工作階段，使用 UI Automation 驗證實際 WPF 視窗及導覽。測試不連接 AirPlay 裝置、不啟動接收器，並以暫存 profile 隔離使用者資料。
- `Category=HardwareE2E` 另外啟動真實 UxPlay 與 scrcpy，包含 Framework 引擎並行測試及透過 WPF 總覽控制 USB Android 的測試；驗證 scrcpy 視窗出現，以及停止 Android 後 AirPlay 程序仍在運行。這不會自動選擇 iPhone/iPad 的 AirPlay 來源，也不量測實際音訊輸出。
- 單元與 UI E2E 測試不驗證區域網路探索、影音播放或外部 Bonjour Service 在目標環境中的實際行為；這些仍需在目標 Windows 與區域網路中確認。
- CI 不執行 UI E2E；若需確認 WPF 實際導覽，仍需在互動式桌面執行：`dotnet test tests/NeoAirPlay.App.Test/NeoAirPlay.App.Test.csproj --filter "Category=E2E"`。
