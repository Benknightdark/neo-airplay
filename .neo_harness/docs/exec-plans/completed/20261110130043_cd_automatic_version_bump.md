# CD 自動遞增版本

## 目的

讓 `main` PR 合併後的 CD 自動恢復重複 tag，成功發布後準備下一個 patch 版本，同時保留 PR 分支流程。

## 背景與目前行為

- CD 在 PR 合併到 `main` 後檢查四處版本一致及 tag 未使用；重複 tag 會使工作流程失敗。
- 專案版本位於 `desktop/package.json`、`desktop/src-tauri/tauri.conf.json`、`desktop/src-tauri/Cargo.toml` 與 Cargo.lock 的 `neo-airplay` 項目；`bun.lock` 沒有根套件版本欄位。
- CI 對 `develop` 的每次 push 都執行驗證並建立或沿用 `develop` 到 `main` 的 promotion PR。

## 預期行為與限制

- 只自動遞增 patch，維持穩定版 `X.Y.Z` 格式及四處版本一致。
- 若 merged 版本 tag 已存在，CD 建立或更新到 `main` 的版本修正 PR，該次略過建置；修正 PR 合併後的新 CD run 才發布。
- Release 成功後建立或更新到 `develop` 的下一個未使用 patch 版本 PR。
- `develop` 相對 `main` 的完整差異只有版本檔時仍執行 CI 驗證，但不建立空的 promotion PR；這也涵蓋先前 CI 被取消後累積的提交。

## 工作順序

1. 新增版本讀取、校驗、patch 遞增、tag 避讓、四檔同步與純版本變更判斷工具及單元測試。
2. 更新 CD 的重複 tag 修正 PR、發布後下一版 PR、權限與重跑行為。
3. 更新 CI 純版本變更保護、命令表與可靠性文件。
4. 執行版本工具測試、workflow lint、Harness 完整驗證。

## 發現與決策

- 使用者選擇 patch 自動遞增；tag 撞號時先建立 `main` 修正 PR，保留 tag 與已提交版本一致。
- 使用者選擇以 `GITHUB_TOKEN` 建立 bot PR，不直接推送受保護分支。
- GitHub Actions repo 層級設定需允許 workflow 建立 PR；本機 GitHub CLI 未登入，無法讀取目前設定。
- 版本 PR 分別使用穩定 bot branch 名稱，重跑時更新或沿用既有 PR。

## 驗證與復原

- 版本工具單元測試覆蓋格式、一致性、patch 遞增、tag 避讓、Cargo.lock 範圍及純版本路徑判斷。
- 以 `.neo_harness/verify.ps1` 執行完整本機驗證；本機無 GitHub 認證，不能建立真實 PR 或執行發佈流程。
- 若有 `actionlint`，執行 `actionlint .github/workflows/ci.yml .github/workflows/cd.yml`；若未安裝，至少以 YAML parser 和 `bash -n` 檢查 workflow 語法並記錄缺口。
- 若自動 PR 或版本選取不正確，可還原 workflow 和版本工具；已建立的 bot PR 需在 GitHub 關閉。

## 進度

- [x] 確認 CD、CI 分支流程及四處版本來源。
- [x] 新增可測試的版本工具與測試。
- [x] 實作 CD/CI 自動 PR 流程。
- [x] 更新文件並完成驗證。

## 結果

已完成版本一致性和 patch 遞增工具、tag 撞號時的 main 修正 PR、Release 後的 develop 下一版 PR，以及純版本提交的 CI promotion 保護。`pwsh -NoProfile -File .neo_harness/verify.ps1` 通過；版本工具 6 項測試通過。`actionlint` 未安裝，因此改以 Ruby YAML parser 與 `bash -n` 檢查兩份 workflow。真實 GitHub PR、Release 流程未執行；repository 的 Actions「允許 workflow 建立 pull request」設定仍需在 GitHub 確認。
