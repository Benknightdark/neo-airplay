| 指令 | 用途 |
| :--- | :--- |
| `bash desktop/scripts/setup.sh` | 在 macOS／Linux 安裝開發工具、投影依賴與前端套件。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\setup.ps1` | 在 Windows 安裝開發工具、投影依賴與前端套件。 |
| `bash desktop/scripts/launch.sh dev` | 自動準備 macOS／Linux 環境與引擎並啟動 Debug。 |
| `bash desktop/scripts/launch.sh release` | 自動準備 macOS／Linux 環境並建置 Release 執行檔（不打包）。 |
| `bash desktop/scripts/launch.sh bundle` | 自動準備 macOS／Linux 環境並建置目前平台安裝套件。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 dev` | 自動準備 Windows 環境與引擎並啟動 Debug。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 release` | 自動準備 Windows 環境並建置 Release 執行檔（不打包）。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 bundle` | 自動準備 Windows 環境並建置安裝套件。 |
| `bash desktop/scripts/install.sh --check` | 檢查 macOS／Linux 投影工具是否就緒（不安裝）。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File desktop/scripts/install.ps1 -Check` | 檢查 Windows 投影工具是否就緒（不安裝）。 |
| `cd desktop && bun run --silent setup:runtime` | 手動準備目前平台的投影工具。 |
| `cd desktop && bun install --frozen-lockfile --silent` | 依 `desktop/bun.lock` 安裝前端套件。 |
| `cd desktop-test && bun install --frozen-lockfile --silent` | 依 `desktop-test/bun.lock` 安裝測試與 E2E 套件。 |
| `cd desktop && bun run --silent dev` | 啟動 Vite 前端開發伺服器。 |
| `cd desktop && bun run tauri dev` | 啟動 Vite 與 Tauri 桌面開發環境。 |
| `cd desktop && bun run --silent typecheck` | 執行 `desktop` 的 Vue/TypeScript 型別檢查。 |
| `cd desktop-test && bun run --silent typecheck` | 執行 `desktop-test` 的 Vue/TypeScript 型別檢查。 |
| `cd desktop-test && bun run --silent test:frontend` | 執行測試型別檢查與 Vitest 前端單元測試。 |
| `cd desktop-test && bun run --silent test:backend` | 執行後端 Rust 單元測試。 |
| `cd desktop-test && bun run --silent verify` | 執行前端與後端單元測試。 |
| `cd desktop && bun run --silent test:unit` | 轉接至 `desktop-test` 執行前端單元測試。 |
| `cd desktop && bun run --silent test:setup` | 以隔離假工具測試安裝、路徑保留、工具鏈修復與提權記錄。 |
| `cd desktop && bun run --silent test:version` | 測試四處版本一致性、patch 遞增、tag 避讓與純版本變更判斷。 |
| `pwsh -NoProfile -NonInteractive -File desktop/scripts/test-windows-setup.ps1 -InstallerPath desktop/scripts/install.ps1` | 以隔離假工具執行 Windows 安裝邏輯測試。 |
| `cd desktop && bun run --silent rust:fmt` | 執行 `cargo fmt --check`。 |
| `cd desktop && bun run --silent rust:test` | 轉接至 `desktop-test` 執行後端 Rust 單元測試。 |
| `cd desktop && bun run --silent rust:clippy` | 對所有 Rust targets/features 執行 Clippy（warning 視為錯誤）。 |
| `cd desktop && bun run --silent verify` | 執行型別、前後端單元測試、安裝與版本測試、Rust 格式、Clippy 與前端建置。 |
| `bash desktop/scripts/verify.sh` | macOS／Linux 專案根目錄統一驗證入口。 |
| `pwsh desktop/scripts/verify.ps1` | Windows 專案根目錄統一驗證入口。 |
| `cd desktop && bun run --silent build` | 執行型別檢查並建置 Vite production 前端。 |
| `cd desktop && bun run --silent preview` | 預覽已建置的 Vite production 前端。 |
| `cd desktop && bun run release` | 自動準備環境並建置 Release 執行檔（不打包）。 |
| `cd desktop-test && bun run --silent e2e:build` | 建置桌面 E2E 專用的 Tauri 執行檔與假引擎。 |
| `cd desktop-test && bun run --silent test:e2e` | 執行已建置的桌面 E2E 測試（需圖形工作階段）。 |
| `cd desktop-test && bun run --silent test:e2e:ci` | 建置 WDIO 測試版並執行桌面 E2E（需圖形工作階段）。 |
| `cd desktop-test && dbus-run-session -- xvfb-run --auto-servernum bun run --silent test:e2e:ci` | 在 Linux 無桌面環境以 D-Bus + Xvfb 執行桌面 E2E。 |
| `cd desktop && bun run --silent e2e:build` | 轉接至 `desktop-test` 建置 E2E 執行檔與假引擎。 |
| `cd desktop && bun run --silent test:e2e` | 轉接至 `desktop-test` 執行桌面 E2E。 |
| `cd desktop && bun run --silent test:e2e:ci` | 轉接至 `desktop-test` 建置並執行桌面 E2E。 |
| `cd desktop && bun run --silent bundle` | 建置目前平台的 Tauri 安裝套件。 |
| `cd desktop && bun run --silent smoke:bundle` | 以隔離暫存目錄啟動正式 bundle 執行檔 8 秒（需圖形工作階段）。 |
| `cd desktop && dbus-run-session -- xvfb-run --auto-servernum bun run --silent smoke:bundle` | 在 Linux 無桌面環境以 D-Bus + Xvfb 執行正式執行檔 smoke check。 |
| `brew install llvm makensis` | 安裝 macOS 交叉建置 Windows NSIS 所需的 LLVM 與 makensis。 |
| `cargo install --locked cargo-xwin` | 安裝 Windows MSVC 交叉建置 runner。 |
| `rustup target add x86_64-pc-windows-msvc` | 新增 Windows x64 MSVC Rust target。 |
| `export PATH="$(brew --prefix llvm)/bin:$PATH"` | 將 Homebrew LLVM 加入目前 shell `PATH`。 |
| `cd desktop && bun run tauri build --ci --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis` | 從 macOS 交叉建置 Windows x64 NSIS 安裝套件（實驗性）。 |
