# Neo AirPlay

Neo AirPlay 是 Windows、macOS 與 Linux 桌面接收器，使用 Tauri 2、Rust、Vue 3、TypeScript 與 Bun。AirPlay 由 UxPlay 處理，Android 由 ADB 與 scrcpy 處理；兩種投影各自執行，可同時使用。

## 功能

- 啟動與停止 AirPlay 接收器，設定接收器名稱、影片 renderer 與全螢幕模式。
- 列出 ADB 已連線的 USB 或網路 Android 裝置，選取裝置後啟動或停止 scrcpy。
- AirPlay 與 Android 各自顯示狀態，診斷頁顯示最近 300 筆記錄；完整記錄寫入使用者記錄目錄。
- 支援登入後自動啟動。關閉主視窗會將程式留在系統匣；從系統匣選單可重新開啟視窗或結束程式。
- 引擎以獨立程序執行。應用程式定期更新程序狀態，結束時會嘗試停止兩個引擎。

## 執行需求

桌面程式每次正常啟動（DEBUG 或 RELEASE）都會在背景檢查 UxPlay、GStreamer、ADB、scrcpy，以及 Linux 的 Avahi 服務；缺少或未啟動時會自動執行準備流程，先前失敗的流程也會在下次啟動時自動重試。開發／建置入口也會先執行 setup 檢查；依賴已齊全時跳過安裝，缺少時才補齊。安裝需要網路，系統套件可能要求管理員授權；macOS 會開啟 Terminal，Linux 使用終端 sudo 或桌面的 polkit 授權，Windows 使用 UAC。介面會顯示 App 執行期間的引擎準備狀態、記錄位置與重試按鈕。

| 平台 | 自動準備方式 |
| :--- | :--- |
| macOS | 準備 Homebrew、Xcode Command Line Tools 及 GStreamer，下載固定 SHA-256 的 scrcpy/ADB 官方套件，從固定來源建置 UxPlay；執行工具與 GStreamer plugins 集中在 `engine/`。 |
| Linux | 透過 apt、dnf 或 pacman 安裝影音及編譯相依套件；x86_64 使用固定 SHA-256 的 scrcpy/ADB 官方套件，建置 UxPlay，並將工具與 GStreamer plugins 集中在 `engine/`。其他架構使用發行版套件後複製執行工具。需要可用的套件庫與系統 Avahi 服務。 |
| Windows | 準備 WinGet、MSYS2 UCRT64 與 GStreamer，下載固定 SHA-256 的 scrcpy/ADB 官方套件，再建置固定 commit 的 UxPlay；執行檔、DLL 與 GStreamer plugins 放入 `engine/`。這條路徑使用官方 **1.74 實驗版**的內建 mDNS，以避開需人工取得的 Bonjour SDK；尚需 Windows 實機驗證。 |

安裝腳本位於 [desktop/scripts/](desktop/scripts/)。UxPlay 原始碼下載後會核對固定 SHA-256。開發版會把工具安裝至 `desktop/engine/`；此目錄只有 `.GITKEEP` 納入版控，其餘內容由 `.gitignore` 排除。打包時 Tauri 會從 `desktop/engine/` 收集工具。正式版啟動後會將套件內的工具複製至使用者資料目錄的 `engine/`，並在該目錄檢查、安裝及執行依賴，避免寫入唯讀的應用程式套件。App 不會從系統 `PATH`、`~/.local/bin` 或 MSYS2／WinGet 目錄啟動 UxPlay、ADB、scrcpy 或 GStreamer 檢查工具。

- [UxPlay 官方專案](https://github.com/FDH2/UxPlay)：依作業系統安裝 UxPlay、GStreamer 及 DNS-SD 服務探索相依項目。Windows、macOS 與 Linux 的建置方式不同。
- [scrcpy 官方專案](https://github.com/Genymobile/scrcpy)：安裝 scrcpy 與 Android Platform Tools 的 ADB。Android USB 投影需要在手機開啟 USB 偵錯並授權此電腦；無線裝置須先由使用者完成 Android 無線偵錯配對。
- 電腦與 Apple 裝置需在可互通的區域網路。多播探索受網路隔離或防火牆設定影響。

Tauri 的開發環境需求依平台而異，請依 [Tauri 官方先決條件](https://v2.tauri.app/start/prerequisites/) 安裝。Linux 建置需要 GTK、WebKitGTK 與 AppIndicator 開發套件；macOS 需要 Xcode Command Line Tools；Windows 需要 Microsoft C++ Build Tools 與 WebView2。

## 開發與驗證

第一次取得專案後，可直接從根目錄選擇 Debug、Release 或打包。啟動入口會先執行平台 setup，檢查開發工具與 `desktop/engine/` 的投影依賴，再執行所選操作。Engine 與系統工具已齊全時會略過下載和安裝；缺少項目才會安裝。前端套件仍會依鎖檔補齊。首次安裝可能需要網路、終端互動或系統授權。

macOS／Linux：

```sh
bash desktop/scripts/launch.sh dev
bash desktop/scripts/launch.sh release
bash desktop/scripts/launch.sh bundle
```

Windows PowerShell：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 dev
powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 release
powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\launch.ps1 bundle
```

啟動入口會呼叫 `setup.sh`／`setup.ps1`，準備 Rust stable、Bun 1.4.2、Node.js 22.23.3、平台建置工具、投影依賴及前端套件。獨立準備環境時，仍可在根目錄直接執行 `bash desktop/scripts/setup.sh`，或在 Windows 執行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\setup.ps1`；完成後開啟新終端載入 PATH 設定。已有開發工具時，也可在 `desktop/` 直接執行以下命令；Tauri CLI 會自動呼叫對應的 setup，Engine 已就緒時跳過其安裝：

```sh
cd desktop && bun run tauri dev
cd desktop && bun run release
cd desktop && bun run bundle
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

在目標作業系統上執行。打包前會自動呼叫 setup 檢查並準備依賴；第一次取得專案且 Bun 尚未安裝時，可使用前一節的 `launch.sh bundle`／`launch.ps1 bundle` 入口：

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
