# 驗證與建置命令

前端命令定義於 `desktop/package.json`，Rust 命令由該檔案帶入 `desktop/src-tauri/Cargo.toml`。Bun 管理前端套件與指令，Node.js 22 執行 Vitest。根目錄 Harness 入口 `.neo_harness/verify.ps1` 呼叫同一個 `bun run verify` 指令。

| 指令 | 用途 |
| :--- | :--- |
| `cd desktop && bun install --frozen-lockfile --silent` | 依 `bun.lock` 安裝固定版本的前端套件。 |
| `cd desktop && bun run tauri dev` | 啟動 Vite 與 Tauri 桌面開發環境。 |
| `pwsh .neo_harness/verify.ps1` | 執行 TypeScript 型別檢查、前端單元測試、Rust 格式檢查、Rust 單元測試、Clippy 及 Vite 建置。 |
| `cd desktop && bun run --silent typecheck` | 執行 Vue/TypeScript 型別檢查。 |
| `cd desktop && bun run --silent test:unit` | 執行 Vitest 前端單元測試。 |
| `cd desktop && bun run --silent rust:fmt` | 執行 `cargo fmt --check`。 |
| `cd desktop && bun run --silent rust:test` | 以所有 Rust features 執行單元測試，包含 WDIO Rust 插件編譯。 |
| `cd desktop && bun run --silent rust:clippy` | 對所有 Rust targets/features 執行 Clippy，warning 視為錯誤。 |
| `cd desktop && bun run --silent build` | 執行型別檢查並建立 Vite production 前端。 |
| `cd desktop && bun run --silent test:e2e:ci` | 建置 WDIO 測試版本並啟動桌面 E2E；需要可互動桌面工作階段。 |
| `cd desktop && dbus-run-session -- xvfb-run --auto-servernum bun run --silent test:e2e:ci` | 在 Linux 無桌面工作階段時以 D-Bus session 和虛擬顯示器執行桌面 E2E。 |
| `cd desktop && bun run --silent bundle` | 建置目前平台的 Tauri 安裝套件。 |
| `cd desktop && bun run --silent smoke:bundle` | 啟動目前平台正式 bundle 執行檔 8 秒；Linux 會使用 AppImage payload 或已安裝 DEB 的執行檔，並需要圖形工作階段。 |
| `cd desktop && dbus-run-session -- xvfb-run --auto-servernum bun run --silent smoke:bundle` | 在 Linux 無桌面工作階段時以 D-Bus session 和虛擬顯示器啟動正式執行檔。 |
| `cd desktop && bun run tauri build --ci --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis` | 依 README 安裝交叉編譯工具後，從 macOS 產生 Windows x64 NSIS 套件；此建置方式屬實驗性。 |

CI 在 Linux 使用 `dbus-run-session -- xvfb-run --auto-servernum bun run --silent test:e2e:ci` 與 `dbus-run-session -- xvfb-run --auto-servernum bun run --silent smoke:bundle` 建立 D-Bus session 和虛擬顯示器。原生安裝套件輸出於 `desktop/src-tauri/target/release/bundle/`；實際檔案格式依建置作業系統而定。

GitHub workflow：`.github/workflows/ci.yml` 在 `develop`、`main` push 及 `main` Pull Request 執行各平台驗證、E2E 與套件建置；產物上傳為保留 14 天的 Actions artifact。
