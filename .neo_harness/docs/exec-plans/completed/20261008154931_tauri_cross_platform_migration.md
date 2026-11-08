# Tauri 跨平台遷移計畫

## 目的

將 Neo AirPlay 從 Windows 專用 WPF/.NET 桌面程式遷移為 Tauri 2、Rust、Vite、Vue 3、TypeScript 與 Bun 專案，保留 AirPlay 與 Android 投影操作，提供 Windows、macOS、Linux 的建置流程、單元測試、桌面端對端測試、安裝套件設定及專案文件。

## 初始狀態

2026-10-08 盤點時，工作樹仍是 Windows 11 x64 WPF/XAML 程式；前端、設定、診斷、登入啟動、UxPlay、scrcpy/ADB、Bonjour 管理、測試及發佈腳本都依賴 .NET 或 Windows 執行環境。當時沒有 Tauri、Rust 應用程式、Vue 前端或跨平台發佈工作流程。

## 實作範圍

- 新桌面應用程式位於 `desktop/`，使用 Tauri 2、Rust、Vue 3、TypeScript、Vite 和 Bun。
- Rust 後端管理設定、每日記錄、引擎程序、ADB 裝置清單、登入啟動及系統匣。UxPlay 與 scrcpy 使用不同程序控制器，可同時運作。
- Rust 啟動時使用固定 revision 的 `fix-path-env` 恢復 GUI 程序的 `PATH`；macOS 與 Linux 透過登入 shell 讀取環境。
- Vue 介面提供總覽、設定及診斷頁，透過 Tauri commands 與 Rust 溝通。
- 前端與 Rust 均有單元測試；WebdriverIO E2E 會啟動真實 Tauri 程式並驗證頁面導覽、設定儲存，以及兩個隔離假引擎程序的並行啟停。
- 單一執行個體保護會將第二次啟動導回既有主視窗；桌面 E2E 涵蓋此流程。
- GitHub Actions 設定原生 Windows、macOS、Linux runner，執行驗證、E2E、原生套件建置並上傳 14 天 Actions artifact。
- README、Harness 架構、安全、可靠性、驗證命令及工作入口已更新；舊 WPF/.NET 專案、測試、腳本及舊 workflow 已移除。
- 預設安裝套件不附帶 UxPlay、GStreamer、ADB、scrcpy 或 Bonjour/Avahi 執行檔；macOS 使用 ad-hoc 簽章，未設定 notarization。

## 完成項目

- [x] 盤點舊使用者功能與安全、可靠性邊界。
- [x] 建立 Tauri、Rust、Vue、TypeScript、Vite、Bun 專案骨架與驗證入口。
- [x] 移植引擎控制、設定、記錄、Android 裝置刷新、登入啟動與系統匣流程。
- [x] 保留單一執行個體行為，並以桌面 E2E 驗證第二次啟動。
- [x] 建立 Dashboard、Settings、Diagnostics、前端與 Rust 單元測試，以及驗證雙引擎並行啟停的桌面 E2E。
- [x] 設定跨平台 CI、原生 bundle target 與安裝套件上傳。
- [x] 重寫專案文件並移除已替代的 Windows 專用專案內容。
- [x] 執行 macOS 本機驗證與桌面 E2E，建置並檢查 macOS 安裝套件。
- [x] 在 Ubuntu 24.04 x86_64 模擬容器執行驗證、桌面 E2E，建置並檢查 Linux 安裝套件。
- [x] 在 macOS arm64 使用 `cargo-xwin` 交叉建置 Windows x64 執行檔與 NSIS 安裝檔。
- [x] 使用 Windows MSVC target 編譯全部 Rust 單元測試目標；測試程式尚未在 Windows 執行。

## 發現與決策

- 2026-10-08：以 Bun 作為前端套件管理與 script 入口，CI 使用 frozen lockfile。
- 2026-10-08：不移植 Bonjour Windows 服務安裝或修復流程；外部引擎及服務探索相依由使用者依平台安裝。
- 2026-10-08：GUI 啟動程序的 `PATH` 可能與互動式終端不同，因此在 Rust `main` 最早階段呼叫 `fix-path-env`；Git 依賴以完整 revision 固定。
- 2026-10-08：Tauri bundle 設為該平台可用的所有格式，CI 於平台原生 runner 建置，不做跨作業系統交叉打包。
- 2026-10-08：使用自訂 Neo AirPlay 圖示；macOS 套件 ad-hoc 簽章可供本機檢查，但不構成公開散布所需的 Developer ID 簽章或 notarization。

