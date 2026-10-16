# 舊版引擎資源目錄

引擎工具目前放在 `desktop/engine/`。此舊目錄不再供 App 搜尋，也不會作為 Tauri 套件的引擎來源。

請勿將大型執行檔放在這裡。`desktop/engine/` 的內容會由 Git 忽略，只有 `.GITKEEP` 追蹤；打包會從該目錄收集本機已準備的引擎檔案。
