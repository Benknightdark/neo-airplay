# GitHub Actions CI/CD 與版本化 Release

## 目的

新增 Windows x64 GitHub Actions CI/CD：develop 變更先建置及執行 Unit 測試，成功後建立或更新到 main 的 PR 並自動合併；main 合併後自動升 patch、建立完整可攜 Release、保留引擎下載套件的 SHA 設定，並在主視窗標題顯示 App 版本。

## 背景與目前行為

- 目前工作樹已有尚未提交的引擎下載、Bonjour 服務管理及 README/ Harness 文件變更；本計畫只疊加 CI/CD、版本與相關說明，不重設或提交其他變更。
- `src/NeoAirPlay.App/NeoAirPlay.App.csproj` 目前 `<Version>` 為 `0.1.0`，視窗標題目前硬編碼 `Neo AirPlay`。
- `.neo_harness/verify.ps1` 建置 solution 並執行 `Category=Unit` 測試；UI E2E 需互動式桌面，硬體 E2E 需外部設備及引擎。
- `engine/` 被 Git 忽略。`scripts/build-engine.ps1` 需要受信任的 Bonjour 安裝，並會 staging UxPlay/GStreamer 及 scrcpy/ADB；公開發佈不得包含 Bonjour 執行檔。
- 遠端目前沒有 develop 分支或 tag；GitHub App、repository secrets/variables、分支規則與自架 runner 狀態無法由本機工作樹確認。

## 決策與限制

- CI 使用 GitHub-hosted Windows runner，必要閘門為 build + Unit 測試，不包含 UI/Hardware E2E。
- develop push 在測試成功後建立/更新 develop → main PR 並設定 auto-merge；main PR 的測試狀態是必要檢查。
- main 每次合併升 SemVer patch。版本寫回 App csproj 和 engine-package.json；第一版從 0.1.0 升至 0.1.1，並同步版本設定到 develop。
- Release 工作使用 Windows self-hosted runner；使用者必須安裝並保持 runner 在線，且提供已安裝 Bonjour 的可信任建置主機。
- Release 提供完整可攜 ZIP（含 App 和 engine，排除 Bonjour）、引擎 ZIP、SHA-256 sidecar、tag `vX.Y.Z` 和 GitHub Release。
- 主視窗標題由組件的 App informational version 產生，移除 build metadata 後顯示 `Neo AirPlay (X.Y.Z)`。
- GitHub App 僅授予 repository contents 與 pull requests 寫入權限；workflow 使用短期 App installation token，PR 測試工作不取得 App 私鑰。
- GitHub App 建立/安裝、secrets/variables、自動合併設定、main required check/規則例外、develop 分支建立及 self-hosted runner 註冊是外部手動設定，程式碼無法自行完成。
- Release workflow 固定使用觸發該 run 的 main commit 建置；版本由來源 csproj 與既有穩定 tag 中較新的版本升 patch。相同 tag 若屬於其他來源 commit，發佈會停止而不覆寫；重跑失敗的來源 workflow 時會按最新 tag 重新取版。
- 自架 runner 會執行已合併的專案建置與測試程式碼；僅授權可信任維護者寫入 develop，並隔離 runner 與個人資料及其他憑證。

## 工作順序

1. 新增版本更新腳本及可在隔離暫存專案驗證的版本升級測試。
2. 將 MainWindow 標題接到 App assembly informational version，更新 UI E2E 標題斷言。
3. 新增 CI workflow：develop push 與 main PR 執行 Harness；develop 的驗證成功 job 使用 GitHub App 建立/更新 PR 並設定 auto-merge。
4. 新增 main Release workflow：在自架 runner 重跑 Harness、產生引擎與 App 套件、驗證套件內容及 SHA、寫回版本 metadata、同步 develop、推 tag 並建立 Release。生成版本提交不得遞迴觸發發佈。
5. 更新 README、Harness 架構/可靠性/安全文件、腳本命令表及本計畫狀態。
6. 執行 `.neo_harness/verify.ps1`、App UI E2E、腳本 parser/暫存 smoke tests、Actions workflow YAML/linter、發佈檔案/版本/SHA/Bonjour 排除檢查及文件連結檢查；將計畫標記完成並移到 completed。

## 進度與結果

- [x] 探索遠端分支、專案版本、測試入口、引擎打包與 Bonjour 建置依賴。
- [x] 新增版本升級工具、視窗標題及版本測試。
- [x] 新增 CI 與 Release workflows。
- [x] 更新 README 與 Harness 文件/命令表。
- [x] 驗證 workflow、測試、腳本語法及文件，記錄無法在本機完成的 Release 和 GitHub 設定。

## 驗證與復原

- CI 設定：GitHub hosted Windows runner 執行 `.neo_harness/verify.ps1`；main PR 僅在 `verify` 成功時可自動合併。尚未在 GitHub 啟用/執行，repository 設定完成前不能做端到端確認。
- 本機：`.neo_harness/verify.ps1` 成功，Framework 18 / App 13 Unit tests 通過，Set-AppVersion smoke tests 通過；`Category=E2E` 1 測試通過。
- GitHub Actions：actionlint 1.7.12、Release/Promotion PowerShell 區塊與腳本 parser 檢查通過；專案 Markdown 相對連結及 scripts.md 命令表格式檢查通過，`git diff --check` 無 whitespace error。
- Release ZIP、真實 GitHub PR/auto-merge、tag、Release 發佈與 App 下載安裝未能在本機端到端驗證；它們需有可信任 Bonjour 的 self-hosted runner 和 GitHub repository 設定。workflow 內含套件內容、版本、引擎 SHA 及 Bonjour 排除檢查。
- 本機驗證不會推送分支、提交版本、建立 tag/Release、建立 GitHub App 或修改 repository settings；這些需在 GitHub 完成設定後才可端到端確認。
- 如 workflow 或版本腳本失敗，僅回退本計畫新增的 workflow/版本/UI/docs 檔案；保留本計畫開始前的所有既有工作樹修改。
