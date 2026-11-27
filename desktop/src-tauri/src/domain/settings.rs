//! 接收器設定值物件、視訊渲染器對應規則與外部引擎命令列參數建構邏輯。

use serde::{Deserialize, Serialize};

/// 預設 AirPlay 接收器廣播名稱。
const DEFAULT_RECEIVER_NAME: &str = "Neo AirPlay";
/// 預設視訊渲染器選項識別字。
const DEFAULT_RENDERER: &str = "Auto";
/// DNS-SD（Bonjour）服務實例名稱允許的最大 UTF-8 位元組長度。
pub const MAX_RECEIVER_NAME_UTF8_BYTES: usize = 63;

/// 判斷字元是否為控制字元或 Unicode 雙向（Bidi）方向覆寫控制字元。
fn is_unsafe_display_char(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'
        )
}

/// 依 UTF-8 字元邊界截斷字串，保證結果位元組長度不超過 `max_bytes`。
fn truncate_utf8_bytes(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

/// AirPlay 區域網路連線認證模式。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AirplayAuthMode {
    /// 不啟用連線 PIN 驗證（維持既有預設行為）。
    #[default]
    None,
    /// 每次啟動 AirPlay 接收器時產生隨機 4 碼一次性 PIN。
    RandomPin,
    /// 使用固定 4 碼數字 PIN 限制連線，並於受控註冊檔記錄已配對裝置。
    FixedPin,
}

/// 驗證並正規化 4 碼數字 PIN；不符合精確 4 位 ASCII 數字時回傳空字串。
pub fn sanitize_airplay_pin(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() == 4 && trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        trimmed.to_owned()
    } else {
        String::new()
    }
}

/// 已正規化的 AirPlay 接收器顯示名稱值物件，確保名稱不為空白、無控制或 Bidi 字元且符合 63 位元組上限。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverName(String);

impl ReceiverName {
    /// 建立 `ReceiverName`，會移除控制與 Bidi 字元、修剪前後空白並截斷至 63 UTF-8 位元組，空字串時退回 `"Neo AirPlay"`。
    ///
    /// # 參數
    /// - `raw`: 使用者輸入或設定檔載入的原始接收器名稱。
    ///
    /// # 回傳值
    /// 回傳保證非空且安全的 `ReceiverName` 值物件。
    pub fn new(raw: &str) -> Self {
        let filtered: String = raw
            .chars()
            .filter(|ch| !is_unsafe_display_char(*ch))
            .collect();
        let trimmed = filtered.trim();
        let bounded = truncate_utf8_bytes(trimmed, MAX_RECEIVER_NAME_UTF8_BYTES).trim_end();
        if bounded.is_empty() {
            Self(DEFAULT_RECEIVER_NAME.to_owned())
        } else {
            Self(bounded.to_owned())
        }
    }

    /// 取得正規化後的接收器名稱字串切片。
    ///
    /// # 回傳值
    /// 回傳接收器名稱的 `&str` 參考。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 視訊渲染器設定值物件，保證內容皆為目前作業系統支援的合法選項。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoRenderer(String);

impl VideoRenderer {
    /// 建立 `VideoRenderer`，若不在目前平台允許的清單中則自動退回 `"Auto"`。
    ///
    /// # 參數
    /// - `raw`: 原始視訊渲染器名稱字串。
    ///
    /// # 回傳值
    /// 回傳已驗證為平台可用選項的 `VideoRenderer` 值物件。
    pub fn new(raw: &str) -> Self {
        if available_renderers().contains(&raw) {
            Self(raw.to_owned())
        } else {
            Self(DEFAULT_RENDERER.to_owned())
        }
    }

    /// 取得正規化後的渲染器選項名稱。
    ///
    /// # 回傳值
    /// 回傳渲染器選項名稱的 `&str` 參考。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 將此視訊渲染器選項對應為 UxPlay `-vs` 參數所需的 GStreamer video sink 名稱。
    ///
    /// # 回傳值
    /// 若需明確指定 `-vs` sink 則回傳 `Some(&'static str)`；若交由 UxPlay 自動選擇則回傳 `None`。
    pub fn gstreamer_sink(&self) -> Option<&'static str> {
        renderer_argument(&self.0)
    }
}

