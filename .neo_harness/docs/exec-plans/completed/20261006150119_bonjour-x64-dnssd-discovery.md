# 修正 x64 DNS-SD 元件分開安裝時的 Bonjour 探測

## 目的

讓 x64 Neo AirPlay 支援可信任的 x86 Bonjour Service 執行檔，並從可信任的 64 位元 Bonjour 位置或 Windows System32 尋找相同版本的 x64 `dnssd.dll`。當服務程式存在但相容 DLL 缺少或不相容時，顯示明確原因。

## 背景與目前行為

- 目前 Bonjour inspector 與 PowerShell helper 要求 `mDNSResponder.exe` 和 `dnssd.dll` 位於同一目錄。
- 使用者已確認現有 Bonjour Service 位於 `%ProgramFiles(x86)%`，其目錄沒有 DNS-SD DLL；`%SystemRoot%\System32\dnssd.dll` 是相同 Bonjour 版本的 x64 DLL，而 SysWOW64 副本是 x86。
- Neo AirPlay 與 UxPlay 是 x64，因此服務可由 x86 Bonjour responder 執行，但 App/建置用 DNS-SD DLL 必須為 x64。
- 現有工作樹含前一個 Bonjour 啟動計畫的未提交變更；不得覆蓋或還原那些變更。

## 預期行為

- 服務 responder 可來自可信任的 `%ProgramFiles%` 或 `%ProgramFiles(x86)%` Bonjour 目錄，PE 架構限定 x86/x64，產品資訊須為 Bonjour。
- DNS-SD DLL 獨立搜尋可信任的 x64 Program Files Bonjour 目錄與 x64 Windows `System32`；DLL 必須為 AMD64、產品資訊為 Bonjour，且 FileVersion 與 responder 一致。
- 拒絕 SysWOW64/x86 DLL、非 Bonjour 檔案、版本不符檔案，以及 Program Files 以外的來源。
- Staging 將通過檢查的 responder 和 DLL 分別複製到受保護的 `%ProgramFiles%\NeoAirPlay\Bonjour`，安裝及還原流程都接受 x86 responder 搭配 x64 DLL。
- 建置 UxPlay 時也使用同一個 x64 DLL 尋找規則。
- 找到 Bonjour responder 但缺少相容 x64 DLL 時，App 明確指出缺少 x64 DNS-SD 元件並提供 Apple 官方安裝資訊；不下載或散布 Bonjour。
- E2E profile、取消 UAC、服務 Disabled 行為及先前服務備份/ACL/還原安全檢查維持原狀。

## 限制與決策

- 維持 Windows x64 App 和 UxPlay；不改成 x86 引擎。
- 只讀取本機受信任安裝來源及 Windows System32，不新增下載來源或公開套件內容。
- 保留既有 `BonjourRuntime.Directory`，只調整內部來源發現回報與腳本 helper。
- PE Machine 欄位用於驗證架構，不以資料夾名稱推斷 DLL 位元數。

## 工作順序

1. 共用設定加入 Program Files (x86) Bonjour executable 候選，實作 C# 與 PowerShell PE/產品/版本/信任路徑驗證。
2. 更新 App inspector、啟動診斷、Bonjour staging/安裝/還原 helper，以及 UxPlay 建置腳本，使 responder 與 x64 DLL 分開配對。
3. 新增 PE parser 與來源候選測試，更新 README 和 Harness 架構、安全、可靠性及腳本索引。
4. 執行 `.neo_harness/verify.ps1`、PowerShell Parser 和唯讀本機來源配對檢查，並審查整個 working tree。

## 驗證與復原

- 單元測試涵蓋 x86/x64 PE machine 讀取、無效 PE、x86 responder + x64 DLL 配對候選、x86 DLL 拒絕及缺少相容 DLL 的診斷。
- PowerShell Parser 檢查 Bonjour helper、Stage、Install、Restore 及 build-engine 腳本。
- 唯讀確認目前 Bonjour responder、System32 DLL 的產品、版本與 PE 架構；不在驗證中修改 Windows 服務或觸發 UAC。
- 若需要復原，使用 Git 回復本計畫新增的程式、腳本、測試與文件變更；既有 Bonjour 服務設定不由驗證流程修改。

## 進度、發現與結果

- [x] 閱讀 README、Harness 架構/安全/可靠性/計畫規範、Bonjour inspector、管理腳本及前一個計畫。
- [x] 確認根因是把 service responder 和 x64 DNS-SD DLL 綁在同一個目錄。
- [x] C# 與 PowerShell 分開來源搜尋及 PE/版本驗證；缺少 x64 DLL 的訊息由共用設定提供。
- [x] 測試、README、Harness 架構/安全/可靠性文件與命令索引更新。
- [x] `.neo_harness/verify.ps1` 通過：0 建置警告/錯誤、Framework Unit 29/29、App Unit 19/19、Set-AppVersion smoke tests 通過。
- [x] PowerShell Parser 接受 Bonjour helper、Stage、Install、Restore 與 build-engine 腳本。
- [x] 唯讀本機檢查確認 Bonjour 2.0.2.0 x86 responder 可配對同版本 System32 x64 DLL；PowerShell helper 與 C# Inspector 都成功找到來源。
- [x] 完成工作樹跨檔案審查；未發現 Critical Issues。Markdown 相對連結、scripts.md 命令表及 `git diff --check` 通過。
- [ ] 實際 UAC 安裝、服務啟動與還原驗收；此項會修改系統服務，需由可還原的 Windows 測試環境完成。

## 結果與剩餘驗收

- C# Inspector 已在目前主機讀取 Bonjour Service 登錄資料，回報 `%ProgramFiles(x86)%\Bonjour` 為可信任來源；配對的 `%SystemRoot%\System32\dnssd.dll` 通過 x64 架構、Bonjour 產品資訊及 FileVersion 檢查。
- 尚未從一般使用者工作階段實際觸發 UAC、切換服務到 Neo AirPlay 共用目錄、重啟服務或透過 Apple 裝置驗證 AirPlay 探索。這些系統服務與實機驗收需在可還原的 Windows 測試環境完成。
