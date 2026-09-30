| 指令 | 用途 |
| :--- | :--- |
| dotnet build NeoAirPlay.slnx | 建置方案；目前 Harness 驗證入口執行的檢查。 |
| dotnet publish src/NeoAirPlay/NeoAirPlay.csproj -c Release -o dist/NeoAirPlay-Release | 建置 Release 發佈輸出。 |
| .\scripts\build-engine.ps1 | 準備建置環境並建置接收引擎及影音執行元件。 |
| .\scripts\Stage-BonjourRuntime.ps1 | 從本機 Bonjour 安裝複製服務執行元件至專案。 |
| .\scripts\Install-BonjourService.ps1 | 以系統管理員權限將 Bonjour Service 切換至專案路徑。 |
| .\scripts\Restore-BonjourService.ps1 | 還原 Bonjour Service 原始路徑。 |
| .\.neo_harness\verify.ps1 | 執行方案建置並傳回建置失敗狀態；不執行行為測試。 |
