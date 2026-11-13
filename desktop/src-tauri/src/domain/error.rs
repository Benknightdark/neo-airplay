//! 領域層錯誤型別與統一回傳結果定義，隔離底層 I/O 與框架細節。

/// 領域錯誤分類，供應用層與表現層決定錯誤處理方式與提示語氣。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// 輸入參數或領域規則驗證失敗。
    Validation,
    /// 找不到指定的資源、工具或裝置。
    NotFound,
    /// 與現行執行狀態衝突（例如重複啟動或資源佔用）。
    Conflict,
    /// 投影工具目前正在安裝或維護中，暫時無法執行操作。
    DependencyBusy,
    /// 目前執行模式已停用內建依賴工具管理。
    DependencyDisabled,
    /// 外部工具或子程序啟動／執行異常。
    ExternalProcess,
    /// 設定讀寫或本地端持久化作業失敗。
    Storage,
}

/// 領域層統一錯誤物件，攜帶語意分類與可直接呈現的使用者訊息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainError {
    /// 錯誤類別，用於識別失敗原因。
    pub kind: ErrorKind,
    /// 說明失敗細節的人類可讀訊息。
    pub message: String,
}

impl DomainError {
    /// 建立驗證失敗類型的 `DomainError`。
    ///
    /// # 參數
    /// - `message`: 說明輸入或規則不符的錯誤訊息。
    ///
    /// # 回傳值
    /// 回傳 `ErrorKind::Validation` 的 `DomainError`。
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Validation,
            message: message.into(),
        }
    }

    /// 建立資源不存在類型的 `DomainError`。
    ///
    /// # 參數
    /// - `message`: 說明找不到目標的錯誤訊息。
    ///
    /// # 回傳值
    /// 回傳 `ErrorKind::NotFound` 的 `DomainError`。
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::NotFound,
            message: message.into(),
        }
    }

    /// 建立狀態衝突類型的 `DomainError`。
    ///
    /// # 參數
    /// - `message`: 說明作業與目前狀態互斥的錯誤訊息。
    ///
    /// # 回傳值
    /// 回傳 `ErrorKind::Conflict` 的 `DomainError`。
    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Conflict,
            message: message.into(),
        }
    }

    /// 建立依賴工具忙碌中類型的 `DomainError`。
    ///
    /// # 參數
    /// - `message`: 說明安裝或修復作業進行中的訊息。
    ///
    /// # 回傳值
    /// 回傳 `ErrorKind::DependencyBusy` 的 `DomainError`。
    pub fn dependency_busy(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::DependencyBusy,
            message: message.into(),
        }
    }

    /// 建立依賴管理已停用類型的 `DomainError`。
    ///
    /// # 參數
    /// - `message`: 說明當前環境不允許自動管理依賴工具的訊息。
    ///
    /// # 回傳值
    /// 回傳 `ErrorKind::DependencyDisabled` 的 `DomainError`。
    pub fn dependency_disabled(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::DependencyDisabled,
            message: message.into(),
        }
    }

    /// 建立外部程序異常類型的 `DomainError`。
    ///
    /// # 參數
    /// - `message`: 說明外部執行檔啟動或運作失敗的訊息。
    ///
    /// # 回傳值
    /// 回傳 `ErrorKind::ExternalProcess` 的 `DomainError`。
    pub fn external_process(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::ExternalProcess,
            message: message.into(),
        }
    }

    /// 建立儲存體讀寫失敗類型的 `DomainError`。
    ///
    /// # 參數
    /// - `message`: 說明設定檔或本機儲存操作失敗的訊息。
    ///
    /// # 回傳值
    /// 回傳 `ErrorKind::Storage` 的 `DomainError`。
    pub fn storage(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Storage,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for DomainError {
    /// 將領域錯誤格式化為人類可讀的訊息字串。
    ///
    /// # 參數
    /// - `f`: Rust 格式化輸出緩衝器。
    ///
    /// # 回傳值
    /// 回傳格式化是否成功的 `std::fmt::Result`。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DomainError {}

impl From<DomainError> for String {
    /// 將 `DomainError` 轉換為前端命令可直接序列化傳回的錯誤字串。
    ///
    /// # 參數
    /// - `error`: 來源領域錯誤實例。
    ///
    /// # 回傳值
    /// 回傳錯誤物件內含的 `message` 字串。
    fn from(error: DomainError) -> Self {
        error.message
    }
}

/// 領域層與應用層共用的標準 `Result` 型別別名。
pub type DomainResult<T> = Result<T, DomainError>;
