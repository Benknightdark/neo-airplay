# 程序生命週期與復原

本文件依據 UxPlayProcessEngine.cs、BonjourServiceInspector.cs、App.xaml.cs 與 README.md 描述目前可觀察的行為。

## 啟動條件與失敗狀態

- 引擎啟動前檢查 engine/uxplay.exe、專案內 Bonjour 執行元件，以及 GStreamer runtime/bin 與 runtime/lib/gstreamer-1.0 目錄。
- 缺少必要元件時狀態設為 DependencyMissing，並寫入錯誤記錄；程序啟動失敗時狀態設為 Faulted。
- 引擎使用非同步生命週期鎖避免啟停操作重疊。若程序已在執行，重複啟動要求會直接返回。

## 停止與意外結束

- 停止時先嘗試關閉程序主視窗；若無法正常關閉，會結束程序樹。
- 正常關閉最多等待五秒；逾時後結束程序樹並等待程序結束。
- 程序意外結束時，結束碼 0 回報 Stopped，非 0 回報 Faulted，並記錄結果。程式碼中沒有自動重新啟動流程。
- 應用程式結束時會呼叫引擎 Dispose，嘗試停止接收程序。

## 診斷與人工復原

- 接收器引擎狀態、啟動失敗原因與子程序輸出可由應用程式診斷頁及記錄檔查看。完整記錄位置為 %LocalAppData%\NeoAirPlay\Logs\；介面保留最近 300 筆。
- 若狀態顯示缺少元件，先依 README.md 檢查建置與 Bonjour staging 步驟，不要在 Harness 驗證中自動執行建置環境或系統服務安裝腳本。
- README 記錄 Bonjour Service 安裝與還原命令；服務切換需要系統管理員權限，操作前後應確認服務路徑。

## 驗證邊界

- .neo_harness/verify.ps1 只建置 NeoAirPlay.slnx。專案目前沒有找到測試專案、CI 閉環或程序生命週期的自動化測試。
- 本文件不承諾網路探索、影音播放或外部服務在特定環境下必定成功；這些行為仍需在目標 Windows 與區域網路中實際確認。
