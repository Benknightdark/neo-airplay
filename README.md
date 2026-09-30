# Neo AirPlay

Windows 11 x64 的 AirPlay 接收器，使用 WPF/XAML 建立操作介面，並以獨立的 UxPlay 程序接收鏡像畫面和音訊。程式原始碼由本專案自行建立；不包含或修改 `uxplay-windows` wrapper。

## 目前功能

- 啟動與停止 AirPlay 接收器，狀態列會顯示缺少的引擎元件或錯誤。
- 接收畫面鏡像與音訊，也支援音訊播放。
- 使用 UxPlay 自己的播放視窗顯示鏡像畫面。
- 設定接收器名稱、影片輸出 renderer、全螢幕播放，以及登入 Windows 後自動啟動。
- 關閉主視窗時留在系統匣；診斷頁保留最近 300 筆記錄，完整記錄寫入使用者的 LocalAppData。

## 建置與啟動

先安裝 .NET 10 SDK，再於專案根目錄執行：

```powershell
dotnet build NeoAirPlay.slnx
```

執行單元測試：

```powershell
dotnet test NeoAirPlay.slnx --filter "Category=Unit"
```

執行 WPF UI E2E 測試需使用可互動的 Windows 桌面工作階段；測試會啟動 App 並操作頁面導覽，不會連接 AirPlay 裝置或啟動接收器：

```powershell
dotnet test tests/NeoAirPlay.App.Test/NeoAirPlay.App.Test.csproj --filter "Category=E2E"
```

`NeoAirPlay.Framework` 放置模型與服務，WPF 介面、ViewModel 和命令留在 `NeoAirPlay.App`。測試專案分別為 `NeoAirPlay.Framework.Test` 與 `NeoAirPlay.App.Test`；`.neo_harness/verify.ps1` 會建置方案並執行單元測試。

第一次建置 UxPlay 與 GStreamer 執行環境時執行：

```powershell
.\scripts\build-engine.ps1
```

腳本會在 `.build\msys64` 安裝建置用的 MSYS2，下載 FDH2/UxPlay v1.73.7 的官方原始碼封存檔，編譯 `engine\uxplay.exe`，並將執行期相依檔整理到 `engine\runtime`。這不是 Git clone，也不會下載 `leapbtw/uxplay-windows`。

首次建置前，Bonjour 執行檔會從 Windows 目前登錄的 Bonjour Service 路徑複製到專案：

```powershell
.\scripts\Stage-BonjourRuntime.ps1
```

該腳本只會複製 `mDNSResponder.exe` 與 `dnssd.dll`，並比對產品名稱、檔案版本和 SHA-256；不會複製 `uxplay-windows.exe` 或其他影音檔案。這台電腦目前的 Bonjour 檔案版本資訊為 3.1.0.2。

將 Windows Bonjour Service 切換到專案路徑需要系統管理員權限：

```powershell
.\scripts\Install-BonjourService.ps1
```

腳本先備份服務原始 `ImagePath` 至 `%ProgramData%\NeoAirPlay\BonjourServiceBackup.json`，再停止服務、切換至 `engine\bonjour\mDNSResponder.exe` 並重新啟動。要還原時，以系統管理員身分執行：

```powershell
.\scripts\Restore-BonjourService.ps1
```

最後建置可發佈版本：

```powershell
dotnet publish src/NeoAirPlay.App/NeoAirPlay.App.csproj -c Release -o dist/NeoAirPlay-Release
```

發佈目錄中的 `engine` 會一併包含 UxPlay、GStreamer 與 Bonjour 執行檔；安裝完成後不需要從 `Downloads\uxplay-windows` 載入任何檔案。若搬移整個專案目錄，需重新執行 Bonjour 安裝腳本，讓 Windows 服務指向新位置。

## 使用者資料

- 設定：`%AppData%\NeoAirPlay\settings.json`
- 記錄：`%LocalAppData%\NeoAirPlay\Logs\`
- GStreamer 登錄快取：`%LocalAppData%\NeoAirPlay\gstreamer-registry.bin`
- 自動啟動：目前使用者的 Windows `Run` 登錄機碼

## 網路與限制

電腦和 Apple 裝置需能在同一個區域網路互相連線。Windows 防火牆可能需要允許專案中的 `engine\uxplay.exe` 通過私人網路。Bonjour Service 使用 mDNS/DNS-SD；若網路隔離裝置間的多播探索，AirPlay 裝置清單可能看不到這台電腦。

此版本使用穩定版 UxPlay v1.73.7，Bonjour Service 由本專案目錄執行。AirPlay 多房間音訊不在此功能範圍內。UxPlay 專案說明 Windows 版本以 MSYS2/MinGW 建置，並將 v1.74 標為實驗版；本專案因此鎖定 v1.73.7。

## 第三方元件與授權

- UxPlay：GPL-3.0；建置後的授權文字位於 `engine\licenses\UxPlay-GPL-3.0.txt`。
- GStreamer 與 MSYS2 執行期套件：授權文字由建置腳本複製到 `engine\runtime\licenses`。
- Apple DNS-SD 標頭：授權文字保留在下載的 `dns_sd.h` 內。
- 本機 Bonjour Service 元件：目前由這台電腦已登錄的 Bonjour 安裝複製，只供本機專案使用。Apple 的 Windows Bonjour 條款限制將 Bonjour 軟體對外再散布；不要把專案中的 Bonjour 執行檔放入公開下載包。授權依據請參閱 [Apple Bonjour for Windows 條款](https://developer.apple.com/licensing-trademarks/files/bonjour4win.pdf)。

參考專案：[FDH2/UxPlay](https://github.com/FDH2/UxPlay)。本專案使用其公開的 UxPlay 原始碼作為外部執行引擎，UI、Windows 後端、設定與服務管理皆在此專案自行撰寫。
