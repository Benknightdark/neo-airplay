# 安全與信任邊界

本文件依目前程式碼與 [README.md](../README.md) 記錄安全控制、信任邊界與發佈限制。

## 應用程式與 IPC 權限

- **最小 IPC 與 CSP**：正式 capability 僅開放 `core:default`，CSP 限制頁面只載入本機資源與 Tauri IPC；WDIO capability 與 `--e2e-profile` 暫存目錄隔離僅在 Rust `wdio` feature 與 `desktop-test/e2e/tauri.e2e.conf.json` 啟用。
- **執行身分與最小提權**：App 以一般使用者權限執行，不自動建立防火牆規則；僅在安裝系統套件時透過 `sudo`、`pkexec` 或 Windows UAC 要求授權。Windows UAC 僅以 `-ElevatedPrerequisitesOnly` 安裝系統層級前置項目（補裝 WinGet 與 `-Development` 的機器層級套件），不以系統管理員身分處理 `engine/`、MSYS2、UxPlay 或執行期檢查，並驗證 `OriginalUserSid` 格式。登入啟動僅透過 Tauri `autostart` 插件註冊或移除。

## 外部程序與本機信任邊界

- **無 Shell 參數執行與絕對系統路徑**：Rust 皆以 `std::process::Command` 引數陣列啟動 `uxplay`、`adb`、`scrcpy` 與 `/usr/bin/dns-sd`，並以 `%SystemRoot%\System32` 絕對路徑啟動 Windows `powershell.exe` 與 `tasklist.exe`（且不將 `engine/` 注入安裝或探測程序 `PATH`）。不接受前端傳入自訂安裝命令或下載 URL；macOS Terminal 安裝驗證路徑不含控制字元，並以 `osascript` `on run argv` 傳遞命令列，避免 AppleScript 字串內插。
- **執行路徑與環境信任邊界**：引擎只從 Debug 的 `desktop/engine/` 或 Release 使用者資料目錄的 `engine/` 載入，不回退系統 `PATH`、MSYS2 或 WinGet；`find_executable` 與 `is_executable_file` 逐層檢查 `symlink_metadata` 拒絕符號連結目錄與符號連結執行檔。macOS／Linux 啟動時透過 `fix-path-env` 讀取使用者登入 shell 環境，因此本機 `engine/` 目錄與使用者 shell 啟動設定同屬程式碼執行信任邊界。
- **輸入限制、檔案原子保護與日誌上限**：Android 只接受剛由 ADB 列舉且狀態為 `device` 的序號（不執行配對或 TCP/IP 連線）；AirPlay renderer 受平台 allowlist 限制。`setup/`（Unix `0700`）、釋放腳本（安裝前再次重寫以消除 TOCTOU）、狀態 JSON 與 `seed_bundled_engine_in_dir` 均採 `create_new` 原子寫入並拒絕符號連結；`clear_engine_directory` 與 `clear_gstreamer_registry` 預先拒絕符號連結（含懸空符號連結）。`read_install_log` 拒絕符號連結並限制回傳尾端 `1 MiB`，安裝啟動前若超過 `5 MiB` 自動截斷保留尾端。
- **診斷記錄隱私**：每日 JSONL 診斷記錄包含外部程序輸出及 Android 裝置型號／序號，對外提供記錄前應先檢視內容。

## 第三方供應鏈與發佈控制

- **固定來源、傳輸強化與簽章／雜湊驗證**：預設套件不含第三方投影二進位檔（`desktop/engine/` 除 `.GITKEEP` 外皆由 Git 忽略）。安裝腳本統一強制 HTTPS、TLS 1.2+ 與防 HTTP 降級重導向檢查，並鎖定 UxPlay commit、scrcpy、`rustup-init` `1.29.3`、Homebrew `install.sh` commit、Bun 與 Node.js 之 SHA-256（失敗自動清除暫存檔）。`install.sh` 套用 `umask 077` 與無符號連結原子 staging 且不啟用第三方 Fedora COPR；`install.ps1` 執行 Zip Slip 路徑檢查與 ReparsePoint/UNC 防護，補裝 WinGet 鎖定 `Microsoft.WinGet.Client` `1.29.380` 並驗證 Microsoft Authenticode 簽章，且保留並自動還原 MSYS2 `pacman` GPG 簽章驗證（`SigLevel = Required DatabaseOptional`）。
- **依賴弱點掃描與上游受阻評估**：CI 針對正式前端 `desktop/` 執行 `bun audit`（目前 0 已知弱點）；後端 Rust 鎖定檔亦以 OSV（RustSec + GHSA）盤點並修補 `tokio`（`>=1.54.1`，`RUSTSEC-2026-0386`）、`rustix`（`>=1.1.6`，`RUSTSEC-2026-0486`），且將 `chrono` 遷移至 `jiff`（`neo-airplay` 編譯目標與 `desktop-test/backend/Cargo.lock` 皆已移除 `chrono` 依賴）。下列項目屬上游鎖定、未編譯或不可達風險並持續追蹤：
  - Linux 限定之 `glib@0.18.5`（`RUSTSEC-2024-0429`，本專案未呼叫 `VariantStrIter`）與 `proc-macro-error@1.0.4`（`RUSTSEC-2024-0370`），受限於 `tauri@2.x` Linux `gtk@^0.18`／`webkit2gtk@^2`。
  - Linux 限定之 `zvariant@5.15.0`、`zvariant_derive@5.15.0` 與 `zbus_names@4.3.4`（`RUSTSEC-2026-0370`／`0371`／`0372`），受限於 `tauri-plugin-single-instance@2.x` 之 `zbus@^5.9`。
  - `chrono@0.4.45`（`RUSTSEC-2026-0543`）僅作為 `tauri-utils` → `serde_with` 未啟用可選功能之鎖定項留存於 `desktop/src-tauri/Cargo.lock`，`cargo tree --all-features --target all --edges all -i chrono` 為空，實際不會編譯進正式二進位檔。
  - 僅存於 `desktop-test/` 測試依賴的 `extract-zip@2.0.1`（`GHSA-jmr9-qjv8-65gv`、`GHSA-7pqw-9j4j-h8q3`，上游無修補版）；因 [wdio.conf.ts](../desktop-test/e2e/wdio.conf.ts#L48) 採用 `driverProvider: "embedded"`，不觸發 `@puppeteer/browsers` 下載解壓，且 `desktop-test/` 不進入正式發佈檔，待 `@wdio/tauri-service@2.x` 正式版發佈後再評估升級。
- **CI/CD 最小權限**：CI 驗證與 CD 打包 jobs 僅具 `contents: read`；自動建立 PR 與發布 Release 的 jobs 僅在必要範圍授予 `pull-requests: write` 與 `contents: write`。
- **尚未啟用的發佈控制**：`desktop/` 已於 CI 啟用 `bun audit`；但自動化 Rust `cargo-audit` CI 步驟、Windows 程式碼簽章憑證與執行檔來源驗證尚未配置，macOS 亦僅使用 ad-hoc 簽章且未執行 Apple Developer ID Notarization。
