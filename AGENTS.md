# 專案工作入口

Neo AirPlay 是以 Tauri 2、Rust、Vue 3、TypeScript 與 Bun 建立的跨平台 AirPlay 與 Android 投影接收器。介面及操作流程以根目錄 [README.md](README.md) 為準。

## 知識地圖

- [架構與依賴](.neo_harness/ARCHITECTURE.md)
- [執行計畫規範](.neo_harness/PLANS.md)
- [已驗證命令清單](.neo_harness/docs/scripts.md)
- [安全與信任邊界](.neo_harness/SECURITY.md)
- [程序生命週期與復原](.neo_harness/RELIABILITY.md)
- Vue 應用程式位於 `desktop/src/`，Rust 命令與服務位於 `desktop/src-tauri/`，前端／後端單元測試及桌面 E2E 位於根目錄 `desktop-test/`。

## 工作規則

1. 開始修改前閱讀相關程式碼與 README；架構描述以目前程式為準，不推定尚未實作的設計。
2. 保持 Tauri commands、Vue API 模組與 Rust state 的依賴關係清楚。若要改動引擎、外部程序或發佈內容，先查閱架構、安全及可靠性文件。
3. 修改程式碼、Vue、Rust 或建置設定後執行根目錄 `.neo_harness/verify.ps1`。需要驗證桌面導覽或設定流程時，另外執行 scripts.md 所列的桌面 E2E 命令。
4. GitHub Actions 位於 `.github/workflows/`：CI 在 Windows、macOS、Linux 執行驗證、桌面 E2E、安裝套件建置及正式程式啟動 smoke check。不可將未實際執行的建置或測試描述成通過。文件變更需核對相對連結和命令表格式。
5. 若檢查失敗，閱讀完整錯誤、修正原因後重跑；無法執行時回報具體原因與未驗證項目。

## 完成條件

- 變更符合目前程式碼、README 所記錄的功能與平台依賴。
- 相關驗證完成，或清楚列出未執行的檢查及原因。
- 新增或更新的 Harness 連結有效，命令均可追溯至專案文件或設定。
