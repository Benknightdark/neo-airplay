# Neo AirPlay

Neo AirPlay 是支援 Windows、macOS 與 Linux 的 AirPlay 和 Android 桌面接收器，使用 Tauri 2、Rust、Vue 3、TypeScript 與 Bun。AirPlay 由 UxPlay 處理，Android 由 ADB 與 scrcpy 處理，兩種投影可同時執行。

## 功能

- 啟動或停止 AirPlay 接收器，設定接收器名稱、影片 renderer 與全螢幕模式。
- 列出 ADB 已連線的 USB 或網路 Android 裝置，並啟動或停止 scrcpy。
- 分別顯示兩種引擎的狀態；診斷頁顯示最近 300 筆記錄。
- 支援登入後自動啟動。關閉主視窗會將程式留在系統匣，可從選單重開視窗或結束程式。
- 引擎以獨立程序執行；程式會定期更新狀態，結束時嘗試停止兩個引擎。

## 執行需求與依賴準備

App 正常啟動時會在背景檢查 UxPlay、GStreamer、ADB、scrcpy、scrcpy-server，以及 Linux 的 Avahi 服務；缺少依賴或服務未啟動時會自動準備，失敗項目會在下次啟動時重試。開發與打包入口也會先檢查依賴，僅安裝缺少的項目。準備流程需要網路，系統套件安裝可能要求授權：macOS 會開啟 Terminal，Linux 使用 sudo 或 polkit，Windows 使用 UAC。App 會顯示準備狀態、記錄位置與重試按鈕。

| 平台 | 自動準備方式 |
| :--- | :--- |
| macOS | 準備 Homebrew、Xcode Command Line Tools 與 GStreamer；下載經 SHA-256 核對的 scrcpy/ADB 官方套件，並從固定來源建置 UxPlay。 |
| Linux | 透過 apt、dnf 或 pacman 安裝影音與建置依賴。x86_64 使用經 SHA-256 核對的 scrcpy/ADB 官方套件並建置 UxPlay；其他架構使用發行版套件並複製執行工具。需要可用的套件庫與 Avahi 服務。 |
| Windows | 準備 WinGet、MSYS2 UCRT64 與 GStreamer；下載經 SHA-256 核對的 scrcpy/ADB 官方套件，並建置固定 commit 的 UxPlay。使用 UxPlay 官方 1.74 實驗版的內建 mDNS，免用需人工取得的 Bonjour SDK；仍需 Windows 實機驗證。 |

安裝腳本位於 [desktop/scripts/](desktop/scripts/)，UxPlay 原始碼下載後會核對固定 SHA-256。開發環境的執行檔與 GStreamer plugins 放在 `desktop/engine/`，Windows 也會收集相依 DLL；只有 `.GITKEEP` 納入版控。Tauri 從該目錄打包工具。正式版會先將套件內的工具複製到使用者資料目錄的 `engine/`，再從該處檢查及執行，避免寫入唯讀的應用程式套件。App 只從這些 `engine/` 目錄尋找引擎，不使用系統 `PATH`、`~/.local/bin` 或 MSYS2／WinGet 目錄中的工具。

