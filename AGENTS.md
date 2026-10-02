# 專案工作入口

Neo AirPlay 是 Windows 11 x64 的 iOS AirPlay 與 Android 投影接收器。WPF/XAML 操作介面以 UxPlay 和 scrcpy 獨立程序處理鏡像畫面與音訊，兩者可同時運作。使用者功能、建置方式、執行環境與授權注意事項以 [README.md](README.md) 為準。

## 知識地圖

- [架構與依賴](.neo_harness/ARCHITECTURE.md)
- [執行計畫規範](.neo_harness/PLANS.md)
- [已驗證命令清單](.neo_harness/docs/scripts.md)
- [安全與信任邊界](.neo_harness/SECURITY.md)
- [程序生命週期與復原](.neo_harness/RELIABILITY.md)
- WPF 應用程式位於 src/NeoAirPlay.App/，模型與服務位於 src/NeoAirPlay.Framework/，測試位於 tests/；引擎建置、scrcpy staging 及 Bonjour 服務管理腳本位於 scripts/，隨附執行元件位於 engine/。

## 工作規則

1. 開始修改前閱讀相關程式碼與 README；架構描述以目前程式為準，不推定尚未實作的設計。
2. 保持既有服務介面與實作的依賴關係清楚。若要改動引擎、Bonjour 服務或發佈內容，先查閱架構及安全文件。
3. 修改程式碼、XAML 或建置設定後執行根目錄的 .neo_harness/verify.ps1；它會建置方案並執行單元測試。需要驗證實際 WPF 導覽時，另外執行 scripts.md 所列的 UI E2E 命令。
4. GitHub Actions 位於 .github/workflows/：CI 在 develop push 與 main PR 執行 verify，Release workflow 在 main push 發佈 Windows 套件；仍無靜態分析設定。不可將未實際執行的建置或測試描述成通過。文件變更需核對相對連結和命令表格式。
5. 若檢查失敗，閱讀完整錯誤、修正原因後重跑；無法執行時回報具體原因與未驗證項目。

## 完成條件

- 變更符合目前程式碼與 README 所記錄的執行限制。
- 相關驗證完成，或清楚列出未執行的檢查及原因。
- 新增或更新的 Harness 連結有效，命令均可追溯至專案文件或設定。
