# 程序生命週期與復原

本文件記錄 Tauri/Rust 執行期的程序狀態轉換、逾時控制與故障復原行為。

## 啟動與依賴準備

- **單一執行個體與初始狀態**：第二次啟動由 single-instance 插件喚醒既有主視窗後退出；App 啟動時載入設定但不自動啟動投影。
- **背景依賴檢查與重試**：啟動時於背景檢查投影依賴，缺少時執行內嵌 `install.sh`／`install.ps1`；前次失敗會在下次啟動自動重試，也可於 UI 查看記錄並手動重試。開發與 Tauri CLI 入口預設先跑 `setup.sh`／`setup.ps1`；設定 `NEO_AIRPLAY_SKIP_SETUP=1` 時略過準備並保留呼叫端工具 `PATH`。
- **Windows 安裝與自癒**：背景程序隱藏主控台視窗（UAC 提示由 OS 處理），輸出與階段寫入 `setup/install.log` 與 `install-progress.json`。MSYS2 pacman 資料庫鎖定時每 5 秒重試（上限 2 分鐘）；核心更新以新 shell 接續（上限 3 輪）；UCRT64 編譯前先跑 C/C++ 探針，失敗時自動重裝 GCC、headers、CRT 與 binutils 一次。背景狀態查詢先回收直接子程序再查 PID，若程序異常退出則保留最後階段並清除啟動鎖供重試。
- **完整性檢查與服務依賴**：Android 要求 `engine/` 內同時具備可執行 `scrcpy`、`adb` 與非空 `scrcpy-server`（缺少時重新佈署）。Linux 安裝腳本啟用 Avahi；macOS 使用系統 Bonjour；Windows 使用 UxPlay 內建 mDNS。

## 執行期、就緒判定與逾時

- **狀態輪詢與輸出排空**：前端每 1.5 秒輪詢狀態；`uxplay` 與 `scrcpy` 的 `stdout`/`stderr` 由背景讀取器持續排空並寫入每日 JSONL 記錄（診斷頁顯示當日最近 300 筆），避免管道填滿阻塞。缺少工具回報 `DependencyMissing`；啟動失敗或非零退出回報 `Faulted`；正常結束回報 `Stopped`。
- **macOS GStreamer 預熱與 AirPlay 就緒**：依賴就緒後若 app cache 尚無 GStreamer registry，會嘗試預熱（上限 90 秒，失敗僅寫入安裝記錄）。macOS 啟動 UxPlay（固定使用 `osxvideosink`、libav 軟體 H.264 與 `-vsync no`）後先處於 `Starting`，待 `/usr/bin/dns-sd -B _airplay._tcp local.` 找到設定服務名稱才轉 `Running`；90 秒逾時或程序提前退出則清理子程序並轉 `Faulted`（啟動期間可取消）。Windows 與 Linux 啟動子程序後直接轉 `Running`。
- **Android ADB 列舉**：僅在使用者手動重新整理時執行，逾時上限 15 秒（逾時即終止 ADB client）。刷新失敗會清空過期清單；若 `scrcpy` 仍在執行則維持 `Running` 以保留停止按鈕，無執行程序時才將舊 `Faulted`/`DependencyMissing` 重設為 `Stopped`。

## 停止、設定交易與維護復原

- **視窗關閉與程序終止**：關閉主視窗僅隱藏至系統匣；使用者按停止、系統匣結束 App 或 `EngineController` drop 時會終止並等待直接子程序（程序意外結束不自動重啟；OS 強制終止 App 時不保證子程序樹清理）。
- **設定原子寫入**：設定先寫入同目錄暫存檔再重新命名為 `settings.json`；寫入失敗時回報錯誤且不更新記憶體狀態，並透過 transaction 鎖同步復原 OS 登入啟動項目。
- **工具升級與重裝**：執行前先阻擋引擎啟動與 ADB 刷新、停止雙引擎並等待舊安裝程序結束，接著清除 GStreamer registry；「重新安裝」另清空目前模式 `engine/` 內的 App 專用檔案，但保留設定、診斷記錄與系統套件。
