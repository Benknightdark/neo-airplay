# Neo AirPlay

Windows 11 x64 的 iOS AirPlay 與 Android 投影接收器，使用 WPF/XAML 建立操作介面。AirPlay 與 Android 使用獨立程序，可同時接收；Android 透過官方 scrcpy Windows 發行包處理畫面、音訊與裝置連線。程式原始碼由本專案自行建立；不包含或修改 `uxplay-windows` wrapper。

## 目前功能

- 啟動與停止 AirPlay 接收器，狀態列會顯示缺少的引擎元件或錯誤。
- 接收畫面鏡像與音訊，也支援音訊播放。
- 使用 UxPlay 自己的播放視窗顯示鏡像畫面。
- Android 可選擇已授權的 USB 或已配對 ADB TCP/IP 裝置，使用 scrcpy 獨立視窗投影；Android 11 以上支援音訊轉送。
- AirPlay 與 Android 有各自的狀態和啟停控制，可以同時投影。
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

執行 WPF UI E2E 測試需使用可互動的 Windows 桌面工作階段；測試會啟動 App、操作頁面導覽並確認 Android 控制項，不會連接裝置或啟動接收器：

```powershell
dotnet test tests/NeoAirPlay.App.Test/NeoAirPlay.App.Test.csproj --filter "Category=E2E"
```

另有硬體 E2E，可在互動式桌面啟動真實 UxPlay 與 scrcpy。一項測試直接驗證兩個引擎並行；另一項啟動 WPF App，透過總覽刷新並選取 USB 手機、開始及停止投影，確認 scrcpy 視窗顯示且 Android 停止後 AirPlay 仍運行：

```powershell
dotnet test tests/NeoAirPlay.App.Test/NeoAirPlay.App.Test.csproj --filter "Category=HardwareE2E"
```

硬體 E2E 需要先建置引擎、安裝並啟動位於 `%ProgramFiles%\NeoAirPlay\Bonjour` 的 Bonjour Service、可互動的 Windows 桌面，以及已開啟 USB 偵錯並授權本電腦的 Android USB 裝置；執行前請先關閉其他 Neo AirPlay 接收器，避免 UxPlay 連接埠衝突。這項自動測試不會操作 iPhone/iPad，也不會量測實際音訊輸出；實機驗收時，請另外從 Apple 裝置選擇本接收器並確認畫面與音訊。服務層測試可使用 ADB 已連線的 USB 或已配對 TCP/IP 裝置；本次 UI 硬體測試只驗 USB。

`NeoAirPlay.Framework` 放置模型與服務，WPF 介面、ViewModel 和命令留在 `NeoAirPlay.App`。測試專案分別為 `NeoAirPlay.Framework.Test` 與 `NeoAirPlay.App.Test`；`.neo_harness/verify.ps1` 會建置方案並執行單元測試。

## GitHub Actions CI/CD 與下載

- `develop` 每次 push 會在 GitHub-hosted Windows runner 執行 `.neo_harness/verify.ps1`。成功後，自動化會建立或更新 `develop` → `main` PR，並啟用 auto-merge；PR 本身也必須通過 `verify` 檢查。
- PR 合併至 `main` 後，Release workflow 會對該次合併的原始碼重跑驗證、產生 Windows x64 可攜套件、建立 `vX.Y.Z` tag，並發布 GitHub Release。版本由目前 App 版本與既有 Release tags 中較新的版本升 patch；目前基準 `0.1.0` 的第一版為 `0.1.1`。Release 版本會寫回 `NeoAirPlay.App.csproj` 與 `engine-package.json`，再同步到 `develop`。主視窗標題會顯示 `Neo AirPlay (X.Y.Z)`。
- GitHub Release 會提供 `NeoAirPlay-X.Y.Z-win-x64.zip`（App、必要引擎及 README，不含 Bonjour）、`neo-airplay-engine-win-x64.zip`（供 App 首次啟動補齊引擎）和 SHA-256 sidecar／`SHA256SUMS.txt`。App ZIP 可解壓後直接執行 `NeoAirPlay.exe`。

啟用工作流程前，請在 GitHub repository 完成以下設定：

1. 從最新 `main` 建立 `develop` 分支。此工作樹目前的遠端尚未有 `develop`。
2. 建立 GitHub App，只授予此 repository 的 `Contents: Read and write`、`Pull requests: Read and write` 權限，安裝到此 repository。將 App 的 Client ID 設為 repository variable `RELEASE_APP_CLIENT_ID`，將私鑰 PEM 設為 repository secret `RELEASE_APP_PRIVATE_KEY`。
3. 在 repository 設定啟用 Allow auto-merge。設定 `main` 規則要求 PR 與 `verify` 成功，並允許安裝的 GitHub App 繞過直接推送限制，以便 Release workflow 寫入版本／引擎 metadata。`.github/` 與 Release helper 腳本設有 CODEOWNERS；分支規則須啟用「Require review from Code Owners」。
4. 註冊 Windows x64 self-hosted runner，labels 設為 `self-hosted`, `windows`, `x64`, `neo-airplay-release`。主機需安裝 .NET 10 SDK、PowerShell 7、Git、GitHub CLI (`gh`) 及可信任的 Bonjour；`scripts/build-engine.ps1` 會讀取本機 Bonjour 安裝以建置 UxPlay。runner 版本需支援 Node 24（2.327.1 以上），並保持在線。

