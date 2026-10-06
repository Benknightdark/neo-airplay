# 安全與信任邊界

本文件只記錄目前原始碼與 README 已明確描述的邊界；它不是安全稽核或法律意見。主要依據為根目錄 README.md 與下列程式檔。

## 權限與系統服務

- README 說明 Install-BonjourService.ps1 與 Restore-BonjourService.ps1 需要系統管理員權限。
- 安裝流程可從 64 位元或 32 位元 Program Files 讀取 Bonjour `mDNSResponder.exe`，並只從 64 位元 Program Files 或 Windows System32 讀取 x64 `dnssd.dll`。兩者都檢查 PE 架構、Bonjour 產品資訊與相同 FileVersion，再 staging 到 `%ProgramFiles%\NeoAirPlay\Bonjour`；不接受 SysWOW64/x86 DLL，也不讀取 App 可寫入路徑。既有服務會備份原始 ImagePath 與執行狀態；服務原先不存在時備份會標示未安裝，安裝透過 Bonjour 執行檔的 `-install` 建立服務。若舊備份無效，只有在目前服務路徑可信任或服務不存在時才依現況更新備份；目前服務路徑不可信時停止安裝。安裝腳本重設共用目錄 ACL、將擁有者設為 Administrators，並移除標準使用者的寫入權限。
- 一般啟動若服務停止、路徑不符或缺少服務項目，App 會說明操作後透過 UAC 啟動 elevated helper；拒絕或取消時不執行服務腳本。若服務在 Windows 中被停用，App 不會改變該設定。
- 還原腳本依備份的服務存在狀態，將既有服務路徑還原，或移除 Neo AirPlay 建立的服務；Bonjour 執行檔不會加入公開套件。
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
- App 下載版本固定的引擎 ZIP，先核對 App 內設定的外層 SHA-256，再檢查逐檔雜湊、必要檔案清單、路徑穿越與符號連結；公開 ZIP 排除 Bonjour。套件由 `scripts/Pack-EngineRuntime.ps1` 產生並附加到對應 GitHub Release。
- 任何發佈內容變更都應重新核對 README 所列授權文件與目前檔案來源。此處只轉述專案文件，沒有獨立驗證授權條款。

## 自動化與未知事項

- Harness 的單元測試使用程序與 Bonjour 替身；App UI E2E 以 `--e2e-profile` 把設定、記錄和 GStreamer 快取導向測試暫存目錄，並使用獨立 Mutex。測試不會啟動接收器或修改 Windows 啟動登錄值。Bonjour ACL 與服務路徑仍需在目標 Windows 機器人工驗收。
- `ci.yml` 僅有 `contents: read` 權限。建立 develop → main PR 的工作流程在 `workflow_run` 上執行，不 checkout 未信任的 develop 程式碼；GitHub App 私鑰只交給 token action，PR 驗證工作不會取得它。
- Release 需要自架 Windows runner 才能讀取可信任的 Bonjour 安裝。Release job 僅由 main push 啟動，先執行驗證及建置，之後才建立短期 GitHub App token；App 的 `Contents` 與 `Pull requests` 權限應只授予必要 repository。`main` 需要允許 App 繞過直接推送限制，因 Release 會寫回版本與引擎 metadata。
- 自架 runner 會執行 main 上的專案建置、測試及 PowerShell 腳本。能寫入 develop 的人因此能提供會在 runner 執行的程式碼；只應授權可信任維護者寫入 develop，限制 runner 的 repository 存取，並用隔離且不保存其他憑證或個人資料的主機。GitHub 設定無法由 repository workflow 自行保證。
- 目前沒有找到安全掃描、權限檢查或網路設定的自動化驗證。
- 本文件未確認防火牆連接埠、威脅模型或記錄內容中的敏感資料種類；相關決策需以實際需求與程式行為確認。
