//! 透過 `reqwest` 實作白名單 HTTPS 官方來源之線上版本爬取配接器。

use crate::application::ToolCatalogFetcher;
use crate::domain::{validate_catalog_url, DomainError, DomainResult};
use reqwest::blocking::Client;
use reqwest::redirect;
use reqwest::tls::Version;
use std::io::Read;
use std::time::Duration;

/// 爬取單一官方來源允許追蹤的最大 HTTPS 重導向次數。
const MAX_CATALOG_REDIRECTS: usize = 5;
/// 單一線上版本回應允許讀取的最大位元組數（512 KiB），防止過大回應消耗記憶體。
const MAX_CATALOG_RESPONSE_BYTES: usize = 512 * 1024;
/// 爬取連線逾時秒數。
const CATALOG_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 單次完整 HTTPS 請求逾時秒數。
const CATALOG_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// 實作 `ToolCatalogFetcher` 的 `reqwest` 同步 HTTPS 用戶端，強制 TLS 1.2+ 與白名單主機檢查。
#[derive(Default)]
pub struct ReqwestToolCatalogFetcher;

impl ToolCatalogFetcher for ReqwestToolCatalogFetcher {
    /// 從白名單官方來源下載純文字版本資訊（JSON、Atom XML 或 HTML）。
    ///
    /// # 參數
    /// - `url`: 目標官方來源網址。
    ///
    /// # 回傳值
    /// 成功回傳經過 UTF-8 解析的回應文字；若網址不在白名單、連線異常、非 2xx 或超過大小限制則回傳錯誤。
    fn fetch_text(&self, url: &str) -> DomainResult<String> {
        validate_catalog_url(url)?;

        let client = Client::builder()
            .https_only(true)
            .tls_version_min(Version::TLS_1_2)
            .connect_timeout(CATALOG_CONNECT_TIMEOUT)
            .timeout(CATALOG_REQUEST_TIMEOUT)
            .user_agent(concat!("NeoAirPlay/", env!("CARGO_PKG_VERSION")))
            .redirect(redirect::Policy::custom(|attempt| {
                if attempt.previous().len() > MAX_CATALOG_REDIRECTS {
                    return attempt.error("官方版本來源重導向次數過多。");
                }
                if let Err(error) = validate_catalog_url(attempt.url().as_str()) {
                    return attempt.error(error.message);
                }
                attempt.follow()
            }))
            .build()
            .map_err(|error| {
                DomainError::external_process(format!("無法建立版本查詢連線：{error}"))
            })?;

        let response = client
            .get(url)
            .header(
                reqwest::header::ACCEPT,
                "application/vnd.github+json, application/atom+xml, application/json, text/html;q=0.8",
            )
            .send()
            .map_err(|error| DomainError::external_process(format!("連線官方版本來源失敗：{error}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(DomainError::external_process(format!(
                "官方版本來源回應狀態碼異常：HTTP {}",
                status.as_u16()
            )));
        }

        let mut bytes = Vec::with_capacity(8192);
        response
            .take((MAX_CATALOG_RESPONSE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                DomainError::external_process(format!("讀取版本來源內容失敗：{error}"))
            })?;

        if bytes.len() > MAX_CATALOG_RESPONSE_BYTES {
            return Err(DomainError::external_process(
                "官方版本來源回應超過安全大小上限（512 KiB）。",
            ));
        }

        String::from_utf8(bytes).map_err(|error| {
            DomainError::external_process(format!("版本來源內容非有效 UTF-8：{error}"))
        })
    }
}