## 驗證結果

- `bun install --frozen-lockfile --silent`：通過。
- `pwsh -NoProfile -NonInteractive -File .neo_harness/verify.ps1`：在 macOS arm64 通過；前端 7 項測試、Rust 10 項測試、Vue/TypeScript 型別檢查、格式檢查、Clippy 及 Vite production build 均成功。
- `bun run --silent verify`：在 Ubuntu 24.04 x86_64 模擬容器通過；前端 7 項測試、Rust 10 項測試、型別檢查、格式檢查、Clippy 及 Vite production build 均成功。
- `bun run --silent verify`：在 Ubuntu 24.04 aarch64 容器通過；前端 7 項測試、Rust 10 項測試、型別檢查、格式檢查、Clippy 及 Vite production build 均成功。容器以 Apple Silicon 原生架構執行，未使用 x86 模擬。
- `pwsh -NoProfile -NonInteractive -File .neo_harness/verify.ps1`：加入單一執行個體插件後在 macOS arm64 通過；前端 7 項測試、Rust 10 項測試、型別檢查、格式檢查、Clippy 及 Vite production build 均成功。
- `bun run --silent test:e2e:ci`：在 macOS arm64 通過；3 項桌面 E2E 成功，包含假 ADB/UxPlay/scrcpy 的程序啟停與並行檢查。
- `bun run --silent test:e2e:ci`：加入單一執行個體案例後在 macOS arm64 通過；4 項桌面 E2E 成功，包含重複啟動時新程序正常結束。
- `xvfb-run --auto-servernum bun run --silent test:e2e:ci`：在 Ubuntu 24.04 x86_64 模擬容器通過；3 項桌面 E2E 成功。容器把 Cargo target 掛載至 `/cargo-target`，因此在容器內建立 `src-tauri/target` symlink，讓 WDIO 找到測試執行檔。
- `bun run --silent bundle`：在 macOS arm64 產生 `.app` 與 `.dmg`；應用程式簽章檢查、DMG checksum 檢查、品牌圖示比對及 engine resource 內容檢查均通過。
- 加入單一執行個體插件後重新執行 `bun run --silent bundle`：macOS arm64 `.app` 與 `.dmg` 建置通過；`codesign --verify --deep --strict` 與 `hdiutil verify` 通過，並以 smoke check 啟動正式 `.app`。
- macOS DMG 位於 `desktop/src-tauri/target/release/bundle/dmg/Neo AirPlay_0.1.0_aarch64.dmg`；SHA-256 為 `a95773b32a2b0a884cab8bc9fca4fb6237aae3a4c20084c66745ed3e024b9141`。
- `bun run --silent bundle`：在 Ubuntu 24.04 x86_64 模擬容器產生 amd64 DEB、x86_64 RPM 與 AppImage；套件中包含應用程式執行檔、品牌圖示及 `engine/README.md`。
- Linux 安裝套件位於 `/tmp/neo-airplay-linux-target/release/bundle/`；AppImage 格式與內容檢查、DEB/RPM 架構及封裝清單檢查均通過。
- `bun run --silent smoke:bundle`：在 macOS arm64 啟動 `.app` 內的正式執行檔 8 秒後停止，通過。
- `CARGO_TARGET_DIR=/cargo-target xvfb-run --auto-servernum bun run --silent smoke:bundle`：在 Ubuntu 24.04 x86_64 模擬容器啟動 AppImage 內的應用程式執行檔 8 秒後停止，通過。
- x86_64 模擬容器中的一次 smoke 執行遇到 WebKit 子程序以 SIGTRAP 結束；之後以相同命令連續 4 次通過。根因尚未確認，仍需原生 Linux x86_64 runner 驗證。
- `CARGO_TARGET_DIR=/cargo-target dbus-run-session -- xvfb-run --auto-servernum bun run --silent test:e2e:ci`：加入單一執行個體插件後，在 Ubuntu 24.04 aarch64 容器通過；4 項桌面 E2E 成功。未建立 D-Bus session 時，重複啟動案例無法通過。
- `CARGO_TARGET_DIR=/tmp/neo-airplay-windows-target cargo xwin test --locked --all-features --no-run --manifest-path desktop/src-tauri/Cargo.toml --target x86_64-pc-windows-msvc -q`：加入單一執行個體插件後，Windows MSVC 測試目標交叉編譯成功；此命令只編譯，沒有執行測試。
- `bun run tauri build --ci --bundles deb`：在 Ubuntu 24.04 aarch64 容器建置 ARM64 DEB；套件架構標記為 `arm64`，內含執行檔確認為 ELF AArch64。使用 `apt-get install` 安裝套件成功，再由 `smoke:bundle` 啟動已安裝執行檔並正常停止。
- ARM64 DEB 位於 `/tmp/Neo AirPlay_0.1.0_arm64.deb`；加入單一執行個體插件後重建、重新安裝並以 D-Bus session 和 Xvfb smoke check 啟動。最新 SHA-256 為 `246b9f2db206c6281d310e193f698bcfcdd7f77f2a0f291e235ac3e601c5a794`。
- Linux 套件另做啟動檢查：DEB 以 `dpkg -i` 安裝；RPM 在 Ubuntu 容器以 `rpm -i --nodeps` 安裝，因 RPM 資料庫無法辨識由 apt 安裝的系統相依套件；AppImage 從其 AppDir 直接啟動應用程式執行檔。三者都在 Xvfb 下保持執行 8 秒。容器沒有 DRI3 裝置或 session bus，因此記錄到圖形環境警告。
- `bun run tauri build --ci --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis`：在 macOS arm64 成功交叉建置 Windows x64 `neo-airplay.exe` 與 NSIS 安裝檔；PE 格式、x64 架構與可執行檔圖示資源檢查通過。
- Windows 交叉建置產物位於 `/tmp/neo-airplay-windows-target/x86_64-pc-windows-msvc/release/`；Tauri 將此路徑標示為實驗性，並略過 Windows 簽章。
- `cargo xwin test --locked --all-features --no-run --manifest-path desktop/src-tauri/Cargo.toml --target x86_64-pc-windows-msvc`：Windows MSVC target 的 Rust 測試執行檔編譯成功；此命令只編譯，沒有執行測試。
- 加入單一執行個體插件後重新交叉建置 Windows x64 NSIS；PE/NSIS 格式檢查通過，SHA-256 為 `c5ae8d0c60e977bcdd6baadef015299b709f6252734672eb19f57df5b57b641a`。安裝與啟動仍需 Windows 原生驗證。
- Ruby YAML parser：GitHub Actions workflow 語法解析通過。
- 相對 Markdown 連結：檢查 12 個 Markdown 檔案中的 12 個相對連結，沒有失效連結。
- `git diff --check`：通過。

