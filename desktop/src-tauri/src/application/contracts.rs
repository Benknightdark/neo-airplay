//! 應用層對表現層輸出資料時使用的組合契約模型。

use crate::domain::{AndroidDevice, AppLogEntry, EngineSnapshot, ReceiverSettings};
use serde::{Deserialize, Serialize};

/// 儀表板畫面一次載入或重新整理所需的完整應用狀態快照。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    /// 目前生效的接收器設定。
    pub settings: ReceiverSettings,
    /// 目前平台所有可用的視訊渲染器選項。
    pub renderer_options: Vec<String>,
    /// AirPlay 引擎最新狀態快照。
    pub airplay: EngineSnapshot,
    /// Android 投影引擎最新狀態快照。
    pub android: EngineSnapshot,
    /// 最近掃描到的 Android 裝置列表。
    pub devices: Vec<AndroidDevice>,
    /// 近期應用程式診斷日誌清單。
    pub logs: Vec<AppLogEntry>,
}
