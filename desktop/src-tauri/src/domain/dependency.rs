//! 外部投影依賴工具的狀態快照、安裝進度與腳本輸出解析邏輯。

use serde::{Deserialize, Serialize};
use std::process::ExitStatus;

/// 使用者於工具管理頁面可觸發的維護動作。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DependencyAction {
    /// 升級現有投影依賴套件至最新版本。
    Upgrade,
    /// 移除並重新安裝投影依賴套件。
    Reinstall,
}

/// 依賴工具背景作業目前或最後一次執行的操作類型。
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DependencyOperation {
    /// 初次自動安裝缺少的投影工具。
    Install,
    /// 升級已安裝的投影工具。
    Upgrade,
    /// 重新安裝所有投影工具。
    Reinstall,
}

impl From<DependencyAction> for DependencyOperation {
    /// 將表現層傳入的維護動作轉換為領域作業類型。
    ///
    /// # 參數
    /// - `action`: 使用者觸發的維護動作。
    ///
    /// # 回傳值
    /// 回傳對應的 `DependencyOperation` 列舉值。
    fn from(action: DependencyAction) -> Self {
        match action {
            DependencyAction::Upgrade => Self::Upgrade,
            DependencyAction::Reinstall => Self::Reinstall,
        }
    }
}

/// 呈現給前端 UI 的依賴工具狀態完整快照。
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DependencySnapshot {
    /// 目前狀態識別字串（如 `disabled`、`checking`、`installing`、`ready`、`failed`）。
    pub status: String,
    /// 正在執行或剛完成的操作類型。
    pub operation: Option<DependencyOperation>,
    /// 顯示給使用者的狀態摘要訊息。
    pub message: String,
    /// 檢查後仍缺少的依賴工具名稱清單。
    pub missing: Vec<String>,
    /// 本次安裝或維護作業的日誌檔案完整路徑。
    pub log_path: String,
}

/// 安裝或檢查程序結束後的結果模型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupOutcome {
    /// 結果狀態（`"ready"` 或 `"failed"`）。
    pub status: String,
    /// 結果說明訊息。
    pub message: String,
    /// 若失敗或仍有缺漏時記錄的缺少工具清單。
    pub missing: Vec<String>,
}

impl SetupOutcome {
    /// 依已完成的操作類型建立就緒（`ready`）結果。
    ///
    /// # 參數
    /// - `operation`: 本次完成的依賴作業種類。
    ///
    /// # 回傳值
    /// 回傳 `missing` 為空的就緒 `SetupOutcome`。
    pub fn ready(operation: DependencyOperation) -> Self {
        Self {
            status: "ready".into(),
            message: match operation {
                DependencyOperation::Install => "投影工具已就緒。".into(),
                DependencyOperation::Upgrade => "投影工具升級完成。".into(),
                DependencyOperation::Reinstall => "投影工具重新安裝完成。".into(),
            },
            missing: Vec::new(),
        }
    }

    /// 建立失敗（`failed`）結果。
    ///
    /// # 參數
    /// - `message`: 描述失敗原因的訊息。
    /// - `missing`: 仍缺少的依賴項目清單。
    ///
    /// # 回傳值
    /// 回傳 `status` 為 `"failed"` 的 `SetupOutcome`。
    pub fn failed(message: String, missing: Vec<String>) -> Self {
        Self {
            status: "failed".into(),
            message,
            missing,
        }
    }
}

/// 安裝腳本寫入結果檔的 JSON 結構。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    /// 實際執行安裝作業的子程序 PID。
    pub pid: u32,
    /// 安裝腳本的結束代碼（`0` 代表成功）。
    pub exit_code: i32,
    /// 發生失敗或結束前所處的安裝階段識別碼。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    /// 安裝腳本回報的詳細結果或錯誤訊息。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl InstallResult {
    /// 將結果欄位組合成適合呈現或寫入錯誤日誌的單行說明字串。
    ///
    /// # 回傳值
    /// 回傳包含結束代碼、中文階段名稱與附帶訊息（若有）的描述文字。
    pub fn detail(&self) -> String {
        let mut detail = format!("安裝程序結束代碼 {}", self.exit_code);
        if let Some(stage) = &self.stage {
            detail.push_str(&format!("（{}）", stage_label(stage)));
        }
        if let Some(message) = self
            .message
            .as_deref()
            .filter(|message| !message.is_empty())
        {
            detail.push_str(&format!("：{message}"));
        }
        detail
    }
}

