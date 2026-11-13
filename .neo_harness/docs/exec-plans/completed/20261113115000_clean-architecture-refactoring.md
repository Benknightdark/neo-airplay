# Clean Architecture 前後端與測試專案重構執行計畫

## 目的與背景

將 Neo AirPlay 的後端（Rust / Tauri 2）、前端（Vue 3 / TypeScript）與獨立測試專案（`desktop-test`）重構為符合 Clean Architecture 原則的四層架構（`Domain`、`Application`、`Infrastructure`、`Presentation`），解決原先 `state.rs` 與 `dependencies.rs` 過度肥大、Tauri command 混合商業交易邏輯、Vue 元件直接耦合 `api.ts` 與重複輪詢邏輯，以及缺乏架構邊界與 Use Case 隔離測試等問題。

## 目前與預期行為

- **重構前**：
  - 後端由 `models.rs`、`state.rs`、`dependencies.rs`、`lib.rs` 組成，領域規則與子程序 I/O、Tauri 狀態混雜。
  - 前端 `DependencyPanel.vue`、`ToolManagementPage.vue`、`InstallLogViewer.vue` 直接呼叫 `../api`，且元件內部實作輪詢與領域計算。
  - `desktop-test` 缺乏架構依賴檢查、共用 Fixture 與 In-Memory Application Use Case 單元測試。
- **重構後**：
  - 前後端嚴格區分 `domain`、`application`、`infrastructure`、`presentation` 四層，遵循向內依賴原則。
  - 保留全部 11 個 Tauri IPC command 與 JSON `camelCase` 契約、所有 `data-testid` 與 E2E 行為。
  - `desktop-test` 包含自動化架構邊界守門員測試、Domain/Application/Infrastructure/Presentation 分層測試與共用 Fixtures。

## 限制與決策

- 不更動對外 Tauri IPC command 名稱與序列化格式，不破壞現有 E2E 測試與正式套件建置。
- 不新增第三方 crate 或 npm 套件，維持現有依賴鎖定。
- 前端保留 `desktop/src/types.ts`、`desktop/src/api.ts`、`desktop/src/lib/status.ts` 作為相容轉出入口。

## 工作步驟

1. 建立本執行計畫文件。
2. 重構後端 Rust (`desktop/src-tauri/src/`) 為 `domain`、`application`、`infrastructure`、`presentation` 四層，並移除舊扁平模組 `models.rs`、`state.rs`、`dependencies.rs`。
3. 重構前端 Vue 3 + TypeScript (`desktop/src/`) 為 `domain`、`application`、`infrastructure`、`presentation`（含共用 Composables `useDependencyState`、`useInstallLogViewer`）四層。
4. 優化 `desktop-test/`，新增架構邊界測試、共用 Fixtures、Domain/Application/Composables 分層測試與後端 In-Memory Use Case 測試。
5. 更新 `.neo_harness/ARCHITECTURE.md`，執行 `desktop-test` 與 `desktop/scripts/verify.sh` 驗證並記錄結果。

## 進度與發現

- [x] 建立進行中執行計畫。
- [x] 完成後端 Rust 四層重構（`domain/`、`application/`、`infrastructure/`、`presentation/`）。
- [x] 完成前端 Vue 3 + TypeScript 四層重構（`domain/`、`application/`、`infrastructure/`、`presentation/di.ts`、`composables/`、`components/`）。
- [x] 完成 `desktop-test` 測試架構優化。
- [x] 完成全量驗證並更新 `.neo_harness/ARCHITECTURE.md`。

## 驗證方式與結果

- `cd desktop-test && bun run --silent verify`：通過（Vitest 18 files / 51 tests passed；Rust backend unit-tests 40 passed）。
- `bash desktop/scripts/verify.sh`：通過（exit code 0，涵蓋 TypeScript typecheck、Vitest 18 files / 51 tests、安裝與版本腳本測試、`cargo fmt --check`、Rust 40 tests、`cargo clippy --all-targets --all-features -- -D warnings`、Vite production build）。
- 桌面 E2E（`cd desktop-test && bun run --silent test:e2e:ci`）：未於本次執行（需圖形工作階段與完整 Tauri WDIO 建置）。

## 復原方式

可透過 Git 還原 `desktop/`、`desktop-test/` 與 `.neo_harness/` 變更。

## 最終結果

已完成前後端與測試專案之 Clean Architecture 四層重構，並通過 `verify.sh` 完整驗證。
