//! [`NetworkFetcher`] 的 reqwest 实现。
//!
//! 启用 `reqwest-backend` feature（默认开启）时编译。

use crate::error::{NetworkError, NetworkResult};
use crate::fetcher::NetworkFetcher;
use crate::response::NetworkResponse;

/// 基于 `reqwest::Client` 的 [`NetworkFetcher`] 实现。
///
/// 复用底层连接池，线程安全可克隆（`reqwest::Client` 内部 `Arc`）。
///
/// F-14（审计 S-6）：Client 配置总超时（[`crate::DEFAULT_TIMEOUT`]）与
/// 连接超时（[`crate::DEFAULT_CONNECT_TIMEOUT`]）——trait 契约承诺超时
/// 错误，slow-loris 不得无限挂起；响应体按 [`crate::MAX_BODY_BYTES`]
/// 上限**流式**读取（chunk 循环边收边检，Content-Length 超限直接拒绝），
/// 敌意无限响应体不再 OOM abort。上限可用 [`Self::with_max_body_bytes`]
/// 自定义。
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
    max_body_bytes: usize,
}

impl ReqwestFetcher {
    fn build_client() -> NetworkResult<reqwest::Client> {
        Ok(reqwest::Client::builder()
            .timeout(crate::DEFAULT_TIMEOUT)
            .connect_timeout(crate::DEFAULT_CONNECT_TIMEOUT)
            .build()?)
    }

    /// 创建默认实现（响应体上限 [`crate::MAX_BODY_BYTES`]）。
    pub fn new() -> NetworkResult<Self> {
        Ok(Self {
            inner: Self::build_client()?,
            max_body_bytes: crate::MAX_BODY_BYTES,
        })
    }

    /// 创建自定义响应体上限的实现（F-14；测试与小内存环境用）。
    pub fn with_max_body_bytes(max_body_bytes: usize) -> NetworkResult<Self> {
        Ok(Self {
            inner: Self::build_client()?,
            max_body_bytes,
        })
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
        // F-14：响应体上限。Content-Length 超限直接拒绝；否则流式读取，
        // 每收一个 chunk 检查累计长度（chunked 无 Content-Length，只能
        // 边收边检）。顺带消除 `Bytes → to_vec()` 的全量拷贝。
        if let Some(len) = resp.content_length() {
            if len as usize > self.max_body_bytes {
                return Err(NetworkError::BodyTooLarge {
                    actual: len as usize,
                    limit: self.max_body_bytes,
                });
            }
        }
        let mut body = Vec::new();
        let mut resp = resp;
        while let Some(chunk) = resp.chunk().await? {
            if body.len() + chunk.len() > self.max_body_bytes {
                return Err(NetworkError::BodyTooLarge {
                    actual: body.len() + chunk.len(),
                    limit: self.max_body_bytes,
                });
            }
            body.extend_from_slice(&chunk);
        }
        Ok(NetworkResponse::new(status, headers, final_url, body))
    }
}
