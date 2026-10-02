# 安全與信任邊界

本文件只記錄目前原始碼與 README 已明確描述的邊界；它不是安全稽核或法律意見。主要依據為根目錄 README.md 與下列程式檔。

## 權限與系統服務

- README 說明 Install-BonjourService.ps1 與 Restore-BonjourService.ps1 需要系統管理員權限。
- 安裝流程會備份 Bonjour Service 原始 ImagePath，再切換服務執行檔位置。執行前應先閱讀腳本；需要還原時依 README 使用還原腳本。
- Harness 驗證入口不會執行這些服務管理腳本。

## 網路與外部程序

- Framework 服務會啟動隨附的接收程序，並將標準輸出及錯誤導入本機記錄。修改程序參數或執行環境時，同時檢查 `src/NeoAirPlay.Framework/Services/UxPlayProcessEngine.cs` 與 `BonjourServiceInspector.cs`。
- Android 服務使用 `ProcessStartInfo.ArgumentList` 呼叫隨附的 ADB 與 scrcpy，將選定裝置序號當成獨立引數，不透過 Shell 組合命令。ADB/scrcpy 輸出會寫入本機診斷記錄；啟動記錄包含裝置序號。
- Android 投影要求使用者在手機開啟 USB 偵錯並授權此電腦。無線裝置須先由使用者完成 Android 無線偵錯配對；App 只列舉 ADB 已連線裝置，不建立配對或連線。
- README 指出電腦與 Apple 裝置需在可互通的區域網路，且 Windows 防火牆可能需要允許 engine/uxplay.exe 通過私人網路。多播探索受網路隔離影響。
- 目前文件沒有列出防火牆連接埠或自動防火牆設定；不要從此文件推定連接埠或新增規則。

## 使用者資料與記錄

- README 與 `src/NeoAirPlay.Framework/Services/JsonSettingsStore.cs` 記載設定存於使用者 AppData；`FileAppLogger.cs` 將記錄寫入使用者 LocalAppData。接收程序輸出也會進入應用程式記錄。
- 檢視或分享診斷記錄前，先確認內容適合分享。專案未提供記錄去識別或分享前清理工具。

## 發佈限制

- README 記載 Bonjour 執行元件取自本機安裝，並指出其授權條款限制對外再散布；README 明確要求不要把這些執行檔放入公開下載包。
- scrcpy/ADB 執行環境從 Genymobile 官方 v4.1 Windows x64 發行包 staging，腳本核對固定 SHA-256，並保留官方封存檔內的 `LICENSE.txt`。scrcpy 專案 README 標示 Apache-2.0；發佈前仍須一併盤點該 Windows 封存包中各個第三方二進位檔的授權與通知。
- 任何發佈內容變更都應重新核對 README 所列授權文件與目前檔案來源。此處只轉述專案文件，沒有獨立驗證授權條款。

## 自動化與未知事項

- Harness 的單元測試使用程序與 Bonjour 替身；App UI E2E 以 `--e2e-profile` 把設定、記錄和 GStreamer 快取導向測試暫存目錄，並使用獨立 Mutex。測試不會啟動接收器或修改 Windows 啟動登錄值。
- 目前沒有找到安全掃描、權限檢查或網路設定的自動化驗證。
- 本文件未確認防火牆連接埠、威脅模型或記錄內容中的敏感資料種類；相關決策需以實際需求與程式行為確認。
