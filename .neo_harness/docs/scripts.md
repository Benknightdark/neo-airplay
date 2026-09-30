| 指令 | 用途 |
| :--- | :--- |
| dotnet build NeoAirPlay.slnx | 建置 App、Framework 與兩個測試專案。 |
| dotnet test NeoAirPlay.slnx --filter "Category=Unit" | 執行 Framework 與 App 單元測試。 |
| dotnet test tests/NeoAirPlay.App.Test/NeoAirPlay.App.Test.csproj --filter "Category=E2E" | 在互動式 Windows 桌面執行 WPF UI E2E 測試。 |
| dotnet publish src/NeoAirPlay.App/NeoAirPlay.App.csproj -c Release -o dist/NeoAirPlay-Release | 建置 Release 發佈輸出。 |
| .\scripts\build-engine.ps1 | 準備建置環境並建置接收引擎及影音執行元件。 |
| .\scripts\Stage-BonjourRuntime.ps1 | 從本機 Bonjour 安裝複製服務執行元件至專案。 |
| .\scripts\Install-BonjourService.ps1 | 以系統管理員權限將 Bonjour Service 切換至專案路徑。 |
| .\scripts\Restore-BonjourService.ps1 | 還原 Bonjour Service 原始路徑。 |
| .\.neo_harness\verify.ps1 | 建置方案並執行標記為 Unit 的測試；不執行 UI E2E。 |
