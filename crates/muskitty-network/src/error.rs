//! 网络层错误类型。

use std::fmt;

/// 网络层错误。
///
/// 不暴露具体后端（reqwest）的错误类型，仅存错误描述字符串，使
/// [`NetworkError`] / [`NetworkResult`] 与实现解耦（可在任意 feature 下使用）。
#[derive(Debug)]
pub enum NetworkError {
    /// HTTP 底层错误（DNS / 连接 / TLS / 协议 / 超时等）。
    Http(String),
    /// URL 解析或格式错误。
    InvalidUrl(String),
    /// 响应体超过上限（F-14，审计 S-6）：敌意服务器可无限流式发送
    /// 响应体（chunked 无需 Content-Length），无上限缓冲 = OOM abort。
    BodyTooLarge {
        /// 已接收 / 声明的字节数。
        actual: usize,
        /// 配置的上限（[`crate::MAX_BODY_BYTES`] 或自定义）。
        limit: usize,
    },
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::Http(e) => write!(f, "HTTP error: {e}"),
            NetworkError::InvalidUrl(u) => write!(f, "invalid URL: {u}"),
            NetworkError::BodyTooLarge { actual, limit } => {
                write!(f, "response body too large: {actual} bytes (limit {limit})")
            }
        }
    }
}

impl std::error::Error for NetworkError {}

#[cfg(feature = "reqwest-backend")]
impl From<reqwest::Error> for NetworkError {
    fn from(e: reqwest::Error) -> Self {
        NetworkError::Http(e.to_string())
    }
}

/// 网络层 Result 别名。
pub type NetworkResult<T> = Result<T, NetworkError>;