Release runner 會執行已合併的專案建置與測試程式碼。只允許可信任的維護者更新 `develop`，限制 runner 的 repository 存取，並在隔離、不存放個人資料或其他憑證的 Windows 主機上執行。Release workflow 只由 `main` push 觸發，不在 PR 上執行。

## 投影裝置

### iPhone、iPad 或 Mac

啟動 AirPlay 接收器後，從 Apple 裝置的螢幕鏡像或音訊播放清單選擇接收器名稱。畫面與音訊會在 UxPlay 獨立視窗中輸出。

### Android

在手機啟用 USB 偵錯並授權這台電腦；使用 Wi-Fi 時，先在 Android 完成無線偵錯配對並確認 ADB 清單看得到裝置。回到總覽按「重新整理」，選擇狀態為「已連線」的裝置，再按「開始投影」。Android 視窗可由 scrcpy 自行關閉，或按「停止 Android 投影」。Android 11 以上支援音訊轉送。

第一次建置 UxPlay、GStreamer 與 Android scrcpy 執行環境時執行：

```powershell
.\scripts\build-engine.ps1
```

腳本會在 `.build\msys64` 安裝建置用的 MSYS2，下載 FDH2/UxPlay v1.73.7 的官方原始碼封存檔，編譯 `engine\uxplay.exe`，將執行期相依檔整理到 `engine\runtime`，並從 Genymobile 官方 GitHub staging scrcpy v4.1 Windows x64 與 ADB 至 `engine\android`。scrcpy 封存檔會先核對官方 SHA-256。這不是 Git clone，也不會下載 `leapbtw/uxplay-windows`。

建置 UxPlay 需要 x64 `dnssd.dll`；建置腳本會從可信任的 64 位元 Bonjour 目錄或 Windows `System32` 尋找，並核對 Bonjour 產品資訊、版本及 PE 架構。Bonjour Service 的 `mDNSResponder.exe` 可位於 `%ProgramFiles%` 或 `%ProgramFiles(x86)%`；服務執行檔與 x64 DNS-SD DLL 不必在同一目錄。建置腳本不會修改或啟動 Windows 服務。一般 App 啟動時會檢查 Bonjour Service，必要時透過 UAC 修復或啟動。

若只需重新 staging Android 執行環境，可單獨執行：

```powershell
.\scripts\Stage-ScrcpyRuntime.ps1
```

需要啟用 AirPlay 時，App 會在說明操作後要求 UAC，從可信任的 Windows Bonjour 安裝複製執行檔到受保護的 `%ProgramFiles%\NeoAirPlay\Bonjour`：

```powershell
.\scripts\Stage-BonjourRuntime.ps1
```

若要人工執行 staging，請以系統管理員身分啟動 PowerShell。腳本會從 `%ProgramFiles%` 或 `%ProgramFiles(x86)%` 的可信任 Bonjour 安裝尋找 `mDNSResponder.exe`，再從 64 位元 Bonjour 目錄或 Windows `System32` 獨立尋找相同版本的 x64 `dnssd.dll`；兩個元件都會核對 Bonjour 產品資訊及 PE 架構，並驗證複製前後的 SHA-256，最後放入 `%ProgramFiles%\NeoAirPlay\Bonjour`。不相容或缺少 x64 DNS-SD 元件時會停止並指出原因，不會改用 SysWOW64 的 x86 DLL。腳本不會複製 `uxplay-windows.exe` 或其他影音檔案，也不會下載 Bonjour。

Neo AirPlay 在 Debug 或 Release 第一次啟動時會檢查 UxPlay/GStreamer 與 scrcpy/ADB 必要檔案。缺少時，App 會在背景下載對應 App 版本的引擎套件、驗證 SHA-256 與套件檔案清單後，再安裝到 App 的 `engine` 目錄。下載失敗時主視窗仍可使用，App 會在下次啟動重試。

建立 Release 引擎套件前，先完成引擎建置，再執行：

```powershell
.\scripts\Pack-EngineRuntime.ps1
```

腳本會建立 `.build\neo-airplay-engine-win-x64.zip` 與 `.sha256` sidecar，並更新 App 內的引擎版本、Release 標籤和 SHA-256 設定。將 ZIP 附加至相同 `v<Version>` 標籤的 GitHub Release；目前 App 版本為 `0.1.0`，因此標籤是 `v0.1.0`。公開引擎套件不包含 Bonjour 元件。

