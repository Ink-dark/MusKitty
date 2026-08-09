//! # muskitty-network
//!
//! MusKitty Phase 5 网络层基础模块。
//!
//! ## 架构：trait 抽象 + 可插拔实现
//!
//! 本 crate 通过 [`NetworkFetcher`] trait 抽象网络获取行为，上层不绑定具体 HTTP 实现：
//!
//! - **当前**：[`ReqwestFetcher`]（启用 `reqwest-backend` feature，默认开启）— 基于 reqwest + rustls
//! - **远期**：自研 HTTP 栈实现（HTTP/1.1 + HTTP/2 + HTTP/3 over QUIC）
//!
//! 切换实现时上层代码零改动，只需换 fetcher 实例。
//!
//! ## 设计原则
//!
//! - 异步（tokio 生态），符合浏览器网络层并发本质
//! - 零 C/C++ 依赖（TLS 用 rustls，不用 native-tls / OpenSSL）
//! - trait 抽象，便于剥离为独立 crate 后被多种上层复用
//! - API 极简，仅暴露 [`NetworkFetcher`] + [`NetworkResponse`] + 便捷 [`fetch`]
//!
//! ## 远期自研路线
//!
//! 详见 `docs/plans/2026-08-09-phase5-network.md`。

mod error;
mod fetcher;
mod response;

pub use error::{NetworkError, NetworkResult};
pub use fetcher::NetworkFetcher;
pub use response::NetworkResponse;

#[cfg(feature = "reqwest-backend")]
mod reqwest_impl;

#[cfg(feature = "reqwest-backend")]
pub use reqwest_impl::ReqwestFetcher;

/// 便捷函数：用默认 fetcher 实现 fetch 一个 URL。
///
/// 需启用 `reqwest-backend` feature（默认开启）。等价于
/// `ReqwestFetcher::new()?.fetch(url).await`。
///
/// # 示例
///
/// ```no_run
/// # async fn run() -> muskitty_network::NetworkResult<()> {
/// let resp = muskitty_network::fetch("https://example.com").await?;
/// assert!(resp.is_success());
/// println!("{}", resp.text());
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "reqwest-backend")]
pub async fn fetch(url: &str) -> NetworkResult<NetworkResponse> {
    ReqwestFetcher::new()?.fetch(url).await
}
