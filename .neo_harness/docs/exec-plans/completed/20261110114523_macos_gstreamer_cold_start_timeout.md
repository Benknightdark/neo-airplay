# 修正 macOS GStreamer 冷啟動逾時

## 目的

避免首次 GStreamer registry 掃描超過目前 30 秒上限後，預熱失敗並讓 UxPlay 在 Bonjour 出現前被終止；不需重開 App 才能連線。

## 背景與目前行為

- 使用者選擇將首次掃描等待放在工具準備階段，讓 AirPlay 按鈕在預熱完成後才可用。
- 本機全新 registry 掃描在 30 秒預熱上限被終止；放寬至 90 秒後，掃描在 34.9 秒完成並建立 1,520,888 位元組 registry。
- App 記錄顯示兩次 Bonjour 等待在 30 秒失敗，registry 建立後第三次在 8.2 秒就緒。

## 預期行為與限制

- GStreamer registry 預熱最多 90 秒，Rust 外層等待最多 100 秒；工具準備面板持續顯示預熱訊息。
- 預熱錯誤記錄退出原因與耗時，部分 registry 仍清除；失敗後依舊允許啟動 AirPlay。
- 冷啟動 Bonjour 後備等待延長至 90 秒，等待中仍可取消；不改變公開 IPC、Windows 或 Linux 行為。

## 工作順序

1. 延長安裝腳本預熱與 Rust 外層 timeout，加入明確 timeout/退出代碼及耗時記錄。
2. 延長 AirPlay Bonjour readiness timeout，錯誤訊息使用實際上限秒數。
3. 更新安裝腳本與 Rust 測試及可靠性文件。
4. 執行 Harness 完整驗證、桌面 E2E 與 bundle smoke；真實 iPhone 連線列為人工驗收。

## 發現與決策

- 工作樹乾淨，分支為 `develop`。
- 使用者選擇「先預熱」：最多 90 秒等待放在工具準備階段；Bonjour 後備上限也是 90 秒。
- 實際 90 秒掃描探測已於 34.9 秒成功；掃描時間可能依主機狀況變動，因此保留 90 秒上限。

## 驗證與復原

- 安裝測試確認傳入 90 秒 timeout、預熱成功建立 registry，以及 timeout 時輸出原因並移除不完整 registry。
- Rust 測試確認 90 秒內的 Bonjour 就緒可轉為 `Running`，超過上限才轉為 `Faulted`。
- 執行 `.neo_harness/verify.ps1`、`bun run --silent test:e2e:ci`，並視本機 bundle 可用性執行 `bun run --silent smoke:bundle`。
- 若 90 秒預熱使工具準備流程不穩定，可還原 installer 與 Rust 預熱上限；若 Bonjour 探索出現新問題，可獨立還原 readiness 上限。

## 進度

- [x] 依本機 App 與安裝記錄確認冷啟動失敗模式。
- [x] 延長 registry 預熱並改善錯誤記錄。
- [x] 延長 Bonjour 後備等待並更新測試、文件。
- [x] 執行驗證並記錄結果。

## 結果

- `bash -n desktop/scripts/install.sh` 與 `git diff --check` 通過。
- `bun run --silent test:setup` 通過。
- `pwsh -NoProfile -File .neo_harness/verify.ps1` 通過；前端 23 項測試及 Rust 27 項測試全數通過，Clippy 與正式前端建置也成功。
- `bun run --silent test:e2e:ci` 通過，5 項桌面流程測試成功。
- `NEO_AIRPLAY_SKIP_SETUP=1 bun run --silent bundle` 與 `bun run --silent smoke:bundle` 通過；本機建置略過 Apple notarization。
- 尚未以真實 iPhone 驗證 Bonjour 探索與投影連線。
