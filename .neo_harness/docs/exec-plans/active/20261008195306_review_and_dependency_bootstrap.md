# 專案檢查與首次依賴準備

## 目的與背景

使用者要求檢查目前專案的未完成項目、問題與可改善處並修正；首次建置、執行與安裝時自動準備 UxPlay 等工具。所有子代理使用 gpt-6-luna、max 與可用的 priority 服務。

## 目前及預期行為

目前 README 要求手動安裝 UxPlay、GStreamer、ADB、scrcpy，建置與 App 沒有安裝入口。預期開發與打包命令先準備依賴；安裝後首次啟動自動檢查並補裝，顯示進度、失敗原因與重試。另提供不依賴 Bun 的初始開發環境準備入口。

## 限制與決策

- 依平台官方套件管理器安裝，不提交第三方二進位檔。
- 系統權限由 OS 授權視窗或終端處理，App UI 保持可操作。
- E2E、smoke 與 CI 必須明確停用系統補裝，使用隔離假引擎。
- Windows、Linux 真實系統安裝及真實裝置串流在本機 macOS 無法證明。
- 不設定憑證、公開發佈或自動配對 Android。

## 工作順序與進度

- [x] 閱讀 README、架構、安全與可靠性文件，盤點前後端、CI、建置設定。
- [x] 分工 Rust、Vue 與平台套件官方資料檢查。
- [x] 修正確認的並行設定寫入、OS 登入啟動交易、Android 狀態提示及前端輪詢競態。
- [x] 實作共用安裝腳本、開發環境入口及建置 hooks；進行實際安裝與收尾修正。
- [x] 實作首次 App 啟動背景檢查、安裝狀態、重試、跨重啟 PID／結果追蹤與可執行檔探索。
- [x] 同步 README、Harness 與命令表；實際驗證結果待補。
- [ ] 執行 Harness 驗證、桌面 E2E、安裝腳本隔離驗證及最終 diff review。

## 發現

1. 設定寫入使用共用 .tmp，原本取得設定 mutex 之前寫入，並行儲存會互相覆蓋。
2. 介面將 offline/unknown 裝置誤稱未授權，與 Rust 解析及 IPC 型別不一致。
3. 本機 Node 預設為 24，有 Node 22.7.0 可用；找不到 pwsh。Harness 驗證需要補備 PowerShell 或明確記錄阻礙。

## 具體步驟與驗證

安裝腳本只執行固定套件命令。Rust 檢查與安裝背景工作分離，避免 IPC 阻塞。完成後檢查腳本語法與模擬命令流程、TypeScript、單元測試、cargo fmt/test/clippy，執行 .neo_harness/verify.ps1 與桌面 E2E。各平台尚未實測項目如實記錄。

## 復原方式

安裝失敗保留診斷，不刪除使用者既有工具或資料；提供重試。程式變更可用 Git diff 個別回復。已安裝系統套件不自動移除。

## 結果

進行中，尚未宣告完成。

## 驗證進度與接續資訊

- 初次 Harness 使用既有 Node22.7.0，Vitest 因 ERR_REQUIRE_ESM 失敗；已確認 jsdom30.1.2 的 engines 至少22.22.2，修正檢查並固定 CI 22.23.3。
- 已下載官方 Node22.23.3 與 PowerShell7.6.6 至 /tmp/neo-airplay-node22、/tmp/neo-airplay-pwsh，兩者下載均核對官方 SHA-256。
- Node22.23.3 上前端22測試通過；第二次 Harness 因編輯中 Rust 尚未格式化中止。
- Backend 子代理完成 cargo fmt/check 及第一輪18個 Rust lib tests，後續新增輸出上限與程序探測回歸後仍需整合確認。
- Windows target 已安裝；cargo check --target x86_64-pc-windows-msvc 在 tauri-winres 找不到 llvm-rc，尚未完成 Windows 編譯檢查。
- 專用 Docker 容器 neo-airplay-setup-audit-20261008（Ubuntu24.04、linux/amd64 模擬）存活；已安裝 CA、dbus、procps，手動建立 /run/dbus 並啟動 system bus；等待安裝腳本穩定後執行真實 runtime 安裝。
- 最新 root Harness session67580，需先查詢 handle，不能因觀察逾時重啟。
