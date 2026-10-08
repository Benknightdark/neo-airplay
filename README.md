# Neo AirPlay

Neo AirPlay 是 Windows、macOS 與 Linux 桌面接收器，使用 Tauri 2、Rust、Vue 3、TypeScript 與 Bun。AirPlay 由 UxPlay 處理，Android 由 ADB 與 scrcpy 處理；兩種投影各自執行，可同時使用。

## 功能

- 啟動與停止 AirPlay 接收器，設定接收器名稱、影片 renderer 與全螢幕模式。
- 列出 ADB 已連線的 USB 或網路 Android 裝置，選取裝置後啟動或停止 scrcpy。
- AirPlay 與 Android 各自顯示狀態，診斷頁顯示最近 300 筆記錄；完整記錄寫入使用者記錄目錄。
- 支援登入後自動啟動。關閉主視窗會將程式留在系統匣；從系統匣選單可重新開啟視窗或結束程式。
- 引擎以獨立程序執行。應用程式定期更新程序狀態，結束時會嘗試停止兩個引擎。

## 執行需求

桌面程式首次啟動會檢查 UxPlay、GStreamer、ADB 與 scrcpy，缺少時自動執行安裝流程；開發與安裝套件建置命令也會先準備這些工具。安裝需要網路，系統套件可能要求管理員授權；macOS 會開啟 Terminal，Linux 使用終端 sudo 或桌面的 polkit 授權，Windows 使用 UAC。介面會顯示安裝狀態、記錄位置與重試按鈕。安裝套件的依賴準備會在安裝後第一次開啟 App 時執行。

| 平台 | 自動準備方式 |
| :--- | :--- |
| macOS | 準備 Homebrew、Xcode Command Line Tools、GStreamer、scrcpy 與 Android Platform Tools，從固定的官方來源建置 UxPlay 至 `~/.local/bin`。 |
| Linux | 透過 apt、dnf 或 pacman 安裝影音及編譯相依套件，準備 ADB、scrcpy、Avahi 並建置 UxPlay。需要可用的套件庫與系統服務管理環境。 |
| Windows | 準備 WinGet、MSYS2 UCRT64、GStreamer、scrcpy 與 ADB，再建置固定 commit 的 UxPlay。這條路徑使用官方 **1.74 實驗版**的內建 mDNS，以避開需人工取得的 Bonjour SDK；尚需 Windows 實機驗證。 |

安裝腳本位於 [desktop/src-tauri/resources/setup/](desktop/src-tauri/resources/setup/)。UxPlay 原始碼下載後會核對固定 SHA-256。套件不預先附帶第三方二進位檔；也可將合法可散布的完整引擎與相依檔案放入 `desktop/src-tauri/resources/engine/`。App 會檢查資源目錄、系統 `PATH`、使用者 `~/.local/bin` 與常見 MSYS2／WinGet 位置。

