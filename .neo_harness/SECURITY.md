# 安全與信任邊界

本文件依目前程式與 [README.md](../README.md) 記錄可確認的安全邊界；不是安全稽核或授權意見。

## 應用程式權限

- 前端透過 Tauri IPC 呼叫已註冊命令。正式 capability 只包含 `core:default`；E2E 使用的 WDIO capability 位於測試覆寫設定，不會加入正式組態。
- Content Security Policy 限制頁面資源為本機來源及 Tauri IPC。前端沒有載入外部腳本或遠端內容。
- App 本身以一般使用者權限執行；依賴安裝腳本可透過 sudo、polkit、UAC 或平台安裝程式要求系統套件安裝授權。App 不建立防火牆規則。
- Tauri autostart plugin 只註冊登入啟動項目；設定為停用時會移除該註冊。

## 外部程序與網路

- Rust 使用 `std::process::Command` 的引數陣列啟動 UxPlay、ADB 與 scrcpy，不經由 shell 組合命令。
- App 啟動時使用 `fix-path-env` 恢復程序 `PATH`；macOS 與 Linux 會啟動使用者設定的登入 shell 讀取環境，因此 shell 啟動設定屬於信任邊界。
- Android 投影只可使用剛由 ADB 列舉且狀態為 `device` 的裝置序號。App 不執行 ADB 配對或 TCP/IP 連線命令。
- 引擎執行檔會從應用程式資源目錄或啟動時取得的系統 `PATH` 尋找。這些位置是程式碼執行信任邊界；使用者應只安裝或放置可信任來源的 UxPlay、ADB、scrcpy 及相依檔案。
- UxPlay 提供區域網路 AirPlay 接收服務。Linux 準備腳本會安裝並啟用 Avahi。macOS 使用系統 Bonjour；Windows 固定採用 UxPlay 官方 1.74 實驗來源的內建 mDNS。OS 防火牆與區域網路可達性仍需由使用者允許。
- App 會將 UxPlay、scrcpy 與 ADB 的輸出寫入本機診斷記錄。Android 投影啟動資訊可能含裝置型號與序號；分享記錄前應先檢查內容。

## 使用者資料與 E2E

- 設定及每日記錄存放在 Tauri path resolver 提供的使用者應用程式設定與記錄目錄。設定以 JSON 儲存，記錄以 JSONL 附加寫入。
- WDIO 測試以 `--e2e-profile` 將設定與記錄導向作業系統暫存目錄。此參數只在 Rust `wdio` feature 編譯時啟用；測試能力只由 `tauri.e2e.conf.json` 及 WDIO build script 啟用。
- 自動化測試不操作實際投影引擎，不會連接 Android 裝置，也不會更改登入啟動設定。

## 第三方元件與發佈

- 預設套件不含 UxPlay、GStreamer、ADB、scrcpy、Bonjour 或 Avahi 二進位檔。使用者安裝時需依上游授權與通知要求保留第三方文件。
- 本機可選擇將引擎檔案放在 `desktop/src-tauri/resources/engine/`；加入任何外部執行檔前，需確認散布授權、來源與必要的授權文件。
- 自動準備會下載並執行官方套件管理器或工具安裝腳本。UxPlay 來源固定於腳本記錄的 commit 並核對 SHA-256；作業系統套件信任其套件庫的簽章與驗證機制。Rust 不接受由前端傳入的安裝指令或 URL。
- GitHub Actions 權限為 `contents: read`，只建置、測試並上傳限期 Actions artifact，不建立 tag、推送版本檔或公開 Release。
- 專案未設定依賴弱點掃描、Windows 發行者憑證、Apple Developer ID 或執行檔來源驗證。macOS 僅使用 ad-hoc 簽章且未 notarize；這些發佈控制不能視為已完成。
