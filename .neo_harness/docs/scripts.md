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
| .\scripts\Stage-BonjourRuntime.ps1 | 以系統管理員權限從 Program Files / Program Files (x86) 找 Bonjour responder，並從可信任 x64 Bonjour 目錄或 System32 找同版本 x64 DNS-SD DLL，驗證後複製至共用目錄。 |
| .\scripts\BonjourRuntimeHelpers.psm1 | Bonjour 服務路徑解析、PE 架構、產品/版本相容性及可信任來源檢查模組，由 Stage、Install 和 build-engine 腳本載入。 |
| .\scripts\BonjourServiceSettings.json | C# Framework 與 PowerShell 共用的 Bonjour 來源執行檔候選、執行目錄、診斷訊息及服務路徑備份位置。 |
| .\scripts\Install-BonjourService.ps1 | 以系統管理員權限設定受保護目錄 ACL，切換並啟動既有 Bonjour Service，或從可信任 staging 執行檔建立並啟動缺少的服務。 |
| .\scripts\Restore-BonjourService.ps1 | 還原 Bonjour Service 原始路徑；若備份記錄服務原先不存在，移除 Neo AirPlay 建立的服務。 |
| .\.neo_harness\verify.ps1 | 建置方案、以 Windows PowerShell 5.1 驗證 Bonjour 腳本與設定編碼、執行標記為 Unit 的測試及 Set-AppVersion smoke tests；不執行 UI E2E。 |

GitHub Actions workflow：`.github/workflows/ci.yml` 執行上述 verify 入口；develop 成功後 `.github/workflows/promote-develop.yml` 建立／更新 PR；main push 由 `.github/workflows/release.yml` 建置並發佈 Windows x64 Release。必要的 repository 設定與自架 runner 請見 README.md 的「GitHub Actions CI/CD 與下載」。