- [UxPlay 官方專案](https://github.com/FDH2/UxPlay)：依作業系統安裝 UxPlay、GStreamer 及 DNS-SD 服務探索相依項目。Windows、macOS 與 Linux 的建置方式不同。
- [scrcpy 官方專案](https://github.com/Genymobile/scrcpy)：安裝 scrcpy 與 Android Platform Tools 的 ADB。Android USB 投影需要在手機開啟 USB 偵錯並授權此電腦；無線裝置須先由使用者完成 Android 無線偵錯配對。
- 電腦與 Apple 裝置需在可互通的區域網路。多播探索受網路隔離或防火牆設定影響。

Tauri 的開發環境需求依平台而異，請依 [Tauri 官方先決條件](https://v2.tauri.app/start/prerequisites/) 安裝。Linux 建置需要 GTK、WebKitGTK 與 AppIndicator 開發套件；macOS 需要 Xcode Command Line Tools；Windows 需要 Microsoft C++ Build Tools 與 WebView2。

## 開發與驗證

第一次取得專案時，在根目錄準備開發工具及投影依賴。macOS／Linux：

```sh
bash setup.sh
```

Windows PowerShell：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\setup.ps1
```

這些入口會準備 Rust stable、Bun 1.4.2、Node.js 22.23.3、平台建置工具及前端套件。完成後開啟新終端，讓已安裝工具的環境設定生效；作業系統安裝程式要求授權或重新啟動時，需完成系統提示。已有開發工具時，可直接在專案根目錄執行：

```sh
cd desktop
bun install --frozen-lockfile
bun run tauri dev
```

執行單元測試、Rust 格式檢查、Clippy 與前端建置：

```powershell
pwsh .neo_harness/verify.ps1
```

實際啟動桌面程式，驗證頁面導覽、設定儲存，以及 AirPlay/Android 引擎程序的並行啟停：

```sh
cd desktop
bun run test:e2e:ci
```

E2E 需要可啟動桌面視窗的工作階段；Linux 無桌面工作階段時，可使用 `dbus-run-session -- xvfb-run --auto-servernum bun run --silent test:e2e:ci`。測試也會確認第二次啟動沿用既有執行個體；並行啟停流程使用隔離的假 ADB、UxPlay 與 scrcpy 程序，不會連接 Android 裝置或啟動使用者安裝的真實引擎。單元測試與桌面 E2E 不驗證區域網路探索、實際影音串流、音訊輸出或 GPU 加速。

## 建置安裝套件

在目標作業系統上執行：

```sh
cd desktop
bun run bundle
```

Tauri 會建置目前作業系統的安裝套件，輸出位於 `desktop/src-tauri/target/release/bundle/`。預設設定啟用該平台可用的所有格式，例如 Windows MSI／NSIS、macOS DMG／APP，以及 Linux DEB／RPM／AppImage。一般建置應使用目標作業系統的原生環境；macOS 也能交叉產生 Windows x64 NSIS 安裝檔。

CI、隔離 E2E 與 bundle smoke check 會設定 `NEO_AIRPLAY_SKIP_SETUP=1`，避免測試過程安裝系統軟體。其他無網路或僅編譯的環境也可明確設定此變數；此時需自行確保實際使用機器的工具可用。

打包後可在建置主機啟動正式應用程式，確認程式至少能正常存活 8 秒：

```sh
cd desktop
bun run --silent smoke:bundle
```

Linux 需要圖形與 D-Bus 工作階段；沒有桌面工作階段時可執行 `dbus-run-session -- xvfb-run --auto-servernum bun run --silent smoke:bundle`。這項檢查會啟動 Linux AppImage 封裝內或 DEB 安裝的應用程式執行檔、macOS `.app` 內的執行檔或 Windows release 執行檔，並使用暫存使用者資料目錄。它檢查正式應用程式啟動，不取代桌面 E2E、引擎串流或安裝後的人工驗收。

### 從 macOS 交叉建置 Windows NSIS

安裝 LLVM、NSIS 與 `cargo-xwin`，再新增 MSVC Rust target：

```sh
brew install llvm makensis
cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc
export PATH="$(brew --prefix llvm)/bin:$PATH"
```

在 `desktop/` 執行：

```sh
bun run tauri build --ci --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
```

Tauri 將此交叉建置方式標示為實驗性，產生的 NSIS 安裝檔仍需在 Windows 驗證安裝與執行；MSI 必須在 Windows 建置。此流程目前不簽署 Windows 執行檔或安裝檔。請參閱 [Tauri Windows 安裝套件文件](https://v2.tauri.app/distribute/windows-installer/)。

目前沒有設定 Windows 發行者憑證或 Apple Developer ID 憑證。macOS 使用 ad-hoc 簽章且未 notarize；從網路下載後，macOS 仍可能要求使用者在「隱私權與安全性」允許開啟。正式公開發佈前，需依 [Tauri 簽章與 notarization 指引](https://v2.tauri.app/distribute/sign/macos/) 配置憑證與 CI secrets。

GitHub Actions 在 `develop` 或 `main` push，以及以 `main` 為目標的 Pull Request 上，於 Windows、macOS、Linux 執行單元檢查和桌面 E2E，並產生該平台的安裝套件。打包後的正式應用程式也會通過啟動 smoke check。成功的工作流程會把套件上傳為保留 14 天的 Actions artifact；目前不會自動建立公開 GitHub Release。

## 使用者資料

設定 JSON 與每日 JSONL 記錄存放在 Tauri 為目前使用者解析出的應用程式設定及記錄目錄。診斷頁載入當日記錄的最新 300 筆。

## 第三方元件與授權

UxPlay、GStreamer、scrcpy、ADB 及其相依元件由準備腳本下載、建置或透過套件管理器安裝；預設安裝套件不附帶這些二進位檔。若建置時加入第三方執行檔，發佈者需保留各元件要求的授權文字與通知，並確認其散布條件。請參閱 [UxPlay](https://github.com/FDH2/UxPlay) 與 [scrcpy](https://github.com/Genymobile/scrcpy) 官方專案說明。

專案檢查的修正、驗證證據及尚待實機驗收的項目記錄於 [Review 記錄](.neo_harness/docs/review-and-setup.md)。
