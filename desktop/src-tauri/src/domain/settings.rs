//! 接收器設定值物件、視訊渲染器對應規則與外部引擎命令列參數建構邏輯。

use serde::{Deserialize, Serialize};

/// 預設 AirPlay 接收器廣播名稱。
const DEFAULT_RECEIVER_NAME: &str = "Neo AirPlay";
/// 預設視訊渲染器選項識別字。
const DEFAULT_RENDERER: &str = "Auto";

/// 已正規化的 AirPlay 接收器顯示名稱值物件，確保名稱不為空白或全空白字元。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverName(String);

impl ReceiverName {
    /// 建立 `ReceiverName`，會修剪前後空白並在空字串時退回預設名稱 `"Neo AirPlay"`。
    ///
    /// # 參數
    /// - `raw`: 使用者輸入或設定檔載入的原始接收器名稱。
    ///
    /// # 回傳值
    /// 回傳保證非空的 `ReceiverName` 值物件。
    pub fn new(raw: &str) -> Self {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            Self(DEFAULT_RECEIVER_NAME.to_owned())
        } else {
            Self(trimmed.to_owned())
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
}

impl Default for ReceiverSettings {
    /// 建立預設接收器設定（名稱 `"Neo AirPlay"`、渲染器 `"Auto"`、非全螢幕、不開機自啟）。
    ///
    /// # 回傳值
    /// 回傳包含預設欄位的 `ReceiverSettings`。
    fn default() -> Self {
        Self {
            receiver_name: default_receiver_name(),
            renderer: default_renderer(),
            fullscreen: false,
            start_on_login: false,
        }
    }
}

impl ReceiverSettings {
    /// 套用領域值物件規則清理並正規化設定欄位。
    ///
    /// # 回傳值
    /// 回傳名稱與渲染器皆已正規化的新 `ReceiverSettings` 實例。
    pub fn sanitized(self) -> Self {
        let receiver_name = ReceiverName::new(&self.receiver_name);
        let renderer = VideoRenderer::new(&self.renderer);
        Self {
            receiver_name: receiver_name.as_str().to_owned(),
            renderer: renderer.as_str().to_owned(),
            fullscreen: self.fullscreen,
            start_on_login: self.start_on_login,
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

    /// 依目前設定組裝啟動 `uxplay` 所需的命令列參數陣列。
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

    /// 依目前設定與指定裝置序號組裝啟動 `scrcpy` 所需的命令列參數陣列。
    ///
    /// # 參數
    /// - `serial`: 目標 Android 裝置的 ADB 序號。
    ///
    /// # 回傳值
    /// 回傳傳遞給 scrcpy 子程序的完整參數清單。
    pub fn android_arguments(&self, serial: &str) -> Vec<String> {
        let mut args = vec!["--serial".to_owned(), serial.to_owned()];
        if self.fullscreen {
            args.push("--fullscreen".to_owned());
        }
        args
    }
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
