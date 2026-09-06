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

/// 默认响应体上限（F-14，审计 S-6）：64 MiB，与 html5-parser 的输入
/// 上限（`MAX_INPUT_BYTES`）对齐。敌意服务器可无限流式发送响应体
/// （chunked 无需 Content-Length），无上限缓冲 = OOM abort。
pub const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

/// 默认总超时（F-14，审计 S-6）：`NetworkFetcher` trait 契约（fetcher.rs）
/// 承诺超时错误，实现必须兑现——slow-loris 服务器不得无限挂起 fetch。
pub const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// 默认连接超时（F-14，审计 S-6）。
pub const DEFAULT_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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

/// 便捷函数：**同步（阻塞）** fetch 一个 URL。
///
/// 内部在当前线程上自建一次性 current_thread 运行时执行异步 fetch，供
/// 没有异步上下文的上层使用（如 chrome 的 winit 事件循环后台线程）。
/// 需启用 `reqwest-backend` feature（默认开启）。
///
/// # 限制
///
/// - 每次调用构建并销毁一个运行时，仅适合导航级别的低频调用；高频批量
///   请求应复用 [`NetworkFetcher`] + 调用方自管的运行时。
/// - **不得在异步上下文内调用**（在运行时线程上嵌套 `block_on` 会 panic）。
///
/// # 示例
///
/// ```no_run
/// let resp = muskitty_network::fetch_blocking("https://example.com")?;
/// assert!(resp.is_success());
/// # Ok::<(), muskitty_network::NetworkError>(())
/// ```
#[cfg(feature = "reqwest-backend")]
pub fn fetch_blocking(url: &str) -> NetworkResult<NetworkResponse> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| NetworkError::Http(format!("build tokio runtime: {e}")))?
        .block_on(ReqwestFetcher::new()?.fetch(url))
}
