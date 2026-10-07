# 驗證與命令

## 跨平台桌面程式

| 指令 | 用途 |
| :--- | :--- |
| `bun install --frozen-lockfile` | 依 `bun.lock` 安裝前端與 Tauri CLI 套件。 |
| `bun run typecheck` | 執行 Vue 與 TypeScript 型別檢查。 |
| `bun run test` | 執行 Vitest 前端單元測試。 |
| `bun run tauri dev` | 啟動 Tauri 開發版桌面程式。 |
| `pwsh ./.neo_harness/verify.ps1` | 執行前端、Rust 格式檢查、strict Clippy、Rust 單元測試與版本腳本 smoke tests；Windows 另外執行 Bonjour PowerShell 5.1 相容檢查及過渡期 .NET 建置與單元測試。 |
| `pwsh ./scripts/Run-UiE2e.ps1` | 建置含 WebdriverIO 測試插件的桌面程式，並以獨立暫存設定檔執行三頁導覽與設定儲存 UI E2E；需要可互動的桌面工作階段。 |
| `cargo test -p neo-core --test hardware -- --ignored --nocapture` | 以實際 UxPlay、mDNS、scrcpy v5 與授權 Android 裝置同時啟動兩種投影；測試結束時會停止兩個引擎。 |
| `NEO_HARDWARE_E2E=1 pwsh ./scripts/Run-UiE2e.ps1` | 額外執行 AirPlay 與 Android 同時投影驗收；需要可互動桌面、UxPlay/mDNS、scrcpy v5 及已授權的 Android 裝置。 |
| `cargo check -p neo-airplay --features e2e` | 只編譯 Tauri UI E2E 所需的測試插件功能。 |
| `pwsh ./scripts/Set-AppVersion.ps1 [-Version X.Y.Z]` | 將 `package.json` 與 Cargo workspace 設為指定穩定版本或升 patch，並更新 workspace lockfile。 |
| `pwsh ./scripts/Stage-ScrcpyRuntime.ps1 -Target <target>` | 核對 Genymobile 官方 scrcpy v5.0 發行包 SHA-256，並 staging 指定平台的 scrcpy、ADB 與授權檔案。 |
| `pwsh ./scripts/Pack-EngineRuntime.ps1 -Target <target> [-Version X.Y.Z]` | 將指定平台的引擎檔案打包為 ZIP、產生逐檔 SHA-256 manifest，並更新 `src-tauri/engine-package.json`。 |
| `pwsh ./scripts/build-engine.ps1 -Target windows-x86_64` | 在 Windows 建置 UxPlay/GStreamer，從可信任的 Bonjour 安裝讀取建置所需 DLL，並 staging scrcpy/ADB。 |
| `pwsh ./scripts/Build-ReleasePackage.ps1 -Version X.Y.Z` | 在 Windows 建置 NSIS 安裝程式、可攜 ZIP 與 Windows 引擎 ZIP；需要可信任的 Bonjour 安裝。 |
| `pwsh ./scripts/Build-UnixReleasePackage.ps1 -Version X.Y.Z -Target <target>` | 在原生 macOS 或 Linux runner 建置指定架構的 DMG、AppImage 或 deb 套件與引擎 ZIP。 |
| `pwsh ./scripts/Publish-GitHubRelease.ps1 -Version X.Y.Z -SourceSha <commit> -AssetsDirectory <path> -Repository <owner/repo>` | 合併各平台引擎 metadata、核對資產雜湊，並以 `GH_TOKEN` 更新版本記錄、建立 tag 及發布 GitHub Release；只供 Release workflow 呼叫。 |
| `./scripts/install-uxplay-macos.sh` | 在 macOS 依固定 SHA-256 下載並編譯 UxPlay，安裝至使用者資料目錄；會安裝必要 Homebrew 相依套件。 |
| `./scripts/install-uxplay-linux.sh [--build-from-source]` | 在 Linux 安裝發行版提供的 UxPlay；使用參數時依固定 SHA-256 下載並編譯 UxPlay。 |

`Stage-ScrcpyRuntime.ps1`、`Pack-EngineRuntime.ps1` 與 `Build-UnixReleasePackage.ps1` 支援的 target 為 `windows-x86_64`、`macos-aarch64`、`macos-x86_64` 及 `linux-x86_64`。Windows 的 UxPlay 建置與完整 Release 仍需使用專案設定的 self-hosted runner。

## Windows Bonjour 維護

| 指令或檔案 | 用途 |
| :--- | :--- |
| `pwsh ./scripts/Stage-BonjourRuntime.ps1` | 從可信任的 Bonjour 安裝尋找並驗證 responder 與同版本 x64 DNS-SD DLL，staging 至受保護的共用目錄。 |
| `pwsh ./scripts/Install-BonjourService.ps1` | 以系統管理員權限切換或建立 Bonjour Service，並儲存可供還原的原始狀態。 |
| `pwsh ./scripts/Restore-BonjourService.ps1` | 還原 Bonjour Service 原始路徑；若備份記錄服務原先不存在，移除 Neo AirPlay 建立的服務。 |
| `scripts/BonjourRuntimeHelpers.psm1` | 提供服務路徑、PE 架構、產品版本與可信任 Bonjour 來源檢查。 |
| `scripts/BonjourServiceSettings.json` | 提供 C# Framework 與 PowerShell 共用的執行檔候選、診斷訊息和服務備份位置。 |
| `pwsh ./tests/scripts/BonjourPowerShellCompatibility.Tests.ps1 -RepositoryRoot . -AppScriptsDirectories ./scripts` | 由 Windows PowerShell 5.1 執行時，驗證 Bonjour 腳本語法與設定檔 UTF-8 內容。 |

## 過渡期 WPF/.NET 檢查

| 指令 | 用途 |
| :--- | :--- |
| `dotnet build NeoAirPlay.slnx -v q --nologo` | 建置既有 WPF App、Framework 與測試專案。 |
| `dotnet test NeoAirPlay.slnx --no-build -v q --nologo --filter Category=Unit` | 執行既有 .NET 單元測試。 |
| `dotnet test tests/NeoAirPlay.App.Test/NeoAirPlay.App.Test.csproj --filter Category=E2E` | 在互動式 Windows 桌面執行 WPF UI E2E 測試。 |
| `dotnet test tests/NeoAirPlay.App.Test/NeoAirPlay.App.Test.csproj --filter Category=HardwareE2E` | 需有 UxPlay/GStreamer、執行中的專案 Bonjour Service、互動式桌面及已授權 USB Android 裝置。 |

GitHub Actions 的 `.github/workflows/ci.yml` 會在 Windows、macOS 與 Linux 執行跨平台 Harness，並以 `verify` 彙總 job 作為必要檢查名稱。`.github/workflows/release.yml` 會建置 Windows x64、macOS arm64/x64 及 Linux x64 套件；Windows 引擎建置與最終發佈仍使用標記為 `neo-airplay-release` 的 self-hosted runner。develop 成功後，`.github/workflows/promote-develop.yml` 建立或更新 PR。
