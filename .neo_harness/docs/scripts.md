| 指令 | 用途 |
| :--- | :--- |
| `bash desktop/scripts/setup.sh` | 在 macOS／Linux 準備開發工具、投影依賴及前端套件。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\setup.ps1` | 在 Windows 準備開發工具、投影依賴及前端套件。 |
| `bash desktop/scripts/launch.sh dev` | 從新 checkout 自動準備開發環境與引擎，然後啟動 Debug。 |
| `bash desktop/scripts/launch.sh release` | 自動準備環境並建置 Release 執行檔，不產生安裝套件。 |
| `bash desktop/scripts/launch.sh bundle` | 自動準備環境並建置目前平台的安裝套件。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 dev` | 從新 checkout 自動準備 Windows 開發環境與引擎，然後啟動 Debug。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 release` | 自動準備 Windows 環境並建置 Release 執行檔，不產生安裝套件。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 bundle` | 自動準備 Windows 環境並建置安裝套件。 |
| `bash desktop/scripts/install.sh --check` | 只檢查 macOS／Linux 投影工具，缺少時回傳非零，不安裝軟體。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File desktop/scripts/install.ps1 -Check` | 只檢查 Windows 投影工具，缺少時回傳非零，不安裝軟體。 |
| `cd desktop && bun run --silent setup:runtime` | 手動準備目前平台的投影工具；正常 DEBUG/RELEASE 啟動會在背景自動檢查及準備。 |
| `cd desktop && bun install --frozen-lockfile --silent` | 依 `bun.lock` 安裝固定版本的前端套件。 |
| `cd desktop-test && bun install --frozen-lockfile --silent` | 依獨立鎖檔安裝前端、後端及桌面 E2E 測試套件。 |
| `cd desktop && bun run --silent dev` | 啟動 Vite 前端開發伺服器。 |
| `cd desktop && bun run tauri dev` | 啟動 Vite 與 Tauri 桌面開發環境。 |
| `cd desktop && bun run --silent typecheck` | 執行 Vue/TypeScript 型別檢查。 |
| `cd desktop-test && bun run --silent typecheck` | 執行獨立測試專案的 Vue/TypeScript 型別檢查。 |
| `cd desktop-test && bun run --silent test:frontend` | 執行前端測試型別檢查與 Vitest 單元測試。 |
| `cd desktop-test && bun run --silent test:backend` | 執行載入桌面後端模組的獨立 Rust 單元測試。 |
| `cd desktop-test && bun run --silent verify` | 執行前端與後端單元測試。 |
| `cd desktop && bun run --silent test:unit` | 相容入口，轉接至獨立測試專案執行前端單元測試。 |
| `cd desktop && bun run --silent test:setup` | 使用隔離的假工具檢查安裝、更新接續、工具鏈修復與提權記錄，不安裝系統套件；Windows 使用 PowerShell，其他平台有 pwsh 時也執行 PowerShell 函式案例。 |
| `cd desktop && bun run --silent test:version` | 驗證穩定版本格式、四處版本一致、patch 遞增與 tag 避讓、Cargo.lock 專案項目更新及純版本變更判斷。 |
| `pwsh -NoProfile -NonInteractive -File desktop/scripts/test-windows-setup.ps1 -InstallerPath desktop/scripts/install.ps1` | 從根目錄以假工具執行 Windows 安裝邏輯測試，不安裝系統套件；主控台檢查只在原生 Windows 執行。 |
| `cd desktop && bun run --silent rust:fmt` | 執行 `cargo fmt --check`。 |
| `cd desktop && bun run --silent rust:test` | 相容入口，轉接至獨立測試專案執行後端 Rust 單元測試。 |
| `cd desktop && bun run --silent rust:clippy` | 對所有 Rust targets/features 執行 Clippy，warning 視為錯誤。 |
| `cd desktop && bun run --silent verify` | 執行應用程式與測試型別檢查、前後端單元測試、安裝流程與版本工具測試、Rust 格式檢查、Clippy 及前端建置。 |
| `bash desktop/scripts/verify.sh` | 從專案根目錄執行 macOS／Linux 統一驗證入口。 |
| `pwsh desktop/scripts/verify.ps1` | 從專案根目錄執行 Windows 統一驗證入口。 |
| `cd desktop && bun run --silent build` | 執行型別檢查並建立 Vite production 前端。 |
| `cd desktop && bun run --silent preview` | 預覽已建置的 Vite production 前端。 |
| `cd desktop && bun run release` | 自動檢查／準備環境，建置 Release 執行檔但不打包安裝程式。 |
| `cd desktop-test && bun run --silent e2e:build` | 建置桌面 E2E 用的 Tauri 執行檔與假引擎。 |
| `cd desktop-test && bun run --silent test:e2e` | 執行已建置的桌面 E2E 測試；需要可互動桌面工作階段。 |
| `cd desktop-test && bun run --silent test:e2e:ci` | 建置 WDIO 測試版本並啟動桌面 E2E；需要可互動桌面工作階段。 |
| `cd desktop-test && dbus-run-session -- xvfb-run --auto-servernum bun run --silent test:e2e:ci` | 在 Linux 無桌面工作階段時以 D-Bus session 和虛擬顯示器執行桌面 E2E。 |
| `cd desktop && bun run --silent e2e:build` | 相容入口，轉接至獨立測試專案建置 E2E 執行檔與假引擎。 |
| `cd desktop && bun run --silent test:e2e` | 相容入口，轉接至獨立測試專案執行桌面 E2E。 |
| `cd desktop && bun run --silent test:e2e:ci` | 相容入口，轉接至獨立測試專案建置並執行桌面 E2E。 |
| `cd desktop && bun run --silent bundle` | 建置目前平台的 Tauri 安裝套件。 |
| `cd desktop && bun run --silent smoke:bundle` | 啟動目前平台正式 bundle 執行檔 8 秒；Linux 會使用 AppImage payload 或已安裝 DEB 的執行檔，並需要圖形工作階段。 |
| `cd desktop && dbus-run-session -- xvfb-run --auto-servernum bun run --silent smoke:bundle` | 在 Linux 無桌面工作階段時以 D-Bus session 和虛擬顯示器啟動正式執行檔。 |
| `brew install llvm makensis` | 安裝 README 所列 macOS Windows NSIS 交叉建置工具。 |
| `cargo install --locked cargo-xwin` | 安裝 README 所列 Windows MSVC 交叉建置 runner。 |
| `rustup target add x86_64-pc-windows-msvc` | 安裝 Windows x64 MSVC Rust target。 |
| `export PATH="$(brew --prefix llvm)/bin:$PATH"` | 將 Homebrew LLVM 加入目前 shell 的 PATH，以供 Windows 交叉建置使用。 |
| `cd desktop && bun run tauri build --ci --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis` | 從 macOS 交叉建置 Windows x64 NSIS 套件；Tauri 將此方式列為實驗性。 |