/// 使用者可配置的投影接收器完整設定資料模型。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverSettings {
    /// AirPlay 區網廣播名稱。
    #[serde(default = "default_receiver_name")]
    pub receiver_name: String,
    /// 使用者選擇的視訊渲染後端名稱。
    #[serde(default = "default_renderer")]
    pub renderer: String,
    /// 是否以全螢幕模式啟動投影視窗。
    #[serde(default)]
    pub fullscreen: bool,
    /// 是否在作業系統登入時自動啟動應用程式。
    #[serde(default)]
    pub start_on_login: bool,
    /// AirPlay 區網連線認證模式（關閉／隨機 PIN／固定 PIN）。
    #[serde(default)]
    pub airplay_auth_mode: AirplayAuthMode,
    /// 固定 PIN 模式下的 4 位數字密碼（非固定模式時保持空字串）。
    #[serde(default)]
    pub airplay_pin: String,
    /// 啟動 `scrcpy` 時是否停用電腦與 Android 裝置間的自動剪貼簿同步。
    #[serde(default = "default_true")]
    pub android_no_clipboard_sync: bool,
    /// 啟動 `scrcpy` 時是否使用唯讀投影模式（禁止視窗操控與檔案拖放安裝）。
    #[serde(default)]
    pub android_read_only: bool,
}

impl Default for ReceiverSettings {
    /// 建立預設接收器設定（名稱 `"Neo AirPlay"`、渲染器 `"Auto"`、Android 預設停用自動剪貼簿同步）。
    ///
    /// # 回傳值
    /// 回傳包含預設欄位的 `ReceiverSettings`。
    fn default() -> Self {
        Self {
            receiver_name: default_receiver_name(),
            renderer: default_renderer(),
            fullscreen: false,
            start_on_login: false,
            airplay_auth_mode: AirplayAuthMode::None,
            airplay_pin: String::new(),
            android_no_clipboard_sync: true,
            android_read_only: false,
        }
    }
}

impl ReceiverSettings {
    /// 套用領域值物件規則清理並正規化設定欄位。
    ///
    /// # 回傳值
    /// 回傳名稱、渲染器與認證欄位皆已正規化的新 `ReceiverSettings` 實例。
    pub fn sanitized(self) -> Self {
        let receiver_name = ReceiverName::new(&self.receiver_name);
        let renderer = VideoRenderer::new(&self.renderer);
        let sanitized_pin = sanitize_airplay_pin(&self.airplay_pin);
        let (airplay_auth_mode, airplay_pin) = match self.airplay_auth_mode {
            AirplayAuthMode::None => (AirplayAuthMode::None, String::new()),
            AirplayAuthMode::RandomPin => (AirplayAuthMode::RandomPin, String::new()),
            AirplayAuthMode::FixedPin if !sanitized_pin.is_empty() => {
                (AirplayAuthMode::FixedPin, sanitized_pin)
            }
            AirplayAuthMode::FixedPin => (AirplayAuthMode::RandomPin, String::new()),
        };
        Self {
            receiver_name: receiver_name.as_str().to_owned(),
            renderer: renderer.as_str().to_owned(),
            fullscreen: self.fullscreen,
            start_on_login: self.start_on_login,
            airplay_auth_mode,
            airplay_pin,
            android_no_clipboard_sync: self.android_no_clipboard_sync,
            android_read_only: self.android_read_only,
        }
    }

    /// 依目前 `receiver_name` 欄位建立 `ReceiverName` 值物件。
    ///
    /// # 回傳值
    /// 回傳正規化後的 `ReceiverName`。
    pub fn receiver_name_value(&self) -> ReceiverName {
        ReceiverName::new(&self.receiver_name)
    }

    /// 依目前 `renderer` 欄位建立 `VideoRenderer` 值物件。
    ///
    /// # 回傳值
    /// 回傳經平台支援驗證後的 `VideoRenderer`。
    pub fn video_renderer_value(&self) -> VideoRenderer {
        VideoRenderer::new(&self.renderer)
    }

    /// 依目前設定組裝啟動 `uxplay` 所需的命令列參數陣列（不包含固定 PIN 明文）。
    ///
    /// # 回傳值
    /// 回傳傳遞給 UxPlay 子程序的完整參數清單。
    pub fn airplay_arguments(&self) -> Vec<String> {
        let mut args = vec![
            "-n".to_owned(),
            self.receiver_name_value().as_str().to_owned(),
            "-nh".to_owned(),
        ];
        if self.fullscreen {
            args.push("-fs".to_owned());
        }
        #[cfg(target_os = "macos")]
        args.extend(["-avdec".to_owned(), "-vsync".to_owned(), "no".to_owned()]);
        if let Some(renderer) = self.video_renderer_value().gstreamer_sink() {
            args.extend(["-vs".to_owned(), renderer.to_owned()]);
        }
        args
    }

