# Debug、Release 與打包前自動準備環境

## 目的與背景

使用者要求初次取得專案後，執行 Debug、Release 或打包時自動執行 `desktop/scripts/setup.sh`／`setup.ps1` 的準備流程；引擎已就緒時略過重複安裝。引擎仍由既有安裝腳本放在 `desktop/engine/`，Git 僅追蹤 `.GITKEEP`。

## 原先及預期行為

原先 setup 需手動執行；`bun run tauri dev`、Release build 與 bundle 不會先呼叫完整開發環境 setup。預期提供不依賴 Bun 的首次啟動入口，並讓一般 Tauri 指令自動呼叫平台 setup。setup 先檢查依賴；全部可用時不重新下載或安裝，缺少時依既有腳本補齊。CI 與明確停用 setup 的流程維持可略過。

## 限制與決策

- `bun run` 本身要求 Bun 已存在，因此初次取得專案另提供 shell／PowerShell 啟動入口，先執行 setup 再啟動指定操作。
- Setup 腳本可要求終端、管理員或套件授權；入口需保留互動輸出與退出代碼。
- 已安裝的工具僅由 `desktop/engine/` 執行；此目錄的二進位檔不加入 Git。
- Engine 已就緒時依安裝腳本的檢查快路徑略過安裝；setup 仍會以鎖檔檢查前端套件。

## 工作順序與進度

- [x] 閱讀 README、架構、安全、可靠性與 setup 腳本，確認 Tauri 執行入口及安裝快路徑。
- [x] 新增首次取得專案可直接使用的跨平台啟動入口。
- [x] 將 Tauri CLI 的一般 Debug／Release／Build 執行接到平台 setup，保留 CI skip。
- [x] 修正 Engine 完整但開發工具缺漏時，Unix 安裝流程重跑 runtime staging 的問題。
- [x] 更新 README、命令表、架構與可靠性說明。
- [x] 執行 Harness、release bundle 啟動檢查，並確認 engine 的 Git 追蹤內容。

## 發現

- `install.sh --development` 先檢查 runtime 與開發工具，兩者完整時會在執行套件安裝器前結束。
- Windows `install.ps1 -Development` 同樣會在 runtime 與開發工具就緒時略過安裝流程。
- `tauri.conf.json` 的 beforeDev/beforeBuild hooks 只建置前端；實際 runtime 檢查另由 Rust App 背景處理。
- `desktop/scripts/build-app.ts` 透過 `bun run tauri build --ci` 打包，可共用包裝後的 Tauri CLI 入口。

## 具體步驟與驗證

新增可測試的 Tauri runner，以 `NEO_AIRPLAY_SKIP_SETUP=1` 保留 CI 行為；新增 macOS/Linux shell 與 Windows PowerShell 首次啟動入口。透過既有 `test:setup` 驗證執行順序、失敗時中止、Windows/macOS/Linux setup 命令與 skip 行為。已執行 `/tmp/neo-airplay-pwsh/pwsh .neo_harness/verify.ps1`、`cd desktop && bun run --silent bundle`、`cd desktop && bun run --silent smoke:bundle`、`git diff --check`，並確認 `git ls-files desktop/engine` 只有 `.GITKEEP`。

## 復原方式

移除新增的 launcher/runner，還原 package script 與文件即可回到手動 setup 流程。已安裝的使用者工具與系統套件不會由復原步驟移除。

## 結果

自動 setup 與首次啟動入口已完成。Linux 安裝腳本 mock case 驗證 Engine 完整時只會要求缺少的開發套件；完整 Harness、macOS Release bundle 與 bundle 啟動檢查皆通過。跨平台實機安裝與真實投影串流仍需各平台驗收。
