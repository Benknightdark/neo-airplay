# Tauri 跨平台遷移執行計畫

## 目的

依據根目錄 `migrate.md`，將 Neo AirPlay 從 Windows WPF/.NET 遷移至 Tauri 2、Rust、Vue 3 與 TypeScript，支援 Windows x64、macOS arm64/x64 及 Linux x64。保留 WPF 專案直到新版本功能對齊並完成可用平台的驗收。

## 背景與行為

- 目前 `develop` 與 `feature/cross-platform` 位於 `9f77c9e`；工作樹上的 `migrate.md` 是本次遷移規格。
- 現行程式是 Windows WPF/.NET 10，服務與介面分層；AirPlay 和 Android 引擎由獨立程序執行。
- 目標行為、資料路徑、依賴版本、IPC、安全限制與驗證要求以 `migrate.md` 為準。
- 移除 .NET 是最後階段，必須等新程式碼對齊並完成必要驗收後才執行。

## 限制

- 保持 UxPlay 與 scrcpy 為獨立子程序；不讓 WebView 直接執行程序。
- 不散布 Bonjour 二進位檔；Windows Service 管理只保留既有受保護流程。
- 引擎下載必須保留 SHA-256、逐檔 manifest 與安全解壓檢查。
- 外部系統工具安裝和需要硬體、Windows 或 Linux 的驗收可能受本機環境限制；不得把未執行的驗證寫成通過。
- 不提交、不覆蓋使用者既有變更；`.build/` 內容維持不動。

## 工作順序與進度

| 階段 | 工作 | 狀態 |
| --- | --- | --- |
| 0 | 固定工具鏈，建立 Rust workspace、Tauri/Vite/Vue/TypeScript 骨架，檢查 TS 7 與 vue-tsc | 完成 |
| 1 | 實作 `neo-core` 模型、路徑、設定、記錄、平台及程序抽象 | 完成 |
| 2 | 實作 UxPlay、scrcpy/ADB、程序生命週期及狀態機 | 程式與單元測試完成；雙引擎硬體測試入口已新增但尚未執行 |
| 3 | 移植 Bonjour/mDNS 檢查與 Windows elevated helper | 程式完成；Windows CI、Bonjour 實機與 Linux Avahi 驗收待處理 |
| 4 | 移植安全的引擎套件下載、驗證與安裝 | 完成 |
| 5 | 實作 Tauri commands、events、tray、啟動參數、自動啟動與結束清理 | 程式完成；Windows 與 Linux 編譯、命令整合測試待處理 |
| 6 | 實作 Vue 頁面、store、診斷介面與前端測試 | 5 個 Vitest 與 macOS 原生 UI 導覽 E2E 通過；手機硬體情境待處理 |
| 7 | 更新引擎 staging、打包、版本同步及 macOS/Linux UxPlay 指引 | 程式完成；Windows/Linux runner 與實機投影驗收待處理 |
| 8 | 更新 Harness、CI/release workflows 與 UI E2E | 本機 Harness 與一般 UI E2E 通過；遠端三平台 CI 及硬體 E2E 待處理 |
| 9 | 完成文件、跨平台驗收後移除 .NET 專案 | 未開始；須先完成平台、UI 與硬體驗收 |

## 發現與決策

