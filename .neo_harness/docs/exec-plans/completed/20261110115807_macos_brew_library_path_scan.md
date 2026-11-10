# 修正 macOS Homebrew 函式庫路徑掃描

## 目的

修正 GitHub Actions macOS 驗證中兩個安裝檢查案例超過 10 秒子程序上限的問題，並避免每次 macOS AirPlay 啟動時枚舉所有 Homebrew formula 目錄。

## 背景與目前行為

- `install.sh` 與 Rust 的 macOS library path helper 都列舉 `/opt/homebrew/opt` 和 `/usr/local/opt` 下的每個 formula，並將其 `lib` 加入 `DYLD_LIBRARY_PATH`。
- `install.sh --check --development` 的 macOS 案例在 GitHub runner 約 10 秒被 Node `spawnSync` 終止；已捕捉的輸出顯示 runtime check 通過後，卡在開發工具檢查期間。
- 本機 Bash trace 顯示 GStreamer 檢查呼叫 library path helper 時展開所有本機 Homebrew formula 路徑；本機 Homebrew 目錄含大量 formula，造成大量 filesystem 查詢與過長的環境變數。
- 本機 `otool -L` 檢查到的 GStreamer 可執行檔、scanner 及必要 plugins 使用絕對 Homebrew install names；GStreamer runtime 本身也會複製至 `engine/lib`。

## 預期行為與限制

- macOS `DYLD_LIBRARY_PATH` 保留 `engine/lib`、Homebrew prefix 的共用 `lib` 目錄及原本繼承路徑，不再枚舉每個 formula。
- Bash 安裝檢查與 Rust 引擎啟動使用一致的路徑集合。
- Windows、Linux、公開 IPC 與安裝相依套件保持不變。

## 工作順序

1. 移除 Bash 與 Rust macOS helper 對 Homebrew `opt/*/lib` 的全量掃描。
2. 加入 Rust 路徑單元測試，並重跑逾時的安裝流程案例。
3. 更新架構文件，執行安裝測試及 Harness 完整驗證。

## 發現與決策

- GitHub 日誌中的 `status: null` 是 `spawnSync` 逾時後的狀態；失敗測試共用 10 秒子程序上限。
- 推論：GitHub runner 上同一段全量路徑展開使原有開發工具檢查超過 10 秒；日誌只顯示 runtime 檢查完成後子程序逾時，本機 trace 證實全量展開存在於該 macOS 路徑。
- 保留現有子程序逾時作為慢測試保護，修正不必要的全量目錄掃描。

## 驗證與復原

- macOS 安裝流程測試確認兩個失敗案例在既有 10 秒子程序上限內完成。
- Rust 單元測試確認 `DYLD_LIBRARY_PATH` 只列出 engine、Homebrew 共用路徑及繼承路徑。
- 執行 `pwsh -NoProfile -File .neo_harness/verify.ps1`。
- 若縮小 library path 後真實 GStreamer 載入失敗，恢復所需的明確依賴目錄，不恢復全量 formula 掃描。

## 進度

- [x] 根據 GitHub log 與 Bash trace 定位慢路徑。
- [x] 收斂 Bash/Rust library path 並加入回歸測試。
- [x] 更新文件並完成驗證。

## 結果

- 四項 macOS 安裝流程案例通過；兩個原逾時案例分別在約 1.7 秒與 0.5 秒完成。
- 本機 Homebrew GStreamer 在縮小後的 `DYLD_LIBRARY_PATH` 下完成真實 plugin scan，找到全部七個必要 element。
- `.neo_harness/verify.ps1` 通過，前端 23 項與 Rust 28 項測試全數通過；`bun run --silent test:e2e:ci` 的五項測試通過。
- 第一次完整驗證中，另一項 Rust 安裝子程序測試未讀到階段記錄；該測試單獨重跑及第二次完整驗證皆通過，這次未修改該測試。
- 尚未由 GitHub Actions runner 執行此分支；需以後續 CI run 確認 runner 上的結果。
