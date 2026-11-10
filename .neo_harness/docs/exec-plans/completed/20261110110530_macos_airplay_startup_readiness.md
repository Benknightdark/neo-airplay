# macOS AirPlay 首次啟動與就緒狀態

## 目的

縮短 macOS 安裝後首次啟動 UxPlay 的等待時間，並且只有在 AirPlay 接收器已出現在 Bonjour 服務探索結果中時才顯示「執行中」。

## 背景與目前行為

- 使用者確認此等待發生於安裝工具後首次啟動；iPhone 的螢幕鏡像清單要等 UxPlay 視窗出現後才會列出接收器。
- `EngineController::start` 目前在 UxPlay 子程序建立成功後立即設定 `Running`。
- macOS 安裝檢查以臨時 GStreamer registry 掃描外掛，並在完成後刪除；執行中的 UxPlay 使用另外設定的外掛路徑。
- GStreamer registry 預熱是縮短首次等待的待驗證改善點，不宣稱為已證實根因。

## 預期行為與限制

- macOS 依賴工具準備完成後，在 Tauri app cache 建立專用 GStreamer registry，並由 UxPlay 與預熱程序共用。
- macOS AirPlay 啟動初始狀態為 `Starting`；Bonjour `_airplay._tcp` 瀏覽發現目前設定的接收器名稱後才轉為 `Running`。
- 30 秒內未發現服務、Bonjour 瀏覽失敗或 UxPlay 提早結束時，停止尚在啟動的子程序並回報 `Faulted`；使用者可重試。
- 啟動中提供停止操作；不新增 IPC 命令或公開資料欄位。不改變 Windows、Linux 的服務啟動語意，也不自動啟動 AirPlay。
- GStreamer 預熱失敗只寫入安裝記錄，不阻止工具就緒或 UxPlay 啟動。

## 工作順序

1. 傳遞 Tauri app cache 的 GStreamer registry 路徑到依賴管理器與 UxPlay。
2. 新增獨立的 macOS registry 預熱模式；讓依賴管理器在 runtime 檢查成功後執行，保留 `--check` 的檢查行為。
3. 為 AirPlay 增加 Bonjour 啟動監控、30 秒逾時、程序清理與啟動耗時記錄。
4. 更新總覽頁啟動中與取消操作，補齊 Rust、Vue、安裝流程測試及 Harness 文件。
5. 執行 `.neo_harness/verify.ps1`，並在目前 macOS 環境可行時執行 bundle smoke check；真實 iPhone 連線列為人工驗收。

## 發現與決策

- 目前工作樹乾淨，分支為 `develop`。
- `EngineStatus::Starting` 已存在於 Rust 與 TypeScript 型別；`DashboardPage` 尚未將它當成可取消的啟動狀態。
- E2E 使用假 UxPlay；測試 profile 必須停用真實 DNS-SD 監控，並以單元測試驗證 Bonjour 輸出解析和狀態轉換。
- 本機平台為 macOS；可執行本機驗證，但沒有已連接的真實 iPhone 驗收證據。

## 驗證與復原

- 自動驗證：`pwsh .neo_harness/verify.ps1`，並視可用桌面環境執行 E2E 與 `bun run --silent smoke:bundle`。
- 安裝腳本測試須確認預熱使用正式外掛目錄與 app cache registry；`--check` 仍只執行 runtime 驗證。
- 人工驗收：乾淨安裝並完成首次工具準備後啟動 AirPlay，確認 iPhone 清單出現接收器時 UI 才轉為執行中，並可成功連線；比較首次啟動與後續啟動耗時。
- 若預熱或 DNS-SD 監控使首次啟動回歸，可停用該新增步驟並保留既有依賴安裝及程序狀態流程。

## 進度

- [x] 盤點程式、測試與 Harness 規範。
- [x] 實作 cache 預熱與 AirPlay readiness 監控。
- [x] 更新 UI、測試與 Harness 文件。
- [x] 執行自動驗證並記錄結果。

## 結果

macOS 依賴工具就緒後會在 app cache 建立可重用的 GStreamer registry；AirPlay 在 Bonjour 找到設定的服務名稱前維持 `Starting`，並提供取消啟動。預熱失敗不會阻止工具就緒。

首次完整驗證遇到 Rust 格式差異和新狀態測試的 PID 預期錯誤，修正後重跑通過。另有一次既有安裝子程序測試在完整測試批次失敗；單獨執行及後續完整驗證均通過，失敗原因未確認。

## 驗證結果

- `bun run --silent test:setup` 通過。
- `pwsh -NoProfile -File .neo_harness/verify.ps1` 通過，包含 23 個 Vue 測試、27 個 Rust 測試、型別檢查、Clippy 與正式前端建置。
- `bun run --silent test:e2e:ci` 通過，5 個桌面 E2E 案例成功。
- `NEO_AIRPLAY_SKIP_SETUP=1 bun run --silent bundle` 成功產生 macOS app 與 DMG。
- `bun run --silent smoke:bundle` 通過。
- 本機 `/usr/bin/dns-sd -B _airplay._tcp local.` 的輸出欄位符合 Bonjour 解析器格式；這不代表 App 與 iPhone 的實際連線驗收。

真實 iPhone 連線及首次、後續啟動耗時比較尚未驗收；因此 GStreamer 預熱對實際等待時間的改善幅度仍未知，且未證實 GStreamer 是原始延遲根因。