- 本機為 Apple Silicon macOS；起始版本為 Rust 1.96.0、Bun 1.3.5、Node 22.18.0，未安裝 PowerShell 7。規格要求 Rust 1.99.0、Bun 1.4.2、Node 22.12+ 與 PowerShell 7.4+。
- 已安裝 Rust 1.99.0 toolchain；`cargo fmt --all --check` 通過。
- TS 7 相容性探測失敗，`vue-tsc --noEmit` 因 `typescript/lib/tsc` 不在 TypeScript 7 exports 內而中止。依規格回退至 `typescript ~6.0`，解析為 6.0.3；回退後 `vue-tsc --noEmit` 通過。
- Bun 已升至 1.4.2；`bun install --frozen-lockfile`、typecheck（TypeScript 6.0.3）、前端測試（1 test）及 Vite build 均通過。
- 首次 Rust workspace test 在 Tauri context macro 階段因預設圖示 `src-tauri/icons/icon.png` 不存在而失敗；已加入 256×256 PNG 圖示並在 Tauri 設定明確列出。
- 修正 context macro 的圖示輸入後，`cargo test --workspace --quiet` 通過（骨架尚無 Rust tests）；`bun tauri dev` 編譯並啟動 macOS app 程序。尚未對實際視窗做人工視覺驗收。
- Homebrew 沒有 `powershell` cask，改用 stable formula 安裝 PowerShell 7.6.6。
- 依 AGENTS.md 執行現有 `.neo_harness/verify.ps1` 失敗：macOS 上舊 WPF 專案回報 `NETSDK1100`，要求 `EnableWindowsTargeting=true`。這是現有 Windows-only 驗證入口的跨平台限制；後續 Harness 階段將依遷移規格只在 Windows runner 執行 .NET 驗證。
- `src-tauri/` 與 `ui/` 目前只有空目錄或被忽略的建置目錄；`node_modules/` 與 `dist/` 是先前生成的忽略輸出。按規格重建時只清理這些輸出，不移除 `.build/`。
- `migrate.md` 範例有重複的 `@tauri-apps/cli`、Vite 與 `@vitejs/plugin-vue` 欄位；實作時採單一欄位，並以 lockfile 固定解析版本。
- 版本 job 必須先將版本及套件 metadata 傳給平行 build jobs；若只在 publish job 執行 `Set-AppVersion.ps1`，打包內容會使用錯誤版本。Release 實作時以此為約束。
- TypeScript 7 與 vue-tsc 的相容性需由實際 `vue-tsc --noEmit` 結果決定；失敗時依規格改用 TypeScript 6 並記錄。
- WebdriverIO Tauri service 與前端外掛鎖在 1.4.0，Rust 測試外掛鎖在 1.4.0；依官方設定使用 embedded WebDriver，E2E profile 會將設定和記錄導向獨立暫存資料夾。
- Tauri workspace 的實際 Cargo 輸出位於根目錄 `target/`，不是 `src-tauri/target/`。依成功的 E2E feature 編譯結果修正 UI E2E 與平台 bundle 腳本的輸出路徑。
- Release 流程由 Windows self-hosted runner 固定版本並產生 metadata artifact，再由 Windows、macOS arm64/x64 與 Linux 平行建置；發佈前會逐一核對四個引擎 ZIP 的 SHA-256，合併 target metadata。
- Windows 專用的完整 Tauri 交叉編譯仍需 Windows runner；macOS 本機完整編譯 `tauri-plugin-wdio` 與 embedded WebDriver 功能成功。
- `neo-core` 加入忽略式雙引擎硬體測試；WebdriverIO 加入需 `NEO_HARDWARE_E2E=1` 的 Android 投影情境。兩者只完成編譯，沒有在本機啟動 UxPlay、scrcpy 或連接 Android 裝置。
- Windows 啟動時會移除舊版 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\NeoAirPlay` 值；macOS 第一次啟動 AirPlay 前顯示內建接收器衝突提示；Linux Avahi 採 systemd、D-Bus 與 PID 檔檢查，ADB 權限錯誤附 udev 指引。
- WebdriverIO 以設定檔所在的 `e2e/` 作為 `rootDir`；修正 spec glob 後才能找到測試。頁面導覽會替換標題 DOM，測試改為每次輪詢重新查詢標題元素。
- macOS `Run-UiE2e.ps1` 已通過，導航測試 1 項成功；Android 硬體規格依 `NEO_HARDWARE_E2E` 未設定而略過。當前主機沒有辨識到 Android/iOS USB 裝置，且 `uxplay`、`adb` 尚未安裝，實際投影驗收仍待設備連接及工具準備。

## 具體步驟

1. 新增固定 toolchain、Cargo workspace、前端 package/config 與最小 Tauri 啟動程式。
2. 生成 lockfiles，安裝工具鏈並確認 Tauri 開發視窗及前後端檢查可執行。
3. 先實作不依賴平台權限的 `neo-core` 基礎，使用單元測試對照目前 C# 行為。
4. 依序完成程序/引擎、mDNS、引擎套件與 Tauri 整合。
5. 移植 UI 與測試，再更新 staging、CI 和 release 流程。
6. 更新文件；只在所有必要驗收有證據後移除 .NET。

## 驗證

- 階段 0：`bun install --frozen-lockfile`、`bun run typecheck`、`bun run test`、`bun run build`、`cargo fmt --all --check`、`cargo test --workspace --locked`、`bun tauri dev`。
- 遷移期間：執行 `.neo_harness/verify.ps1`；Windows 保留既有 Bonjour PowerShell 5.1 相容性測試及 .NET 單元測試，直到階段 9。
- 原生 UI E2E：`pwsh ./scripts/Run-UiE2e.ps1`；macOS 導覽與設定儲存測試通過。硬體情境使用 `NEO_HARDWARE_E2E=1 pwsh ./scripts/Run-UiE2e.ps1`，尚未執行。
- macOS 已執行一般 Tauri 與 `e2e` feature 的 `--debug --no-bundle` 建置；Windows/Linux 及 release bundler 的建置結果需由遠端 runner 提供。
- 嘗試以 macOS 交叉檢查 Linux x64 `neo-core`，但 `ring` 與 `zstd-sys` 需要未安裝的 `x86_64-linux-gnu-gcc`，在專案 crate 編譯前中止；Linux 專屬 Rust 程式仍需由 Ubuntu runner 驗證。
- 分平台 CI、UI E2E、硬體 E2E 及手動驗收逐項記錄；無法在本機執行時標明平台與原因。

## 復原方式

- 遷移功能分階段新增；如單一階段無法通過驗證，保留既有 WPF 路徑並修正或回退該階段變更。
- 不刪除 `.build/`、`engine/` 或尚未納入版本控制的使用者資料。
- .NET 專案只在最後階段、跨平台功能對齊且必要驗收完成後刪除。

## 結果

- 階段 0 完成：Rust 1.99.0 與 Bun 1.4.2 固定，TS 7 探測後回退至 TS 6.0.3；Vite/Vue/TypeScript 與 Tauri skeleton 可執行。
- 階段 1 完成：`neo-core` 已有 PascalCase 相容設定模型、平台資料路徑、renderer 規則、原子 JSON settings store、非阻塞失敗處理的檔案/記憶體 logger 及 Tokio 程序介面。15 個 Rust 測試、strict Clippy、Rust format、前端 typecheck/test/build 均通過。
- `neo-core` 已移植 UxPlay、scrcpy/ADB 與程序群組監督器；引擎套件程式涵蓋路徑穿越、重複項目、符號連結及雜湊錯誤。最新 Harness 通過 30 個 Rust 測試、strict Clippy、格式檢查、Vitest、Vue 型別檢查、Vite build 與 `Set-AppVersion` smoke tests。
- 新版 `.neo_harness/verify.ps1` 已取代舊 Windows-only 驗證入口。macOS 本機完整 verify 通過；Windows .NET 與 Bonjour PowerShell 5.1 步驟以及 Linux/Windows Tauri build 尚待 CI。
- 已新增三平台 CI 彙總 job、分平台 Release 建置與資產合併、Windows Bonjour PowerShell 5.1 驗證、跨平台版本同步、scrcpy v5.0 staging/打包、macOS/Linux UxPlay 安裝指引，以及隔離設定檔的 WebdriverIO E2E。
- 新增 Pinia store 的選取、設定儲存與錯誤狀態單元測試；新增 opt-in 的 Android UI 硬體 E2E 與 `neo-core` 雙引擎並行硬體測試。最新 Harness 中 5 個 Vitest 與 30 個 Rust 單元測試通過；硬體測試已編譯但依 `#[ignore]` 略過。
- Windows 舊版自動啟動值清理、macOS AirPlay 衝突提示、Linux systemd/D-Bus Avahi 探測與 Android udev 權限指引已實作；尚未在 Windows/Linux 或 macOS 互動桌面做平台操作驗收。
- macOS 一般 Tauri `--debug --no-bundle` 建置、embedded WebDriver E2E feature 建置與原生 UI 導覽/設定儲存 E2E 通過。硬體投影與其他作業系統互動仍未驗收。
- 未移除 .NET 專案。Windows、Linux、AirPlay/Android 硬體及跨平台 Release workflow 尚無遠端執行結果，必須保留 .NET 並維持此計畫 active。