## 尚待外部驗證

- Linux x86_64 E2E 與 DEB/RPM/AppImage 在 Apple Silicon 上以 Ubuntu 24.04 x86_64 容器模擬建置；其中 smoke 曾有一次 WebKit 子程序 SIGTRAP，後續 4 次通過，根因未確認。Linux aarch64 已在 Apple Silicon 原生架構容器執行驗證、E2E、ARM64 DEB 安裝及程式啟動，但兩者都尚未在獨立實體 Linux runner 驗證。Windows NSIS 已交叉建置，Rust 測試目標也已編譯，但安裝檔、Windows E2E 與單元測試尚未在 Windows 執行；Windows MSI 仍需由 Windows runner 建置。尚未執行 GitHub Actions 原生 runner 工作流程與 artifact。
- 本工作階段的 `gh auth status` 顯示未登入 GitHub，因此未觸發遠端 Actions workflow；目前也沒有原生 Windows runner。
- 自動化沒有實際連接 Apple 或 Android 裝置；AirPlay/mDNS 探索、影音與音訊、GPU renderer、ADB 配對及各作業系統登入啟動行為仍需實機驗收。
- 預設資源資料夾只含說明檔，不代表 UxPlay、GStreamer、ADB、scrcpy 或 DNS-SD 依賴已隨套件安裝。
- macOS 產物使用 ad-hoc 簽章且未 notarize；正式公開發佈仍需配置 Apple Developer ID、notarization 及 Windows 簽章憑證。

## 復原方式

舊版檔案仍可從遷移前的 Git revision 還原。若需回復，應從該 revision 還原舊專案、測試、腳本及 workflow，並一併撤銷本計畫新增的 Tauri 專案與文件更新。

## 結果

跨平台遷移程式碼、文件、測試及 CI 設定已完成；macOS arm64、Linux aarch64 原生架構容器，以及 Linux x86_64 模擬環境的驗證與 E2E 通過；macOS、Linux x86_64 與 Linux arm64 安裝套件均已產生並完成啟動檢查。Windows x64 執行檔與 NSIS 安裝檔交叉建置成功，Windows MSVC Rust 測試目標編譯通過。Windows 原生安裝與 E2E、單元測試執行、MSI、實體 Linux runner 及實際硬體串流仍待其對應環境確認。
