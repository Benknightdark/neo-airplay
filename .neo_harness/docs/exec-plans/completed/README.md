# 已完成的計畫

| 計畫 | 目的 | 狀態 |
| :--- | :--- | :--- |
| [Tauri 跨平台遷移](20261008154931_tauri_cross_platform_migration.md) | 將 Windows 專用 WPF 系統遷移為 Tauri、Rust、Vue、TypeScript 與 Bun 跨平台桌面應用程式。 | macOS 原生、Linux x86_64 模擬、Windows x64 交叉建置與測試編譯通過；原生執行及實機串流待驗證 |
| [專案檢查與首次依賴準備](20261008195306_review_and_dependency_bootstrap.md) | 修正專案檢查發現，實作 DEBUG／RELEASE 背景依賴準備，並將安裝工具固定放入 `desktop/engine/`。 | macOS 安裝、Harness、Release 封裝與啟動 smoke check 通過；Windows／Linux 實機安裝及真實裝置串流待驗證 |
