# 驗證與建置命令

前端命令定義於 `desktop/package.json`，Rust 命令由該檔案帶入 `desktop/src-tauri/Cargo.toml`。Bun 管理前端套件與指令，Node.js 22.22.2 以上的 22.x 版本 執行 Vitest。根目錄 Harness 入口 `.neo_harness/verify.ps1` 呼叫同一個 `bun run verify` 指令。

| 指令 | 用途 |
| :--- | :--- |
| `bash desktop/scripts/setup.sh` | 在 macOS／Linux 準備開發工具、投影依賴及前端套件。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\setup.ps1` | 在 Windows 準備開發工具、投影依賴及前端套件。 |
| `bash desktop/scripts/launch.sh dev` | 從新 checkout 自動準備開發環境與引擎，然後啟動 Debug。 |
| `bash desktop/scripts/launch.sh release` | 自動準備環境並建置 Release 執行檔，不產生安裝套件。 |
| `bash desktop/scripts/launch.sh bundle` | 自動準備環境並建置目前平台的安裝套件。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 dev` | Windows 新 checkout 的自動準備與 Debug 入口；`release` 與 `bundle` 選項同名。 |
| `cd desktop && bun run --silent setup:runtime` | 手動準備目前平台的投影工具；正常 DEBUG/RELEASE 啟動會在背景自動檢查及準備。 |
| `bash desktop/scripts/install.sh --check` | 只檢查 macOS／Linux 投影工具，缺少時回傳非零，不安裝軟體。 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File desktop/scripts/install.ps1 -Check` | 只檢查 Windows 投影工具，缺少時回傳非零，不安裝軟體。 |
| `cd desktop && bun install --frozen-lockfile --silent` | 依 `bun.lock` 安裝固定版本的前端套件。 |
| `cd desktop && bun run tauri dev` | 啟動 Vite 與 Tauri 桌面開發環境。 |
| `cd desktop && bun run release` | 自動檢查／準備環境，建置 Release 執行檔但不打包安裝程式。 |
| `pwsh .neo_harness/verify.ps1` | 執行 TypeScript 型別檢查、前端單元測試、Rust 格式檢查、Rust 單元測試、Clippy 及 Vite 建置。 |
| `cd desktop && bun run --silent typecheck` | 執行 Vue/TypeScript 型別檢查。 |
| `cd desktop && bun run --silent test:unit` | 執行 Vitest 前端單元測試。 |
| `cd desktop && bun run --silent test:setup` | 使用隔離的假工具執行安裝腳本檢查及跳過安裝情境，不安裝系統套件；Windows 略過 Bash 平台案例。 |
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

Tauri CLI 由 `desktop/scripts/run-tauri.mjs` 包裝，執行 Debug、Release 或 build 時會先呼叫平台 setup；首次取得專案可透過 `launch.sh`／`launch.ps1` 在 Bun 未安裝前啟動。安裝腳本會先檢查依賴，全部就緒時略過安裝。CI 全域設定 `NEO_AIRPLAY_SKIP_SETUP=1`；E2E build 與 smoke script 也明確設定，避免驗證時安裝系統工具。安裝腳本與其平台限制見根目錄 README。只有實際執行過的檢查結果會記錄於 [Review 記錄](review-and-setup.md)。

GitHub workflow：`.github/workflows/ci.yml` 在 `develop`、`main` push 及 `main` Pull Request 執行各平台驗證、E2E 與套件建置；產物上傳為保留 14 天的 Actions artifact。
