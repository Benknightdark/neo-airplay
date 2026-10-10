# 程序生命週期與復原

本文件描述目前 Tauri/Rust 程式可觀察的行為。App 只從 Debug 的 `desktop/engine/` 或 Release 使用者資料目錄的 `engine/` 啟動工具；GStreamer 與 DNS-SD 服務也需具備該平台要求的執行相依項目。

## 啟動與狀態更新

- App 啟動時載入設定並建立兩個獨立的引擎控制器；不會自動啟動 AirPlay 或 Android 投影。
- App 每次啟動都在背景檢查投影工具，缺少時執行內嵌平台安裝腳本；先前安裝失敗時，下次啟動會自動重試。介面顯示進度；失敗可查看安裝記錄並手動重試。E2E profile 或 `NEO_AIRPLAY_SKIP_SETUP=1` 停用此流程。
- 第二次啟動會交由單一執行個體插件轉送至既有 App，顯示主視窗後結束新程序。
- 引擎執行程序的 `PATH` 僅包含目前使用的 `engine/` 目錄及其 Android 子目錄；找不到該目錄中的工具時會回報缺少依賴，不從系統 `PATH` 回退。
- Vue 總覽每 1.5 秒重新讀取程序狀態。ADB 裝置清單只在使用者按重新整理時更新。
- 找不到引擎會回報 `DependencyMissing`；程序無法啟動、意外非零退出或狀態查詢失敗會回報 `Faulted`。成功退出回報 `Stopped`。
- 重複啟動已執行的引擎會返回目前狀態。AirPlay 和 Android 各有自己的程序、鎖與狀態。
- UxPlay/scrcpy stdout 和 stderr 由背景讀取器持續排空並寫入診斷記錄，避免外部程序因輸出管道填滿而停住。
- Android ADB 列舉逾時上限為 15 秒；逾時會終止 ADB client。輸出可含多筆裝置，所以 stdout 和 stderr 會在輪詢程序期間分別讀取。
- ADB 刷新失敗時清除過期裝置清單。若 scrcpy 程序仍在執行，Android 狀態會保持 Running，確保停止操作仍可用；成功列舉且沒有執行中的程序時，舊的 Faulted 或 DependencyMissing 狀態會重設為 Stopped。

## 停止與非預期結束

- 使用者按停止時，App 終止並等待對應的直接子程序。若程序已先行退出，停止流程會採用已取得的退出狀態。
- 關閉主視窗會隱藏視窗並保留 App。系統匣「結束 Neo AirPlay」會結束 App；`EngineController` drop 時嘗試終止並等待其仍持有的引擎程序。
- 程序意外結束時，下次狀態輪詢會反映退出狀態及代碼並寫入記錄。程式沒有自動重新啟動引擎的流程。
- App 被作業系統強制結束時，不保證子程序樹能完整終止；目前程式只直接管理 UxPlay/scrcpy 子程序。

## 設定、記錄與復原

- 設定寫入同目錄暫存 JSON 後重新命名至 `settings.json`，寫入及記憶體更新共用設定鎖；command 另以 transaction 鎖序列化登入啟動變更及儲存。若無法寫入，命令會回報錯誤，記憶體中的設定不會更新。
- 記錄依本地日期寫入每日 JSONL 檔；診斷頁顯示當日最近 300 筆。檔案開啟或寫入失敗目前不會回報給 UI。
- 若 App 顯示工具安裝失敗，依畫面列出的記錄檔排除網路、套件庫或授權問題後，按「重新檢查並安裝」。Debug 的本機引擎檔案放在 `desktop/engine/`；Release 先將套件資源複製至使用者資料目錄的 `engine/`。
- Linux 準備腳本會安裝並啟用 Avahi；macOS 使用系統 Bonjour，Windows 使用實驗版內建 mDNS。App 不建立防火牆規則，也不會自動配對 Android 裝置。
- 若 Android 裝置未列出，先在終端執行 `adb devices -l`，確認裝置已授權且狀態為 `device`；App 只列出 ADB 已建立的連線。

## 驗證界線

- `.neo_harness/verify.ps1` 執行前端型別與單元測試、Rust 格式與單元測試、Clippy 及前端建置。
- `bun run test:e2e:ci` 啟動實際桌面程式，驗證第二次啟動、總覽/設定/診斷頁導覽、設定儲存，以及兩個假引擎程序的並行啟停；測試使用隔離 profile，不啟動真實引擎。Linux headless 執行時需使用 `dbus-run-session -- xvfb-run --auto-servernum bun run --silent test:e2e:ci`。
- CI 在 Windows、macOS、Linux runner 建置並執行桌面 E2E，再建置各平台原生安裝套件並啟動正式執行檔 8 秒。尚未執行的外部 workflow 不代表通過。
- `bun run smoke:bundle` 使用暫存使用者資料目錄啟動目前平台的正式 bundle 執行檔；Linux headless 執行時使用 `dbus-run-session -- xvfb-run --auto-servernum bun run --silent smoke:bundle`。此檢查不取代安裝後測試或實際 UI E2E。
- 單元與 UI E2E 不驗證真實 Apple/Android 裝置、mDNS 探索、影像、音訊、GPU renderer 或安裝後 OS 啟動項目；需在各目標平台人工驗收。
