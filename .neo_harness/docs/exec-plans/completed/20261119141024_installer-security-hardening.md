# 小工具安裝流程安全性強化執行計畫

## 目的與背景

盤點 Neo AirPlay 跨平台投影工具（UxPlay、GStreamer、ADB、scrcpy）及開發環境（Rust、Node.js、Bun、Homebrew、WinGet、MSYS2）的檢查、下載、提權、暫存與執行流程後，發現 Windows MSYS2 停用 GPG 簽章、Windows 全腳本 UAC 提權導致本機權限提升（LPE）、部分遠端腳本與第三方套件庫未鎖定版本／雜湊、Windows 系統命令受 `PATH` 劫持，以及符號連結／AppleScript／日誌邊界防護不足等風險。本計畫落實全套安全強化措施並補齊自動化驗證。

## 目前與預期行為

- **修改前**：
  - `install.ps1` 將 MSYS2 `pacman.conf` 改寫為 `SigLevel = Never`；UAC 提權執行整支腳本，使系統管理員程序直接執行與讀取標準使用者目錄（`engine/`、`$env:NEO_AIRPLAY_MSYS2_ROOT`、`OriginalUserProfile`）；`rustup-init.exe` 比對同來源浮動 `.sha256`；`Microsoft.WinGet.Client` 未鎖定版本或驗簽；`Expand-Archive` 未做 Zip Slip 檢查。
  - `install.sh` 執行浮動 `HEAD` 的 Homebrew 與 `sh.rustup.rs` 腳本；ARM64 Fedora 自動啟用第三方 COPR `zeno/scrcpy`；下載缺乏 `--proto-redir '=https'` 與失敗後自動清理；覆寫與暫存檔缺乏 `umask 077`、原子寫入與符號連結防護。
  - Rust 後端以裸名稱呼叫 `powershell.exe` / `tasklist.exe` 並將 `engine_dir` 置於 `PATH` 前端；`setup_dir` 釋放腳本與 `seed_bundled_engine`、`find_executable`、`read_install_log` 未全面拒絕符號連結與未設權限／大小上限；macOS `osascript` 以字串內插組裝命令列。
- **修改後**：
  - `install.ps1` 保留並自動修復 MSYS2 `SigLevel = Required DatabaseOptional` 與 `msys2-keyring`；UAC 僅透過 `-ElevatedPrerequisitesOnly` 執行 WinGet 與機器層級開發套件，其餘全部以一般使用者身分執行；強制 HTTPS/TLS 1.2+ 防降級下載與 SHA-256 驗證、鎖定 `rustup-init` `1.29.3` 雜湊、鎖定並驗簽 `Microsoft.WinGet.Client` `1.29.380`、Zip Slip 防護與 ReparsePoint/SID/系統路徑檢查。
  - `install.sh` 設定 `umask 077` 與統一暫存目錄清理；鎖定 Homebrew `install.sh` commit 與 `rustup-init` `1.29.3` 雜湊；移除第三方 COPR 啟用；統一 `download_verified`（防重導向降級 + 逾時 + 雜湊）與無符號連結原子 staging，檢查階段亦拒絕符號連結工具。
  - Rust 後端與啟動腳本使用 `%SystemRoot%\System32` 絕對路徑執行系統工具且不將 `engine_dir` 注入安裝或探測程序 `PATH`；`setup_dir`（`0700`）與檔案（`0600`/`0700`）使用 `create_new` 原子寫入並拒絕符號連結；安裝／預熱前重寫腳本消除 TOCTOU；macOS `osascript` 改用 `on run argv` 與控制字元驗證；`read_install_log` 限制尾端 `1 MiB` 並拒絕符號連結，安裝啟動前超過 `5 MiB` 自動修剪。

## 限制與決策

- **Homebrew 安裝策略（方案 A）**：鎖定 `Homebrew/install` commit `eaa2af62ca1d5349c9743f82b1b0c4e5d365fb2d` 與 SHA-256 `8fd2f1a5287a7c2e3d7362924c677edcc04c5a0e16448c77132d056bc88cf1ed` 驗證後執行，兼顧無 `brew` 機器上的首次自動化與供應鏈安全。
- **既有 MSYS2 `SigLevel = Never`（預設方案）**：若偵測既有 `pacman.conf` 遭舊版寫成 `SigLevel = Never`，自動還原為 `Required DatabaseOptional` 並於安裝日誌說明。
- 不新增 Cargo 或 npm 第三方套件；不更動前端對外 Tauri IPC 契約與 E2E 流程。

## 工作步驟

1. 建立本執行計畫文件。
2. 強化 Rust 後端安全邊界（`process.rs`、`dependencies.rs`、`bootstrap.rs`）。
3. 強化 `install.sh` 與 `install.ps1` 安裝腳本安全性。
4. 強化 `prepare-runtime.mjs`、`run-tauri.mjs`、`setup.ps1` 與 `launch.ps1` 的 Windows PowerShell 絕對路徑解析。
5. 補齊前後端／腳本安全回歸測試並更新 `.neo_harness/SECURITY.md`。
6. 執行 `verify.sh`、`verify.ps1`、`test-windows-setup.ps1` 與 `test:e2e:ci` 驗證並歸檔計畫。

## 進度與發現

- [x] 建立進行中執行計畫。
- [x] 強化 Rust 後端安全邊界（含跨平台安全 I/O 與符號連結拒絕）。
- [x] 強化 `install.sh` 與 `install.ps1`（並同步於檢查路徑補齊拒絕符號連結）。
- [x] 強化輔助啟動腳本（保持 `prepare-runtime.mjs` 單檔獨立以符合隔離測試）。
- [x] 補齊安全單元／腳本測試與更新 `.neo_harness/SECURITY.md`，完成全量驗證。

## 驗證方式與結果

- `bash desktop/scripts/verify.sh`：通過（exit code 0；涵蓋 TypeScript typecheck、Vitest 18 files / 51 tests、`test:setup` 49 tests 含 `test-windows-setup.ps1` 22 scenarios、`test:version`、`cargo fmt --check`、Rust 48 tests、`cargo clippy --all-targets --all-features -- -D warnings`、Vite production build）。
- `pwsh desktop/scripts/verify.ps1`：通過（exit code 0）。
- `pwsh -NoProfile -NonInteractive -File desktop/scripts/test-windows-setup.ps1 -InstallerPath desktop/scripts/install.ps1`：通過（22 installer scenarios passed）。
- `cd desktop-test && bun run --silent test:e2e:ci`：通過（5/5 WebdriverIO 桌面 E2E 測試）。
- `bash desktop/scripts/install.sh --check`：於 macOS 本機完成唯讀檢查執行（回報環境缺少待裝工具，無寫入副作用）。
- 尚未在本機直接驗證項目：真實 Windows 環境上的 UAC 提示、WinGet Authenticode 驗簽與真實 MSYS2 GPG 套件更新（僅於 macOS `pwsh` 執行模擬測試；實機行為需待 Windows 環境或 GitHub Actions runner 驗證）。

## 復原方式

可透過 Git 還原 `desktop/`、`desktop-test/` 與 `.neo_harness/` 的所有變更。

## 最終結果

已完成小工具安裝流程全鏈路安全性強化並通過本機自動化與桌面 E2E 驗證。
