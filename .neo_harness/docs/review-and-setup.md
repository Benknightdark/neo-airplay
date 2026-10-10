# 專案檢查與工具準備

本記錄對應 [README](../../README.md) 的目前程式。日期：2026-10-11。

## 確認並修正的問題

| 問題 | 修正與證據 |
| :--- | :--- |
| 首次使用缺少外部引擎只能手動處理 | `desktop/scripts/install.sh`、`install.ps1` 共用平台安裝流程；`desktop/scripts/setup.sh`、`setup.ps1` 準備開發工具；Rust 啟動時在背景準備投影工具。 |
| 並行設定儲存共用暫存檔 | `state.rs::save_settings` 取得設定鎖後才寫檔，新增並行寫入回歸案例。 |
| 登入啟動與 JSON 儲存可被兩個 command 交錯 | `lib.rs::SettingsTransaction` 涵蓋 OS 更新、儲存及回復。 |
| 輪詢舊結果可覆蓋新設定或動作結果 | `useAppState.ts` 依請求序號提交結果，避免堆疊輪詢及重複動作。 |
| 安裝重試可被舊輪詢覆蓋，IPC 失敗會鎖住操作 | `DependencyPanel.vue` 使舊請求失效、保留安裝進度及重試、處理失敗與清理計時器。 |
| 離線及未知 Android 裝置誤報未授權 | `DashboardPage.vue` 依實際狀態顯示處理方式，新增各狀態及混合狀態回歸案例。 |
| Unix 沒有執行權限的檔案仍被當作引擎 | `state.rs::is_executable_file` 檢查 execute bits；新增回歸案例。 |
| UxPlay 安裝完成後仍被判定版本不足 | `desktop/scripts/install.sh` 與 `install.ps1` 僅解析 `UxPlay version <版本>`；不會把輸出前段的 GStreamer 版本當成 UxPlay 版本。 |
| App 會從系統 PATH、Homebrew 或 MSYS2 直接執行依賴 | 執行檔只從 `engine/` 找；Debug 使用 `desktop/engine/`，Release 使用者資料目錄的 `engine/`。Tauri 從 `desktop/engine/` 打包，Git 只追蹤 `.GITKEEP`。 |
| 打包時安裝器無法接收終端輸入 | `scripts/build-app.ts` 繼承 stdin／stdout／stderr。 |

## 驗證結果

2026-10-11 驗證結果：

- `cd desktop && bun run --silent verify` 通過：前端 22 項、安裝流程 17 項、Rust 20 項測試，以及 rustfmt、Clippy、Vue/TypeScript 檢查與 Vite production build。
- `bash desktop/scripts/install.sh --check` 通過；本機 `desktop/engine/` 的 UxPlay 1.73.7、scrcpy 5.0.1、ADB 與必要 GStreamer 元件均通過檢查。
- `bash -n desktop/scripts/setup.sh desktop/scripts/install.sh` 及兩個 PowerShell 腳本解析檢查通過。
- `cd desktop && bun run --silent bundle` 通過，產生 macOS `.app`（230.15 MiB）及 DMG（82.97 MiB）；引擎資源來自 `desktop/engine/`，封裝內容沒有符號連結。
- `cd desktop && bun run --silent smoke:bundle` 通過，打包後應用程式成功啟動。
- Git 忽略 `desktop/engine/` 內依賴檔案，只顯示 `.GITKEEP`。

## 仍需各平台實機驗收

- Windows 與 Linux 套件管理器完整安裝、UAC／polkit、重新啟動後 PATH 及封裝安裝驗收。
- Windows 採用固定來源的 UxPlay 1.74 實驗版內建 mDNS，需驗證探索及影音行為。
- Apple／Android 真實裝置串流、mDNS 跨防火牆探索、音訊輸出與 GPU renderer。
- 公開發行仍缺少 Windows 發行者憑證、Apple Developer ID 與 notarization；目前沒有設定公開 GitHub Release。

安裝失敗時保留記錄並可重試。自動安裝不取代 OS 權限授權、網路連通性及 Android 裝置上的偵錯授權。