/// 安裝腳本即時回寫進度檔案的 JSON 結構。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InstallProgress {
    /// 回報進度的子程序 PID。
    pub pid: u32,
    /// 目前執行中的階段識別碼。
    pub stage: String,
    /// 目前階段的進度描述訊息。
    pub message: String,
}

/// 記錄單次安裝流程啟動資訊，用於比對跨行程或非同步產生的結果檔案。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRecord {
    /// 發起安裝的父行程或包裝腳本 PID。
    pub owner_pid: u32,
    /// 若經過提權或背景轉發，實際執行中的工作者 PID。
    pub worker_pid: Option<u32>,
    /// 安裝啟動的 Unix 時間戳記（秒）。
    pub started_at: u64,
}

/// 執行環境健康檢查的原始結果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeCheck {
    /// 檢測後缺少的投影工具名稱清單。
    pub missing: Vec<String>,
}

impl RuntimeCheck {
    /// 檢查是否所有必要投影工具皆已就緒。
    ///
    /// # 回傳值
    /// 若 `missing` 清單為空則回傳 `true`。
    pub fn is_ready(&self) -> bool {
        self.missing.is_empty()
    }
}

/// 將安裝腳本輸出的英文階段識別碼轉譯為繁體中文介面標籤。
///
/// # 參數
/// - `stage`: 安裝腳本定義的階段識別碼字串。
///
/// # 回傳值
/// 回傳對應的繁體中文階段說明字串。
pub fn stage_label(stage: &str) -> &'static str {
    match stage {
        "checking" => "檢查投影工具",
        "authorizing" => "系統授權",
        "msys2" => "安裝建置工具",
        "updating" => "更新工具套件",
        "packages" => "安裝工具套件",
        "toolchain" => "驗證編譯工具",
        "repairing" => "修復編譯工具",
        "uxplay-download" => "下載 AirPlay 工具",
        "uxplay-build" => "編譯 AirPlay 工具",
        "uxplay-install" => "安裝 AirPlay 工具",
        "scrcpy-download" => "下載 Android 工具",
        "staging" => "整理工具檔案",
        "verifying" => "驗證投影工具",
        "ready" => "準備完成",
        _ => "工具安裝",
    }
}

/// 從檢查腳本的標準輸出解析缺少的工具清單（以 `- ` 開頭的行）。
///
/// # 參數
/// - `output`: 環境檢查腳本回傳的原始輸出文字。
///
/// # 回傳值
/// 回傳去掉前導項目符號並過濾空行後的缺少工具名稱向量。
pub fn parse_missing_dependencies(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.trim().strip_prefix('-').map(str::trim))
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 從環境檢查輸出取得第一行具有實質診斷價值的文字（略過固定標題列）。
///
/// # 參數
/// - `output`: 檢查腳本的標準輸出或錯誤輸出文字。
///
/// # 回傳值
/// 找到有效文字行時回傳 `Some(line)`，否則回傳 `None`。
pub fn first_useful_line(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("Missing runtime dependencies:"))
        .map(str::to_owned)
}

/// 當安裝程序意外終止且未寫入結果檔時，組裝診斷用的詳細失敗說明。
///
/// # 參數
/// - `pid`: 相關安裝程序的 PID（若已知）。
/// - `status`: 作業系統回傳的結束狀態（若已收集到）。
/// - `progress`: 最後一次讀到的安裝進度快照（若有）。
///
/// # 回傳值
/// 回傳整合 PID、結束狀態與最後階段的繁體中文診斷字串。
pub fn interrupted_install_detail(
    pid: Option<u32>,
    status: Option<ExitStatus>,
    progress: Option<&InstallProgress>,
) -> String {
    let mut detail = pid
        .map(|pid| format!("安裝程序 {pid} 已結束，但沒有寫入完成結果。"))
        .unwrap_or_else(|| "安裝程序未寫入完成結果。".into());
    if let Some(status) = status {
        detail.push_str(&format!("程序退出狀態：{status}。"));
    }
    if let Some(progress) = progress {
        detail.push_str(&format!("最後階段：{}。", stage_label(&progress.stage)));
    }
    detail
}
