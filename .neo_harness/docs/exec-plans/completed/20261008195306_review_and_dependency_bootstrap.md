# 專案檢查與首次依賴準備

## 目的與背景

使用者要求檢查目前專案的未完成項目、問題與可改善處並修正；準備 UxPlay 等執行工具。最新需求明確指定 DEBUG 與 RELEASE App 每次啟動都要檢查缺少的依賴，並在背景自動準備。後續補充指定本機引擎目錄為 `desktop/engine/`，打包與 Debug 都以此為來源，且 Git 只追蹤 `.GITKEEP`；App 啟動工具只能從當前 `engine/` 目錄執行，平台安裝腳本 `install.sh`、`install.ps1` 及開發環境入口 `setup.sh`、`setup.ps1` 均放在 `desktop/scripts/`。

## 目前及預期行為

目前 Rust 啟動流程已能在背景檢查及準備依賴；DEBUG 使用 `desktop/engine/`，RELEASE 將打包引擎複製到使用者資料目錄的 `engine/`。預期 DEBUG 與 RELEASE App 每次啟動都在背景檢查 UxPlay、GStreamer、ADB、scrcpy 及 Linux Avahi；缺少時自動安裝，安裝失敗後下次啟動自動重試，介面顯示進度、失敗原因與重試入口。`setup:runtime` 保留為手動準備命令。App 不從系統 PATH 尋找或啟動投影工具。

## 限制與決策

- 大型執行工具放在 `desktop/engine/`，以 `.gitignore` 排除；只追蹤 `.GITKEEP`。套件管理器仍負責 OS 函式庫、GStreamer build dependencies 與 Linux Avahi 服務。
- 系統權限由 OS 授權視窗或終端處理，App UI 保持可操作。
- E2E、smoke 與 CI 必須明確停用系統補裝，使用隔離假引擎。
- Windows、Linux 真實系統安裝及真實裝置串流在本機 macOS 無法證明。
- 不設定憑證、公開發佈或自動配對 Android。

## 工作順序與進度

- [x] 閱讀 README、架構、安全與可靠性文件，盤點前後端、CI、建置設定。
- [x] 分工 Rust、Vue 與平台套件官方資料檢查。
- [x] 修正確認的並行設定寫入、OS 登入啟動交易、Android 狀態提示及前端輪詢競態。
- [x] 實作共用安裝腳本、開發環境入口及手動 runtime 準備命令；進行實際安裝與收尾修正。
- [x] 實作每次 DEBUG/RELEASE App 啟動背景檢查、缺漏自動安裝、失敗後重試、跨重啟 PID／結果追蹤與可執行檔探索。
- [x] 同步 README、Harness 與命令表；執行 Harness 驗證及隔離安裝腳本驗證。
- [x] 執行 Harness 驗證、桌面 E2E、安裝腳本隔離驗證及最終 diff review；Harness 與隔離安裝腳本測試通過，桌面 E2E 有一項 ADB 阻塞案例失敗。

## 發現

1. 設定寫入使用共用 .tmp，原本取得設定 mutex 之前寫入，並行儲存會互相覆蓋。
2. 介面將 offline/unknown 裝置誤稱未授權，與 Rust 解析及 IPC 型別不一致。
3. 本機找不到 `pwsh`，無法直接執行 PowerShell Harness wrapper；`bun run --silent verify` 是 wrapper 指向的專案入口，本輪已成功執行。
4. 2026-10-11 使用者安裝記錄顯示 UxPlay 已建置為 1.73.7，但版本檢查先取到 GStreamer 1.22，造成安裝後誤判。版本檢查改為只解析 UxPlay 標示行。
5. App 原本可從系統 PATH、Homebrew、MSYS2 或 WinGet 搜尋引擎；目前改為只從 `engine/` 目錄解析工具，Debug 指向 `desktop/engine/`，Release 使用者資料目錄用來提供可寫位置。

## 具體步驟與驗證

安裝腳本只執行固定套件命令及核對 SHA-256 的官方工具套件。Rust 檢查與安裝背景工作分離，避免 IPC 阻塞。完成後檢查腳本語法與模擬命令流程、TypeScript、單元測試、cargo fmt/test/clippy，執行 `.neo_harness/verify.ps1` 與桌面 E2E。各平台尚未實測項目如實記錄。

## 復原方式

安裝失敗保留診斷，不刪除使用者既有工具或資料；提供重試。程式變更可用 Git diff 個別回復。已安裝系統套件不自動移除。

## 結果

本輪依賴修復與資料夾調整已完成。macOS 安裝、驗證、Release 封裝及啟動 smoke check 通過；Windows／Linux 實機安裝與裝置串流仍待各平台驗收。

## 驗證進度與接續資訊

- 初次 Harness 使用既有 Node22.7.0，Vitest 因 ERR_REQUIRE_ESM 失敗；已確認 jsdom30.1.2 的 engines 至少22.22.2，修正檢查並固定 CI 22.23.3。
- 已下載官方 Node22.23.3 與 PowerShell7.6.6 至 /tmp/neo-airplay-node22、/tmp/neo-airplay-pwsh，兩者下載均核對官方 SHA-256。
- Node22.23.3 上前端22測試通過；第二次 Harness 因編輯中 Rust 尚未格式化中止。
- 2026-10-11：`pwsh` 不在 PATH，無法直接執行 `.neo_harness/verify.ps1`；依該腳本入口改跑 `cd desktop && bun run --silent verify`，完整通過（前端22、安裝腳本15、Rust19 測試，以及 rustfmt、Clippy、Vite build）。
- 2026-10-11：`cd desktop && bun run --silent test:e2e:ci` 的啟動、第二執行個體、頁面導覽/設定、引擎並行案例通過；ADB 阻塞案例未產生 `adb.started`，5 項中4項通過。E2E 尚未通過。
- Backend 子代理先前完成第一輪 cargo fmt/check 與18個 Rust lib tests；後續以目前工作樹的 Harness 結果確認整合。
- 2026-10-11：更新啟動依賴重試及 PID 未知狀態等待後，Rust 格式、19 個 Rust 測試、Clippy 與 Vite build 均通過。
- Windows target 已安裝；cargo check --target x86_64-pc-windows-msvc 在 tauri-winres 找不到 llvm-rc，尚未完成 Windows 編譯檢查。
- 曾建立 Ubuntu 24.04 Docker 容器作為 Linux 安裝檢查環境；本輪未在該容器重跑完整 runtime 安裝，Linux 實際安裝仍未驗證。
- 2026-10-11：修正 UxPlay 1.73.7 被 GStreamer 1.22 遮蔽的版本解析錯誤；安裝與執行目錄改為 `desktop/engine/`，安裝腳本移至 `desktop/scripts/`。
- 2026-10-11：完整 `bun run --silent verify` 通過（前端22、setup17、Rust20）；macOS `.app`/DMG bundle 與 `smoke:bundle` 通過。`desktop/engine/` 的依賴檔案均忽略於 Git，只有 `.GITKEEP` 列為未追蹤檔案。
