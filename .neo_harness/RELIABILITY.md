# 程序生命週期與復原

本文件描述目前 Tauri/Rust 程式可觀察的行為。App 只從 Debug 的 `desktop/engine/` 或 Release 使用者資料目錄的 `engine/` 啟動工具；GStreamer 與 DNS-SD 服務也需具備該平台要求的執行相依項目。

## 啟動與狀態更新

- App 啟動時載入設定並建立兩個獨立的引擎控制器；不會自動啟動 AirPlay 或 Android 投影。
- App 每次啟動都在背景檢查投影工具，缺少時執行內嵌平台安裝腳本；先前安裝失敗時，下次啟動會自動重試。介面顯示進度；失敗可查看安裝記錄並手動重試。E2E profile 或 `NEO_AIRPLAY_SKIP_SETUP=1` 停用此流程。
- Windows 背景程序禁止建立主控台視窗；需要提權時保留 UAC 授權提示，安裝 PowerShell 視窗隱藏。背景檢查、套件管理、編譯及提權後的輸出寫入同一份 `setup/install.log`，介面透過 `install-progress.json` 顯示目前階段。
- Windows 核心套件更新要求重新啟動時，以新 shell 接續完整更新，最多三輪；一般錯誤直接失敗，不重跑相同編譯。編譯前使用固定 UCRT64 路徑與隔離的編譯環境，實際編譯並執行 C/C++ 探針；失敗時記錄套件完整性與標頭搜尋路徑，重新安裝 GCC、headers、CRT 與 binutils 一次後再驗證。
- 開發與建置入口會先執行 `setup.sh`／`setup.ps1`，檢查開發工具與投影依賴；全部就緒時略過安裝，缺少時依平台腳本補齊。新 checkout 可使用 `launch.sh`／`launch.ps1`，先 setup 再啟動 Debug、Release 或打包；直接呼叫 Tauri CLI 也會自動執行 setup。CI 可用 `NEO_AIRPLAY_SKIP_SETUP=1` 停用。
- 第二次啟動會交由單一執行個體插件轉送至既有 App，顯示主視窗後結束新程序。
- 引擎執行程序的 `PATH` 僅包含目前使用的 `engine/` 目錄及其 Android 子目錄；找不到該目錄中的工具時會回報缺少依賴，不從系統 `PATH` 回退。
- macOS 啟動 UxPlay 時使用原生 `osxvideosink`、libav 軟體 H.264 解碼及 `-vsync no`；「Auto」輸出選項會採用原生 sink。其他平台維持 UxPlay 自動選擇。
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
- Windows 安裝結果包含階段與錯誤摘要；取消授權會顯示明確訊息。背景安裝先回收直接子程序的退出狀態，再查詢 PID，避免將未回收的程序判成仍在執行。程序未寫入結果就退出時，保留可取得的退出狀態與最後階段，確認程序結束後清除啟動記錄，供使用者重試；程序存在狀態不明時維持避免重複安裝的保護。
- Runtime 檢查要求 Engine 內同時有可執行的 `scrcpy`、ADB 及非空的 `scrcpy-server`；缺少 server 時會重新準備 scrcpy 套件，並將完整工具組寫入 Engine。
- Linux 準備腳本會安裝並啟用 Avahi；macOS 使用系統 Bonjour，Windows 使用實驗版內建 mDNS。App 不建立防火牆規則，也不會自動配對 Android 裝置。
- 若 Android 裝置未列出，先在終端執行 `adb devices -l`，確認裝置已授權且狀態為 `device`；App 只列出 ADB 已建立的連線。

## 驗證界線

- `.neo_harness/verify.ps1` 執行前端型別與單元測試、Rust 格式與單元測試、Clippy 及前端建置。
- `bun run test:e2e:ci` 啟動實際桌面程式，驗證第二次啟動、總覽/設定/診斷頁導覽、設定儲存，以及兩個假引擎程序的並行啟停；測試使用隔離 profile，不啟動真實引擎。Linux headless 執行時需使用 `dbus-run-session -- xvfb-run --auto-servernum bun run --silent test:e2e:ci`。
- CI 在 `develop` 更新時，於 Windows、macOS、Linux runner 執行驗證和桌面 E2E；三平台全數成功後才建立或沿用 `develop` 到 `main` 的 PR。新提交會取消舊的 CI 執行，PR 工作也會確認目前 `develop` 仍是通過檢查的提交。
- CD 僅在 PR 合併到 `main` 後執行，於三平台建立 Tauri 設定中的安裝格式；任一平台建置或上傳失敗時不建立 Release。每個合併 PR 都需使用一致且未發布過的 `X.Y.Z` 版本。
- `bun run smoke:bundle` 仍可手動使用隔離的暫存使用者資料目錄啟動目前平台的正式 bundle 執行檔；Linux headless 執行時使用 `dbus-run-session -- xvfb-run --auto-servernum bun run --silent smoke:bundle`。CI/CD 不執行此啟動檢查。
- 單元與 UI E2E 不驗證真實 Apple/Android 裝置、mDNS 探索、影像、音訊、GPU renderer 或安裝後 OS 啟動項目；需在各目標平台人工驗收。
- 安裝流程測試以隔離模擬驗證核心更新接續、工具鏈修復、略過安裝及提權記錄，並執行子程序驗證引數、輸出與 Windows 主控台行為，不安裝系統套件。乾淨 Windows 正式套件首次啟動、真實套件下載／編譯與 UAC 提示仍需原生環境驗收。
