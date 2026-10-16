# Windows Debug、Release 與打包驗證與修復

## 目的與背景

使用者要求檢查此專案在 Windows 作業系統下的 Debug、Release 與安裝套件打包（Bundle）是否能正常運作。先前專案主要於 macOS 完成開發與實機驗證，Windows 實機環境準備、桌面 E2E、Release 與 MSI／NSIS 打包尚待在 Windows 實機完整驗證。

## 目前及預期行為

目前在 Windows 實機執行 `install.ps1 -Check -Development` 會因 `Test-WebView2Runtime` 在 `Set-StrictMode -Version Latest` 下讀取不存在的登錄機碼屬性而拋出 `PropertyNotFoundStrict` 中止；同時 `Test-VisualStudioCppTools` 未能識別已安裝於 Visual Studio 2022／2026 Professional 的版本化 MSVC 工具鏈，`Install-Bun` 未檢查已安裝的 VC++ Redistributable 且無條件要求 UAC 提權，`.neo_harness/verify.ps1` 亦未將 Windows 受管工具路徑置於 `Machine PATH` 之前。

預期修正上述腳本問題後，能在本機 Windows 自動完成開發與投影引擎依賴準備（`desktop/engine/`），並通過 `.neo_harness/verify.ps1`、桌面 E2E（Debug 建置）、Release 建置、MSI／NSIS 打包與 `smoke:bundle` 正式版啟動檢查。

## 限制與決策

- 投影引擎與相依 DLL 僅安裝與收集於 `desktop/engine/`，Git 僅追蹤 `desktop/engine/.GITKEEP`。
- 受管 Node.js 22.23.3 與 Bun 1.4.2 安裝於使用者目錄下，不覆蓋系統 `Machine PATH` 既有的 Node.js 版本。
- 僅在缺少需 `--scope machine` 安裝的系統級元件（MSVC Build Tools、WebView2 Runtime、VC++ Redistributable）時才請求 UAC 提權。

## 工作順序與進度

- [x] 盤點本機 Windows 開發環境與 `desktop/engine/` 狀態，定位 `install.ps1` 與 `verify.ps1` 阻礙問題。
- [ ] 修正 `desktop/scripts/install.ps1`、`.neo_harness/verify.ps1` 並補充 `desktop/scripts/test-setup.mjs` Windows 測試案例。
- [ ] 執行 `desktop/scripts/setup.ps1` 準備 Bun 1.4.2、Node 22.23.3、MSYS2 UCRT64、GStreamer、UxPlay 1.74、scrcpy 5.0.1 與 ADB。
- [ ] 執行 `pwsh .neo_harness/verify.ps1` 驗證型別、前端／設定／Rust 單元測試、Clippy 與前端建置。
- [ ] 驗證 Debug 建置與桌面 E2E（`cd desktop && bun run --silent test:e2e:ci` 及 `launch.ps1 dev`）。
- [ ] 驗證 Release 建置（`launch.ps1 release`）。
- [ ] 驗證安裝套件打包（`launch.ps1 bundle`）與正式版啟動檢查（`smoke:bundle`）。
- [ ] 更新 `.neo_harness/docs/review-and-setup.md` 並將本計畫移至 `completed/`。

## 發現

- `HKEY_USERS\<SID>\Software\Microsoft\EdgeUpdate\Clients` 下存在不含 `name` 或 `pv` 屬性的機碼，在 `Set-StrictMode -Version Latest` 下直接存取 `$properties.name` 會拋出 `PropertyNotFoundStrict`；且繁體中文 Windows 下 WebView2 顯示名稱為 `Microsoft Edge WebView2 執行階段`。
- Visual Studio 2022／2026 Professional 安裝版本化 MSVC 工具組時，`vswhere -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64` 可能回傳空值，但 `vswhere -find "VC\Tools\MSVC\**\bin\Host*\x64\cl.exe"` 與 Windows SDK 實際可用。
- Windows 的 `Machine PATH` 優先於 `User PATH`，若系統已於 `C:\Program Files\nodejs` 安裝 Node 24，`.neo_harness/verify.ps1` 需同 `launch.ps1` 與 `run-tauri.mjs` 先載入受管 Node 22 路徑。

## 具體步驟與驗證

修正腳本後依序執行：
1. `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\install.ps1 -Check -Development`
2. `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\setup.ps1`
3. `pwsh .neo_harness/verify.ps1`
4. `cd desktop && bun run --silent test:e2e:ci`
5. `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 release`
6. `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 bundle`
7. `cd desktop && bun run --silent smoke:bundle`

## 復原方式

腳本與測試變更可透過 `git checkout` 還原；`desktop/engine/` 與 `desktop/src-tauri/target/` 皆為 Git 忽略之建置與執行目錄，可直接清理。

## 結果

尚未完成（驗證執行中）。
