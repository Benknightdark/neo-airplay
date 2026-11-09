# Windows 背景依賴安裝修復

## 目的與背景

修復 Windows 安裝後首次準備工具時，UxPlay 編譯找不到標準標頭檔、安裝程序沒有完成結果，以及 CMD 視窗反覆閃現的問題。使用者已選擇沿用背景下載與編譯，只顯示必要的系統授權提示。

## 目前及預期行為

目前 MSYS2 指令失敗一律重試三次，編譯前不驗證工具鏈完整性，Rust 啟動外部程序未禁止建立主控台。提權程序也沒有明確的背景記錄機制。預期以新 shell 接續核心更新、編譯前檢查並最多修復工具鏈一次、背景程序不建立主控台，並在既有介面回報階段與失敗摘要。

## 限制與決策

- 沿用現有固定工具版本、來源與 SHA-256 核對；不變更安裝包交付方式。
- Windows 特有行為採平台條件編譯。維持既有 IPC 與其他平台安裝參數相容。
- 本機為 macOS；原生 Windows 的 UAC、視窗及乾淨環境安裝驗收尚未執行。
- 標頭檔在使用者 Windows 上是否缺損或搜尋路徑異常仍未知，需記錄完整性與編譯器診斷。

## 工作順序與進度

- [x] 修正共用 Windows 背景程序啟動設定。
- [x] 修正 PowerShell 原生命令執行、MSYS2 更新與工具鏈驗證／修復。
- [x] 補上安裝階段、提權記錄、完成結果與中斷診斷。
- [x] 新增隔離測試並更新可靠性與命令文件。
- [x] 執行統一驗證、桌面 E2E 與變更審查。

## 發現與依據

- `Invoke-Msys` 對更新與確定性的編譯失敗使用同一套三次重試。
- `process_presence` 每 500 毫秒啟動 `tasklist.exe`；Rust 未設定 `CREATE_NO_WINDOW`。
- MSYS2 官方更新文件要求核心更新後重新啟動 shell：https://www.msys2.org/docs/updating/
- 官方 UCRT64 headers 套件包含記錄所缺的標準標頭：https://packages.msys2.org/packages/mingw-w64-ucrt-x86_64-headers
- 中斷測試發現等待程序先查 PID、未先回收直接子程序，會在 Unix 將殭屍程序視為存活而持續等待。已調整順序，並保留退出狀態供中斷訊息使用。
- Windows 原始碼換行可能為 CRLF；送入 Bash 的多行命令統一轉成 LF，並新增相應案例。

## 具體步驟與驗證

對 PowerShell 函式執行隔離測試，模擬更新重啟、工具鏈損壞與修復、不可重試的編譯錯誤、提權與結果寫入；以實際子程序測試輸出排空與引數傳遞。Rust 測試驗證階段回報、無結果退出及 Windows 無主控台。執行 `pwsh .neo_harness/verify.ps1` 與 `cd desktop && bun run --silent test:e2e:ci`。

## 復原方式

還原本次腳本、Rust、測試與文件變更即可回復程式行為；驗證使用暫存隔離資料，不安裝或修復本機系統套件。

## 結果

實作完成，本機驗證通過。

- `cargo check --quiet --locked --all-features --manifest-path desktop/src-tauri/Cargo.toml` 通過。
- `pwsh .neo_harness/verify.ps1` 最終通過：型別檢查、22 個前端測試、安裝流程測試、Rust 格式、24 個 Rust 測試、Clippy 與前端正式建置。
- PowerShell 在 macOS 執行 14 個隔離安裝案例，涵蓋更新接續、修復、輸出、引數、略過安裝、提權與結果。Windows 專用主控台及環境案例未在本機執行。
- `cd desktop && bun run --silent test:e2e:ci` 在 macOS 通過全部 5 個桌面案例。
- `git diff --check` 通過；變更審查涵蓋所有追蹤與未追蹤的本次檔案，核對程序啟動、階段／結果、重試與原使用者路徑的一致性，未發現交付阻礙。
- 首次統一驗證因前端套件未安裝而找不到 `vue-tsc`，依鎖定檔執行 `bun install --frozen-lockfile --silent` 後重測通過。
- 首次 PowerShell 測試發現 `File.Replace` 的 `$null` 備份路徑被轉成空字串，已改用 `[NullString]::Value` 並重測通過。首次 Rust 中斷測試因上述殭屍程序問題卡住，停止該次測試、修正等待順序後，最終 24 個測試全數通過。

原生 Windows 的正式套件首次安裝、實際 UCRT64 修復、實際 UAC／視窗顯示與 Windows 專用測試尚未執行；本機只有 macOS，不能將這些項目描述為已通過。未建立新安裝包或發佈版本。
