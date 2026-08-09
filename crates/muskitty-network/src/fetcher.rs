//! 网络层核心抽象：[`NetworkFetcher`] trait。
//!
//! 本 crate 通过 trait 将"网络获取行为"抽象出来，使上层不绑定具体 HTTP 实现。
//! 当前提供基于 reqwest 的实现（[`crate::ReqwestFetcher`]，启用 `reqwest-backend` feature），
//! 未来可替换为自研 HTTP 栈实现，上层代码零改动。

use std::future::Future;

use crate::error::NetworkResult;
use crate::response::NetworkResponse;

/// 网络获取抽象。
///
/// 上层依赖此 trait 而非具体实现，便于：
/// - 当前用 reqwest 快速跑起来（[`crate::ReqwestFetcher`]）
/// - 未来切换到自研 HTTP 栈时上层零改动
/// - 测试时可注入 mock 实现
/// - 剥离为独立 crate 后被多种上层复用
///
/// # 实现要求
///
/// - `fetch` 不因 HTTP 4xx/5xx 报错，调用方自行判断 [`NetworkResponse::status`]
/// - 网络层错误（DNS / 连接 / TLS / 超时）返回 [`NetworkError`](crate::NetworkError)
/// - 返回的 `Future` 必须是 `Send`（浏览器网络层需跨线程并发 fetch 多资源）
///
/// # 关于 `dyn` 分发
///
/// 当前 trait 方法返回 `impl Future + Send`，不支持 `dyn NetworkFetcher`，
/// 上层用泛型约束 `F: NetworkFetcher`。若未来需要 trait 对象分发，
/// 引入 `async-trait` 或 `trait_variant::trait_variant` 即可，接口签名不变。
///
/// # 示例
///
/// ```no_run
/// # async fn run<F: muskitty_network::NetworkFetcher>(fetcher: &F) -> muskitty_network::NetworkResult<()> {
/// let resp = fetcher.fetch("https://example.com").await?;
/// assert!(resp.is_success());
/// # Ok(())
/// # }
/// ```
pub trait NetworkFetcher {
    /// GET 请求指定 URL，返回响应。
    ///
    /// 返回 `impl Future + Send` 而非 `async fn`，显式要求 `Send` bound
    /// 以支持跨线程并发（浏览器并发 fetch 多资源的核心场景）。
    fn fetch(&self, url: &str) -> impl Future<Output = NetworkResult<NetworkResponse>> + Send;
}