    /// 產生寫入受控 `0600` `UXPLAYRC` 設定檔的內容（用於套用 PIN 保護並隔離外部 `.uxplayrc`）。
    pub fn uxplay_rc_contents(&self, register_path: Option<&str>) -> String {
        let mut lines = Vec::new();
        match self.airplay_auth_mode {
            AirplayAuthMode::None => return String::new(),
            AirplayAuthMode::RandomPin => lines.push("pin".to_owned()),
            AirplayAuthMode::FixedPin => {
                let pin = sanitize_airplay_pin(&self.airplay_pin);
                if pin.is_empty() {
                    lines.push("pin".to_owned());
                } else {
                    lines.push(format!("pin {pin}"));
                }
            }
        }
        if let Some(path) = register_path {
            if !path.is_empty() && !path.contains('"') && !path.chars().any(is_unsafe_display_char)
            {
                lines.push(format!("reg \"{path}\""));
            }
        }
        format!("{}\n", lines.join("\n"))
    }

    /// 依目前設定與指定裝置序號組裝啟動 `scrcpy` 所需的命令列參數陣列。
    ///
    /// # 參數
    /// - `serial`: 目標 Android 裝置的 ADB 序號。
    ///
    /// # 回傳值
    /// 回傳傳遞給 scrcpy 子程序的完整參數清單。
    pub fn android_arguments(&self, serial: &str) -> Vec<String> {
        let mut args = vec![format!("--serial={serial}")];
        if self.fullscreen {
            args.push("--fullscreen".to_owned());
        }
        if self.android_no_clipboard_sync {
            args.push("--no-clipboard-autosync".to_owned());
        }
        if self.android_read_only {
            args.push("--no-control".to_owned());
        }
        args
    }
}

/// Serde 反序列化缺漏 `androidNoClipboardSync` 時使用的預設值工廠函式。
fn default_true() -> bool {
    true
}

/// Serde 反序列化缺漏 `receiverName` 時使用的預設值工廠函式。
///
/// # 回傳值
/// 回傳預設接收器名稱字串。
fn default_receiver_name() -> String {
    DEFAULT_RECEIVER_NAME.to_owned()
}

/// Serde 反序列化缺漏 `renderer` 時使用的預設值工廠函式。
///
/// # 回傳值
/// 回傳預設渲染器名稱字串。
fn default_renderer() -> String {
    DEFAULT_RENDERER.to_owned()
}

/// 取得目前作業系統平台支援的所有視訊渲染器下拉選項清單。
///
/// # 回傳值
/// 回傳依平台編譯條件決定的靜態字串切片。
pub fn available_renderers() -> &'static [&'static str] {
    #[cfg(target_os = "windows")]
    {
        &["Auto", "D3D11", "D3D12"]
    }
    #[cfg(target_os = "macos")]
    {
        &["Auto", "osxvideosink", "glimagesink"]
    }
    #[cfg(target_os = "linux")]
    {
        &["Auto", "glimagesink", "ximagesink", "waylandsink"]
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        &["Auto"]
    }
}

/// 將介面顯示用的渲染器名稱對應至底層 GStreamer video sink 參數值。
///
/// # 參數
/// - `renderer`: 使用者或設定檔指定的渲染器選項字串。
///
/// # 回傳值
/// 若該選項需要傳入 `-vs` 旗標則回傳 `Some(sink_name)`；否則回傳 `None`。
pub fn renderer_argument(renderer: &str) -> Option<&'static str> {
    match renderer {
        "D3D11" => Some("d3d11videosink"),
        "D3D12" => Some("d3d12videosink"),
        "osxvideosink" => Some("osxvideosink"),
        "glimagesink" => Some("glimagesink"),
        "ximagesink" => Some("ximagesink"),
        "waylandsink" => Some("waylandsink"),
        "Auto" if cfg!(target_os = "macos") => Some("osxvideosink"),
        _ => None,
    }
}
