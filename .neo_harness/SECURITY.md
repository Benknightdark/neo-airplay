# 安全與信任邊界

本文件依目前程式碼與 [README.md](../README.md) 記錄安全控制、信任邊界與發佈限制。

## 應用程式與 IPC 權限

- **最小 IPC 與 CSP**：正式 capability 僅開放 `core:default`，CSP 限制頁面只載入本機資源與 Tauri IPC；WDIO capability 與 `--e2e-profile` 暫存目錄隔離僅在 Rust `wdio` feature 與 `desktop-test/e2e/tauri.e2e.conf.json` 啟用。
- **執行身分與提權**：App 以一般使用者權限執行，不自動建立防火牆規則；僅依賴安裝腳本在安裝系統套件時透過 sudo、polkit 或 UAC 要求作業系統授權。登入啟動僅透過 Tauri `autostart` 插件註冊或移除。

## 外部程序與本機信任邊界

- **無 Shell 參數執行**：Rust 皆以 `std::process::Command` 引數陣列啟動 `uxplay`、`adb`、`scrcpy` 與 `/usr/bin/dns-sd`，不經 shell 字串拼接；不接受前端傳入自訂安裝命令或下載 URL。
- **執行路徑與環境信任邊界**：引擎只從 Debug 的 `desktop/engine/` 或 Release 使用者資料目錄的 `engine/` 載入，不回退系統 `PATH`、MSYS2 或 WinGet；macOS／Linux 啟動時透過 `fix-path-env` 讀取使用者登入 shell 環境，因此本機 `engine/` 目錄與使用者 shell 啟動設定同屬程式碼執行信任邊界。
- **輸入限制與檔案清理保護**：Android 只接受剛由 ADB 列舉且狀態為 `device` 的序號（不執行配對或 TCP/IP 連線）；AirPlay renderer 受平台 allowlist 限制。工具維護清理 `engine/` 時會拒絕目錄本身為符號連結的情況，目錄內的符號連結只移除連結本身。
- **診斷記錄隱私**：每日 JSONL 診斷記錄包含外部程序輸出及 Android 裝置型號／序號，對外提供記錄前應先檢視內容。

## 第三方供應鏈與發佈控制

- **固定來源與雜湊驗證**：預設套件不含第三方投影二進位檔（`desktop/engine/` 除 `.GITKEEP` 外皆由 Git 忽略）。自動安裝腳本固定 UxPlay commit 與 Node.js／元件 SHA-256，系統套件則依賴各 OS 套件庫簽章機制。
- **CI/CD 最小權限**：CI 驗證與 CD 打包 jobs 僅具 `contents: read`；自動建立 PR 與發布 Release 的 jobs 僅在必要範圍授予 `pull-requests: write` 與 `contents: write`。
- **尚未啟用的發佈控制**：目前未配置自動依賴弱點掃描、Windows 程式碼簽章憑證、執行檔來源驗證，macOS 亦僅使用 ad-hoc 簽章且未執行 Apple Developer ID Notarization。