一般啟動時，App 會確認 Bonjour Service 正在執行且指向受保護的 `%ProgramFiles%\NeoAirPlay\Bonjour`。若服務停止、路徑不符或服務項目不存在，App 會從 `%ProgramFiles%` 或 `%ProgramFiles(x86)%` 尋找 Bonjour `mDNSResponder.exe`，再從受信任的 64 位元 Bonjour 目錄或 Windows `System32` 尋找產品資訊與版本相符的 x64 `dnssd.dll`。因此既有 x86 Service 可搭配獨立安裝的 x64 DNS-SD DLL。兩個元件通過架構、產品、版本與來源路徑檢查後，App 會簡短說明並顯示 UAC；核准後將元件 staging 至受保護目錄，再啟動或建立服務。若找到服務執行檔但沒有相符的 x64 DNS-SD 元件，App 會明確指出缺少元件並提供 Apple 官方安裝資訊。既有服務的原始路徑會備份，服務項目原先不存在時也會記錄此狀態，供還原腳本移除新建服務。若舊備份無效，App 只會在目前服務路徑可信任或服務目前不存在時依現況更新備份；目前服務路徑不可信時會停止修改服務。資料夾僅授予系統管理員修改權限和必要的讀取權限，服務不會執行 App 可寫入目錄中的檔案；不會自動下載或散布 Bonjour 二進位檔。取消 UAC 或服務在 Windows 中被停用時，AirPlay 暫不可用，Android 投影仍可使用。

將 Windows Bonjour Service 切換到 Neo AirPlay 的受保護共用路徑需要系統管理員權限：

```powershell
.\scripts\Install-BonjourService.ps1
```

腳本先備份原服務狀態至 `%ProgramData%\NeoAirPlay\BonjourServiceBackup.json`，再將既有服務切換至 `%ProgramFiles%\NeoAirPlay\Bonjour\mDNSResponder.exe` 並重新啟動；若服務原本不存在，會從 staging 執行檔建立並啟動。要還原時，以系統管理員身分執行：

```powershell
.\scripts\Restore-BonjourService.ps1
```

若備份記錄服務原先不存在，還原腳本只會在服務路徑仍指向 Neo AirPlay 共用執行檔時移除該服務；受保護目錄中的 Bonjour 檔案會保留，讓 App 下次啟動可重試。服務備份存於 `%ProgramData%\NeoAirPlay`，一般使用者只有讀取權限。

最後建置可發佈版本：

```powershell
dotnet publish src/NeoAirPlay.App/NeoAirPlay.App.csproj -c Release -o dist/NeoAirPlay-Release
```

發佈目錄會包含建置時已有的 UxPlay、GStreamer 與 scrcpy/ADB 引擎檔案；Bonjour 執行檔不會進入 Debug 或 Release App 輸出。若發佈時沒有引擎檔案，首次啟動會從相同版本的 GitHub Release 下載上述引擎套件。Bonjour Service 使用固定的系統共用路徑，因此搬移 App 目錄不影響服務。

## 使用者資料

- 設定：`%AppData%\NeoAirPlay\settings.json`
- 記錄：`%LocalAppData%\NeoAirPlay\Logs\`
- GStreamer 登錄快取：`%LocalAppData%\NeoAirPlay\gstreamer-registry.bin`
- 自動啟動：目前使用者的 Windows `Run` 登錄機碼

## 網路與限制

電腦和 Apple 裝置需能在同一個區域網路互相連線。Windows 防火牆可能需要允許專案中的 `engine\uxplay.exe` 通過私人網路。Bonjour Service 使用 mDNS/DNS-SD；若網路隔離裝置間的多播探索，AirPlay 裝置清單可能看不到這台電腦。Android USB 投影需在手機開啟 USB 偵錯並接受本電腦的授權提示；無線投影需先在 Android 完成無線偵錯配對，並讓 ADB 裝置清單顯示該裝置。本 App 不會自動配對無線裝置。Android 11 以下可以投影畫面，但 scrcpy 音訊轉送需要 Android 11 以上。

此版本使用穩定版 UxPlay v1.73.7，Bonjour Service 由受保護的 Neo AirPlay 系統共用目錄執行。AirPlay 多房間音訊不在此功能範圍內。UxPlay 專案說明 Windows 版本以 MSYS2/MinGW 建置，並將 v1.74 標為實驗版；本專案因此鎖定 v1.73.7。

## 第三方元件與授權

- UxPlay：GPL-3.0；建置後的授權文字位於 `engine\licenses\UxPlay-GPL-3.0.txt`。
- scrcpy：Apache-2.0；官方發行包隨附的 `LICENSE.txt` 會保留在 `engine\android`。
- GStreamer 與 MSYS2 執行期套件：授權文字由建置腳本複製到 `engine\runtime\licenses`。
- Apple DNS-SD 標頭：授權文字保留在下載的 `dns_sd.h` 內。
- 本機 Bonjour Service 元件：目前由這台電腦已登錄的 Bonjour 安裝複製，只供本機專案使用。Apple 的 Windows Bonjour 條款限制將 Bonjour 軟體對外再散布；不要把專案中的 Bonjour 執行檔放入公開下載包。授權依據請參閱 [Apple Bonjour for Windows 條款](https://developer.apple.com/licensing-trademarks/files/bonjour4win.pdf)。

參考專案：[FDH2/UxPlay](https://github.com/FDH2/UxPlay)。本專案使用其公開的 UxPlay 原始碼作為外部執行引擎，UI、Windows 後端、設定與服務管理皆在此專案自行撰寫。
