//! 後端單元測試共用測試套件掛載進入點，透過 `#[path]` 對應至主專案的領域、應用與基礎設施層模組。

#![allow(dead_code)]

/// 掛載後端應用層模組以執行使用案例與測試替身單元測試。
#[path = "../../../desktop/src-tauri/src/application/mod.rs"]
pub mod application;
/// 掛載後端領域層模組以執行領域模型與值物件單元測試。
#[path = "../../../desktop/src-tauri/src/domain/mod.rs"]
pub mod domain;
/// 掛載後端基礎設施層模組以執行狀態、持久化與依賴管理單元測試。
#[path = "../../../desktop/src-tauri/src/infrastructure/mod.rs"]
pub mod infrastructure;
