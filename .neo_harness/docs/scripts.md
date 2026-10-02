| 指令 | 用途 |
| :--- | :--- |
| dotnet build NeoAirPlay.slnx | 建置 App、Framework 與兩個測試專案。 |
| dotnet test NeoAirPlay.slnx --filter "Category=Unit" | 執行 Framework 與 App 單元測試。 |
| dotnet test tests/NeoAirPlay.App.Test/NeoAirPlay.App.Test.csproj --filter "Category=E2E" | 在互動式 Windows 桌面執行 WPF UI E2E 測試。 |
| dotnet test tests/NeoAirPlay.App.Test/NeoAirPlay.App.Test.csproj --filter "Category=HardwareE2E" | 需先關閉其他接收器，並有 UxPlay/GStreamer、執行中的專案 Bonjour Service、互動式桌面及已授權 USB Android；驗證引擎並行與 WPF 總覽啟停 Android 投影。 |
| dotnet publish src/NeoAirPlay.App/NeoAirPlay.App.csproj -c Release -o dist/NeoAirPlay-Release | 建置 Release 發佈輸出。 |
| .\scripts\Set-AppVersion.ps1 [-Version X.Y.Z] | 將 App csproj 版本升 patch，或設定指定穩定 SemVer 版本。 |
| .\scripts\build-engine.ps1 | 準備建置環境、建置 UxPlay/GStreamer，從可信任的 Bonjour 安裝讀取建置所需 DLL，並 staging 官方 scrcpy/ADB Android 執行元件。 |
| .\scripts\Pack-EngineRuntime.ps1 | 將目前建置的 UxPlay/GStreamer 與 scrcpy/ADB 打包成公開 Release 引擎 ZIP（排除 Bonjour），並更新 App 的套件版本與 SHA-256 設定。 |
| .\scripts\Build-ReleasePackage.ps1 -Version X.Y.Z | 建置引擎、發佈 self-contained Windows x64 App，輸出完整可攜 ZIP、引擎 ZIP、SHA-256 sidecar 與 manifest；需要可信任的 Bonjour 安裝。 |
| .\scripts\Publish-GitHubRelease.ps1 -Version X.Y.Z -SourceSha <commit> -AssetsDirectory <path> -Repository <owner/repo> | 以 `GH_TOKEN` 寫回 main/develop 版本 metadata、建立 tag，並發布 GitHub Release。只供 Release workflow 呼叫。 |
| .\scripts\Stage-ScrcpyRuntime.ps1 | 從 Genymobile 官方 scrcpy v4.1 發行包核對 SHA-256 並 staging Windows x64 工具與授權文件。 |
| .\scripts\Stage-BonjourRuntime.ps1 | 以系統管理員權限從可信任的 Bonjour 安裝複製服務元件至 Program Files 共用目錄。 |
| .\scripts\BonjourRuntimeHelpers.psm1 | Bonjour 服務路徑解析、元件驗證與可信任來源檢查模組，由 Stage 和 Install 腳本載入。 |
| .\scripts\BonjourServiceSettings.json | C# Framework 與 PowerShell 共用的 Bonjour 執行目錄及服務路徑備份位置。 |
| .\scripts\Install-BonjourService.ps1 | 以系統管理員權限將 Bonjour Service 切換至受保護的 Program Files 共用目錄並設定 ACL。 |
| .\scripts\Restore-BonjourService.ps1 | 還原 Bonjour Service 原始路徑。 |
| .\.neo_harness\verify.ps1 | 建置方案、執行標記為 Unit 的測試，並執行 Set-AppVersion PowerShell smoke tests；不執行 UI E2E。 |

GitHub Actions workflow：`.github/workflows/ci.yml` 執行上述 verify 入口；develop 成功後 `.github/workflows/promote-develop.yml` 建立／更新 PR；main push 由 `.github/workflows/release.yml` 建置並發佈 Windows x64 Release。必要的 repository 設定與自架 runner 請見 README.md 的「GitHub Actions CI/CD 與下載」。
