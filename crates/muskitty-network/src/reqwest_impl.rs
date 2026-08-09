//! [`NetworkFetcher`] 的 reqwest 实现。
//!
//! 启用 `reqwest-backend` feature（默认开启）时编译。

use crate::error::NetworkResult;
use crate::fetcher::NetworkFetcher;
use crate::response::NetworkResponse;

/// 基于 `reqwest::Client` 的 [`NetworkFetcher`] 实现。
///
/// 复用底层连接池，线程安全可克隆（`reqwest::Client` 内部 `Arc`）。
///
/// # 示例
///
/// ```no_run
/// # async fn run() -> muskitty_network::NetworkResult<()> {
/// use muskitty_network::{NetworkFetcher, ReqwestFetcher};
///
/// let fetcher = ReqwestFetcher::new()?;
/// let resp = fetcher.fetch("https://example.com").await?;
/// if resp.is_success() {
///     println!("body: {}", resp.text());
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ReqwestFetcher {
    inner: reqwest::Client,
}

impl ReqwestFetcher {
    /// 创建默认实现。
    pub fn new() -> NetworkResult<Self> {
        let inner = reqwest::Client::builder().build()?;
        Ok(Self { inner })
    }
}

impl NetworkFetcher for ReqwestFetcher {
    async fn fetch(&self, url: &str) -> NetworkResult<NetworkResponse> {
        let resp = self.inner.get(url).send().await?;
        let status = resp.status().as_u16();
        let final_url = resp.url().to_string();
        let headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|s| (k.as_str().to_string(), s.to_string()))
            })
            .collect();
        let body_bytes = resp.bytes().await?.to_vec();
        Ok(NetworkResponse::new(status, headers, final_url, body_bytes))
    }
}