- [UxPlay](https://github.com/FDH2/UxPlay) 需要 GStreamer 與 DNS-SD 服務探索相依項目；各平台安裝方式不同。
- [scrcpy](https://github.com/Genymobile/scrcpy) 使用 Android Platform Tools 的 ADB。USB 投影需在手機開啟 USB 偵錯並授權此電腦；無線投影需先完成 Android 無線偵錯配對。
- 電腦與 Apple 裝置需位於可互通的區域網路。網路隔離或防火牆可能阻擋多播探索。

Tauri 建置需求依平台而異，請參閱[官方先決條件](https://v2.tauri.app/start/prerequisites/)：Linux 需要 GTK、WebKitGTK 與 AppIndicator 開發套件；macOS 需要 Xcode Command Line Tools；Windows 需要 Microsoft C++ Build Tools 與 WebView2。

## 開發與驗證

首次取得專案時，使用啟動入口準備環境並選擇 Debug、Release 或打包。入口會準備 Rust stable、Bun 1.4.2、Node.js 22.23.3、平台建置工具、投影依賴與前端套件；已齊全的項目會略過。獨立準備環境可執行 `bash desktop/scripts/setup.sh`，Windows 則執行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\desktop\scripts\setup.ps1`；完成後開啟新終端載入 PATH 設定。

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

已準備好的環境也可在 `desktop/` 執行：

```sh
bun run tauri dev
bun run release
bun run bundle
```

執行單元測試、Rust 格式檢查、Clippy 與前端建置：

```powershell
pwsh .neo_harness/verify.ps1
```

桌面 E2E 會驗證頁面導覽、設定儲存、第二次啟動沿用既有執行個體，以及兩個假引擎程序並行啟停：

```sh
cd desktop && bun run test:e2e:ci
```

E2E 需要桌面工作階段。Linux 沒有桌面工作階段時，可使用 `cd desktop && dbus-run-session -- xvfb-run --auto-servernum bun run --silent test:e2e:ci`。測試使用假 ADB、UxPlay 與 scrcpy，不連接 Android 裝置或啟動真實引擎；單元測試與 E2E 不驗證區域網路探索、影音串流、音訊輸出或 GPU 加速。

## 建置與發佈

在目標作業系統建置安裝套件：

```sh
cd desktop && bun run bundle
```

輸出位於 `desktop/src-tauri/target/release/bundle/`，格式依平台提供，例如 Windows MSI／NSIS、macOS DMG／APP、Linux DEB／RPM／AppImage。macOS 可交叉建置 Windows x64 NSIS，但 Tauri 將此方式列為實驗性，安裝與執行仍須在 Windows 驗證；MSI 必須在 Windows 建置。

CI、隔離 E2E 與 bundle smoke check 會設定 `NEO_AIRPLAY_SKIP_SETUP=1`，避免驗證時安裝系統軟體。無網路或僅編譯的環境也可設定此變數，但須自行確保執行所需工具已備妥。

打包後可啟動正式應用程式至少 8 秒，確認程式能正常啟動：

```sh
cd desktop && bun run --silent smoke:bundle
```

Linux 需要圖形與 D-Bus 工作階段；沒有桌面工作階段時，可執行 `cd desktop && dbus-run-session -- xvfb-run --auto-servernum bun run --silent smoke:bundle`。此檢查使用暫存使用者資料目錄，只確認正式執行檔能啟動，不取代 E2E、串流或安裝後驗收。

macOS 交叉建置 Windows NSIS 需要 LLVM、NSIS、`cargo-xwin` 與 MSVC Rust target：

```sh
brew install llvm makensis
cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc
export PATH="$(brew --prefix llvm)/bin:$PATH"
cd desktop
bun run tauri build --ci --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
```

目前沒有 Windows 發行者憑證或 Apple Developer ID。macOS 使用 ad-hoc 簽章且未 notarize，下載後可能需要在「隱私權與安全性」允許開啟；正式發佈前需依 [Tauri 簽章與 notarization 指引](https://v2.tauri.app/distribute/sign/macos/) 設定憑證與 CI secrets。交叉建置的 NSIS 也不會簽署 Windows 執行檔或安裝檔；Tauri 的限制見[Windows 安裝套件文件](https://v2.tauri.app/distribute/windows-installer/)。

GitHub Actions 會在 `develop`、`main` push 及以 `main` 為目標的 Pull Request 上，於 Windows、macOS、Linux 執行驗證、桌面 E2E、安裝套件建置與正式程式啟動檢查。成功的 workflow 會上傳保留 14 天的 Actions artifact，不會自動建立公開 GitHub Release。

## 使用者資料與第三方授權

設定 JSON 與每日 JSONL 記錄位於 Tauri 為目前使用者解析出的設定及記錄目錄；診斷頁顯示當日最新 300 筆。

UxPlay、GStreamer、scrcpy、ADB 及其相依元件由準備腳本下載、建置或透過套件管理器安裝；預設安裝套件不附帶這些二進位檔。若發佈時加入第三方執行檔，需保留各元件要求的授權文字與通知，並確認散布條件；詳情見 [UxPlay](https://github.com/FDH2/UxPlay) 與 [scrcpy](https://github.com/Genymobile/scrcpy) 官方說明。

專案檢查的修正、驗證證據及尚待實機驗收的項目記錄於 [Review 記錄](.neo_harness/docs/review-and-setup.md)。
